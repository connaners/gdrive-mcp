# Implementation Plan: gdrive-mcp v0.1.0 Release

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Menyiapkan `gdrive-mcp` menjadi Google Drive CLI dan Model Context Protocol (MCP) server siap rilis di GitHub, dilengkapi OAuth login & auto-refresh token, tool manipulasi file lengkap (update, folder, safe trash, shared drives), CLI upload/download file lokal, serta CI/CD release workflow multi-platform.

**Architecture:** 
- Workspace Cargo modular: `gdrive-core` (OAuth, token storage, Drive v3 REST API), `gdrive-mcp` (rmcp stdio tool server dengan schema-safe parameters), `gdrive-cli` (clap CLI dispatcher).
- Token persistence di `~/.config/gdrive-mcp/token.json` dengan expiry tracking dan auto-refresh token.
- Multi-OS releases otomatis via GitHub Actions untuk macOS (Apple Silicon & Intel), Linux x86_64, dan Windows x86_64.

**Tech Stack:** Rust 2021, `rmcp` (MCP protocol 2.1), `reqwest` (multipart, JSON, rustls), `tokio`, `clap`, `schemars`, `tracing`.

---

## 📁 File Structure Map

- **`crates/gdrive-core/`**:
  - `src/auth.rs`: OAuth2 PKCE/authorization code flow, loopback HTTP server, token refresh logic, custom & fallback credentials.
  - `src/client.rs`: HTTP client Google Drive API v3 (search, read/export, create, update, trash, create folder, list drives, auto-refresh retry).
  - `src/models.rs`: Structs untuk file, folder, drive list, token data, dan upload/update options.
  - `tests/`: Unit test untuk token storage, expiry calculation, dan parameter formatting.

- **`crates/gdrive-mcp/`**:
  - `src/models.rs`: Tool argument schemas dengan doc-comments eksplisit (`SearchFilesArgs`, `ReadFileArgs`, `ListFolderArgs`, `CreateFileArgs`, `UpdateFileArgs`, `CreateFolderArgs`, `TrashFileArgs`, `ListDrivesArgs`).
  - `src/server.rs`: Tool router dan handler implementations untuk seluruh MCP tools.

- **`crates/gdrive-cli/`**:
  - `src/main.rs`: CLI subcommands (`auth login`, `auth status`, `auth logout`, `search`, `read`, `create`, `update`, `upload`, `download`, `mkdir`, `trash`, `drives`, `mcp serve`).

- **Root & CI/CD**:
  - `LICENSE`: Lisensi MIT.
  - `README.md`: Dokumentasi lengkap fitur, CLI usage, panduan setup OAuth Google Cloud, dan integrasi MCP (Hermes, Claude Desktop, Cursor, OpenCode).
  - `.github/workflows/ci.yml`: CI check, clippy, dan cargo test.
  - `.github/workflows/release.yml`: Matrix build multi-platform binary saat tag `v*` di-push.

---

## 🔨 Implementation Tasks

### Phase 1: Authentication & Token Lifecycle

#### Task 1: Refactor Token Model & Expiry Tracking
**Files:**
- Modify: `crates/gdrive-core/src/auth.rs`
- Modify: `crates/gdrive-core/src/models.rs`

- [x] **Step 1: Tambahkan field `expiry: Option<u64>` (Unix timestamp detik) pada `TokenData`**
- [x] **Step 2: Buat helper method `is_expired(&self) -> bool` yang mengecek apakah token sudah kedaluwarsa atau sisa waktu < 300 detik (5 menit)**
- [x] **Step 3: Tambahkan method `AuthManager::logout()` untuk menghapus `token.json`**
- [x] **Step 4: Tulis unit test untuk verifikasi perhitungan `is_expired`**
- [x] **Step 5: Verifikasi `cargo check` dan `cargo test -p gdrive-core` lolos**

#### Task 2: Implementasi OAuth2 Authorization & Loopback Server
**Files:**
- Modify: `crates/gdrive-core/src/auth.rs`

- [x] **Step 1: Tambahkan default desktop Client ID & Client Secret (dengan support override via env `GOOGLE_CLIENT_ID` dan `GOOGLE_CLIENT_SECRET`)**
- [x] **Step 2: Implementasikan helper pembuatan URL otorisasi Google OAuth2 dengan scope `drive` dan `drive.file` serta `access_type=offline`, `prompt=consent`**
- [x] **Step 3: Implementasikan loopback HTTP server lokal via `tokio::net::TcpListener` pada port dinamis/tetap (e.g., 8085) untuk menangkap code callback dari browser**
- [x] **Step 4: Tambahkan fallback manual input code / URL jika browser gagal dibuka atau dijalankan dengan flag `--no-browser`**
- [x] **Step 5: Implementasikan pertukaran authorization code menjadi `TokenData` (access token + refresh token + expires_in) dan simpan ke disk**
- [x] **Step 6: Jalankan `cargo check`**

#### Task 3: Auto-Refresh Access Token (Proaktif & Reaktif)
**Files:**
- Modify: `crates/gdrive-core/src/auth.rs`
- Modify: `crates/gdrive-core/src/client.rs`

- [x] **Step 1: Implementasikan method `AuthManager::refresh_token(refresh_token: &str) -> Result<TokenData>` yang memanggil endpoint `https://oauth2.googleapis.com/token`**
- [x] **Step 2: Implementasikan `AuthManager::get_valid_token() -> Result<String>` yang otomatis melakukan refresh dan rewrite `token.json` jika token expired**
- [x] **Step 3: Update `GDriveClient` agar memiliki referensi token refresher atau me-reload token saat HTTP 401 dan melakukan retry satu kali**
- [x] **Step 4: Jalankan `cargo test`**

#### Task 4: CLI Auth Commands (`login`, `status`, `logout`)
**Files:**
- Modify: `crates/gdrive-cli/src/main.rs`

- [x] **Step 1: Tambahkan subcommands `Login { #[arg(long)] no_browser: bool }` dan `Logout` pada enum `AuthCommands`**
- [x] **Step 2: Sambungkan handler `AuthCommands::Login` ke flow `AuthManager::login`**
- [x] **Step 3: Sambungkan handler `AuthCommands::Logout` ke `AuthManager::logout`**
- [x] **Step 4: Perjelas output `AuthCommands::Status` (tampilkan expiry time dan akun jika tersedia)**
- [x] **Step 5: Uji jalankan `cargo run -p gdrive-cli -- auth status`**

---

### Phase 2: Core Drive API Operations & MCP Tools

#### Task 5: Tool & Client: Update File (`gdrive_update_file`)
**Files:**
- Modify: `crates/gdrive-core/src/client.rs`
- Modify: `crates/gdrive-mcp/src/models.rs`
- Modify: `crates/gdrive-mcp/src/server.rs`

- [x] **Step 1: Tambahkan struct `UpdateFileOptions { file_id: String, content: String, append: bool }` di `gdrive-core`**
- [x] **Step 2: Implementasikan `client.update_file(...)` di `gdrive-core` (jika append=true: baca konten lama terlebih dahulu lalu gabungkan)**
- [x] **Step 3: Tambahkan struct schema `UpdateFileArgs` di `gdrive-mcp/src/models.rs` dengan doc-comment lengkap di setiap field**
- [x] **Step 4: Daftarkan tool `#[tool(name = "gdrive_update_file", ...)]` di `gdrive-mcp/src/server.rs`**
- [x] **Step 5: Verifikasi strict schema safety dan jalankan `cargo check`**

#### Task 6: Tool & Client: Create Folder (`gdrive_create_folder`)
**Files:**
- Modify: `crates/gdrive-core/src/client.rs`
- Modify: `crates/gdrive-mcp/src/models.rs`
- Modify: `crates/gdrive-mcp/src/server.rs`

- [x] **Step 1: Implementasikan `client.create_folder(name: &str, parent_id: Option<&str>)` dengan MIME `application/vnd.google-apps.folder`**
- [x] **Step 2: Tambahkan struct schema `CreateFolderArgs` di `gdrive-mcp/src/models.rs` dengan doc-comment lengkap**
- [x] **Step 3: Daftarkan tool `#[tool(name = "gdrive_create_folder", ...)]` di `gdrive-mcp/src/server.rs`**
- [x] **Step 4: Verifikasi `cargo check`**

#### Task 7: Tool & Client: Safe Trash (`gdrive_trash_file`)
**Files:**
- Modify: `crates/gdrive-core/src/client.rs`
- Modify: `crates/gdrive-mcp/src/models.rs`
- Modify: `crates/gdrive-mcp/src/server.rs`

- [x] **Step 1: Implementasikan `client.trash_file(file_id: &str)` (PATCH ke Drive API dengan body `{"trashed": true}`)**
- [x] **Step 2: Tambahkan struct schema `TrashFileArgs` di `gdrive-mcp/src/models.rs` dengan doc-comment lengkap**
- [x] **Step 3: Daftarkan tool `#[tool(name = "gdrive_trash_file", ...)]` di `gdrive-mcp/src/server.rs`**
- [x] **Step 4: Verifikasi `cargo check`**

#### Task 8: Tool & Client: List Shared Drives (`gdrive_list_drives`)
**Files:**
- Modify: `crates/gdrive-core/src/client.rs`
- Modify: `crates/gdrive-core/src/models.rs`
- Modify: `crates/gdrive-mcp/src/models.rs`
- Modify: `crates/gdrive-mcp/src/server.rs`

- [x] **Step 1: Tambahkan data model `DriveItem` dan `DriveItemList` di `gdrive-core/src/models.rs`**
- [x] **Step 2: Implementasikan `client.list_drives(page_size: Option<u32>)` memanggil `GET /drives` dengan `pageSize`**
- [x] **Step 3: Tambahkan struct schema `ListDrivesArgs` di `gdrive-mcp/src/models.rs` dengan doc-comment lengkap**
- [x] **Step 4: Daftarkan tool `#[tool(name = "gdrive_list_drives", ...)]` di `gdrive-mcp/src/server.rs`**
- [x] **Step 5: Update prompt instruksi server di `get_info().with_instructions(...)` agar model tahu cara memakai tool baru**
- [x] **Step 6: Jalankan `cargo check`**

---

### Phase 3: CLI Subcommands & Local File I/O

#### Task 9: CLI Upload Local File (`gdrive upload`)
**Files:**
- Modify: `crates/gdrive-core/src/client.rs`
- Modify: `crates/gdrive-cli/src/main.rs`

- [x] **Step 1: Tambahkan fungsi upload streaming atau file upload dari path lokal pada `gdrive-core` dengan mime detection otomatis (atau fallback `application/octet-stream`)**
- [x] **Step 2: Tambahkan command `Upload { path: PathBuf, name: Option<String>, parent: Option<String>, docs: bool }` di `gdrive-cli`**
- [x] **Step 3: Implementasikan handler CLI `Upload` dengan progress message ke `stderr` dan print file ID / web link ke `stdout`**
- [x] **Step 4: Jalankan `cargo check`**

#### Task 10: CLI Download File (`gdrive download`)
**Files:**
- Modify: `crates/gdrive-core/src/client.rs`
- Modify: `crates/gdrive-cli/src/main.rs`

- [x] **Step 1: Tambahkan fungsi `client.download_file_bytes(file_id: &str)` di `gdrive-core`**
- [x] **Step 2: Tambahkan command `Download { file_id: String, output: Option<PathBuf> }` di `gdrive-cli`**
- [x] **Step 3: Implementasikan handler CLI `Download`: jika output tidak diisi, gunakan nama asli file dari metadata Drive**
- [x] **Step 4: Jalankan `cargo check`**

#### Task 11: CLI Commands Pendukung (`mkdir`, `trash`, `drives`, `update`)
**Files:**
- Modify: `crates/gdrive-cli/src/main.rs`

- [x] **Step 1: Tambahkan command `Mkdir { name: String, parent: Option<String> }`**
- [x] **Step 2: Tambahkan command `Trash { file_id: String }`**
- [x] **Step 3: Tambahkan command `Drives`**
- [x] **Step 4: Tambahkan command `Update { file_id: String, content: String, #[arg(long)] append: bool }`**
- [x] **Step 5: Verifikasi seluruh sub-command dengan `cargo run -p gdrive-cli -- --help`**

---

### Phase 4: Release Engineering, CI/CD, & Documentation

#### Task 12: LICENSE & Quality Assurance
**Files:**
- Create: `LICENSE`
- Modify: Unit tests di workspace

- [x] **Step 1: Buat file `LICENSE` (MIT) dengan copyright `connaners`**
- [x] **Step 2: Jalankan `cargo fmt --check` dan `cargo clippy --all-targets -- -D warnings`**
- [x] **Step 3: Pastikan semua unit tests lolos: `cargo test`**

#### Task 13: GitHub Actions CI Workflow
**Files:**
- Create: `.github/workflows/ci.yml`

- [x] **Step 1: Buat workflow GitHub Actions untuk trigger pada `push` ke `main` dan `pull_request`**
- [x] **Step 2: Jalankan step check, formatting check, clippy lint, dan test di Ubuntu runner**

#### Task 14: Automated Multi-Platform Release Workflow
**Files:**
- Create: `.github/workflows/release.yml`

- [x] **Step 1: Buat workflow release dengan trigger `push: tags: ['v*']`**
- [x] **Step 2: Setup matrix build untuk target:**
  - `aarch64-apple-darwin` (macOS Apple Silicon)
  - `x86_64-apple-darwin` (macOS Intel)
  - `x86_64-unknown-linux-gnu` (Linux x86_64)
  - `x86_64-pc-windows-msvc` (Windows x86_64)
- [x] **Step 3: Archive binary ke tar.gz / zip dan upload aset ke GitHub Releases via `softprops/action-gh-release`**

#### Task 15: Dokumentasi Lengkap README.md
**Files:**
- Modify: `README.md`

- [x] **Step 1: Tulis dokumentasi langkah demi langkah cara mendapatkan Google OAuth credentials (jika ingin pakai custom client)**
- [x] **Step 2: Dokumentasikan alur `gdrive auth login`**
- [x] **Step 3: Dokumentasikan daftar lengkap CLI commands dan flag-nya**
- [x] **Step 4: Dokumentasikan daftar lengkap 8 MCP tools beserta spesifikasinya**
- [x] **Step 5: Berikan contoh konfigurasi konfigurasi MCP server untuk:**
  - Hermes Agent (`~/.hermes/config.yaml`)
  - Claude Desktop (`claude_desktop_config.json`)
  - Cursor / VS Code (`mcp.json`)
  - OpenCode (`opencode.json`)

---

## 🏁 Verification Checklist Sebelum GitHub Release

1. [x] `cargo check --workspace` berhasil tanpa error.
2. [x] `cargo clippy --workspace --all-targets -- -D warnings` bersih tanpa warning.
3. [x] `cargo test --workspace` semua test pass.
4. [x] `cargo build --release` menghasilkan executable `target/release/gdrive`.
5. [x] Cek file rahasia: Pastikan `.gitignore` mengecualikan `token.json`, `credentials.json`, `client_secret*.json`, `.env`.
6. [x] Cek schema MCP: Tidak ada bare boolean schema (`Schema::Bool(true)`) pada `crates/gdrive-mcp/src/models.rs`.
7. [x] Cek stdout safety: Tidak ada `println!` yang dieksekusi saat MCP mode running (semua logging terarah ke `stderr`).
