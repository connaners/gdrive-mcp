use std::io::Write;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use futures_util::StreamExt;
use gdrive_core::{AuthManager, CreateFileOptions, GDriveClient, UpdateFileOptions};
use indicatif::{ProgressBar, ProgressStyle};
use tracing_subscriber::EnvFilter;

mod format;

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
        /// Cari di seluruh isi dokumen teks, bukan hanya nama file
        #[arg(short = 't', long)]
        full_text: bool,
        /// Urutan hasil pencarian (misal: 'modifiedTime desc', 'name')
        #[arg(short = 's', long)]
        order_by: Option<String>,
        /// Token halaman untuk melanjutkan pencarian (pagination)
        #[arg(long)]
        page_token: Option<String>,
        /// Tampilkan output dalam format JSON mentah
        #[arg(long)]
        json: bool,
        /// Tampilkan output dalam format teks sederhana (ID | Name | Mime)
        #[arg(long)]
        plain: bool,
    },
    /// Ganti nama file atau folder di Google Drive
    Rename {
        /// ID file atau folder yang ingin diubah namanya
        file_id: String,
        /// Nama baru file atau folder
        new_name: String,
    },
    /// Pindahkan file atau folder ke folder tujuan
    Move {
        /// ID file atau folder yang ingin dipindahkan
        file_id: String,
        /// ID folder tujuan
        target_folder: String,
        /// ID folder asal jika diketahui (opsional)
        #[arg(long)]
        from: Option<String>,
    },
    /// Baca isi file dari Google Drive (teks / markdown / docs)
    Read {
        /// ID file yang ingin dibaca
        file_id: String,
    },
    /// Buat dokumen baru dari teks langsung
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
    /// Perbarui isi file yang sudah ada di Google Drive
    Update {
        /// ID file yang ingin diperbarui
        file_id: String,
        /// Isi teks konten baru
        content: String,
        /// Tambahkan di baris akhir alih-alih menimpa
        #[arg(short, long)]
        append: bool,
    },
    /// Upload file lokal dari disk ke Google Drive
    Upload {
        /// Path file lokal yang ingin diunggah
        path: PathBuf,
        /// Nama file di Drive (default: nama file lokal)
        #[arg(short, long)]
        name: Option<String>,
        /// ID folder tujuan (opsional)
        #[arg(short, long)]
        parent: Option<String>,
        /// Konversi menjadi native Google Docs
        #[arg(long)]
        docs: bool,
    },
    /// Download file dari Google Drive ke disk lokal (berdasarkan ID, URL, atau nama file)
    Download {
        /// ID file, URL Google Drive, atau nama file yang ingin diunduh
        target: String,
        /// Path file atau folder output tujuan (default: nama file asli dari Drive di folder saat ini)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Buat folder baru di Google Drive
    Mkdir {
        /// Nama folder baru
        name: String,
        /// ID folder induk (opsional)
        #[arg(short, long)]
        parent: Option<String>,
    },
    /// Pindahkan file atau folder ke tempat sampah (Safe Trash)
    Trash {
        /// ID file atau folder yang ingin dipindahkan ke trash
        file_id: String,
    },
    /// Tampilkan daftar Shared Drives (Team Drives) yang dapat diakses
    Drives {
        /// Jumlah maksimal hasil
        #[arg(short, long, default_value = "50")]
        limit: u32,
    },
    /// Jalankan MCP server untuk AI agents (Hermes, Claude, OpenCode)
    Mcp {
        #[command(subcommand)]
        action: McpCommands,
    },
}

#[derive(Subcommand)]
enum AuthCommands {
    /// Login interaktif menggunakan Google OAuth2 (browser lokal atau remote terminal)
    Login {
        /// Jangan membuka browser secara otomatis (hanya tampilkan URL otorisasi)
        #[arg(long)]
        no_browser: bool,
    },
    /// Jalankan interactive setup wizard untuk konfigurasi Google OAuth Client ID & Secret
    Setup,
    /// Cek status token autentikasi yang tersimpan saat ini
    Status,
    /// Hapus token autentikasi yang tersimpan (logout)
    Logout,
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
        Commands::Auth { action } => handle_auth(action).await,
        Commands::Search {
            query,
            folder,
            limit,
            full_text,
            order_by,
            page_token,
            json,
            plain,
        } => {
            handle_search(
                query, folder, limit, full_text, order_by, page_token, json, plain,
            )
            .await
        }
        Commands::Rename { file_id, new_name } => handle_rename(file_id, new_name).await,
        Commands::Move {
            file_id,
            target_folder,
            from,
        } => handle_move(file_id, target_folder, from).await,
        Commands::Read { file_id } => handle_read(file_id).await,
        Commands::Create {
            name,
            content,
            parent,
            docs,
        } => handle_create(name, content, parent, docs).await,
        Commands::Update {
            file_id,
            content,
            append,
        } => handle_update(file_id, content, append).await,
        Commands::Upload {
            path,
            name,
            parent,
            docs,
        } => handle_upload(path, name, parent, docs).await,
        Commands::Download { target, output } => handle_download(target, output).await,
        Commands::Mkdir { name, parent } => handle_mkdir(name, parent).await,
        Commands::Trash { file_id } => handle_trash(file_id).await,
        Commands::Drives { limit } => handle_drives(limit).await,
        Commands::Mcp { action } => handle_mcp(action).await,
    }
}

async fn handle_auth(action: AuthCommands) -> Result<()> {
    match action {
        AuthCommands::Login { no_browser } => {
            let auth = match AuthManager::from_env_or_config() {
                Ok(a) => a,
                Err(err) => {
                    use std::io::Write;
                    eprintln!("⚠️  {}", err);
                    eprint!("\nApakah Anda ingin menjalankan Interactive Setup Wizard sekarang? [Y/n]: ");
                    std::io::stderr().flush()?;
                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input)?;
                    let trimmed = input.trim();
                    if trimmed.is_empty()
                        || trimmed.eq_ignore_ascii_case("y")
                        || trimmed.eq_ignore_ascii_case("yes")
                    {
                        handle_setup().await?;
                        return Ok(());
                    } else {
                        return Err(err);
                    }
                }
            };
            auth.login(no_browser).await?;
            eprintln!("Otorisasi Google Drive berhasil! Token telah disimpan.");
        }
        AuthCommands::Setup => {
            handle_setup().await?;
        }
        AuthCommands::Status => match AuthManager::load_saved_token() {
            Ok(token) => {
                let path = AuthManager::token_path()?;
                eprintln!("Status: Terautentikasi");
                eprintln!("Lokasi token: {:?}", path);
                if let Some(remaining) = token.seconds_remaining() {
                    let mins = remaining / 60;
                    let secs = remaining % 60;
                    eprintln!("Masa berlaku token: {}m {}s tersisa", mins, secs);
                } else {
                    eprintln!("Masa berlaku token: Tidak ada info expiry");
                }
                if token.refresh_token.is_some() {
                    eprintln!("Auto-refresh: Aktif (refresh token tersedia)");
                } else {
                    eprintln!("Auto-refresh: Non-aktif (refresh token tidak tersedia)");
                }
            }
            Err(e) => {
                eprintln!("Status: Belum terautentikasi ({})", e);
            }
        },
        AuthCommands::Logout => {
            AuthManager::logout()?;
            eprintln!("Berhasil logout. File token telah dihapus.");
        }
    }
    Ok(())
}

async fn handle_setup() -> Result<()> {
    use std::io::Write;

    eprintln!("\n=======================================================");
    eprintln!("        🚀 Google Drive OAuth Setup Wizard");
    eprintln!("=======================================================");
    eprintln!("Wizard ini akan memandu Anda menyiapkan Google OAuth credentials");
    eprintln!("agar gdrive-mcp dapat terhubung dengan aman ke akun Google Drive Anda.\n");

    // Langkah 1: Enable Google Drive API
    let api_url = "https://console.cloud.google.com/flows/enableapi?apiid=drive.googleapis.com";
    eprintln!("📍 LANGKAH 1: Aktifkan Google Drive API");
    eprintln!("   Link: {}", api_url);
    eprintln!("   Checklist:");
    eprintln!("   [ ] Pilih atau buat Google Cloud Project (bebas)");
    eprintln!("   [ ] Klik tombol 'Enable' / 'Aktifkan'\n");
    eprint!("   Buka link Langkah 1 di browser sekarang? [Y/n]: ");
    std::io::stderr().flush()?;
    let mut ans1 = String::new();
    std::io::stdin().read_line(&mut ans1)?;
    if ans1.trim().is_empty()
        || ans1.trim().eq_ignore_ascii_case("y")
        || ans1.trim().eq_ignore_ascii_case("yes")
    {
        let _ = open::that(api_url);
    }

    // Langkah 2: OAuth Consent Screen & Test Users
    let consent_url = "https://console.cloud.google.com/apis/credentials/consent";
    eprintln!("\n-------------------------------------------------------");
    eprintln!("📍 LANGKAH 2: Konfigurasi OAuth Consent Screen & Test Users");
    eprintln!("   Link: {}", consent_url);
    eprintln!("   Checklist:");
    eprintln!("   [ ] User Type: Pilih 'External' lalu klik Create");
    eprintln!("   [ ] Isi App Name: 'gdrive-cli' & email support Anda");
    eprintln!("   [ ] Pada tab 'Test Users': TAMBAHKAN EMAIL GOOGLE ANDA (Wajib)");
    eprintln!("   ℹ️  Tips: Peringatan 'Google hasn't verified this app' saat login");
    eprintln!("       adalah normal untuk project pribadi (klik Advanced > Go to gdrive-cli).\n");
    eprint!("   Buka link Langkah 2 di browser sekarang? [Y/n]: ");
    std::io::stderr().flush()?;
    let mut ans2 = String::new();
    std::io::stdin().read_line(&mut ans2)?;
    if ans2.trim().is_empty()
        || ans2.trim().eq_ignore_ascii_case("y")
        || ans2.trim().eq_ignore_ascii_case("yes")
    {
        let _ = open::that(consent_url);
    }

    // Langkah 3: Buat OAuth Client ID (Desktop App)
    let cred_url = "https://console.cloud.google.com/apis/credentials/oauthclient";
    eprintln!("\n-------------------------------------------------------");
    eprintln!("📍 LANGKAH 3: Buat OAuth Client ID (Desktop App)");
    eprintln!("   Link: {}", cred_url);
    eprintln!("   Checklist:");
    eprintln!("   [ ] Application type: Pilih 'Desktop App'");
    eprintln!("   [ ] Name: 'gdrive-cli' (bebas)");
    eprintln!("   [ ] Klik tombol 'Create'\n");
    eprint!("   Buka link Langkah 3 di browser sekarang? [Y/n]: ");
    std::io::stderr().flush()?;
    let mut ans3 = String::new();
    std::io::stdin().read_line(&mut ans3)?;
    if ans3.trim().is_empty()
        || ans3.trim().eq_ignore_ascii_case("y")
        || ans3.trim().eq_ignore_ascii_case("yes")
    {
        let _ = open::that(cred_url);
    }

    // Langkah 4: Masukkan Client ID & Secret
    eprintln!("\n-------------------------------------------------------");
    eprintln!("📍 LANGKAH 4: Masukkan Credentials");
    eprintln!("Salin nilai yang muncul di pop-up Google Cloud Console:\n");
    let mut client_id = String::new();
    loop {
        eprint!("👉 Masukkan Client ID: ");
        std::io::stderr().flush()?;
        client_id.clear();
        std::io::stdin().read_line(&mut client_id)?;
        let trimmed = client_id.trim();
        if !trimmed.is_empty() {
            client_id = trimmed.to_string();
            break;
        }
        eprintln!("⚠️  Client ID tidak boleh kosong!");
    }

    let mut client_secret = String::new();
    loop {
        eprint!("👉 Masukkan Client Secret: ");
        std::io::stderr().flush()?;
        client_secret.clear();
        std::io::stdin().read_line(&mut client_secret)?;
        let trimmed = client_secret.trim();
        if !trimmed.is_empty() {
            client_secret = trimmed.to_string();
            break;
        }
        eprintln!("⚠️  Client Secret tidak boleh kosong!");
    }

    // Simpan credentials
    let saved_path = AuthManager::save_credentials(&client_id, &client_secret)?;
    eprintln!("\n✅ Credentials berhasil disimpan di: {:?}", saved_path);

    // Langsung lanjut proses otorisasi login Google
    eprintln!("\nMelanjutkan langsung ke proses otorisasi login Google...");
    let auth = AuthManager::from_env_or_config()?;
    auth.login(false).await?;
    eprintln!(
        "\n🎉 Otorisasi Google Drive berhasil! Token telah disimpan dan gdrive siap digunakan!"
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_search(
    query: Option<String>,
    folder: Option<String>,
    limit: u32,
    full_text: bool,
    order_by: Option<String>,
    page_token: Option<String>,
    json: bool,
    plain: bool,
) -> Result<()> {
    let client = GDriveClient::from_saved_token().await?;
    let res = client
        .search_files_with_options(gdrive_core::SearchFileOptions {
            query,
            folder_id: folder,
            page_size: Some(limit),
            page_token,
            full_text,
            order_by,
        })
        .await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&res.files)?);
    } else if plain {
        for f in &res.files {
            println!("{} | {} | {}", f.id, f.name, f.mime_type);
        }
    } else {
        println!("{}", format::render_files_table(&res.files));
    }

    if let Some(token) = res.next_page_token {
        eprintln!(
            "\n👉 Halaman berikutnya tersedia. Gunakan --page-token {}",
            token
        );
    }
    Ok(())
}

async fn handle_rename(file_id: String, new_name: String) -> Result<()> {
    let client = GDriveClient::from_saved_token().await?;
    let renamed = client.rename_file(&file_id, &new_name).await?;
    eprintln!("Berhasil mengubah nama: ID={}", renamed.id);
    println!("ID: {}\nNama: {}", renamed.id, renamed.name);
    Ok(())
}

async fn handle_move(file_id: String, target_folder: String, from: Option<String>) -> Result<()> {
    let client = GDriveClient::from_saved_token().await?;
    let moved = client
        .move_file(&file_id, &target_folder, from.as_deref())
        .await?;
    eprintln!("Berhasil memindahkan: ID={}", moved.id);
    println!("ID: {}\nNama: {}", moved.id, moved.name);
    Ok(())
}

async fn handle_read(file_id: String) -> Result<()> {
    let client = GDriveClient::from_saved_token().await?;
    let text = client.read_file(&file_id).await?;
    println!("{}", text);
    Ok(())
}

async fn handle_create(
    name: String,
    content: String,
    parent: Option<String>,
    docs: bool,
) -> Result<()> {
    let client = GDriveClient::from_saved_token().await?;
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
    Ok(())
}

async fn handle_update(file_id: String, content: String, append: bool) -> Result<()> {
    let client = GDriveClient::from_saved_token().await?;
    let updated = client
        .update_file(UpdateFileOptions {
            file_id,
            content,
            append,
        })
        .await?;
    eprintln!("File berhasil diperbarui: ID={}", updated.id);
    if let Some(link) = updated.web_view_link {
        println!("Link: {}", link);
    }
    Ok(())
}

async fn handle_upload(
    path: PathBuf,
    name: Option<String>,
    parent: Option<String>,
    docs: bool,
) -> Result<()> {
    let client = GDriveClient::from_saved_token().await?;
    eprintln!("Mengunggah file dari {:?}...", path);
    let uploaded = client
        .upload_file_path(&path, name.as_deref(), parent.as_deref(), docs)
        .await?;
    eprintln!("File berhasil diunggah: ID={}", uploaded.id);
    println!("ID: {}", uploaded.id);
    if let Some(link) = uploaded.web_view_link {
        println!("Link: {}", link);
    }
    Ok(())
}

async fn resolve_download_target(client: &GDriveClient, target: &str) -> Result<(String, String)> {
    let cleaned = GDriveClient::clean_file_id(target);
    let looks_like_id = cleaned.len() >= 25 && !cleaned.contains(' ') && !cleaned.contains('.');

    if looks_like_id {
        return Ok((cleaned.to_string(), String::new()));
    }

    // Cari berdasarkan nama file
    eprintln!("🔍 Mencari file dengan nama: \"{}\"...", target);
    let candidates = client.find_files_by_name(target, 10).await?;

    if candidates.is_empty() {
        bail!(
            "Tidak ditemukan file dengan nama \"{}\".\nTips: Gunakan 'gdrive search \"{}\"' untuk mencari lebih lanjut.",
            target, target
        );
    }

    if candidates.len() == 1 {
        let f = &candidates[0];
        eprintln!("🎯 Ditemukan: {} (ID: {})", f.name, f.id);
        return Ok((f.id.clone(), f.name.clone()));
    }

    // Tampilkan pilihan interaktif
    eprintln!("\n⚠️  Ditemukan beberapa file yang cocok. Silakan pilih nomor file:");
    for (idx, f) in candidates.iter().enumerate() {
        let (icon, _label) = format::format_mime(&f.mime_type);
        let size = format::format_size(f.size.as_deref());
        eprintln!(
            "  [{}] {} {} ({}) - ID: {}",
            idx + 1,
            icon,
            f.name,
            size,
            f.id
        );
    }

    loop {
        eprint!("\n👉 Masukkan nomor pilihan [1-{}]: ", candidates.len());
        std::io::stderr().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if let Ok(choice) = input.trim().parse::<usize>() {
            if (1..=candidates.len()).contains(&choice) {
                let chosen = &candidates[choice - 1];
                return Ok((chosen.id.clone(), chosen.name.clone()));
            }
        }
        eprintln!("Nomor tidak valid, silakan coba lagi.");
    }
}

async fn handle_download(target: String, output: Option<PathBuf>) -> Result<()> {
    let client = GDriveClient::from_saved_token().await?;
    let (file_id, _) = resolve_download_target(&client, &target).await?;

    let download_stream = client.download_file_stream(&file_id).await?;
    let orig_name = download_stream.name;
    let total_size = download_stream.size;

    let out_path = match output {
        Some(p) => {
            if p.is_dir() {
                p.join(&orig_name)
            } else {
                p
            }
        }
        None => std::env::current_dir()?.join(&orig_name),
    };

    let pb = match total_size {
        Some(len) => {
            let p = ProgressBar::new(len);
            p.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
                    .progress_chars("#>-"),
            );
            p
        }
        None => {
            let p = ProgressBar::new_spinner();
            p.set_style(
                ProgressStyle::default_spinner().template("{spinner:.green} {bytes} downloaded")?,
            );
            p
        }
    };

    let mut file = std::fs::File::create(&out_path)
        .with_context(|| format!("Gagal membuat file lokal di {:?}", out_path))?;
    let mut stream = download_stream.stream;
    let mut downloaded_bytes: u64 = 0;

    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res.context("Koneksi terputus saat streaming download file")?;
        file.write_all(&chunk)?;
        downloaded_bytes += chunk.len() as u64;
        pb.inc(chunk.len() as u64);
    }

    pb.finish_and_clear();

    let abs_path = std::fs::canonicalize(&out_path).unwrap_or(out_path);
    let size_display = format::format_size(Some(&downloaded_bytes.to_string()));

    eprintln!("✅ Berhasil mengunduh \"{}\" ({})", orig_name, size_display);
    eprintln!("📁 Lokasi file: {}", abs_path.display());
    println!("{}", abs_path.display());
    Ok(())
}

async fn handle_mkdir(name: String, parent: Option<String>) -> Result<()> {
    let client = GDriveClient::from_saved_token().await?;
    let folder = client.create_folder(&name, parent.as_deref()).await?;
    eprintln!("Folder berhasil dibuat: ID={}", folder.id);
    println!("ID: {}", folder.id);
    if let Some(link) = folder.web_view_link {
        println!("Link: {}", link);
    }
    Ok(())
}

async fn handle_trash(file_id: String) -> Result<()> {
    let client = GDriveClient::from_saved_token().await?;
    let trashed = client.trash_file(&file_id).await?;
    eprintln!(
        "File berhasil dipindahkan ke tempat sampah: ID={}",
        trashed.id
    );
    println!("Trashed ID: {}", trashed.id);
    Ok(())
}

async fn handle_drives(limit: u32) -> Result<()> {
    let client = GDriveClient::from_saved_token().await?;
    let drives = client.list_drives(Some(limit)).await?;
    if drives.is_empty() {
        eprintln!("Tidak ada Shared Drives yang ditemukan.");
    } else {
        for d in drives {
            println!("{} | {}", d.id, d.name);
        }
    }
    Ok(())
}

async fn handle_mcp(action: McpCommands) -> Result<()> {
    match action {
        McpCommands::Serve => {
            gdrive_mcp::run_stdio().await?;
        }
    }
    Ok(())
}
