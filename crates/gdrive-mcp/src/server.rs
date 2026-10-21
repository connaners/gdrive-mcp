use anyhow::Result;
use gdrive_core::{AuthManager, CreateFileOptions, GDriveClient};
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

use crate::models::{CreateFileArgs, ListFolderArgs, ReadFileArgs, SearchFilesArgs};

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

    fn client() -> Result<GDriveClient, ErrorData> {
        let token = AuthManager::load_saved_token()
            .map_err(|e| ErrorData::internal_error(format!("Auth error: {e}. Jalankan 'gdrive auth login' terlebih dahulu."), None))?;
        Ok(GDriveClient::new(token.access_token))
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
                .with_description("MCP tools for searching, reading, creating, and updating Google Drive files & Docs"),
        )
        .with_instructions(
            "Use gdrive_search to find files and folders across personal and shared drives. Use gdrive_read_file to read document contents (auto-converts Google Docs to markdown). Use gdrive_create_file to write or upload documents, with optional conversion to native Google Docs.",
        )
    }
}

#[tool_router(router = tool_router)]
impl GDriveMcpServer {
    #[tool(
        name = "gdrive_search",
        description = "Cari file atau folder di Google Drive pribadi maupun Shared Drives menggunakan query nama atau folder."
    )]
    pub async fn gdrive_search(
        &self,
        Parameters(args): Parameters<SearchFilesArgs>,
    ) -> Result<Json<ToolResponse>, ErrorData> {
        let client = Self::client()?;
        let files = client
            .search_files(args.query.as_deref(), args.folder_id.as_deref(), args.page_size)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        Ok(Json(ToolResponse {
            result: serde_json::to_value(files).unwrap_or(Value::Null),
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
        let client = Self::client()?;
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
        let client = Self::client()?;
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
        let client = Self::client()?;
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
}

pub async fn run_stdio() -> Result<()> {
    let server = GDriveMcpServer::new().serve(stdio()).await?;
    server.waiting().await?;
    Ok(())
}
