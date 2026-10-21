use anyhow::Result;
use clap::{Parser, Subcommand};
use gdrive_core::{AuthManager, CreateFileOptions, GDriveClient};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "gdrive",
    author = "connaners",
    version = "0.1.0",
    about = "Google Drive CLI & MCP Server"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manajemen autentikasi Google OAuth2
    Auth {
        #[command(subcommand)]
        action: AuthCommands,
    },
    /// Cari file atau folder di Google Drive
    Search {
        /// Kata kunci query nama file
        query: Option<String>,
        /// ID folder spesifik jika ingin membatasi pencarian
        #[arg(short, long)]
        folder: Option<String>,
        /// Jumlah maksimal hasil
        #[arg(short, long, default_value = "20")]
        limit: u32,
    },
    /// Baca isi file dari Google Drive
    Read {
        /// ID file yang ingin dibaca
        file_id: String,
    },
    /// Buat atau upload file baru ke Google Drive
    Create {
        /// Nama file (misal: 'Ringkasan.md')
        name: String,
        /// Isi teks konten dokumen
        content: String,
        /// ID folder tujuan (opsional)
        #[arg(short, long)]
        parent: Option<String>,
        /// Konversi menjadi native Google Docs
        #[arg(long)]
        docs: bool,
    },
    /// Jalankan MCP server untuk AI agents (Hermes, Claude, OpenCode)
    Mcp {
        #[command(subcommand)]
        action: McpCommands,
    },
}

#[derive(Subcommand)]
enum AuthCommands {
    /// Cek status token yang tersimpan saat ini
    Status,
}

#[derive(Subcommand)]
enum McpCommands {
    /// Jalankan MCP server via stdio (standard input/output)
    Serve,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Arahkan semua logging ke stderr agar stdout murni untuk JSON-RPC MCP
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Auth { action } => match action {
            AuthCommands::Status => match AuthManager::load_saved_token() {
                Ok(_) => {
                    let path = AuthManager::token_path()?;
                    eprintln!("Status: Terautentikasi (Token tersimpan di {:?})", path);
                }
                Err(e) => {
                    eprintln!("Status: Belum terautentikasi ({})", e);
                }
            },
        },
        Commands::Search {
            query,
            folder,
            limit,
        } => {
            let token = AuthManager::load_saved_token()?;
            let client = GDriveClient::new(token.access_token);
            let files = client
                .search_files(query.as_deref(), folder.as_deref(), Some(limit))
                .await?;
            for f in files {
                println!("{} | {} | {}", f.id, f.name, f.mime_type);
            }
        }
        Commands::Read { file_id } => {
            let token = AuthManager::load_saved_token()?;
            let client = GDriveClient::new(token.access_token);
            let text = client.read_file(&file_id).await?;
            println!("{}", text);
        }
        Commands::Create {
            name,
            content,
            parent,
            docs,
        } => {
            let token = AuthManager::load_saved_token()?;
            let client = GDriveClient::new(token.access_token);
            let created = client
                .create_file(CreateFileOptions {
                    name,
                    content,
                    parent_folder_id: parent,
                    as_google_doc: docs,
                })
                .await?;
            eprintln!("File berhasil dibuat: ID={}", created.id);
            if let Some(link) = created.web_view_link {
                println!("Link: {}", link);
            }
        }
        Commands::Mcp { action } => match action {
            McpCommands::Serve => {
                gdrive_mcp::run_stdio().await?;
            }
        },
    }

    Ok(())
}
