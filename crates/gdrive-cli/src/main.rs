use std::io::{IsTerminal, Write};
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use comfy_table::{presets::UTF8_FULL, Attribute, Cell, Color, ContentArrangement, Table};
use futures_util::StreamExt;
use gdrive_core::{
    AuthManager, CreateFileOptions, CreatePermissionOptions, GDriveClient, UpdateFileOptions,
};
use indicatif::{ProgressBar, ProgressStyle};
use tracing_subscriber::EnvFilter;

mod format;

#[derive(Parser)]
#[command(
    name = "gdrive",
    author = "connaners",
    version,
    about = "Google Drive CLI & MCP Server"
)]
struct Cli {
    /// Nama profile Google Drive yang digunakan (contoh: work, personal, default)
    #[arg(long, global = true)]
    pub profile: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Manajemen autentikasi Google OAuth2
    Auth {
        #[command(subcommand)]
        action: AuthCommands,
    },
    /// Tampilkan informasi akun Google Drive dan kuota penyimpanan (storage quota)
    About {
        /// Tampilkan output dalam format JSON mentah
        #[arg(long)]
        json: bool,
    },
    /// Cari file atau folder di Google Drive
    Search {
        /// Kata kunci query nama file (dapat berupa banyak kata tanpa tanda petik, misal: gdrive search iboss v2)
        #[arg(num_args(0..))]
        query: Vec<String>,
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
    /// Tampilkan informasi metadata detail file atau folder di Google Drive
    Info {
        /// ID file atau folder yang ingin diperiksa
        file_id: String,
        /// Tampilkan output dalam format JSON mentah
        #[arg(long)]
        json: bool,
    },
    /// Salin (duplicate) file di Google Drive
    Copy {
        /// ID file yang ingin disalin
        file_id: String,
        /// Nama baru untuk file salinan (opsional)
        #[arg(short, long)]
        name: Option<String>,
        /// ID folder tujuan file salinan (opsional)
        #[arg(short, long)]
        parent: Option<String>,
        /// Tampilkan output dalam format JSON mentah
        #[arg(long)]
        json: bool,
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
    /// Perbarui isi file yang sudah ada di Google Drive (Catatan: Untuk memperbarui binary aplikasi gdrive, gunakan 'gdrive self-update' atau 'gdrive upgrade')
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
    /// Kelola tempat sampah (Trash) Google Drive
    Trash {
        #[command(subcommand)]
        action: Option<TrashCommands>,
        /// ID file atau folder yang ingin dipindahkan ke tempat sampah (shortcut: gdrive trash <file_id>)
        #[arg(value_name = "FILE_ID")]
        file_id: Option<String>,
    },
    /// Tampilkan daftar Shared Drives (Team Drives) yang dapat diakses
    Drives {
        /// Jumlah maksimal hasil
        #[arg(short, long, default_value = "50")]
        limit: u32,
    },
    /// Bagikan file atau folder ke pengguna, grup, domain, atau publik (anyone)
    Share {
        /// ID file atau folder yang ingin dibagikan
        file_id: String,
        /// Role / hak akses ('reader', 'commenter', 'writer', 'fileOrganizer')
        #[arg(short, long, default_value = "reader")]
        role: String,
        /// Tipe target ('anyone', 'user', 'group', 'domain')
        #[arg(short = 't', long, default_value = "anyone")]
        target_type: String,
        /// Alamat email target (wajib jika target-type adalah 'user' atau 'group')
        #[arg(short, long)]
        email: Option<String>,
        /// Nama domain (wajib jika target-type adalah 'domain', misal: company.com)
        #[arg(long)]
        domain: Option<String>,
        /// Kirim notifikasi email ke target saat dibagikan
        #[arg(long)]
        notification: bool,
        /// Tampilkan output dalam format JSON mentah
        #[arg(long)]
        json: bool,
    },
    /// Kelola hak akses (permissions) file atau folder di Google Drive
    Permissions {
        #[command(subcommand)]
        action: PermissionsCommands,
    },
    /// Kelola komentar dan review pada file di Google Drive
    Comments {
        #[command(subcommand)]
        action: CommentsCommands,
    },
    /// Kelola riwayat revisi (versi) file di Google Drive
    Revisions {
        #[command(subcommand)]
        action: RevisionsCommands,
    },
    /// Jalankan MCP server untuk AI agents (Hermes, Claude, OpenCode)
    Mcp {
        #[command(subcommand)]
        action: McpCommands,
    },
    /// Periksa dan perbarui binary aplikasi gdrive ke versi terbaru dari GitHub Releases
    #[command(name = "self-update", alias = "upgrade")]
    SelfUpdate {
        /// Hanya periksa ketersediaan versi terbaru tanpa melakukan pengunduhan
        #[arg(short, long)]
        check: bool,
        /// Tampilkan catatan rilis (changelog) lengkap dari versi terbaru
        #[arg(long)]
        changelog: bool,
        /// Tampilkan output dalam format JSON mentah
        #[arg(long)]
        json: bool,
    },
    /// Buka Terminal User Interface (TUI) interaktif untuk navigasi Google Drive
    #[command(name = "tui", alias = "ui")]
    Tui {
        /// ID folder awal yang ingin dibuka (opsional)
        #[arg(short, long)]
        folder: Option<String>,
        /// Jalankan dalam mode demo interaktif dengan mock data tanpa perlu login Google
        #[arg(long)]
        demo: bool,
    },
    /// Jalankan pemeriksaan diagnostik lingkungan kerja, kredensial, konektivitas, dan terminal
    Doctor,
}

#[derive(Subcommand)]
enum PermissionsCommands {
    /// Tampilkan daftar permission yang dimiliki oleh suatu file atau folder
    List {
        /// ID file atau folder yang ingin diperiksa
        file_id: String,
        /// Tampilkan output dalam format JSON mentah
        #[arg(long)]
        json: bool,
    },
    /// Hapus/cabut hak akses (permission) dari suatu file atau folder
    Revoke {
        /// ID file atau folder
        file_id: String,
        /// ID permission yang ingin dihapus
        permission_id: String,
    },
}

#[derive(Subcommand)]
enum TrashCommands {
    /// Pindahkan file atau folder ke tempat sampah (Safe Trash)
    Put {
        /// ID file atau folder
        file_id: String,
    },
    /// Tampilkan daftar file dan folder yang berada di tempat sampah
    List {
        /// Batas jumlah file yang ditampilkan
        #[arg(short, long, default_value = "50")]
        limit: u32,
        /// Token halaman untuk melanjutkan pagination
        #[arg(long)]
        page_token: Option<String>,
        /// Tampilkan output dalam format JSON mentah
        #[arg(long)]
        json: bool,
    },
    /// Pulihkan file atau folder dari tempat sampah (untrash)
    Restore {
        /// ID file atau folder yang ingin dipulihkan
        file_id: String,
    },
    /// Kosongkan seluruh tempat sampah secara permanen
    Empty {
        /// Lewati konfirmasi interaktif
        #[arg(short = 'y', long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
enum CommentsCommands {
    /// Tampilkan daftar komentar pada file Google Drive
    List {
        /// ID file yang ingin diperiksa komentarnya
        file_id: String,
        /// Batas jumlah komentar (default: 50)
        #[arg(short, long, default_value = "50")]
        limit: u32,
        /// Tampilkan output dalam format JSON mentah
        #[arg(long)]
        json: bool,
    },
    /// Tambahkan komentar baru pada file Google Drive
    Add {
        /// ID file target
        file_id: String,
        /// Isi teks komentar
        content: String,
        /// Tampilkan output dalam format JSON mentah
        #[arg(long)]
        json: bool,
    },
    /// Tandai komentar sebagai terselesaikan (resolved)
    Resolve {
        /// ID file target
        file_id: String,
        /// ID komentar yang ingin diselesaikan
        comment_id: String,
    },
}

#[derive(Subcommand)]
enum RevisionsCommands {
    /// Tampilkan riwayat revisi/versi dari suatu file
    List {
        /// ID file yang ingin diperiksa
        file_id: String,
        /// Tampilkan output dalam format JSON mentah
        #[arg(long)]
        json: bool,
    },
    /// Unduh file pada revisi/versi spesifik
    Download {
        /// ID file
        file_id: String,
        /// ID revisi yang ingin diunduh
        revision_id: String,
        /// Path output tujuan penyimpanan file (opsional)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum AuthCommands {
    /// Login interaktif menggunakan Google OAuth2 (browser lokal atau remote terminal)
    Login {
        /// Jangan membuka browser secara otomatis (hanya tampilkan URL otorisasi)
        #[arg(long)]
        no_browser: bool,
        /// Login ke profile spesifik
        #[arg(long)]
        profile: Option<String>,
    },
    /// Jalankan interactive setup wizard untuk konfigurasi Google OAuth Client ID & Secret
    Setup {
        /// Lewati browser otomatis & otorisasi OAuth langsung (hanya simpan credentials)
        #[arg(long)]
        skip_login: bool,
    },
    /// Cek status token autentikasi yang tersimpan saat ini
    Status {
        /// Periksa status profile tertentu
        #[arg(long)]
        profile: Option<String>,
    },
    /// Hapus token autentikasi yang tersimpan (logout)
    Logout {
        /// Logout dari profile tertentu
        #[arg(long)]
        profile: Option<String>,
    },
    /// Kelola profil akun Google Drive (multi-account)
    Profile {
        #[command(subcommand)]
        action: ProfileCommands,
    },
}

#[derive(Subcommand)]
enum ProfileCommands {
    /// Tampilkan daftar profil dan tandai mana yang aktif
    List,
    /// Ganti profil aktif
    Switch {
        /// Nama profil yang ingin diaktifkan
        name: String,
    },
    /// Hapus profil dan token yang terkait
    Remove {
        /// Nama profil yang ingin dihapus
        name: String,
    },
}

#[derive(Subcommand)]
enum McpCommands {
    /// Jalankan MCP server via stdio (standard input/output)
    Serve,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli_res = Cli::try_parse();

    // Deteksi apakah sedang menjalankan mode TUI untuk mengalihkan tracing ke sink agar tidak mengotori alternate screen
    let is_tui = match &cli_res {
        Ok(cli) => match &cli.command {
            Some(Commands::Tui { .. }) => true,
            None => std::io::stdin().is_terminal() && std::io::stdout().is_terminal(),
            _ => false,
        },
        Err(_) => false,
    };

    if is_tui {
        tracing_subscriber::fmt().with_writer(std::io::sink).init();
    } else {
        // Arahkan semua logging ke stderr agar stdout murni untuk JSON-RPC MCP
        tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()),
            )
            .with_writer(std::io::stderr)
            .init();
    }

    let cli = match cli_res {
        Ok(c) => c,
        Err(e) => e.exit(),
    };

    if let Err(err) = run(cli).await {
        eprintln!("❌ Error: {:#}", err);
        std::process::exit(1);
    }

    Ok(())
}

async fn dispatch_file_mutation(cmd: Commands) -> Result<Option<Commands>> {
    match cmd {
        Commands::Copy {
            file_id,
            name,
            parent,
            json,
        } => {
            handle_copy(file_id, name, parent, json).await?;
            Ok(None)
        }
        Commands::Rename { file_id, new_name } => {
            handle_rename(file_id, new_name).await?;
            Ok(None)
        }
        Commands::Move {
            file_id,
            target_folder,
            from,
        } => {
            handle_move(file_id, target_folder, from).await?;
            Ok(None)
        }
        Commands::Create {
            name,
            content,
            parent,
            docs,
        } => {
            handle_create(name, content, parent, docs).await?;
            Ok(None)
        }
        Commands::Update {
            file_id,
            content,
            append,
        } => {
            handle_update(file_id, content, append).await?;
            Ok(None)
        }
        Commands::Upload {
            path,
            name,
            parent,
            docs,
        } => {
            handle_upload(path, name, parent, docs).await?;
            Ok(None)
        }
        Commands::Mkdir { name, parent } => {
            handle_mkdir(name, parent).await?;
            Ok(None)
        }
        Commands::Trash { action, file_id } => {
            handle_trash(action, file_id).await?;
            Ok(None)
        }
        other => Ok(Some(other)),
    }
}

async fn dispatch_query_command(cmd: Commands) -> Result<Option<Commands>> {
    match cmd {
        Commands::About { json } => {
            handle_about(json).await?;
            Ok(None)
        }
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
            .await?;
            Ok(None)
        }
        Commands::Info { file_id, json } => {
            handle_info(file_id, json).await?;
            Ok(None)
        }
        Commands::Read { file_id } => {
            handle_read(file_id).await?;
            Ok(None)
        }
        Commands::Download { target, output } => {
            handle_download(target, output).await?;
            Ok(None)
        }
        Commands::Drives { limit } => {
            handle_drives(limit).await?;
            Ok(None)
        }
        other => Ok(Some(other)),
    }
}

async fn dispatch_collaboration_command(cmd: Commands) -> Result<()> {
    match cmd {
        Commands::Auth { action } => handle_auth(action).await,
        Commands::Share {
            file_id,
            role,
            target_type,
            email,
            domain,
            notification,
            json,
        } => {
            handle_share(
                file_id,
                role,
                target_type,
                email,
                domain,
                notification,
                json,
            )
            .await
        }
        Commands::Permissions { action } => handle_permissions(action).await,
        Commands::Comments { action } => handle_comments(action).await,
        Commands::Revisions { action } => handle_revisions(action).await,
        Commands::Mcp { action } => handle_mcp(action).await,
        Commands::SelfUpdate {
            check,
            changelog,
            json,
        } => handle_self_update(check, changelog, json).await,
        Commands::Doctor => handle_doctor().await,
        _ => unreachable!(),
    }
}

async fn run(cli: Cli) -> Result<()> {
    if let Some(ref p) = cli.profile {
        let trimmed = p.trim();
        if !trimmed.is_empty() {
            std::env::set_var("GDRIVE_PROFILE", trimmed);
        }
    }

    let cmd = match cli.command {
        Some(c) => c,
        None => {
            if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
                return handle_tui(cli.profile, None, false).await;
            } else {
                Cli::command().print_help()?;
                println!();
                return Ok(());
            }
        }
    };

    if let Commands::Tui { folder, demo } = cmd {
        return handle_tui(cli.profile, folder, demo).await;
    }

    let cmd = match dispatch_file_mutation(cmd).await? {
        Some(c) => c,
        None => return Ok(()),
    };

    let cmd = match dispatch_query_command(cmd).await? {
        Some(c) => c,
        None => return Ok(()),
    };

    dispatch_collaboration_command(cmd).await
}

async fn handle_tui(profile: Option<String>, folder: Option<String>, demo: bool) -> Result<()> {
    gdrive_tui::run_tui(profile, folder, demo).await
}

async fn handle_doctor() -> Result<()> {
    println!("\n=======================================================");
    println!("        🩺 Google Drive MCP & CLI Doctor");
    println!("=======================================================");

    let is_term = std::io::stdout().is_terminal();
    let term_size = crossterm::terminal::size().ok();
    let terminal_info = Some((is_term, term_size));

    let report = gdrive_core::run_doctor_checks(None, terminal_info).await;

    println!("{}", format::render_doctor_table(&report));
    println!(
        "\nHasil Pemeriksaan: {} OK, {} Warning, {} Failure\n",
        report.ok_count, report.warn_count, report.fail_count
    );

    Ok(())
}

async fn handle_auth_login(no_browser: bool, profile: Option<String>) -> Result<()> {
    let env_profile = std::env::var("GDRIVE_PROFILE").ok();
    let target_profile = profile
        .as_deref()
        .or_else(|| env_profile.as_deref().filter(|s| !s.trim().is_empty()));
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
                handle_setup(false).await?;
                return Ok(());
            } else {
                return Err(err);
            }
        }
    };
    if let Some(p) = target_profile {
        eprintln!("Memulai login untuk profil '{}'...", p);
    }
    auth.login_for_profile(target_profile, no_browser).await?;
    eprintln!("Otorisasi Google Drive berhasil! Token telah disimpan.");
    Ok(())
}

fn handle_auth_status(profile: Option<String>) -> Result<()> {
    let active = AuthManager::get_active_profile();
    let target = profile.as_deref().unwrap_or(&active);
    println!("Profil: '{}' (aktif: '{}')", target, active);
    match AuthManager::load_saved_token_for_profile(Some(target)) {
        Ok(token) => {
            let path = AuthManager::token_path_for_profile(Some(target))?;
            println!("Status: Terautentikasi");
            let name = token
                .display_name
                .as_deref()
                .filter(|s| !s.trim().is_empty());
            let email = token
                .account_email
                .as_deref()
                .filter(|s| !s.trim().is_empty());
            match (name, email) {
                (Some(name), Some(email)) => println!("Akun Google: {} ({})", name, email),
                (None, Some(email)) => println!("Akun Google: {}", email),
                (Some(name), None) => println!("Akun Google: {}", name),
                (None, None) => println!("Akun Google: Terhubung (tanpa detail email)"),
            }
            println!("Lokasi token: {:?}", path);
            if let Some(remaining) = token.seconds_remaining() {
                let mins = remaining / 60;
                let secs = remaining % 60;
                println!("Masa berlaku token: {}m {}s tersisa", mins, secs);
            } else {
                println!("Masa berlaku token: Tidak ada info expiry");
            }
            if token.refresh_token.is_some() {
                println!("Auto-refresh: Aktif (refresh token tersedia)");
            } else {
                println!("Auto-refresh: Non-aktif (refresh token tidak tersedia)");
            }
        }
        Err(e) => {
            println!("Status: Belum terautentikasi ({})", e);
        }
    }
    Ok(())
}

fn handle_auth_logout(profile: Option<String>) -> Result<()> {
    let active = AuthManager::get_active_profile();
    let target = profile.as_deref().unwrap_or(&active);
    AuthManager::logout_for_profile(Some(target))?;
    eprintln!(
        "Berhasil logout dari profil '{}'. File token telah dihapus.",
        target
    );
    Ok(())
}

fn handle_auth_profile(action: ProfileCommands) -> Result<()> {
    match action {
        ProfileCommands::List => {
            let statuses = AuthManager::get_all_profiles_status()?;
            println!("{}", format::render_profiles_table(&statuses));
        }
        ProfileCommands::Switch { name } => {
            let trimmed = AuthManager::validate_profile_name(&name)?;
            let is_authenticated = AuthManager::load_saved_token_for_profile(Some(trimmed)).is_ok();
            AuthManager::set_active_profile(trimmed)?;
            eprintln!("✅ Berhasil beralih ke profil '{}'.", trimmed);
            if !is_authenticated {
                eprintln!(
                    "⚠️ Profil '{}' belum terautentikasi. Jalankan 'gdrive auth login --profile {}' untuk melakukan login.",
                    trimmed, trimmed
                );
            }
            println!("Active profile: {}", trimmed);
        }
        ProfileCommands::Remove { name } => {
            AuthManager::remove_profile(&name)?;
            eprintln!("✅ Berhasil menghapus profil '{}'.", name);
            println!("Removed profile: {}", name);
        }
    }
    Ok(())
}

async fn handle_auth(action: AuthCommands) -> Result<()> {
    match action {
        AuthCommands::Login {
            no_browser,
            profile,
        } => handle_auth_login(no_browser, profile).await,
        AuthCommands::Setup { skip_login } => handle_setup(skip_login).await,
        AuthCommands::Status { profile } => handle_auth_status(profile),
        AuthCommands::Logout { profile } => handle_auth_logout(profile),
        AuthCommands::Profile { action } => handle_auth_profile(action),
    }
}

async fn handle_setup(skip_login: bool) -> Result<()> {
    use std::io::Write;

    eprintln!("\n=======================================================");
    eprintln!("        🚀 Google Drive OAuth Setup Wizard");
    eprintln!("=======================================================");
    eprintln!("Wizard ini akan memandu Anda menyiapkan Google OAuth credentials");
    eprintln!("agar gdrive-mcp dapat terhubung dengan aman ke akun Google Drive Anda.\n");

    if !skip_login {
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
        eprintln!(
            "       adalah normal untuk project pribadi (klik Advanced > Go to gdrive-cli).\n"
        );
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
        let bytes_read = std::io::stdin().read_line(&mut client_id)?;
        if bytes_read == 0 {
            bail!("Input stream berakhir sebelum Client ID dimasukkan");
        }
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
        let bytes_read = std::io::stdin().read_line(&mut client_secret)?;
        if bytes_read == 0 {
            bail!("Input stream berakhir sebelum Client Secret dimasukkan");
        }
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

    if skip_login {
        return Ok(());
    }

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
    query: Vec<String>,
    folder: Option<String>,
    limit: u32,
    full_text: bool,
    order_by: Option<String>,
    page_token: Option<String>,
    json: bool,
    plain: bool,
) -> Result<()> {
    let query_str = if query.is_empty() {
        None
    } else {
        let joined = query.join(" ").trim().to_string();
        if joined.is_empty() {
            None
        } else {
            Some(joined)
        }
    };

    let client = GDriveClient::from_saved_token().await?;
    let res = client
        .search_files_with_options(gdrive_core::SearchFileOptions {
            query: query_str,
            folder_id: folder,
            page_size: Some(limit),
            page_token,
            full_text,
            order_by,
        })
        .await?;

    if json {
        let output = serde_json::json!({
            "files": res.files,
            "next_page_token": res.next_page_token,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
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

async fn handle_info(file_id: String, json: bool) -> Result<()> {
    let client = GDriveClient::from_saved_token().await?;
    let cleaned = GDriveClient::clean_file_id(&file_id);
    let meta = client.get_file_metadata(cleaned).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&meta)?);
    } else {
        println!("{}", format::render_file_info(&meta));
    }
    Ok(())
}

async fn handle_copy(
    file_id: String,
    name: Option<String>,
    parent: Option<String>,
    json: bool,
) -> Result<()> {
    let client = GDriveClient::from_saved_token().await?;
    let copied = client
        .copy_file(&file_id, name.as_deref(), parent.as_deref())
        .await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&copied)?);
    } else {
        eprintln!("✅ Berhasil menyalin file ke Google Drive.");
        println!("{}", format::render_file_info(&copied));
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
        if let Ok(meta) = client.get_file_metadata(cleaned).await {
            return Ok((meta.id, meta.name));
        }
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
        let bytes_read = std::io::stdin().read_line(&mut input)?;
        if bytes_read == 0 {
            bail!("Input ditutup (EOF) sebelum pilihan file ditentukan.");
        }
        if let Ok(choice) = input.trim().parse::<usize>() {
            if (1..=candidates.len()).contains(&choice) {
                let chosen = &candidates[choice - 1];
                return Ok((chosen.id.clone(), chosen.name.clone()));
            }
        }
        eprintln!("Nomor tidak valid, silakan coba lagi.");
    }
}

async fn write_chunks_to_part_file(
    stream: &mut gdrive_core::DownloadByteStream,
    part_path: &std::path::Path,
    pb: &ProgressBar,
) -> Result<u64> {
    let mut file = std::fs::File::create(part_path)
        .with_context(|| format!("Gagal membuat file sementara di {:?}", part_path))?;
    let mut downloaded_bytes: u64 = 0;

    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res.context("Koneksi terputus saat streaming download file")?;
        file.write_all(&chunk)?;
        downloaded_bytes += chunk.len() as u64;
        pb.inc(chunk.len() as u64);
    }

    file.flush()?;
    Ok(downloaded_bytes)
}

async fn handle_download(target: String, output: Option<PathBuf>) -> Result<()> {
    let client = GDriveClient::from_saved_token().await?;
    let (file_id, _) = resolve_download_target(&client, &target).await?;

    let download_stream = client.download_file_stream(&file_id).await?;
    let orig_name = download_stream.name;
    let total_size = download_stream.size;

    let sanitized_name = format::sanitize_download_filename(&orig_name);

    let out_path = match output {
        Some(p) => {
            if p.is_dir() {
                p.join(&sanitized_name)
            } else {
                p
            }
        }
        None => std::env::current_dir()?.join(&sanitized_name),
    };

    if let Ok(meta) = std::fs::symlink_metadata(&out_path) {
        if meta.file_type().is_symlink() {
            bail!(
                "Tujuan download {:?} adalah symbolic link yang tidak diizinkan.",
                out_path
            );
        }
    }

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

    let part_path = {
        let mut os_str = out_path.as_os_str().to_os_string();
        os_str.push(".gdrive_part");
        PathBuf::from(os_str)
    };

    let mut downloaded_bytes: u64 = 0;
    let mut current_stream = download_stream.stream;
    let mut stream_err = None;

    for attempt in 1..=3 {
        pb.set_position(0);
        match write_chunks_to_part_file(&mut current_stream, &part_path, &pb).await {
            Ok(bytes) => {
                downloaded_bytes = bytes;
                stream_err = None;
                break;
            }
            Err(_e) if attempt < 3 => {
                tracing::warn!("Streaming terputus, mengulang ({}/3)...", attempt);
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                match client.download_file_stream(&file_id).await {
                    Ok(new_dl) => current_stream = new_dl.stream,
                    Err(e) => {
                        stream_err = Some(e);
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

    std::fs::rename(&part_path, &out_path).with_context(|| {
        format!(
            "Gagal memindahkan file dari {:?} ke {:?}",
            part_path, out_path
        )
    })?;

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

async fn handle_trash(action: Option<TrashCommands>, file_id: Option<String>) -> Result<()> {
    let client = GDriveClient::from_saved_token().await?;
    match action {
        Some(TrashCommands::Put { file_id }) => {
            let trashed = client.trash_file(&file_id).await?;
            eprintln!(
                "File berhasil dipindahkan ke tempat sampah: ID={}",
                trashed.id
            );
            println!("Trashed ID: {}", trashed.id);
        }
        Some(TrashCommands::List {
            limit,
            page_token,
            json,
        }) => {
            let list = client
                .list_trash(Some(limit), page_token.as_deref())
                .await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&list)?);
            } else {
                let table = format::render_files_table(&list.files);
                println!("{table}");
            }
        }
        Some(TrashCommands::Restore { file_id }) => {
            let restored = client.restore_trash_file(&file_id).await?;
            eprintln!(
                "✅ File '{}' ({}) berhasil dipulihkan dari tempat sampah.",
                restored.name, restored.id
            );
            println!("Restored ID: {}", restored.id);
        }
        Some(TrashCommands::Empty { yes }) => {
            if !yes {
                eprint!("⚠️ Apakah Anda yakin ingin MENGOSONGKAN tempat sampah secara PERMANEN? (y/N): ");
                std::io::stderr().flush()?;
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    eprintln!("Dibatalkan.");
                    return Ok(());
                }
            }
            client.empty_trash().await?;
            eprintln!("✅ Tempat sampah Google Drive berhasil dikosongkan.");
            println!("Status: trash_emptied");
        }
        None => {
            if let Some(fid) = file_id {
                let trashed = client.trash_file(&fid).await?;
                eprintln!(
                    "File berhasil dipindahkan ke tempat sampah: ID={}",
                    trashed.id
                );
                println!("Trashed ID: {}", trashed.id);
            } else {
                bail!("Harap tentukan file_id yang ingin di-trash atau gunakan subcommand: list, restore, empty, put. Jalankan 'gdrive trash --help'.");
            }
        }
    }
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

async fn handle_about(json: bool) -> Result<()> {
    let client = GDriveClient::from_saved_token().await?;
    let about = client.get_about().await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&about)?);
    } else {
        println!("{}", format::render_about(&about));
    }

    Ok(())
}

async fn handle_share(
    file_id: String,
    role: String,
    target_type: String,
    email: Option<String>,
    domain: Option<String>,
    notification: bool,
    json: bool,
) -> Result<()> {
    let target_type = if target_type == "anyone"
        && email
            .as_deref()
            .map(|e| !e.trim().is_empty())
            .unwrap_or(false)
    {
        "user".to_string()
    } else {
        target_type
    };

    if (target_type == "user" || target_type == "group")
        && email.as_deref().unwrap_or("").trim().is_empty()
    {
        bail!(
            "Argumen --email wajib diisi jika --target-type adalah '{}'.",
            target_type
        );
    }
    if target_type == "domain" && domain.as_deref().unwrap_or("").trim().is_empty() {
        bail!("Argumen --domain wajib diisi jika --target-type adalah 'domain'.");
    }

    let client = GDriveClient::from_saved_token().await?;
    let opts = CreatePermissionOptions {
        role: role.clone(),
        permission_type: target_type.clone(),
        email_address: email.clone(),
        domain: domain.clone(),
        send_notification_email: notification,
    };

    let perm = client.create_permission(&file_id, opts).await?;
    let link = client.get_shareable_link(&file_id, false).await.ok();

    if json {
        let output = serde_json::json!({
            "file_id": file_id,
            "permission": perm,
            "shareable_link": link,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        eprintln!("✅ File berhasil dibagikan!");
        println!("Permission ID : {}", perm.id);
        println!("Role          : {}", perm.role);
        println!("Target Tipe   : {}", perm.permission_type);
        if let Some(target_email) = perm.email_address {
            println!("Email         : {}", target_email);
        }
        if let Some(l) = link {
            println!("Link Berbagi  : {}", l);
        }
    }

    Ok(())
}

async fn handle_permissions(action: PermissionsCommands) -> Result<()> {
    let client = GDriveClient::from_saved_token().await?;
    match action {
        PermissionsCommands::List { file_id, json } => {
            let perms = client.list_permissions(&file_id).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&perms)?);
            } else {
                let table = format::render_permissions_table(&perms);
                println!("{table}");
            }
        }
        PermissionsCommands::Revoke {
            file_id,
            permission_id,
        } => {
            client.delete_permission(&file_id, &permission_id).await?;
            eprintln!(
                "✅ Permission '{}' berhasil dicabut dari file '{}'.",
                permission_id, file_id
            );
            println!("Status: revoked");
        }
    }
    Ok(())
}

async fn handle_comments(action: CommentsCommands) -> Result<()> {
    let client = GDriveClient::from_saved_token().await?;
    match action {
        CommentsCommands::List {
            file_id,
            limit,
            json,
        } => {
            let comments = client.list_comments(&file_id, Some(limit)).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&comments)?);
            } else {
                let table = format::render_comments_table(&comments);
                println!("{table}");
            }
        }
        CommentsCommands::Add {
            file_id,
            content,
            json,
        } => {
            let comment = client.create_comment(&file_id, &content).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&comment)?);
            } else {
                eprintln!("✅ Komentar berhasil ditambahkan ke file '{}'.", file_id);
                println!("ID: {}", comment.id);
                println!("Content: {}", comment.content);
            }
        }
        CommentsCommands::Resolve {
            file_id,
            comment_id,
        } => {
            let comment = client.resolve_comment(&file_id, &comment_id).await?;
            eprintln!(
                "✅ Komentar '{}' berhasil ditandai resolved pada file '{}'.",
                comment_id, file_id
            );
            println!("Status: resolved (ID: {})", comment.id);
        }
    }
    Ok(())
}

async fn handle_revisions(action: RevisionsCommands) -> Result<()> {
    let client = GDriveClient::from_saved_token().await?;
    match action {
        RevisionsCommands::List { file_id, json } => {
            let revs = client.list_revisions(&file_id).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&revs)?);
            } else {
                let table = format::render_revisions_table(&revs);
                println!("{table}");
            }
        }
        RevisionsCommands::Download {
            file_id,
            revision_id,
            output,
        } => {
            let (filename, bytes) = client
                .download_revision_bytes(&file_id, &revision_id)
                .await?;
            let sanitized = format::sanitize_download_filename(&filename);
            let rev_filename = format!("rev_{}_{}", revision_id, sanitized);
            let target_path = match output {
                Some(p) => {
                    if p.is_dir() {
                        p.join(rev_filename)
                    } else {
                        p
                    }
                }
                None => PathBuf::from(rev_filename),
            };

            if target_path.is_symlink() {
                bail!("Destinasi output {:?} adalah symbolic link.", target_path);
            }

            let part_path = target_path.with_extension("part");
            std::fs::write(&part_path, &bytes)
                .with_context(|| format!("Gagal menulis temporary part file ke {:?}", part_path))?;
            std::fs::rename(&part_path, &target_path)
                .with_context(|| format!("Gagal menyimpan revisi file ke {:?}", target_path))?;
            eprintln!(
                "✅ Revisi '{}' berhasil diunduh ke {:?}",
                revision_id, target_path
            );
            println!("{}", target_path.display());
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

async fn handle_self_update(check: bool, changelog: bool, json: bool) -> Result<()> {
    if check {
        let info = gdrive_core::check_for_update(None)
            .await
            .context("Gagal memeriksa pembaruan")?;

        if json {
            println!("{}", serde_json::to_string_pretty(&info)?);
            return Ok(());
        }

        let mut table = Table::new();
        table.load_preset(UTF8_FULL);
        if table.width().unwrap_or(0) >= 60 {
            table.set_content_arrangement(ContentArrangement::Dynamic);
        }

        let status_display = if info.update_available {
            "Ya (Tersedia)"
        } else {
            "Tidak (Versi Terbaru)"
        };

        table.add_row(vec![
            Cell::new("Versi Saat Ini")
                .add_attribute(Attribute::Bold)
                .fg(Color::Cyan),
            Cell::new(format!("v{}", info.current_version)),
        ]);
        table.add_row(vec![
            Cell::new("Versi Terbaru")
                .add_attribute(Attribute::Bold)
                .fg(if info.update_available {
                    Color::Yellow
                } else {
                    Color::Green
                }),
            Cell::new(format!("v{}", info.latest_version)),
        ]);
        table.add_row(vec![
            Cell::new("Update Tersedia").add_attribute(Attribute::Bold),
            Cell::new(status_display).fg(if info.update_available {
                Color::Yellow
            } else {
                Color::Green
            }),
        ]);
        table.add_row(vec![
            Cell::new("URL Rilis").add_attribute(Attribute::Bold),
            Cell::new(&info.release_url),
        ]);
        table.add_row(vec![
            Cell::new("Tanggal Publikasi").add_attribute(Attribute::Bold),
            Cell::new(info.published_at.as_deref().unwrap_or("-")),
        ]);

        println!("{table}");

        if changelog {
            match info
                .release_notes
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                Some(notes) => println!("\nCatatan Rilis (v{}):\n{}", info.latest_version, notes),
                None => println!(
                    "\nCatatan Rilis (v{}):\n(Tidak ada catatan rilis)",
                    info.latest_version
                ),
            }
        }

        if info.update_available {
            eprintln!("\n💡 Versi baru tersedia! Jalankan 'gdrive self-update' atau 'gdrive upgrade' untuk memperbarui.");
        } else {
            eprintln!(
                "✅ Anda sudah menggunakan versi terbaru (v{}).",
                info.current_version
            );
        }

        Ok(())
    } else {
        if changelog {
            if let Ok(info) = gdrive_core::check_for_update(None).await {
                if let Some(notes) = &info.release_notes {
                    println!("\nCatatan Rilis (v{}):\n{}", info.latest_version, notes);
                }
            }
        }
        eprintln!("Memeriksa dan memperbarui binary gdrive...");
        let success = gdrive_core::execute_self_update(None, None).await?;

        if json {
            println!("{}", serde_json::to_string_pretty(&success)?);
        } else {
            eprintln!("✅ {}", success.message);
            if success.current_version != success.updated_to {
                println!("Updated to: v{}", success.updated_to);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_search_multi_word_unquoted() {
        let cli = Cli::try_parse_from(["gdrive", "search", "iboss", "v2"]).unwrap();
        match cli.command.unwrap() {
            Commands::Search { query, .. } => {
                assert_eq!(query, vec!["iboss", "v2"]);
            }
            _ => panic!("Expected Search command"),
        }
    }

    #[test]
    fn test_search_single_quoted_word() {
        let cli = Cli::try_parse_from(["gdrive", "search", "iboss v2"]).unwrap();
        match cli.command.unwrap() {
            Commands::Search { query, .. } => {
                assert_eq!(query, vec!["iboss v2"]);
            }
            _ => panic!("Expected Search command"),
        }
    }

    #[test]
    fn test_search_empty_query_with_flags() {
        let cli = Cli::try_parse_from(["gdrive", "search", "--folder", "root"]).unwrap();
        match cli.command.unwrap() {
            Commands::Search { query, folder, .. } => {
                assert!(query.is_empty());
                assert_eq!(folder.as_deref(), Some("root"));
            }
            _ => panic!("Expected Search command"),
        }
    }

    #[test]
    fn test_info_command_parsing() {
        let cli = Cli::try_parse_from(["gdrive", "info", "file_123", "--json"]).unwrap();
        match cli.command.unwrap() {
            Commands::Info { file_id, json } => {
                assert_eq!(file_id, "file_123");
                assert!(json);
            }
            _ => panic!("Expected Info command"),
        }
    }

    #[test]
    fn test_copy_command_parsing() {
        let cli = Cli::try_parse_from([
            "gdrive",
            "copy",
            "file_123",
            "--name",
            "My Copy.md",
            "--parent",
            "folder_456",
        ])
        .unwrap();
        match cli.command.unwrap() {
            Commands::Copy {
                file_id,
                name,
                parent,
                json,
            } => {
                assert_eq!(file_id, "file_123");
                assert_eq!(name.as_deref(), Some("My Copy.md"));
                assert_eq!(parent.as_deref(), Some("folder_456"));
                assert!(!json);
            }
            _ => panic!("Expected Copy command"),
        }
    }

    #[test]
    fn test_share_command_parsing() {
        let cli = Cli::try_parse_from([
            "gdrive",
            "share",
            "file_123",
            "--role",
            "writer",
            "--target-type",
            "user",
            "--email",
            "test@example.com",
            "--notification",
            "--json",
        ])
        .unwrap();
        match cli.command.unwrap() {
            Commands::Share {
                file_id,
                role,
                target_type,
                email,
                domain: _,
                notification,
                json,
            } => {
                assert_eq!(file_id, "file_123");
                assert_eq!(role, "writer");
                assert_eq!(target_type, "user");
                assert_eq!(email.as_deref(), Some("test@example.com"));
                assert!(notification);
                assert!(json);
            }
            _ => panic!("Expected Share command"),
        }
    }

    #[test]
    fn test_share_command_defaults() {
        let cli = Cli::try_parse_from(["gdrive", "share", "file_123"]).unwrap();
        match cli.command.unwrap() {
            Commands::Share {
                file_id,
                role,
                target_type,
                email,
                domain,
                notification,
                json,
            } => {
                assert_eq!(file_id, "file_123");
                assert_eq!(role, "reader");
                assert_eq!(target_type, "anyone");
                assert_eq!(email, None);
                assert!(!notification);
                assert_eq!(domain, None);
                assert!(!json);
            }
            _ => panic!("Expected Share command"),
        }
    }

    #[test]
    fn test_permissions_list_parsing() {
        let cli =
            Cli::try_parse_from(["gdrive", "permissions", "list", "file_123", "--json"]).unwrap();
        match cli.command.unwrap() {
            Commands::Permissions {
                action: PermissionsCommands::List { file_id, json },
            } => {
                assert_eq!(file_id, "file_123");
                assert!(json);
            }
            _ => panic!("Expected Permissions List command"),
        }
    }

    #[test]
    fn test_permissions_revoke_parsing() {
        let cli = Cli::try_parse_from(["gdrive", "permissions", "revoke", "file_123", "perm_456"])
            .unwrap();
        match cli.command.unwrap() {
            Commands::Permissions {
                action:
                    PermissionsCommands::Revoke {
                        file_id,
                        permission_id,
                    },
            } => {
                assert_eq!(file_id, "file_123");
                assert_eq!(permission_id, "perm_456");
            }
            _ => panic!("Expected Permissions Revoke command"),
        }
    }

    #[test]
    fn test_comments_list_parsing() {
        let cli =
            Cli::try_parse_from(["gdrive", "comments", "list", "file_123", "--json"]).unwrap();
        match cli.command.unwrap() {
            Commands::Comments {
                action:
                    CommentsCommands::List {
                        file_id,
                        limit,
                        json,
                    },
            } => {
                assert_eq!(file_id, "file_123");
                assert_eq!(limit, 50);
                assert!(json);
            }
            _ => panic!("Expected Comments List command"),
        }
    }

    #[test]
    fn test_comments_add_parsing() {
        let cli =
            Cli::try_parse_from(["gdrive", "comments", "add", "file_123", "Nice work!"]).unwrap();
        match cli.command.unwrap() {
            Commands::Comments {
                action:
                    CommentsCommands::Add {
                        file_id,
                        content,
                        json,
                    },
            } => {
                assert_eq!(file_id, "file_123");
                assert_eq!(content, "Nice work!");
                assert!(!json);
            }
            _ => panic!("Expected Comments Add command"),
        }
    }

    #[test]
    fn test_comments_resolve_parsing() {
        let cli =
            Cli::try_parse_from(["gdrive", "comments", "resolve", "file_123", "comm_456"]).unwrap();
        match cli.command.unwrap() {
            Commands::Comments {
                action:
                    CommentsCommands::Resolve {
                        file_id,
                        comment_id,
                    },
            } => {
                assert_eq!(file_id, "file_123");
                assert_eq!(comment_id, "comm_456");
            }
            _ => panic!("Expected Comments Resolve command"),
        }
    }

    #[test]
    fn test_trash_shortcut_parsing() {
        let cli = Cli::try_parse_from(["gdrive", "trash", "file_abc"]).unwrap();
        match cli.command.unwrap() {
            Commands::Trash { action, file_id } => {
                assert!(action.is_none());
                assert_eq!(file_id.as_deref(), Some("file_abc"));
            }
            _ => panic!("Expected Trash shortcut command"),
        }
    }

    #[test]
    fn test_trash_subcommands_parsing() {
        let cli_list = Cli::try_parse_from(["gdrive", "trash", "list", "--json"]).unwrap();
        match cli_list.command.unwrap() {
            Commands::Trash { action, file_id } => {
                assert!(file_id.is_none());
                match action.unwrap() {
                    TrashCommands::List { limit, json, .. } => {
                        assert_eq!(limit, 50);
                        assert!(json);
                    }
                    _ => panic!("Expected Trash List subcommand"),
                }
            }
            _ => panic!("Expected Trash command"),
        }

        let cli_restore = Cli::try_parse_from(["gdrive", "trash", "restore", "file_xyz"]).unwrap();
        match cli_restore.command.unwrap() {
            Commands::Trash { action, file_id } => {
                assert!(file_id.is_none());
                match action.unwrap() {
                    TrashCommands::Restore { file_id } => {
                        assert_eq!(file_id, "file_xyz");
                    }
                    _ => panic!("Expected Trash Restore subcommand"),
                }
            }
            _ => panic!("Expected Trash command"),
        }

        let cli_empty = Cli::try_parse_from(["gdrive", "trash", "empty", "--yes"]).unwrap();
        match cli_empty.command.unwrap() {
            Commands::Trash { action, file_id } => {
                assert!(file_id.is_none());
                match action.unwrap() {
                    TrashCommands::Empty { yes } => {
                        assert!(yes);
                    }
                    _ => panic!("Expected Trash Empty subcommand"),
                }
            }
            _ => panic!("Expected Trash command"),
        }
    }

    #[test]
    fn test_revisions_list_parsing() {
        let cli =
            Cli::try_parse_from(["gdrive", "revisions", "list", "file_123", "--json"]).unwrap();
        match cli.command.unwrap() {
            Commands::Revisions {
                action: RevisionsCommands::List { file_id, json },
            } => {
                assert_eq!(file_id, "file_123");
                assert!(json);
            }
            _ => panic!("Expected Revisions List command"),
        }
    }

    #[test]
    fn test_revisions_download_parsing() {
        let cli = Cli::try_parse_from([
            "gdrive",
            "revisions",
            "download",
            "file_123",
            "rev_456",
            "-o",
            "out.pdf",
        ])
        .unwrap();
        match cli.command.unwrap() {
            Commands::Revisions {
                action:
                    RevisionsCommands::Download {
                        file_id,
                        revision_id,
                        output,
                    },
            } => {
                assert_eq!(file_id, "file_123");
                assert_eq!(revision_id, "rev_456");
                assert_eq!(output, Some(PathBuf::from("out.pdf")));
            }
            _ => panic!("Expected Revisions Download command"),
        }
    }

    #[test]
    fn test_global_profile_parsing() {
        let cli = Cli::try_parse_from(["gdrive", "--profile", "work", "info", "file_123"]).unwrap();
        assert_eq!(cli.profile.as_deref(), Some("work"));
        match cli.command.unwrap() {
            Commands::Info { file_id, .. } => {
                assert_eq!(file_id, "file_123");
            }
            _ => panic!("Expected Info command"),
        }
    }

    #[test]
    fn test_auth_profile_subcommands_parsing() {
        let cli_list = Cli::try_parse_from(["gdrive", "auth", "profile", "list"]).unwrap();
        match cli_list.command.unwrap() {
            Commands::Auth {
                action: AuthCommands::Profile { action },
            } => match action {
                ProfileCommands::List => {}
                _ => panic!("Expected Profile List subcommand"),
            },
            _ => panic!("Expected Auth command"),
        }

        let cli_switch =
            Cli::try_parse_from(["gdrive", "auth", "profile", "switch", "work"]).unwrap();
        match cli_switch.command.unwrap() {
            Commands::Auth {
                action: AuthCommands::Profile { action },
            } => match action {
                ProfileCommands::Switch { name } => {
                    assert_eq!(name, "work");
                }
                _ => panic!("Expected Profile Switch subcommand"),
            },
            _ => panic!("Expected Auth command"),
        }

        let cli_remove =
            Cli::try_parse_from(["gdrive", "auth", "profile", "remove", "work"]).unwrap();
        match cli_remove.command.unwrap() {
            Commands::Auth {
                action: AuthCommands::Profile { action },
            } => match action {
                ProfileCommands::Remove { name } => {
                    assert_eq!(name, "work");
                }
                _ => panic!("Expected Profile Remove subcommand"),
            },
            _ => panic!("Expected Auth command"),
        }
    }

    #[test]
    fn test_self_update_check_parsing() {
        let cli = Cli::try_parse_from(["gdrive", "self-update", "--check", "--json"]).unwrap();
        match cli.command.unwrap() {
            Commands::SelfUpdate {
                check,
                changelog,
                json,
            } => {
                assert!(check);
                assert!(!changelog);
                assert!(json);
            }
            _ => panic!("Expected SelfUpdate command"),
        }
    }

    #[test]
    fn test_self_update_upgrade_alias_parsing() {
        let cli = Cli::try_parse_from(["gdrive", "upgrade", "--changelog"]).unwrap();
        match cli.command.unwrap() {
            Commands::SelfUpdate {
                check,
                changelog,
                json,
            } => {
                assert!(!check);
                assert!(changelog);
                assert!(!json);
            }
            _ => panic!("Expected SelfUpdate command via upgrade alias"),
        }
    }
    #[test]
    fn test_zero_args_parsing() {
        let cli = Cli::try_parse_from(["gdrive"]).unwrap();
        assert!(cli.command.is_none());
        assert!(cli.profile.is_none());
    }

    #[test]
    fn test_tui_command_parsing() {
        let cli = Cli::try_parse_from(["gdrive", "tui"]).unwrap();
        match cli.command.unwrap() {
            Commands::Tui { folder, demo } => {
                assert!(folder.is_none());
                assert!(!demo);
            }
            _ => panic!("Expected Tui command"),
        }

        let cli_alias =
            Cli::try_parse_from(["gdrive", "ui", "--folder", "folder_xyz", "--demo"]).unwrap();
        match cli_alias.command.unwrap() {
            Commands::Tui { folder, demo } => {
                assert_eq!(folder.as_deref(), Some("folder_xyz"));
                assert!(demo);
            }
            _ => panic!("Expected Tui command via ui alias"),
        }
    }
}
