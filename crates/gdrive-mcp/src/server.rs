use anyhow::Result;
use gdrive_core::{CreateFileOptions, GDriveClient, UpdateFileOptions};
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

use crate::models::{
    CreateFileArgs, CreateFolderArgs, ListDrivesArgs, ListFolderArgs, MoveFileArgs, ReadFileArgs,
    RenameFileArgs, SearchFilesArgs, TrashFileArgs, UpdateFileArgs,
};

fn any_json_object(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({ "type": "object", "additionalProperties": true })
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolResponse {
    #[schemars(schema_with = "any_json_object")]
    pub result: Value,
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

    async fn client() -> Result<GDriveClient, ErrorData> {
        GDriveClient::from_saved_token().await.map_err(|e| {
            ErrorData::internal_error(
                format!("Auth error: {e}. Jalankan 'gdrive auth login' terlebih dahulu."),
                None,
            )
        })
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
            Use gdrive_create_file to write or upload documents, with optional conversion to native Google Docs. \
            Use gdrive_update_file to modify or append content to existing files. \
            Use gdrive_create_folder to create directories. \
            Use gdrive_rename_file to rename a file or folder. \
            Use gdrive_move_file to move a file or folder to a target folder. \
            Use gdrive_trash_file to safely move files or folders to trash. \
            Use gdrive_list_drives to list accessible Shared Drives.",
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
}

pub async fn run_stdio() -> Result<()> {
    let server = GDriveMcpServer::new().serve(stdio()).await?;
    server.waiting().await?;
    Ok(())
}
