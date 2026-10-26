# MCP Feature Parity & Seamless Agent Enhancements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Menghadirkan paritas fitur lengkap dan pengalaman yang seamless pada `gdrive-mcp` dengan menambahkan 6 kapabilitas baru (`upload_file`, `export_file`, `list_tree`, `get_file_path`, `star_file`, `batch_ops`), filter cerdas pada `gdrive_search`, multi-format download, prompt instructions baru, serta sinkronisasi release binary.

**Architecture:** Mengikuti arsitektur 4-layer workspace (`gdrive-core` -> `gdrive-mcp` -> `gdrive-cli` -> testing/verification). Semua tool baru mengimplementasikan Rule 1 (clean stdout/stderr logging) dan Rule 2 (strict schema safety dengan doc-comments pada seluruh field struct parameter tanpa boolean literals).

**Tech Stack:** Rust 2021, `rmcp` (MCP protocol implementation), `tokio`, `reqwest`, `serde`, `serde_json`, `schemars`, `assert_cmd`.

---

## Ringkasan Fitur yang Di-update / Ditambahkan

Total ada **7 peningkatan utama (37 tools total)**:

1. 📤 **`gdrive_upload_file` (New Tool)**: Mengunggah file lokal apapun (PDF, script, gambar, zip, dokumen) dari disk ke Google Drive dengan deteksi MIME otomatis dan opsional konversi ke native Google Docs.
2. 📄 **`gdrive_export_file` & Multi-Format Download (New Tool & Enhanced Tool)**: 
   - New tool `gdrive_export_file`: Ekspor dokumen Google Workspace ke format `pdf`, `docx`, `xlsx`, `csv`, `txt`, atau `html`.
   - Update tool `gdrive_download_file`: Penambahan parameter opsional `export_format` agar agent bisa langsung mengunduh Google Docs sebagai PDF/DOCX atau Sheets sebagai XLSX/CSV.
3. 🌳 **`gdrive_list_tree` (New Tool)**: Menampilkan struktur pohon direktori (folder tree) hingga kedalaman `max_depth` (1–4) dalam satu panggilan tool, menghemat puluhan roundtrip agent.
4. 🧭 **`gdrive_get_file_path` (New Tool)**: Menghasilkan human-readable breadcrumb path dari root Drive sampai file tujuan (`/My Drive/Projects/Backend/config.json`).
5. ⭐ **`gdrive_star_file` & Filter Cerdas `gdrive_search` (New Tool & Enhanced Tool)**:
   - New tool `gdrive_star_file`: Memberi atau mencabut bintang (star/favorite) pada file atau folder.
   - Enhanced `gdrive_search`: Penambahan parameter `file_type` (`folder`, `doc`, `sheet`, `slide`, `pdf`, `image`, `video`, `archive`) dan `starred_only`.
6. 📦 **`gdrive_batch_ops` (New Tool)**: Menjalankan serangkaian aksi file (move, trash, restore, copy, star) sekaligus dalam satu request dengan per-item error isolation.
7. 🧠 **MCP Prompt Instructions & Schema Enforcement**: Memperbarui instruksi model context di `server.rs:get_info()` dan memverifikasi seluruh kontrak schema via `mcp_stdio_contract.rs`.

---

## File Structure & Responsibilities

- **`crates/gdrive-core/src/models.rs`**:
  - Menambahkan field `starred` pada `DriveFile`.
  - Menambahkan field `file_type` dan `starred_only` pada `SearchFileOptions`.
  - Menambahkan model data untuk tree node (`DriveTreeNode`) dan batch operations (`DriveBatchAction`, `DriveBatchResult`).
- **`crates/gdrive-core/src/client.rs`**:
  - Menambahkan method `export_file_bytes(&self, raw_id: &str, target_mime: &str)`.
  - Menambahkan method `star_file(&self, raw_id: &str, starred: bool)`.
  - Menambahkan method `get_file_breadcrumbs(&self, raw_id: &str)`.
  - Menambahkan method `get_folder_tree(&self, root_id: &str, max_depth: u32)`.
  - Mengupdate `build_search_query` untuk memproses `file_type` dan `starred_only`.
- **`crates/gdrive-mcp/src/models.rs`**:
  - Mendefinisikan struct argument baru dengan doc-comments wajib di setiap field:
    - `UploadFileArgs`
    - `ExportFileArgs`
    - `ListTreeArgs`
    - `GetFilePathArgs`
    - `StarFileArgs`
    - `BatchOpsArgs`, `BatchActionItem`
    - Update `SearchFilesArgs` (`file_type`, `starred_only`)
    - Update `DownloadFileArgs` (`export_format`)
- **`crates/gdrive-mcp/src/server.rs`**:
  - Mendaftarkan tool handlers baru dengan anotasi `#[tool(...)]`.
  - Mengupdate instruksi router di `get_info().with_instructions(...)`.
- **`crates/gdrive-cli/tests/mcp_stdio_contract.rs`**:
  - Menjalankan iterasi kontrak schema untuk memvalidasi zero stdout pollution dan validitas skema seluruh tools baru.

---

### Task 1: Core API & Models Expansion (`gdrive-core`)

**Files:**
- Modify: `crates/gdrive-core/src/models.rs`
- Modify: `crates/gdrive-core/src/client.rs`
- Test: `crates/gdrive-core/tests/client_test.rs` (atau unit test di `models.rs`/`client.rs`)

- [x] **Step 1: Write unit tests for new query building and model serialization**
Tambah test di `crates/gdrive-core/src/client.rs`:
```rust
#[test]
fn test_build_search_query_with_file_type_and_starred() {
    let opts = SearchFileOptions {
        query: Some("laporan".to_string()),
        folder_id: None,
        page_size: Some(20),
        page_token: None,
        full_text: false,
        order_by: None,
        file_type: Some("pdf".to_string()),
        starred_only: Some(true),
    };
    let q = GDriveClient::build_search_query(&opts);
    assert!(q.contains("trashed = false"));
    assert!(q.contains("name contains 'laporan'"));
    assert!(q.contains("mimeType = 'application/pdf'"));
    assert!(q.contains("starred = true"));
}
```

- [x] **Step 2: Run test to verify failure**
Run: `cargo test -p gdrive-core -- test_build_search_query_with_file_type_and_starred`
Expected: FAIL (fields `file_type` and `starred_only` do not exist).

- [x] **Step 3: Update `models.rs` and `client.rs` in `gdrive-core`**
1. Tambahkan `starred` pada `DriveFile`:
```rust
#[serde(default)]
pub starred: Option<bool>,
```
2. Tambahkan `file_type` dan `starred_only` pada `SearchFileOptions`:
```rust
pub file_type: Option<String>,
pub starred_only: Option<bool>,
```
3. Update `build_search_query` di `client.rs` untuk memetakan `file_type` ke MIME query dan `starred_only` ke `starred = true`.
4. Tambahkan method `star_file`, `export_file_bytes`, `get_file_breadcrumbs`, dan `get_folder_tree` di `client.rs`.

- [x] **Step 4: Run test to verify pass**
Run: `cargo test -p gdrive-core`
Expected: PASS

- [x] **Step 5: Commit**
```bash
git add crates/gdrive-core
git commit -m "feat(core): add star, export, tree, breadcrumbs APIs and search filter options"
```

---

### Task 2: Implement Tool `gdrive_upload_file` in MCP

**Files:**
- Modify: `crates/gdrive-mcp/src/models.rs`
- Modify: `crates/gdrive-mcp/src/server.rs`
- Test: `crates/gdrive-cli/tests/mcp_stdio_contract.rs`

- [x] **Step 1: Define `UploadFileArgs` in `crates/gdrive-mcp/src/models.rs`**
```rust
/// Parameter untuk tool gdrive_upload_file
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadFileArgs {
    /// Path lengkap ke file lokal di disk yang ingin diunggah ke Google Drive.
    pub local_path: String,
    /// Nama file yang akan tersimpan di Google Drive (opsional, jika kosong menggunakan nama file asli di disk).
    pub name: Option<String>,
    /// ID folder induk di Google Drive tempat menyimpan file (opsional, jika kosong disimpan di root Drive).
    pub parent_folder_id: Option<String>,
    /// Jika true, otomatis mengonversi dokumen teks/markdown menjadi native Google Docs yang dapat diedit di web.
    pub as_google_doc: Option<bool>,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}
```

- [x] **Step 2: Add `#[tool(name = "gdrive_upload_file", ...)]` in `crates/gdrive-mcp/src/server.rs`**
Implementasikan pemanggilan `client.upload_file_path(...)` dengan verifikasi path lokal exists, canonicalization, dan respon metadata file hasil upload (`id`, `name`, `webViewLink`, `size`, `mimeType`).

- [x] **Step 3: Run contract check**
Run: `cargo test -p gdrive-cli --test mcp_stdio_contract`
Expected: PASS with `gdrive_upload_file` registered and schema-compliant.

- [x] **Step 4: Commit**
```bash
git add crates/gdrive-mcp
git commit -m "feat(mcp): add gdrive_upload_file tool for local disk uploads"
```

---

### Task 3: Implement Tool `gdrive_export_file` & Multi-Format Download

**Files:**
- Modify: `crates/gdrive-mcp/src/models.rs`
- Modify: `crates/gdrive-mcp/src/server.rs`

- [x] **Step 1: Define `ExportFileArgs` and update `DownloadFileArgs` in `models.rs`**
1. Tambahkan `ExportFileArgs`:
```rust
/// Parameter untuk tool gdrive_export_file
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExportFileArgs {
    /// ID file atau tautan Google Docs, Sheets, atau Slides yang ingin diekspor.
    pub file_id: String,
    /// Format tujuan ekspor: 'pdf', 'docx', 'xlsx', 'csv', 'txt', atau 'html'.
    pub format: String,
    /// Path lokasi penyimpanan file lokal hasil ekspor (opsional, jika kosong disimpan di direktori saat ini).
    pub destination_path: Option<String>,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional).
    pub profile: Option<String>,
}
```
2. Tambahkan `export_format` pada `DownloadFileArgs`:
```rust
/// Format ekspor jika file berupa Google Workspace docs (opsional: 'pdf', 'docx', 'xlsx', 'csv', 'txt', 'html').
pub export_format: Option<String>,
```

- [x] **Step 2: Implement `gdrive_export_file` and update `gdrive_download_file` in `server.rs`**
- Petakan format pendek (`pdf`, `docx`, dll.) ke Google Workspace export MIME type.
- Simpan file ke tujuan atau direktori kerja dengan nama `filename.<ext>`.
- Kembalikan hasil terstruktur berisi path lokal, ukuran byte, dan format ekspor.

- [x] **Step 3: Run contract tests**
Run: `cargo test -p gdrive-cli --test mcp_stdio_contract`
Expected: PASS

- [x] **Step 4: Commit**
```bash
git add crates/gdrive-mcp
git commit -m "feat(mcp): add gdrive_export_file and multi-format download capabilities"
```

---

### Task 4: Implement Navigation Tools: `gdrive_list_tree` & `gdrive_get_file_path`

**Files:**
- Modify: `crates/gdrive-mcp/src/models.rs`
- Modify: `crates/gdrive-mcp/src/server.rs`

- [x] **Step 1: Define `ListTreeArgs` and `GetFilePathArgs` in `models.rs`**
```rust
/// Parameter untuk tool gdrive_list_tree
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListTreeArgs {
    /// ID folder awal atau 'root' (opsional, default: root My Drive).
    pub folder_id: Option<String>,
    /// Kedalaman maksimal pohon folder (opsional, rentang 1-4, default: 2).
    pub max_depth: Option<u32>,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional).
    pub profile: Option<String>,
}

/// Parameter untuk tool gdrive_get_file_path
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetFilePathArgs {
    /// ID file atau folder yang ingin diketahui jalur hierarki path lengkapnya.
    pub file_id: String,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional).
    pub profile: Option<String>,
}
```

- [x] **Step 2: Implement handlers in `server.rs`**
- `gdrive_list_tree`: Mengembalikan pohon JSON rekursif dari folder dan file di dalamnya hingga `max_depth`.
- `gdrive_get_file_path`: Mengembalikan string `path` (`/My Drive/Work/Report.docx`) dan array `breadcrumbs` `[{"id": "...", "name": "..."}]`.

- [x] **Step 3: Run contract tests**
Run: `cargo test -p gdrive-cli --test mcp_stdio_contract`
Expected: PASS

- [x] **Step 4: Commit**
```bash
git add crates/gdrive-mcp
git commit -m "feat(mcp): add gdrive_list_tree and gdrive_get_file_path tools"
```

---

### Task 5: Implement `gdrive_star_file` and Smart Search Filters

**Files:**
- Modify: `crates/gdrive-mcp/src/models.rs`
- Modify: `crates/gdrive-mcp/src/server.rs`

- [x] **Step 1: Define `StarFileArgs` and update `SearchFilesArgs` in `models.rs`**
1. Tambahkan `StarFileArgs`:
```rust
/// Parameter untuk tool gdrive_star_file
#[derive(Debug, Deserialize, JsonSchema)]
pub struct StarFileArgs {
    /// ID file atau folder yang ingin ditandai atau dicabut bintangnya.
    pub file_id: String,
    /// True untuk menambahkan bintang (star), false untuk mencabut bintang (unstar).
    pub starred: bool,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional).
    pub profile: Option<String>,
}
```
2. Tambahkan `file_type` dan `starred_only` pada `SearchFilesArgs`:
```rust
/// Filter jenis file: 'folder', 'doc', 'sheet', 'slide', 'pdf', 'image', 'video', atau 'archive' (opsional).
pub file_type: Option<String>,
/// Hanya tampilkan file yang dibintangi/difavoritkan (opsional).
pub starred_only: Option<bool>,
```

- [x] **Step 2: Implement handlers in `server.rs`**
- Pasang handler `gdrive_star_file`.
- Sambungkan opsi `file_type` dan `starred_only` di handler `gdrive_search`.

- [x] **Step 3: Run contract tests**
Run: `cargo test -p gdrive-cli --test mcp_stdio_contract`
Expected: PASS

- [x] **Step 4: Commit**
```bash
git add crates/gdrive-mcp
git commit -m "feat(mcp): add gdrive_star_file and smart search filters"
```

---

### Task 6: Implement Tool `gdrive_batch_ops` for High-Efficiency Workflows

**Files:**
- Modify: `crates/gdrive-mcp/src/models.rs`
- Modify: `crates/gdrive-mcp/src/server.rs`

- [x] **Step 1: Define `BatchOpsArgs` and `BatchActionItem` in `models.rs`**
```rust
/// Satu item tindakan dalam operasi batch
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BatchActionItem {
    /// Tipe tindakan: 'move', 'trash', 'restore', 'copy', atau 'star'.
    pub action: String,
    /// ID file atau folder target.
    pub file_id: String,
    /// ID folder tujuan (wajib untuk tindakan 'move' dan opsional untuk 'copy').
    pub target_folder: Option<String>,
    /// Nama baru file (opsional untuk tindakan 'copy').
    pub new_name: Option<String>,
    /// Status bintang (wajib untuk tindakan 'star': true untuk star, false untuk unstar).
    pub starred: Option<bool>,
}

/// Parameter untuk tool gdrive_batch_ops
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BatchOpsArgs {
    /// Daftar operasi yang ingin dijalankan secara massal.
    pub operations: Vec<BatchActionItem>,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional).
    pub profile: Option<String>,
}
```

- [x] **Step 2: Implement `gdrive_batch_ops` in `server.rs`**
Jalankan setiap operasi secara terisolasi, tangani error per-item, dan kembalikan ringkasan `total_operations`, `succeeded`, `failed`, dan daftar hasil detail.

- [x] **Step 3: Run contract tests**
Run: `cargo test -p gdrive-cli --test mcp_stdio_contract`
Expected: PASS

- [x] **Step 4: Commit**
```bash
git add crates/gdrive-mcp
git commit -m "feat(mcp): add gdrive_batch_ops tool for multi-action execution"
```

---

### Task 7: Update MCP Instructions & CLI Subcommands Parity

**Files:**
- Modify: `crates/gdrive-mcp/src/server.rs` (update `get_info().with_instructions(...)`)
- Modify: `crates/gdrive-cli/src/main.rs` (pastikan CLI juga punya parity `tree` dan `star` jika relevan)
- Test: `crates/gdrive-cli/tests/cli_e2e.rs`

- [x] **Step 1: Update instructions in `server.rs`**
Kelompokkan panduan tools menjadi kategori yang jelas untuk AI Agent:
- *Discovery & Navigation*: `gdrive_search`, `gdrive_list_folder`, `gdrive_list_tree`, `gdrive_get_file_path`, `gdrive_list_drives`.
- *Reading & Writing*: `gdrive_read_file`, `gdrive_create_file`, `gdrive_update_file`, `gdrive_upload_file`, `gdrive_export_file`, `gdrive_download_file`.
- *Organization & Lifecycle*: `gdrive_rename_file`, `gdrive_move_file`, `gdrive_copy_file`, `gdrive_star_file`, `gdrive_batch_ops`, `gdrive_trash_file`, `gdrive_restore_file`, `gdrive_empty_trash`.
- *Collaboration*: `gdrive_share_file`, `gdrive_get_shareable_link`, `gdrive_list_permissions`, `gdrive_delete_permission`, `gdrive_list_comments`, `gdrive_add_comment`, `gdrive_resolve_comment`.
- *Revisions & History*: `gdrive_list_revisions`, `gdrive_get_revision`.
- *Diagnostics & Management*: `gdrive_list_profiles`, `gdrive_switch_profile`, `gdrive_get_about`, `gdrive_doctor`, `gdrive_check_update`, `gdrive_self_update`.

- [x] **Step 2: Commit**
```bash
git add crates/gdrive-mcp crates/gdrive-cli
git commit -m "docs(mcp): update system instructions for comprehensive 37 tools suite"
```

---

### Task 8: Comprehensive Quality Verification, Release Build & Installation

**Files:**
- Test suite: `./scripts/smoke_test.sh`
- Binary: `cargo build --release` -> install ke `~/.local/bin/gdrive`

- [x] **Step 1: Run full pre-PR verification script**
Run: `./scripts/smoke_test.sh`
Expected: Format check, clippy, unit tests, CLI tests, MCP contract tests, TUI render tests, security guards all PASS.

- [x] **Step 2: Build release binary & deploy locally**
Run:
```bash
cargo build --release
cp target/release/gdrive ~/.local/bin/gdrive
```
Pastikan `~/.local/bin/gdrive --version` dan `~/.local/bin/gdrive doctor` berjalan sukses.

- [x] **Step 3: Verification commit**
```bash
git commit --allow-empty -m "chore: complete pre-PR smoke tests and release installation"
```
