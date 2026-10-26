use anyhow::Result;
use futures_util::StreamExt;
use gdrive_core::{CreateFileOptions, CreatePermissionOptions, GDriveClient, UpdateFileOptions};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
    transport::stdio,
    ErrorData, Json, ServerHandler, ServiceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Write;

use crate::models::{
    AddCommentArgs, CheckUpdateArgs, CopyFileArgs, CreateFileArgs, CreateFolderArgs,
    DeletePermissionArgs, DownloadFileArgs, EmptyTrashArgs, ExportFileArgs, GetAboutArgs,
    GetFileInfoArgs, GetRevisionArgs, GetShareableLinkArgs, ListCommentsArgs, ListDrivesArgs,
    ListFolderArgs, ListPermissionsArgs, ListProfilesArgs, ListRevisionsArgs, ListTrashArgs,
    MoveFileArgs, ReadFileArgs, RenameFileArgs, ResolveCommentArgs, RestoreFileArgs, RunDoctorArgs,
    SearchFilesArgs, SelfUpdateArgs, ShareFileArgs, SwitchProfileArgs, TrashFileArgs,
    UpdateFileArgs, UploadFileArgs,
};

fn any_json_object(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({ "type": "object", "additionalProperties": true })
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolResponse {
    #[schemars(schema_with = "any_json_object")]
    pub result: Value,
}

async fn write_chunks_to_part_file(
    stream: &mut gdrive_core::DownloadByteStream,
    part_path: &std::path::Path,
) -> std::result::Result<u64, ErrorData> {
    let mut file = std::fs::File::create(part_path).map_err(|e| {
        ErrorData::internal_error(format!("Gagal membuat file sementara: {}", e), None)
    })?;
    let mut downloaded_bytes: u64 = 0;

    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res.map_err(|e| {
            ErrorData::internal_error(format!("Koneksi streaming terputus: {}", e), None)
        })?;
        file.write_all(&chunk).map_err(|e| {
            ErrorData::internal_error(format!("Gagal menulis byte ke disk: {}", e), None)
        })?;
        downloaded_bytes += chunk.len() as u64;
    }

    file.flush().map_err(|e| {
        ErrorData::internal_error(format!("Gagal flush file sementara: {}", e), None)
    })?;
    Ok(downloaded_bytes)
}

async fn resolve_mcp_target(
    client: &GDriveClient,
    raw_target: &str,
) -> std::result::Result<String, ErrorData> {
    let target = GDriveClient::clean_file_id(raw_target);
    if target.len() >= 25 && !target.contains(' ') && !target.contains('.') {
        if let Ok(meta) = client.get_file_metadata(target).await {
            return Ok(meta.id);
        }
    }

    let files = client
        .find_files_by_name(target, 5)
        .await
        .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

    if files.is_empty() {
        return Err(ErrorData::invalid_params(
            format!(
                "File dengan ID atau nama '{}' tidak ditemukan di Google Drive.",
                target
            ),
            None,
        ));
    }

    if files.len() > 1 {
        let matches_desc: Vec<String> = files
            .iter()
            .map(|f| {
                format!(
                    "- ID: {}, Nama: '{}', Terakhir diubah: {}",
                    f.id,
                    f.name,
                    f.modified_time.as_deref().unwrap_or("-")
                )
            })
            .collect();
        return Err(ErrorData::invalid_params(
            format!(
                "Ditemukan {} file yang cocok dengan nama '{}'. Harap spesifikasikan ID file secara eksplisit:\n{}",
                files.len(),
                target,
                matches_desc.join("\n")
            ),
            None,
        ));
    }

    Ok(files[0].id.clone())
}

fn map_export_format(
    format_str: &str,
    mime_type: &str,
) -> Result<(&'static str, &'static str), ErrorData> {
    let fmt = format_str.trim().to_lowercase();
    match mime_type {
        "application/vnd.google-apps.document" => match fmt.as_str() {
            "pdf" => Ok(("application/pdf", "pdf")),
            "docx" => Ok((
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                "docx",
            )),
            "txt" | "text" => Ok(("text/plain", "txt")),
            "html" => Ok(("text/html", "html")),
            "rtf" => Ok(("application/rtf", "rtf")),
            "epub" => Ok(("application/epub+zip", "epub")),
            _ => Err(ErrorData::invalid_params(
                format!(
                    "Format '{}' tidak didukung untuk Google Docs. Format yang valid: pdf, docx, txt, html, rtf, epub.",
                    format_str
                ),
                None,
            )),
        },
        "application/vnd.google-apps.spreadsheet" => match fmt.as_str() {
            "xlsx" => Ok((
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                "xlsx",
            )),
            "csv" => Ok(("text/csv", "csv")),
            "pdf" => Ok(("application/pdf", "pdf")),
            "tsv" => Ok(("text/tab-separated-values", "tsv")),
            "html" => Ok(("application/zip", "zip")),
            _ => Err(ErrorData::invalid_params(
                format!(
                    "Format '{}' tidak didukung untuk Google Sheets. Format yang valid: xlsx, csv, pdf, tsv.",
                    format_str
                ),
                None,
            )),
        },
        "application/vnd.google-apps.presentation" => match fmt.as_str() {
            "pdf" => Ok(("application/pdf", "pdf")),
            "pptx" => Ok((
                "application/vnd.openxmlformats-officedocument.presentationml.presentation",
                "pptx",
            )),
            "txt" | "text" => Ok(("text/plain", "txt")),
            _ => Err(ErrorData::invalid_params(
                format!(
                    "Format '{}' tidak didukung untuk Google Slides. Format yang valid: pdf, pptx, txt.",
                    format_str
                ),
                None,
            )),
        },
        _ => Err(ErrorData::invalid_params(
            format!(
                "File dengan mimeType '{}' bukan dokumen Google Workspace (Docs, Sheets, Slides) sehingga tidak dapat diekspor. Gunakan gdrive_download_file tanpa parameter export_format.",
                mime_type
            ),
            None,
        )),
    }
}

#[derive(Clone)]
pub struct GDriveMcpServer {
    tool_router: ToolRouter<Self>,
}

impl GDriveMcpServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
    pub async fn client_for_profile(profile: Option<&str>) -> Result<GDriveClient, ErrorData> {
        GDriveClient::from_profile(profile).await.map_err(|e| {
            ErrorData::internal_error(
                format!("Auth error: {e}. Jalankan 'gdrive auth login' terlebih dahulu."),
                None,
            )
        })
    }

    pub async fn client() -> Result<GDriveClient, ErrorData> {
        Self::client_for_profile(None).await
    }
}

impl Default for GDriveMcpServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for GDriveMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new("gdrive-mcp", env!("CARGO_PKG_VERSION"))
                .with_title("Google Drive MCP Server")
                .with_description("MCP tools for searching, reading, creating, updating, and organizing Google Drive files & Docs"),
        )
        .with_instructions(
            "Use gdrive_list_profiles to see configured Google Drive accounts/profiles and which one is active. \
            Use gdrive_switch_profile to switch the active profile. \
            Use gdrive_get_about to check storage quota (usage and limits) and current user identity. \
            Use gdrive_search to find files and folders across personal and shared drives with optional full-text search, ordering, and pagination. \
            Use gdrive_read_file to read document contents (auto-converts Google Docs to markdown, Sheets to CSV, Slides to text). \
            Use gdrive_get_file_info to inspect file or folder metadata (size, mime type, parents, webViewLink, modified date) without reading full contents. \
            Use gdrive_create_file to write or upload documents, with optional conversion to native Google Docs. \
            Use gdrive_update_file to modify or append content to existing files. \
            Use gdrive_copy_file to duplicate files with optional new name or destination folder. \
            Use gdrive_create_folder to create directories. \
            Use gdrive_rename_file to rename a file or folder. \
            Use gdrive_move_file to move a file or folder to a target folder. \
            Use gdrive_trash_file to safely move files or folders to trash. \
            Use gdrive_list_drives to list accessible Shared Drives. \
            Use gdrive_download_file to download a file from Google Drive to local disk by file ID or name. \
            Use gdrive_list_permissions to view who has access to a file or folder. \
            Use gdrive_share_file to share a file or folder with users, groups, domains, or anyone with link. \
            Use gdrive_delete_permission to revoke access for a specific permission ID. \
            Use gdrive_get_shareable_link to get the web view link with optional automatic public sharing. \
            Use gdrive_list_comments to view comments and discussions on a document. \
            Use gdrive_add_comment to add feedback or review comments to a file. \
            Use gdrive_resolve_comment to mark a comment as resolved. \
            Use gdrive_list_trash to view items in the trash. \
            Use gdrive_restore_file to recover/untrash a file. \
            Use gdrive_empty_trash to permanently empty the trash bin. \
            Use gdrive_list_revisions to view previous versions of a file. \
            Use gdrive_get_revision to inspect or retrieve content from a specific past revision. \
            Use gdrive_check_update to check for new releases and updates of gdrive-mcp from GitHub. \
            Use gdrive_self_update to automatically update the gdrive binary to the latest release. \
            Use gdrive_doctor to run health checks on config directory, credentials, token, and API connectivity.",
        )
    }
}

#[tool_router(router = tool_router)]
impl GDriveMcpServer {
    #[tool(
        name = "gdrive_list_profiles",
        description = "Melihat daftar profil akun Google Drive yang tersimpan (multi-account), status aktif, email akun, dan masa berlaku token."
    )]
    pub async fn gdrive_list_profiles(
        &self,
        Parameters(_args): Parameters<ListProfilesArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let statuses = gdrive_core::AuthManager::get_all_profiles_status()
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(ToolResponse {
            result: serde_json::to_value(statuses)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?,
        }))
    }

    #[tool(
        name = "gdrive_switch_profile",
        description = "Beralih ke profil akun Google Drive lain agar menjadi profil aktif default."
    )]
    pub async fn gdrive_switch_profile(
        &self,
        Parameters(args): Parameters<SwitchProfileArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let name = gdrive_core::AuthManager::validate_profile_name(&args.name)
            .map_err(|e| ErrorData::invalid_params(e.to_string(), None))?;

        gdrive_core::AuthManager::set_active_profile(name)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let token_exists = gdrive_core::AuthManager::token_path_for_profile(Some(name))
            .map(|p| p.exists())
            .unwrap_or(false);

        Ok(Json(ToolResponse {
            result: serde_json::json!({
                "status": "success",
                "active_profile": name,
                "is_authenticated": token_exists
            }),
        }))
    }

    #[tool(
        name = "gdrive_get_about",
        description = "Melihat informasi akun Google Drive (nama, email pengguna) dan kuota penyimpanan (storage quota limit, usage, usage in Drive & Trash)."
    )]
    pub async fn gdrive_get_about(
        &self,
        Parameters(args): Parameters<GetAboutArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        let about = client
            .get_about()
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(ToolResponse {
            result: serde_json::to_value(about)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?,
        }))
    }

    #[tool(
        name = "gdrive_search",
        description = "Cari file atau folder di Google Drive pribadi maupun Shared Drives menggunakan query nama atau folder. Mendukung full-text search, ordering, dan pagination."
    )]
    pub async fn gdrive_search(
        &self,
        Parameters(args): Parameters<SearchFilesArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        let res = client
            .search_files_with_options(gdrive_core::SearchFileOptions {
                query: args.query,
                folder_id: args.folder_id,
                page_size: args.page_size,
                page_token: args.page_token,
                full_text: args.full_text.unwrap_or(false),
                order_by: args.order_by,
                ..Default::default()
            })
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(ToolResponse {
            result: serde_json::json!({
                "files": res.files,
                "next_page_token": res.next_page_token
            }),
        }))
    }

    #[tool(
        name = "gdrive_read_file",
        description = "Membaca konten file dari Google Drive. Jika berupa Google Docs/Sheets, otomatis diekspor ke format Markdown/Teks."
    )]
    pub async fn gdrive_read_file(
        &self,
        Parameters(args): Parameters<ReadFileArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        let content = client
            .read_file(&args.file_id)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(ToolResponse {
            result: serde_json::json!({
                "file_id": args.file_id,
                "content": content
            }),
        }))
    }

    #[tool(
        name = "gdrive_list_folder",
        description = "Melihat daftar file dan subfolder di dalam suatu folder Google Drive."
    )]
    pub async fn gdrive_list_folder(
        &self,
        Parameters(args): Parameters<ListFolderArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        let files = client
            .search_files(None, args.folder_id.as_deref(), args.page_size)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(ToolResponse {
            result: serde_json::to_value(files).unwrap_or(Value::Null),
        }))
    }

    #[tool(
        name = "gdrive_create_file",
        description = "Membuat file baru di Google Drive. Bisa disimpan murni sebagai Markdown (.md) atau otomatis dikonversi jadi Google Docs agar bisa diedit di browser."
    )]
    pub async fn gdrive_create_file(
        &self,
        Parameters(args): Parameters<CreateFileArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        let created = client
            .create_file(CreateFileOptions {
                name: args.name,
                content: args.content,
                parent_folder_id: args.parent_folder_id,
                as_google_doc: args.as_google_doc.unwrap_or(false),
            })
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(ToolResponse {
            result: serde_json::to_value(created).unwrap_or(Value::Null),
        }))
    }

    #[tool(
        name = "gdrive_upload_file",
        description = "Mengunggah file dari disk lokal ke Google Drive dengan deteksi MIME otomatis dan opsi konversi ke native Google Docs."
    )]
    pub async fn gdrive_upload_file(
        &self,
        Parameters(args): Parameters<UploadFileArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        let raw_path = args.local_path.trim();
        let path_buf = std::path::PathBuf::from(raw_path);
        let local_path = if path_buf.is_relative() {
            std::env::current_dir()
                .map(|cwd| cwd.join(&path_buf))
                .unwrap_or(path_buf)
        } else {
            path_buf
        };

        if !local_path.exists() {
            return Err(ErrorData::invalid_params(
                format!("File lokal '{:?}' tidak ditemukan di disk.", local_path),
                None,
            ));
        }

        if !local_path.is_file() {
            return Err(ErrorData::invalid_params(
                format!("Path '{:?}' adalah direktori, bukan file.", local_path),
                None,
            ));
        }

        let uploaded = client
            .upload_file_path(
                &local_path,
                args.name.as_deref(),
                args.parent_folder_id.as_deref(),
                args.as_google_doc.unwrap_or(false),
            )
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(ToolResponse {
            result: serde_json::to_value(uploaded).unwrap_or(Value::Null),
        }))
    }

    #[tool(
        name = "gdrive_update_file",
        description = "Memperbarui isi file yang sudah ada di Google Drive (bisa menimpa seluruh file atau menambahkan teks di baris akhir)."
    )]
    pub async fn gdrive_update_file(
        &self,
        Parameters(args): Parameters<UpdateFileArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        let updated = client
            .update_file(UpdateFileOptions {
                file_id: args.file_id,
                content: args.content,
                append: args.append.unwrap_or(false),
            })
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(ToolResponse {
            result: serde_json::to_value(updated).unwrap_or(Value::Null),
        }))
    }

    #[tool(
        name = "gdrive_create_folder",
        description = "Membuat folder baru di Google Drive (pribadi atau Shared Drives)."
    )]
    pub async fn gdrive_create_folder(
        &self,
        Parameters(args): Parameters<CreateFolderArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        let folder = client
            .create_folder(&args.name, args.parent_folder_id.as_deref())
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(ToolResponse {
            result: serde_json::to_value(folder).unwrap_or(Value::Null),
        }))
    }

    #[tool(
        name = "gdrive_trash_file",
        description = "Memindahkan file atau folder ke tempat sampah Google Drive (Safe Trash - mencegah penghapusan permanen fatal)."
    )]
    pub async fn gdrive_trash_file(
        &self,
        Parameters(args): Parameters<TrashFileArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        let trashed = client
            .trash_file(&args.file_id)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(ToolResponse {
            result: serde_json::to_value(trashed).unwrap_or(Value::Null),
        }))
    }

    #[tool(
        name = "gdrive_rename_file",
        description = "Mengubah nama file atau folder di Google Drive."
    )]
    pub async fn gdrive_rename_file(
        &self,
        Parameters(args): Parameters<RenameFileArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        let renamed = client
            .rename_file(&args.file_id, &args.new_name)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(ToolResponse {
            result: serde_json::to_value(renamed).unwrap_or(Value::Null),
        }))
    }

    #[tool(
        name = "gdrive_move_file",
        description = "Memindahkan file atau folder ke folder tujuan di Google Drive."
    )]
    pub async fn gdrive_move_file(
        &self,
        Parameters(args): Parameters<MoveFileArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        let moved = client
            .move_file(
                &args.file_id,
                &args.new_parent_id,
                args.old_parent_id.as_deref(),
            )
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(ToolResponse {
            result: serde_json::to_value(moved).unwrap_or(Value::Null),
        }))
    }

    #[tool(
        name = "gdrive_list_drives",
        description = "Melihat daftar Shared Drives (Team Drives) yang dapat diakses."
    )]
    pub async fn gdrive_list_drives(
        &self,
        Parameters(args): Parameters<ListDrivesArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        let drives = client
            .list_drives(args.page_size)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(ToolResponse {
            result: serde_json::to_value(drives).unwrap_or(Value::Null),
        }))
    }

    #[tool(
        name = "gdrive_download_file",
        description = "Mengunduh file dari Google Drive ke disk lokal komputer. Menerima File ID atau nama file, serta opsional destination_path."
    )]
    pub async fn gdrive_download_file(
        &self,
        Parameters(args): Parameters<DownloadFileArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        let target = args.file_id_or_name.trim();

        // Resolusi target ke File ID
        let file_id = resolve_mcp_target(&client, target).await?;

        let stream_data = client
            .download_file_stream(&file_id)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let orig_name = stream_data.name.clone();
        let mime_type = stream_data.mime_type.clone();

        if let Some(export_fmt) = &args.export_format {
            let (target_mime, ext) = map_export_format(export_fmt, &mime_type)?;
            let (exported_name, bytes) = client
                .export_file_bytes(&file_id, target_mime)
                .await
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

            let stem = std::path::Path::new(&exported_name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("document");
            let target_filename = format!("{}.{}", stem, ext);

            let final_path = match args.destination_path {
                Some(p) => {
                    let pb = std::path::PathBuf::from(p);
                    if pb.is_dir() {
                        pb.join(&target_filename)
                    } else {
                        pb
                    }
                }
                None => std::env::current_dir()
                    .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
                    .join(&target_filename),
            };

            if let Ok(meta) = std::fs::symlink_metadata(&final_path) {
                if meta.file_type().is_symlink() {
                    return Err(ErrorData::invalid_params(
                        format!(
                            "Tujuan download {:?} adalah symbolic link yang tidak diizinkan.",
                            final_path
                        ),
                        None,
                    ));
                }
            }

            std::fs::write(&final_path, &bytes).map_err(|e| {
                ErrorData::internal_error(
                    format!("Gagal menulis file hasil download ekspor: {}", e),
                    None,
                )
            })?;

            let abs_path = std::fs::canonicalize(&final_path).unwrap_or(final_path);

            return Ok(Json(ToolResponse {
                result: serde_json::json!({
                    "file_id": file_id,
                    "file_name": exported_name,
                    "local_path": abs_path.to_string_lossy().to_string(),
                    "size_bytes": bytes.len(),
                    "mime_type": target_mime,
                    "exported_format": ext,
                    "status": "success"
                }),
            }));
        }
        let clean_name = orig_name.replace('\\', "/");
        let sanitized_name = std::path::Path::new(&clean_name)
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.trim_start_matches('.'))
            .filter(|n| !n.is_empty())
            .unwrap_or("downloaded_file");

        let out_path = match args.destination_path {
            Some(p) => {
                let pb = std::path::PathBuf::from(p);
                if pb.is_dir() {
                    pb.join(sanitized_name)
                } else {
                    pb
                }
            }
            None => std::env::current_dir()
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
                .join(sanitized_name),
        };

        if let Ok(meta) = std::fs::symlink_metadata(&out_path) {
            if meta.file_type().is_symlink() {
                return Err(ErrorData::invalid_params(
                    format!(
                        "Tujuan download {:?} adalah symbolic link yang tidak diizinkan.",
                        out_path
                    ),
                    None,
                ));
            }
        }

        let part_path = {
            let mut os_str = out_path.as_os_str().to_os_string();
            os_str.push(".gdrive_part");
            std::path::PathBuf::from(os_str)
        };

        let mut downloaded_bytes: u64 = 0;
        let mut current_stream = stream_data.stream;
        let mut stream_err = None;

        for attempt in 1..=3 {
            match write_chunks_to_part_file(&mut current_stream, &part_path).await {
                Ok(bytes) => {
                    downloaded_bytes = bytes;
                    stream_err = None;
                    break;
                }
                Err(_e) if attempt < 3 => {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    match client.download_file_stream(&file_id).await {
                        Ok(new_dl) => current_stream = new_dl.stream,
                        Err(err) => {
                            stream_err = Some(ErrorData::internal_error(err.to_string(), None));
                            break;
                        }
                    }
                }
                Err(e) => {
                    stream_err = Some(e);
                }
            }
        }

        if let Some(e) = stream_err {
            let _ = std::fs::remove_file(&part_path);
            return Err(e);
        }

        std::fs::rename(&part_path, &out_path).map_err(|e| {
            ErrorData::internal_error(format!("Gagal memindahkan file ke tujuan: {}", e), None)
        })?;

        let abs_path = std::fs::canonicalize(&out_path).unwrap_or(out_path);

        Ok(Json(ToolResponse {
            result: serde_json::json!({
                "file_id": file_id,
                "file_name": orig_name,
                "local_path": abs_path.to_string_lossy().to_string(),
                "size_bytes": downloaded_bytes,
                "mime_type": mime_type,
                "status": "success"
            }),
        }))
    }

    #[tool(
        name = "gdrive_export_file",
        description = "Mengekspor dokumen Google Workspace (Docs, Sheets, Slides) ke format standar (PDF, DOCX, XLSX, CSV, TXT, HTML) dan menyimpannya ke disk lokal."
    )]
    pub async fn gdrive_export_file(
        &self,
        Parameters(args): Parameters<ExportFileArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        let file_id = resolve_mcp_target(&client, &args.file_id).await?;
        let meta = client
            .get_file_metadata(&file_id)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let (target_mime, ext) = map_export_format(&args.format, &meta.mime_type)?;
        let (orig_name, bytes) = client
            .export_file_bytes(&file_id, target_mime)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let stem = std::path::Path::new(&orig_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("document");
        let target_filename = format!("{}.{}", stem, ext);

        let out_path = match args.destination_path {
            Some(p) => {
                let pb = std::path::PathBuf::from(p);
                if pb.is_dir() {
                    pb.join(&target_filename)
                } else {
                    pb
                }
            }
            None => std::env::current_dir()
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
                .join(&target_filename),
        };

        if let Ok(m) = std::fs::symlink_metadata(&out_path) {
            if m.file_type().is_symlink() {
                return Err(ErrorData::invalid_params(
                    format!(
                        "Tujuan ekspor {:?} adalah symbolic link yang tidak diizinkan.",
                        out_path
                    ),
                    None,
                ));
            }
        }

        std::fs::write(&out_path, &bytes).map_err(|e| {
            ErrorData::internal_error(format!("Gagal menulis file ekspor: {}", e), None)
        })?;

        let abs_path = std::fs::canonicalize(&out_path).unwrap_or(out_path);

        Ok(Json(ToolResponse {
            result: serde_json::json!({
                "file_id": file_id,
                "document_name": orig_name,
                "exported_format": ext,
                "mime_type": target_mime,
                "local_path": abs_path.to_string_lossy().to_string(),
                "size_bytes": bytes.len(),
                "status": "success"
            }),
        }))
    }

    #[tool(
        name = "gdrive_copy_file",
        description = "Menyalin (duplicate) file di Google Drive dengan opsi nama baru dan folder tujuan baru."
    )]
    pub async fn gdrive_copy_file(
        &self,
        Parameters(args): Parameters<CopyFileArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        let copied = client
            .copy_file(
                &args.file_id,
                args.new_name.as_deref(),
                args.parent_folder_id.as_deref(),
            )
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(ToolResponse {
            result: serde_json::to_value(copied)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?,
        }))
    }

    #[tool(
        name = "gdrive_get_file_info",
        description = "Melihat metadata lengkap file atau folder (ID, nama, tipe mime, ukuran file, tanggal modifikasi, link web, dan parent folder) tanpa perlu membaca seluruh isinya."
    )]
    pub async fn gdrive_get_file_info(
        &self,
        Parameters(args): Parameters<GetFileInfoArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        let file_id = resolve_mcp_target(&client, &args.file_id).await?;
        let meta = client
            .get_file_metadata(&file_id)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(ToolResponse {
            result: serde_json::to_value(meta)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?,
        }))
    }

    #[tool(
        name = "gdrive_list_permissions",
        description = "Melihat daftar pengguna, grup, domain, atau publik yang memiliki akses (permissions) ke suatu file atau folder di Google Drive."
    )]
    pub async fn gdrive_list_permissions(
        &self,
        Parameters(args): Parameters<ListPermissionsArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        let file_id = resolve_mcp_target(&client, &args.file_id).await?;
        let perms = client
            .list_permissions(&file_id)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(ToolResponse {
            result: serde_json::to_value(perms)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?,
        }))
    }

    #[tool(
        name = "gdrive_share_file",
        description = "Membagikan file atau folder di Google Drive ke pengguna, grup, domain, atau publik dengan hak akses tertentu ('reader', 'commenter', 'writer', atau 'fileOrganizer')."
    )]
    pub async fn gdrive_share_file(
        &self,
        Parameters(args): Parameters<ShareFileArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        let file_id = resolve_mcp_target(&client, &args.file_id).await?;
        let permission_type = if args.permission_type == "anyone"
            && args
                .email_address
                .as_deref()
                .map(|e| !e.trim().is_empty())
                .unwrap_or(false)
        {
            "user".to_string()
        } else {
            args.permission_type
        };

        if (permission_type == "user" || permission_type == "group")
            && args
                .email_address
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
        {
            return Err(ErrorData::invalid_params(
                format!(
                    "Parameter 'email_address' wajib diisi jika permission_type adalah '{}'.",
                    permission_type
                ),
                None,
            ));
        }
        if permission_type == "domain" && args.domain.as_deref().unwrap_or("").trim().is_empty() {
            return Err(ErrorData::invalid_params(
                "Parameter 'domain' wajib diisi jika permission_type adalah 'domain'.",
                None,
            ));
        }

        let opts = CreatePermissionOptions {
            role: args.role,
            permission_type,
            email_address: args.email_address,
            domain: args.domain,
            send_notification_email: args.send_notification.unwrap_or(false),
        };

        let perm = client
            .create_permission(&file_id, opts)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(ToolResponse {
            result: serde_json::to_value(perm)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?,
        }))
    }

    #[tool(
        name = "gdrive_delete_permission",
        description = "Menghapus hak akses (permission) dari suatu file atau folder di Google Drive."
    )]
    pub async fn gdrive_delete_permission(
        &self,
        Parameters(args): Parameters<DeletePermissionArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        let file_id = resolve_mcp_target(&client, &args.file_id).await?;

        client
            .delete_permission(&file_id, &args.permission_id)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(ToolResponse {
            result: serde_json::json!({
                "file_id": file_id,
                "permission_id": args.permission_id,
                "status": "revoked"
            }),
        }))
    }

    #[tool(
        name = "gdrive_get_shareable_link",
        description = "Mendapatkan tautan Google Drive (shareable link) untuk file atau folder, dengan opsi membuatnya publik secara otomatis."
    )]
    pub async fn gdrive_get_shareable_link(
        &self,
        Parameters(args): Parameters<GetShareableLinkArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        let file_id = resolve_mcp_target(&client, &args.file_id).await?;
        let make_public = args.make_public.unwrap_or(false);

        let link = client
            .get_shareable_link(&file_id, make_public)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(ToolResponse {
            result: serde_json::json!({
                "file_id": file_id,
                "shareable_link": link,
                "public": make_public
            }),
        }))
    }

    #[tool(
        name = "gdrive_list_comments",
        description = "Melihat daftar komentar pada suatu file di Google Drive (termasuk status resolved dan data author)."
    )]
    pub async fn gdrive_list_comments(
        &self,
        Parameters(args): Parameters<ListCommentsArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        let file_id = resolve_mcp_target(&client, &args.file_id).await?;
        let comments = client
            .list_comments(&file_id, args.page_size)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(ToolResponse {
            result: serde_json::to_value(comments)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?,
        }))
    }

    #[tool(
        name = "gdrive_add_comment",
        description = "Menambahkan komentar baru atau feedback pada file Google Drive."
    )]
    pub async fn gdrive_add_comment(
        &self,
        Parameters(args): Parameters<AddCommentArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        let file_id = resolve_mcp_target(&client, &args.file_id).await?;
        let comment = client
            .create_comment(&file_id, &args.content)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(ToolResponse {
            result: serde_json::to_value(comment)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?,
        }))
    }

    #[tool(
        name = "gdrive_resolve_comment",
        description = "Menandai komentar pada file Google Drive sebagai terselesaikan (resolved)."
    )]
    pub async fn gdrive_resolve_comment(
        &self,
        Parameters(args): Parameters<ResolveCommentArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        let file_id = resolve_mcp_target(&client, &args.file_id).await?;
        let comment = client
            .resolve_comment(&file_id, &args.comment_id)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(ToolResponse {
            result: serde_json::to_value(comment)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?,
        }))
    }

    #[tool(
        name = "gdrive_list_trash",
        description = "Melihat daftar file dan folder yang berada di tempat sampah Google Drive."
    )]
    pub async fn gdrive_list_trash(
        &self,
        Parameters(args): Parameters<ListTrashArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        let res = client
            .list_trash(args.page_size, args.page_token.as_deref())
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(ToolResponse {
            result: serde_json::to_value(res)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?,
        }))
    }

    #[tool(
        name = "gdrive_restore_file",
        description = "Memulihkan file atau folder dari tempat sampah Google Drive (untrash)."
    )]
    pub async fn gdrive_restore_file(
        &self,
        Parameters(args): Parameters<RestoreFileArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        let restored = client
            .restore_trash_file(&args.file_id)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(ToolResponse {
            result: serde_json::to_value(restored)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?,
        }))
    }

    #[tool(
        name = "gdrive_empty_trash",
        description = "Mengosongkan tempat sampah Google Drive secara permanen (memerlukan confirm: true)."
    )]
    pub async fn gdrive_empty_trash(
        &self,
        Parameters(args): Parameters<EmptyTrashArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        if !args.confirm {
            return Err(ErrorData::invalid_params(
                "Konfirmasi 'confirm: true' diperlukan untuk mengosongkan tempat sampah.",
                None,
            ));
        }

        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        client
            .empty_trash()
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(ToolResponse {
            result: serde_json::json!({
                "status": "trash_emptied"
            }),
        }))
    }

    #[tool(
        name = "gdrive_list_revisions",
        description = "Melihat riwayat versi/revisi dari suatu file di Google Drive (termasuk modifiedTime, size, dan user)."
    )]
    pub async fn gdrive_list_revisions(
        &self,
        Parameters(args): Parameters<ListRevisionsArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        let file_id = resolve_mcp_target(&client, &args.file_id).await?;
        let revisions = client
            .list_revisions(&file_id)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(ToolResponse {
            result: serde_json::to_value(revisions)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?,
        }))
    }

    #[tool(
        name = "gdrive_get_revision",
        description = "Mengambil isi atau metadata dari suatu revisi/versi masa lalu dari file Google Drive."
    )]
    pub async fn gdrive_get_revision(
        &self,
        Parameters(args): Parameters<GetRevisionArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client_for_profile(args.profile.as_deref()).await?;
        let file_id = resolve_mcp_target(&client, &args.file_id).await?;
        let (filename, bytes) = client
            .download_revision_bytes(&file_id, &args.revision_id)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let is_utf8 = std::str::from_utf8(&bytes).is_ok();
        let content_or_info = if is_utf8 {
            String::from_utf8_lossy(&bytes).to_string()
        } else {
            format!("[Binary file content: {} bytes]", bytes.len())
        };

        Ok(Json(ToolResponse {
            result: serde_json::json!({
                "file_id": file_id,
                "revision_id": args.revision_id,
                "filename": filename,
                "size_bytes": bytes.len(),
                "content": content_or_info
            }),
        }))
    }

    #[tool(
        name = "gdrive_check_update",
        description = "Periksa ketersediaan versi terbaru gdrive-mcp dari GitHub Releases beserta informasi rilis dan link unduhan."
    )]
    pub async fn gdrive_check_update(
        &self,
        Parameters(args): Parameters<CheckUpdateArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let mut info = gdrive_core::check_for_update(None)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        if args.include_changelog != Some(true) {
            info.release_notes = None;
        }

        Ok(Json(ToolResponse {
            result: serde_json::to_value(info)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?,
        }))
    }

    #[tool(
        name = "gdrive_self_update",
        description = "Perbarui binary gdrive ke versi terbaru dari GitHub Releases secara otomatis."
    )]
    pub async fn gdrive_self_update(
        &self,
        Parameters(args): Parameters<SelfUpdateArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let success = gdrive_core::execute_self_update(None, None)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let restarting = args.restart == Some(true);
        if restarting {
            tokio::spawn(async {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                std::process::exit(0);
            });
        }

        Ok(Json(ToolResponse {
            result: serde_json::json!({
                "status": "success",
                "message": success.message,
                "current_version": success.current_version,
                "updated_to": success.updated_to,
                "binary_path": success.binary_path,
                "restarting": restarting
            }),
        }))
    }

    #[tool(
        name = "gdrive_doctor",
        description = "Pemeriksaan diagnostik kesehatan sistem gdrive-mcp: direktori config, kredensial OAuth, validitas token profil, dan konektivitas API Google Drive."
    )]
    pub async fn gdrive_doctor(
        &self,
        Parameters(args): Parameters<RunDoctorArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let report = gdrive_core::run_doctor_checks(args.profile.as_deref(), None).await;
        Ok(Json(ToolResponse {
            result: serde_json::to_value(report)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?,
        }))
    }
}

pub async fn run_stdio() -> Result<()> {
    let server = GDriveMcpServer::new().serve(stdio()).await?;
    server.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_info_instructions() {
        let server = GDriveMcpServer::new();
        let info = server.get_info();
        let instructions = info.instructions.unwrap_or_default();
        assert!(instructions.contains("gdrive_list_profiles"));
        assert!(instructions.contains("gdrive_switch_profile"));
        assert!(instructions.contains("gdrive_get_about"));
    }
}
