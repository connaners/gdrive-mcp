# Search & Download UX Enhancement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform Google Drive CLI (`gdrive`) and MCP server with rich table search output, smart download targeting (by File ID, URL, or Name with interactive disambiguation), streaming downloads with `indicatif` progress bar, exponential backoff transient retries, clear absolute path feedback, and a dedicated `gdrive_download_file` MCP tool.

**Architecture:** 
1. `gdrive-core` provides streaming download abstractions and exponential backoff retry logic on network/rate-limit errors.
2. `gdrive-cli` integrates `comfy-table` for terminal tables with `--json`/`--plain` flags, `indicatif` for progress bars during download, name resolution logic with interactive disambiguation, and canonical path printing.
3. `gdrive-mcp` exposes a schema-safe `gdrive_download_file` tool compliant with `AGENTS.md` (no stdout logging, all fields documented).

**Tech Stack:** Rust (2021 edition), `comfy-table` (v7), `indicatif` (v0.17), `futures-util` (v0.3), `tokio`, `reqwest`, `rmcp`.

---

## File Structure

```
gdrive-mcp/
├── Cargo.toml                                 # Workspace dependencies (add comfy-table, indicatif)
├── crates/
│   ├── gdrive-core/
│   │   ├── Cargo.toml                         # Add futures-util
│   │   ├── src/client.rs                      # Exponential backoff retry & download_file_stream
│   │   └── src/lib.rs
│   │
│   ├── gdrive-cli/
│   │   ├── Cargo.toml                         # Add comfy-table, indicatif
│   │   ├── src/format.rs                      # NEW: Table styling, human-readable sizes, icons & dates
│   │   ├── src/main.rs                        # Updated search with table/json/plain & smart download
│   │   └── tests/cli_test.rs                  # Formatting and argument parsing tests
│   │
│   └── gdrive-mcp/
│       ├── src/models.rs                      # DownloadFileArgs with #[derive(JsonSchema)] & doc-comments
│       └── src/server.rs                      # gdrive_download_file tool
```

---

### Task 1: Add Dependencies to Workspace & Crate Manifests

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/gdrive-core/Cargo.toml`
- Modify: `crates/gdrive-cli/Cargo.toml`

- [ ] **Step 1: Update workspace `Cargo.toml`**

Add `comfy-table` and `indicatif` to `[workspace.dependencies]`:

```toml
comfy-table = "7"
indicatif = "0.17"
futures-util = "0.3"
```

- [ ] **Step 2: Update `crates/gdrive-core/Cargo.toml`**

Add `futures-util.workspace = true` under `[dependencies]`.

- [ ] **Step 3: Update `crates/gdrive-cli/Cargo.toml`**

Add `comfy-table.workspace = true` and `indicatif.workspace = true` under `[dependencies]`.

- [ ] **Step 4: Verify workspace resolves and compiles dependencies**

Run: `cargo check`
Expected: Resolves crates and completes with zero errors.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock crates/gdrive-core/Cargo.toml crates/gdrive-cli/Cargo.toml
git commit -m "chore: add comfy-table, indicatif, and futures-util dependencies"
```

---

### Task 2: Exponential Backoff & Transient Retry in `gdrive-core`

**Files:**
- Modify: `crates/gdrive-core/src/client.rs`

- [ ] **Step 1: Write unit test for exponential backoff helper**

Create test in `crates/gdrive-core/src/client.rs` (or within `mod tests` at the bottom):

```rust
#[cfg(test)]
mod retry_tests {
    use super::*;

    #[test]
    fn test_is_transient_status() {
        assert!(is_transient_status(reqwest::StatusCode::TOO_MANY_REQUESTS));
        assert!(is_transient_status(reqwest::StatusCode::INTERNAL_SERVER_ERROR));
        assert!(is_transient_status(reqwest::StatusCode::BAD_GATEWAY));
        assert!(is_transient_status(reqwest::StatusCode::SERVICE_UNAVAILABLE));
        assert!(is_transient_status(reqwest::StatusCode::GATEWAY_TIMEOUT));
        assert!(!is_transient_status(reqwest::StatusCode::NOT_FOUND));
        assert!(!is_transient_status(reqwest::StatusCode::FORBIDDEN));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p gdrive-core test_is_transient_status`
Expected: FAIL (function `is_transient_status` not found).

- [ ] **Step 3: Implement `is_transient_status` and exponential backoff in `send_with_retry`**

In `crates/gdrive-core/src/client.rs`:

```rust
pub fn is_transient_status(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS
        || status.is_server_error()
}

impl GDriveClient {
    /// Mengirim request dengan mekanisme retry otomatis:
    /// 1. HTTP 401 Unauthorized -> Refresh token dan coba kembali
    /// 2. Transient status (HTTP 429, 500, 502, 503, 504) atau network failure -> Exponential backoff hingga 3x
    pub async fn send_with_retry<F>(&self, build: F) -> Result<reqwest::Response>
    where
        F: Fn(&str) -> reqwest::RequestBuilder,
    {
        const MAX_RETRIES: u32 = 3;
        let mut attempt = 0;
        let mut backoff = std::time::Duration::from_millis(500);

        loop {
            let token = self.get_token().await;
            let resp_result = build(&token).send().await;

            match resp_result {
                Ok(resp) => {
                    // Cek jika 401 Unauthorized
                    if resp.status() == reqwest::StatusCode::UNAUTHORIZED && self.auto_refresh {
                        if let Ok(new_token) = self.force_refresh().await {
                            return Ok(build(&new_token).send().await?);
                        }
                    }

                    // Cek transient status (429, 5xx)
                    if is_transient_status(resp.status()) && attempt < MAX_RETRIES {
                        attempt += 1;
                        tracing::warn!(
                            "Menerima status {} dari Google Drive API. Mencoba ulang ({}/{})...",
                            resp.status(),
                            attempt,
                            MAX_RETRIES
                        );
                        tokio::time::sleep(backoff).await;
                        backoff *= 2;
                        continue;
                    }

                    return Ok(resp);
                }
                Err(err) => {
                    // Network IO / timeout errors
                    if (err.is_connect() || err.is_timeout() || err.is_request()) && attempt < MAX_RETRIES {
                        attempt += 1;
                        tracing::warn!(
                            "Gagal koneksi ke Google Drive API: {}. Mencoba ulang ({}/{})...",
                            err,
                            attempt,
                            MAX_RETRIES
                        );
                        tokio::time::sleep(backoff).await;
                        backoff *= 2;
                        continue;
                    }
                    return Err(err.into());
                }
            }
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p gdrive-core test_is_transient_status`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/gdrive-core/src/client.rs
git commit -m "feat(core): add transient and rate-limit exponential backoff retry"
```

---

### Task 3: Streaming File Download & Name Search Helper in `gdrive-core`

**Files:**
- Modify: `crates/gdrive-core/src/client.rs`

- [ ] **Step 1: Add `DownloadStream` struct and `download_file_stream` method**

Add `DownloadStream` in `crates/gdrive-core/src/client.rs`:

```rust
use futures_util::Stream;
use bytes::Bytes;

pub struct DownloadStream {
    pub name: String,
    pub size: Option<u64>,
    pub mime_type: String,
    pub stream: Box<dyn Stream<Item = reqwest::Result<Bytes>> + Unpin + Send>,
}
```

Implement `download_file_stream` and `find_files_by_name`:

```rust
impl GDriveClient {
    /// Mengambil file sebagai stream byte untuk penghematan memori dan rendering progress bar
    pub async fn download_file_stream(&self, raw_id: &str) -> Result<DownloadStream> {
        let file_id = Self::clean_file_id(raw_id);
        let meta_url = format!("{}/files/{}", DRIVE_API_BASE, file_id);
        let meta_resp = self
            .send_with_retry(|t| {
                self.http.get(&meta_url).bearer_auth(t).query(&[
                    ("fields", "id, name, mimeType, size"),
                    ("supportsAllDrives", "true"),
                ])
            })
            .await
            .context("Gagal mengambil metadata file untuk download")?;

        if !meta_resp.status().is_success() {
            let err = meta_resp.text().await.unwrap_or_default();
            bail!("Gagal mengambil metadata file: {}", err);
        }

        let meta: DriveFile = meta_resp.json().await?;
        let parsed_size = meta.size.as_deref().and_then(|s| s.parse::<u64>().ok());

        // Jika Google Docs, ekspor ke teks
        if meta.mime_type == "application/vnd.google-apps.document" {
            let export_url = format!("{}/files/{}/export", DRIVE_API_BASE, file_id);
            let resp = self
                .send_with_retry(|t| {
                    self.http
                        .get(&export_url)
                        .bearer_auth(t)
                        .query(&[("mimeType", "text/plain")])
                })
                .await?;
            let filename = format!("{}.txt", meta.name);
            let len = resp.content_length();
            return Ok(DownloadStream {
                name: filename,
                size: len,
                mime_type: "text/plain".to_string(),
                stream: Box::new(resp.bytes_stream()),
            });
        }

        // Regular file media download
        let download_url = format!(
            "{}/files/{}?alt=media&supportsAllDrives=true",
            DRIVE_API_BASE, file_id
        );
        let resp = self
            .send_with_retry(|t| self.http.get(&download_url).bearer_auth(t))
            .await?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            bail!("Gagal mengunduh file media: {}", err);
        }

        let total_size = parsed_size.or_else(|| resp.content_length());

        Ok(DownloadStream {
            name: meta.name,
            size: total_size,
            mime_type: meta.mime_type,
            stream: Box::new(resp.bytes_stream()),
        })
    }

    /// Mencari file berdasarkan nama (exact match atau contains)
    pub async fn find_files_by_name(&self, name_query: &str, limit: u32) -> Result<Vec<DriveFile>> {
        let res = self
            .search_files_with_options(SearchFileOptions {
                query: Some(name_query.to_string()),
                folder_id: None,
                page_size: Some(limit),
                page_token: None,
                full_text: false,
                order_by: Some("folder,name,modifiedTime desc".to_string()),
            })
            .await?;
        Ok(res.files)
    }
}
```

- [ ] **Step 2: Verify `gdrive-core` compiles**

Run: `cargo check -p gdrive-core`
Expected: PASS with no warnings.

- [ ] **Step 3: Commit**

```bash
git add crates/gdrive-core/src/client.rs
git commit -m "feat(core): add download_file_stream and find_files_by_name"
```

---

### Task 4: CLI Helper Module `format.rs` for Pretty Tables & Metadata

**Files:**
- Create: `crates/gdrive-cli/src/format.rs`
- Modify: `crates/gdrive-cli/src/main.rs`

- [ ] **Step 1: Create `crates/gdrive-cli/src/format.rs`**

```rust
use comfy_table::{presets::UTF8_FULL, Attribute, Cell, Color, ContentArrangement, Table};
use gdrive_core::DriveFile;

pub fn format_size(bytes_str: Option<&str>) -> String {
    let bytes = match bytes_str.and_then(|s| s.parse::<u64>().ok()) {
        Some(b) => b,
        None => return "-".to_string(),
    };

    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

pub fn format_mime(mime: &str) -> (&'static str, &'static str) {
    match mime {
        "application/vnd.google-apps.folder" => ("📁", "Folder"),
        "application/vnd.google-apps.document" => ("📝", "Google Doc"),
        "application/vnd.google-apps.spreadsheet" => ("📊", "Google Sheet"),
        "application/vnd.google-apps.presentation" => ("📽️", "Google Slides"),
        "application/pdf" => ("📄", "PDF"),
        m if m.starts_with("image/") => ("🖼️", "Image"),
        m if m.starts_with("video/") => ("🎬", "Video"),
        m if m.starts_with("audio/") => ("🎵", "Audio"),
        m if m.starts_with("text/") => ("📜", "Text"),
        "application/zip" | "application/x-tar" | "application/gzip" => ("📦", "Archive"),
        _ => ("📎", "File"),
    }
}

pub fn format_date(iso_date: Option<&str>) -> String {
    match iso_date {
        Some(d) if d.len() >= 16 => {
            // "2026-10-22T01:38:55.000Z" -> "2026-10-22 01:38"
            let date = &d[..10];
            let time = &d[11..16];
            format!("{} {}", date, time)
        }
        Some(d) => d.to_string(),
        None => "-".to_string(),
    }
}

pub fn render_files_table(files: &[DriveFile]) -> String {
    if files.is_empty() {
        return "Tidak ada file atau folder yang ditemukan.".to_string();
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("ID").add_attribute(Attribute::Bold).fg(Color::Cyan),
            Cell::new("Tipe").add_attribute(Attribute::Bold).fg(Color::Yellow),
            Cell::new("Nama").add_attribute(Attribute::Bold).fg(Color::Green),
            Cell::new("Ukuran").add_attribute(Attribute::Bold).fg(Color::Magenta),
            Cell::new("Terakhir Diubah").add_attribute(Attribute::Bold).fg(Color::Blue),
        ]);

    for f in files {
        let (icon, label) = format_mime(&f.mime_type);
        let type_display = format!("{} {}", icon, label);
        let size_display = format_size(f.size.as_deref());
        let date_display = format_date(f.modified_time.as_deref());

        table.add_row(vec![
            Cell::new(&f.id).fg(Color::DarkGrey),
            Cell::new(type_display),
            Cell::new(&f.name),
            Cell::new(size_display),
            Cell::new(date_display),
        ]);
    }

    table.to_string()
}
```

- [ ] **Step 2: Add unit tests for `format.rs`**

Add unit tests at bottom of `crates/gdrive-cli/src/format.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(None), "-");
        assert_eq!(format_size(Some("500")), "500 B");
        assert_eq!(format_size(Some("2048")), "2.0 KB");
        assert_eq!(format_size(Some("10485760")), "10.00 MB");
    }

    #[test]
    fn test_format_mime() {
        let (icon, label) = format_mime("application/vnd.google-apps.folder");
        assert_eq!(icon, "📁");
        assert_eq!(label, "Folder");
    }

    #[test]
    fn test_format_date() {
        assert_eq!(format_date(Some("2026-10-22T01:38:55.000Z")), "2026-10-22 01:38");
        assert_eq!(format_date(None), "-");
    }
}
```

- [ ] **Step 3: Run test to verify format module passes**

Run: `cargo test -p gdrive-cli format::tests`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/gdrive-cli/src/format.rs
git commit -m "feat(cli): add format helper for comfy-table and human-readable metadata"
```

---

### Task 5: Enhance `gdrive search` with Table View, `--json`, and `--plain`

**Files:**
- Modify: `crates/gdrive-cli/src/main.rs`

- [ ] **Step 1: Add `--json` and `--plain` flags to `Commands::Search`**

In `crates/gdrive-cli/src/main.rs`:

```rust
    /// Mencari file atau folder di Google Drive
    Search {
        /// Kata kunci pencarian nama atau konten file
        query: Option<String>,
        /// Filter berdasarkan ID folder induk
        #[arg(short, long)]
        folder: Option<String>,
        /// Batas jumlah hasil (default: 20)
        #[arg(short, long, default_value = "20")]
        limit: u32,
        /// Cari di dalam isi konten file (full-text search)
        #[arg(long)]
        full_text: bool,
        /// Kriteria pengurutan Google Drive
        #[arg(long)]
        order_by: Option<String>,
        /// Token halaman untuk melanjutkan pagination
        #[arg(long)]
        page_token: Option<String>,
        /// Tampilkan output dalam format JSON mentah
        #[arg(long)]
        json: bool,
        /// Tampilkan output dalam format teks sederhana (ID | Name | Mime)
        #[arg(long)]
        plain: bool,
    },
```

- [ ] **Step 2: Update `handle_search` implementation**

In `crates/gdrive-cli/src/main.rs`:

```rust
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
```

- [ ] **Step 3: Run check on CLI**

Run: `cargo check -p gdrive-cli`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/gdrive-cli/src/main.rs
git commit -m "feat(cli): enhance search output with comfy-table and json/plain flags"
```

---

### Task 6: Smart Download (ID, URL, Name Resolution, Progress Bar, & Absolute Path)

**Files:**
- Modify: `crates/gdrive-cli/src/main.rs`

- [ ] **Step 1: Implement download target resolution logic**

Add target resolution in `crates/gdrive-cli/src/main.rs`:
- If `target` is a Drive URL (contains `/d/`, `/folders/`, `id=`) or an exact alphanumeric Google Drive ID (e.g. 25-44 chars without whitespace and no file extension): try direct ID download.
- Otherwise, search Google Drive files via `client.find_files_by_name(&target, 10)`.
- If 0 found: print error suggesting search.
- If 1 found: proceed with that file's ID.
- If >1 found: display a numbered list in `stderr` and read user input from `stdin` (choice 1..N).

- [ ] **Step 2: Implement streaming download with `indicatif::ProgressBar` and absolute path reporting**

In `crates/gdrive-cli/src/main.rs`:

```rust
use indicatif::{ProgressBar, ProgressStyle};
use futures_util::StreamExt;
use std::io::Write;

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
        let (icon, label) = format::format_mime(&f.mime_type);
        let size = format::format_size(f.size.as_deref());
        eprintln!("  [{}] {} {} ({}) - ID: {}", idx + 1, icon, f.name, size, f.id);
    }

    loop {
        eprint!("\n👉 Masukkan nomor pilihan [1-{}]: ", candidates.len());
        std::io::stderr().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if let Ok(choice) = input.trim().parse::<usize>() {
            if choice >= 1 && choice <= candidates.len() {
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
            p.set_style(ProgressStyle::default_spinner().template("{spinner:.green} {bytes} downloaded")?);
            p
        }
    };

    let mut file = std::fs::File::create(&out_path)
        .context(format!("Gagal membuat file lokal di {:?}", out_path))?;
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
```

- [ ] **Step 3: Verify CLI compiles and passes build**

Run: `cargo check -p gdrive-cli`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/gdrive-cli/src/main.rs
git commit -m "feat(cli): smart download with name resolution, progress bar, and absolute path output"
```

---

### Task 7: Add `gdrive_download_file` MCP Tool

**Files:**
- Modify: `crates/gdrive-mcp/src/models.rs`
- Modify: `crates/gdrive-mcp/src/server.rs`

- [ ] **Step 1: Define `DownloadFileArgs` in `crates/gdrive-mcp/src/models.rs`**

Ensure strict schema safety with doc-comments on all fields:

```rust
/// Parameter untuk tool gdrive_download_file
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DownloadFileArgs {
    /// ID Google Drive, URL Google Drive, atau nama file yang ingin diunduh ke disk lokal
    pub file_id_or_name: String,
    /// Path folder atau file tujuan penyimpanan lokal (opsional, default: direktori kerja saat ini)
    pub destination_path: Option<String>,
}
```

- [ ] **Step 2: Add `gdrive_download_file` in `crates/gdrive-mcp/src/server.rs`**

```rust
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
        let file_id = if target.len() >= 25 && !target.contains(' ') && !target.contains('.') {
            target.to_string()
        } else {
            let files = client
                .find_files_by_name(target, 5)
                .await
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
            if files.is_empty() {
                return Err(ErrorData::invalid_params(
                    format!("File dengan nama '{}' tidak ditemukan di Google Drive.", target),
                    None,
                ));
            }
            files[0].id.clone()
        };

        let stream_data = client
            .download_file_stream(&file_id)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;

        let orig_name = stream_data.name.clone();
        let mime_type = stream_data.mime_type.clone();

        let out_path = match args.destination_path {
            Some(p) => {
                let pb = std::path::PathBuf::from(p);
                if pb.is_dir() {
                    pb.join(&orig_name)
                } else {
                    pb
                }
            }
            None => std::env::current_dir()
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
                .join(&orig_name),
        };

        let mut file = std::fs::File::create(&out_path)
            .map_err(|e| ErrorData::internal_error(format!("Gagal membuat file lokal: {}", e), None))?;

        use futures_util::StreamExt;
        use std::io::Write;
        let mut stream = stream_data.stream;
        let mut downloaded_bytes: u64 = 0;

        while let Some(chunk_res) = stream.next().await {
            let chunk = chunk_res
                .map_err(|e| ErrorData::internal_error(format!("Koneksi streaming terputus: {}", e), None))?;
            file.write_all(&chunk)
                .map_err(|e| ErrorData::internal_error(format!("Gagal menulis byte ke disk: {}", e), None))?;
            downloaded_bytes += chunk.len() as u64;
        }

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
```

Update instructions in `get_info().with_instructions(...)` to document `gdrive_download_file`.

- [ ] **Step 3: Verify `gdrive-mcp` compiles without warnings**

Run: `cargo check -p gdrive-mcp`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/gdrive-mcp/src/models.rs crates/gdrive-mcp/src/server.rs
git commit -m "feat(mcp): add gdrive_download_file tool with strict schema safety"
```

---

### Task 8: Full Workspace Build & Verification

**Files:**
- Test: Entire workspace

- [ ] **Step 1: Run workspace compilation check**

Run: `cargo check --workspace`
Expected: Zero compilation errors across all workspace crates.

- [ ] **Step 2: Run all unit and integration tests**

Run: `cargo test --workspace`
Expected: All tests pass.

- [ ] **Step 3: Build release binary**

Run: `cargo build --release`
Expected: Binary `target/release/gdrive` built successfully.

- [ ] **Step 4: Update Documentation**

Update `README.md` to document the new `gdrive search` table view, `--json`/`--plain` options, download by name/ID, progress bar, and MCP `gdrive_download_file` tool.

- [ ] **Step 5: Final Commit**

```bash
git add README.md
git commit -m "docs: update README with search table, smart download, and download MCP tool"
```
