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
    AddCommentArgs, CopyFileArgs, CreateFileArgs, CreateFolderArgs, DeletePermissionArgs,
    DownloadFileArgs, EmptyTrashArgs, GetFileInfoArgs, GetRevisionArgs, GetShareableLinkArgs,
    ListCommentsArgs, ListDrivesArgs, ListFolderArgs, ListPermissionsArgs, ListRevisionsArgs,
    ListTrashArgs, MoveFileArgs, ReadFileArgs, RenameFileArgs, ResolveCommentArgs, RestoreFileArgs,
    SearchFilesArgs, ShareFileArgs, TrashFileArgs, UpdateFileArgs,
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

    Ok(files[0].id.clone())
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

    async fn client() -> Result<GDriveClient, ErrorData> {
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
            "Use gdrive_search to find files and folders across personal and shared drives with optional full-text search, ordering, and pagination. \
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
            Use gdrive_get_revision to inspect or retrieve content from a specific past revision.",
        )
    }
}

#[tool_router(router = tool_router)]
impl GDriveMcpServer {
    #[tool(
        name = "gdrive_search",
        description = "Cari file atau folder di Google Drive pribadi maupun Shared Drives menggunakan query nama atau folder. Mendukung full-text search, ordering, dan pagination."
    )]
    pub async fn gdrive_search(
        &self,
        Parameters(args): Parameters<SearchFilesArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client().await?;
        let res = client
            .search_files_with_options(gdrive_core::SearchFileOptions {
                query: args.query,
                folder_id: args.folder_id,
                page_size: args.page_size,
                page_token: args.page_token,
                full_text: args.full_text.unwrap_or(false),
                order_by: args.order_by,
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
        let client = Self::client().await?;
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
        let client = Self::client().await?;
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
        let client = Self::client().await?;
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
        name = "gdrive_update_file",
        description = "Memperbarui isi file yang sudah ada di Google Drive (bisa menimpa seluruh file atau menambahkan teks di baris akhir)."
    )]
    pub async fn gdrive_update_file(
        &self,
        Parameters(args): Parameters<UpdateFileArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client().await?;
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
        let client = Self::client().await?;
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
        let client = Self::client().await?;
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
        let client = Self::client().await?;
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
        let client = Self::client().await?;
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
        let client = Self::client().await?;
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
        let client = Self::client().await?;
        let target = args.file_id_or_name.trim();

        // Resolusi target ke File ID
        let file_id = resolve_mcp_target(&client, target).await?;

        let stream_data = client
            .download_file_stream(&file_id)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let orig_name = stream_data.name.clone();
        let mime_type = stream_data.mime_type.clone();

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
        name = "gdrive_copy_file",
        description = "Menyalin (duplicate) file di Google Drive dengan opsi nama baru dan folder tujuan baru."
    )]
    pub async fn gdrive_copy_file(
        &self,
        Parameters(args): Parameters<CopyFileArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client().await?;
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
        let client = Self::client().await?;
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
        let client = Self::client().await?;
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
        let client = Self::client().await?;
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

        let opts = CreatePermissionOptions {
            role: args.role,
            permission_type,
            email_address: args.email_address,
            domain: None,
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
        let client = Self::client().await?;
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
        let client = Self::client().await?;
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
        let client = Self::client().await?;
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
        let client = Self::client().await?;
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
        let client = Self::client().await?;
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
        let client = Self::client().await?;
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
        let client = Self::client().await?;
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

        let client = Self::client().await?;
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
        let client = Self::client().await?;
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
        let client = Self::client().await?;
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
}

pub async fn run_stdio() -> Result<()> {
    let server = GDriveMcpServer::new().serve(stdio()).await?;
    server.waiting().await?;
    Ok(())
}
