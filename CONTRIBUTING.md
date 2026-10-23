# Contributing to gdrive-mcp

Terima kasih atas minat Anda berkontribusi pada `gdrive-mcp`! Proyek ini adalah Google Drive CLI dan Model Context Protocol (MCP) server performa tinggi berbasis Rust.

Dokumen ini memandu Anda dalam menyiapkan environment, memahami standar arsitektur, dan mengajukan Pull Request (PR).

---

## 1. Persiapan Awal (Setup)

1. **Fork & Clone Repository**:
   ```bash
   git clone https://github.com/<username>/gdrive-mcp.git
   cd gdrive-mcp
   ```

2. **Pastikan Toolchain Rust Terpasang**:
   ```bash
   rustup component add clippy rustfmt
   ```

3. **Pasang Local Git Pre-commit Hook**:
   Jalankan script ini agar formatting, linter, dan fast tests otomatis berjalan sebelum commit:
   ```bash
   ./scripts/setup_git_hooks.sh
   ```

---

## 2. Aturan Wajib Arsitektur & Kualitas

Setiap perubahan kode **wajib** mematuhi 4 aturan inti:

### Rule 1: DILARANG Print ke `stdout` pada Path MCP Server
MCP stdio transport menggunakan `stdout` murni untuk frame JSON-RPC.
- Jangan gunakan `println!` di `gdrive-core` atau `gdrive-mcp`.
- Gunakan `tracing::info!`, `tracing::warn!`, `tracing::error!`, atau `eprintln!` (ke `stderr`).

### Rule 2: Strict Schema Safety (NO Raw Boolean Schemas)
Backend LLM (seperti Gemini / Claude) menolak schema properti dengan literal boolean mentah.
- **Setiap field struct** pada `crates/gdrive-mcp/src/models.rs` **wajib** memiliki Rust doc-comment (`/// ...`).
- Jangan gunakan `Option<serde_json::Value>` tanpa doc-comment eksplisit.

### Rule 3: Wajib Mendukung Shared Drives
Setiap panggilan Google Drive API baru pada `crates/gdrive-core/src/client.rs` harus menyertakan:
- `supportsAllDrives=true`
- `includeItemsFromAllDrives=true`
- `corpora=allDrives` (untuk listing/search)

### Rule 4: Zero Secret Leaks
- Jangan pernah meng-commit `token.json`, `credentials.json`, `client_secret*.json`, atau file `.env`.
- Mock auth atau gunakan path konfigurasi terisolasi saat menulis unit/integration tests.

---

## 3. Menjalankan Uji Kualitas Lokal

Sebelum membuka Pull Request, jalankan script verifikasi lengkap:

```bash
./scripts/smoke_test.sh
```

Script ini otomatis memeriksa:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace` (termasuk unit tests, integration tests, contract test `mcp_stdio_contract.rs`, dan TUI tests)
- Security credential check

---

## 4. Alur Pull Request (PR)

1. Buat branch baru dari `main`:
   ```bash
   git checkout -b feat/fitur-keren-kamu
   ```
2. Commit perubahan dengan pesan deskriptif.
3. Push ke fork Anda dan buat PR ke branch `main`.
4. CI (CircleCI) akan otomatis menjalankan pipeline `test-and-lint`. PR hanya dapat di-merge jika status check **`ci/circleci: test-and-lint`** lulus dan telah mendapatkan review approval dari maintainer.
