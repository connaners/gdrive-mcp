# gdrive-mcp

[![CircleCI](https://dl.circleci.com/status-badge/img/gh/connaners/gdrive-mcp/tree/main.svg?style=shield)](https://circleci.com/gh/connaners/gdrive-mcp)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

High-performance, schema-safe **Google Drive CLI & Model Context Protocol (MCP) Server** written in Rust.

`gdrive-mcp` connects AI Coding Agents (such as **Hermes**, **Claude Desktop**, **Cursor**, and **OpenCode**) and human developers directly to personal Google Drive and **Shared Drives (Team Drives)**.

---

## ✨ Features

- **Dual-Mode Capability:** Standalone developer CLI (`gdrive search`, `gdrive upload`, etc.) + Model Context Protocol (MCP) server over `stdio`.
- **Seamless OAuth2 Authentication:** One-command interactive login with a local loopback server (`http://127.0.0.1:8085`) and remote/headless fallback for headless servers.
- **Proactive & Reactive Auto-Refresh:** Never worry about expired access tokens—automatic proactive refresh before expiration and reactive retry on HTTP 401.
- **Full Shared Drives Support:** Transparently access and manage team files across all enterprise Shared Drives (`supportsAllDrives=true`, `includeItemsFromAllDrives=true`).
- **Auto-Export Google Docs:** Automatically converts native Google Docs/Sheets into readable Markdown/Plain text for AI agents.
- **Safe Trash:** Prevents catastrophic accidental data loss by utilizing Google Drive's trash bin rather than permanent deletions.
- **Zero-Pollution JSON-RPC:** Strict stdio isolation—all diagnostic logs are directed to `stderr`, keeping `stdout` pristine for JSON-RPC 2.0 frames.
- **Strict Schema Safety:** Every tool argument parameter is typed and documented with complete metadata, preventing HTTP 400 schema validation errors on Gemini, Claude, and OpenAI backends.

---

## 📁 Workspace Architecture

```text
gdrive-mcp/
├── Cargo.toml                  # Workspace manifest
├── crates/
│   ├── gdrive-core/            # Pure Drive v3 REST API client, OAuth2 & token lifecycle
│   ├── gdrive-mcp/             # MCP server implementation using `rmcp` (8 tools)
│   └── gdrive-cli/             # Main executable binary (`gdrive`)
```

---

## 🚀 Installation

### 1. One-Line Installer (Recommended — Tanpa Git Clone & Tanpa Rust)

Otomatis mendeteksi sistem operasi, mengunduh prebuilt binary resmi dari GitHub Releases, dan memasangnya ke `~/.local/bin/gdrive`:

```bash
curl -fsSL https://raw.githubusercontent.com/connaners/gdrive-mcp/main/install.sh | bash
```

### 2. Via Cargo (Tanpa Git Clone Manual)

Jika Anda sudah memiliki Rust/Cargo terpasang di komputer:

```bash
cargo install --git https://github.com/connaners/gdrive-mcp.git gdrive-cli
```

### 3. Unduh Prebuilt Binary Manual

Unduh arsip binary langsung dari halaman [GitHub Releases](https://github.com/connaners/gdrive-mcp/releases/latest), ekstrak, dan pindahkan `gdrive` ke `$PATH` Anda.

### 4. Build dari Source (Untuk Developer & Kontributor)

```bash
git clone https://github.com/connaners/gdrive-mcp.git
cd gdrive-mcp
cargo build --release
sudo cp target/release/gdrive /usr/local/bin/
```

---

## 🔑 Authentication Setup

`gdrive-mcp` stores OAuth tokens locally in `~/.config/gdrive-mcp/token.json` (or OS user config directory).

### 1. Interactive Setup Wizard (First-Time Onboarding)

Jika ini pertama kali Anda menggunakan `gdrive-mcp`, jalankan:

```bash
gdrive auth setup
```

Wizard interaktif ini akan memandu Anda:
1. Membuka link one-click aktivasi Google Drive API di Google Cloud Console.
2. Membuka link pembuatan **OAuth 2.0 Client ID** (Desktop App).
3. Meminta Anda memasukkan Client ID & Client Secret ke terminal.
4. Menyimpan credentials secara permanen ke `credentials.json` dan langsung melanjutkan otorisasi browser!

> **Tip:** Jika Anda langsung menjalankan `gdrive auth login` tanpa konfigurasi sebelumnya, CLI akan otomatis menawarkan untuk menjalankan Setup Wizard ini.

### 2. Login, Status, & Logout

```bash
# Login menggunakan Google OAuth2 (buka browser lokal)
gdrive auth login

# Untuk terminal remote/headless tanpa browser GUI
gdrive auth login --no-browser

# Cek masa berlaku token dan status koneksi
gdrive auth status

# Hapus token yang tersimpan (logout)
gdrive auth logout
```

### 3. Alternatif Manual (Environment Variables)

Jika tidak ingin menggunakan file `credentials.json`, Anda bisa menyetel environment variable:

```bash
export GOOGLE_CLIENT_ID="your-client-id.apps.googleusercontent.com"
export GOOGLE_CLIENT_SECRET="your-client-secret"
```

---

## 💻 CLI Usage Guide

```text
Google Drive CLI & MCP Server

Usage: gdrive <COMMAND>

Commands:
  auth      Manajemen autentikasi Google OAuth2 (login, status, logout)
  search    Cari file atau folder di Google Drive (support kata jamak tanpa petik, full-text, & order-by)
  info      Tampilkan informasi metadata detail file atau folder di Google Drive
  copy      Salin (duplicate) file di Google Drive
  rename    Ganti nama file atau folder di Google Drive
  move      Pindahkan file atau folder ke folder tujuan
  read      Baca isi file dari Google Drive (teks / markdown / docs / sheets / slides)
  create    Buat dokumen baru dari teks langsung
  update    Perbarui isi file yang sudah ada di Google Drive
  upload    Upload file lokal dari disk ke Google Drive
  download  Download file dari Google Drive ke disk lokal
  mkdir     Buat folder baru di Google Drive
  trash        Kelola tempat sampah (Trash) Google Drive
  drives       Tampilkan daftar Shared Drives (Team Drives) yang dapat diakses
  share        Bagikan file atau folder ke pengguna, grup, domain, atau publik (anyone)
  permissions  Kelola hak akses (permissions) file atau folder di Google Drive
  comments     Kelola komentar dan review pada file di Google Drive
  revisions    Kelola riwayat revisi (versi) file di Google Drive
  mcp          Jalankan MCP server untuk AI agents (Hermes, Claude, OpenCode)
```

### Examples

```bash
# Search files with pretty table output (supports multi-word queries without quotes)
gdrive search iboss v2 --limit 10 -s "modifiedTime desc"
gdrive search "Roadmap Q3" --limit 10

# Inspect detailed metadata of a file or folder
gdrive info 1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms
gdrive info 1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms --json

# Copy/duplicate a file
gdrive copy 1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms --name "Roadmap-Backup.md"

# Full-text search inside document contents
gdrive search "API Authentication" --full-text

# Output in JSON or plain format for scripts and piping
gdrive search Roadmap --json
gdrive search Roadmap --plain
# Rename a file
gdrive rename 1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms "Roadmap-v2.md"

# Move a file to another folder
gdrive move 1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms 1A2B3C4D5E

# Read file content (Docs to markdown, Sheets to CSV, Slides to text)
gdrive read 1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms

# Create a new markdown document
gdrive create "Changelog.md" "# Changelog\n- Initial release"

# Create a native Google Doc in a specific folder
gdrive create "Project Proposal" "Details..." --parent 1A2B3C4D5E --docs

# Update or append text to an existing file
gdrive update 1A2B3C4D5E "## New Section" --append

# Upload a local file
gdrive upload ./report.pdf --parent 1A2B3C4D5E

# Smart download: supports File ID, Drive URL, or File Name with interactive disambiguation
gdrive download "Annual Report.pdf"
gdrive download "https://drive.google.com/file/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/view"
gdrive download 1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms -o ~/Downloads/report.pdf

# Create a folder
gdrive mkdir "Sprint 42" --parent 1A2B3C4D5E

# Safe trash
gdrive trash 1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms

# List accessible Shared Drives
gdrive drives

# Share a file with anyone (public reader link)
gdrive share 1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms

# Share a file with a specific user as writer
gdrive share 1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms --role writer --target-type user --email alice@example.com --notification

# List permissions on a file
gdrive permissions list 1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms

# Revoke/delete a permission
gdrive permissions revoke 1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms perm_id_123

# Comments & Review collaboration
gdrive comments list 1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms
gdrive comments add 1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms "Mohon tinjau bagian arsitektur"
gdrive comments resolve 1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms comment_id_123

# Trash lifecycle & recovery
gdrive trash list
gdrive trash restore 1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms
gdrive trash empty --yes

# Revisions & version history
gdrive revisions list 1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms
gdrive revisions download 1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms rev_id_123 -o ./previous-version.docx

# Multi-account profiles
gdrive auth profile list
gdrive auth profile switch work
gdrive --profile personal search "Invoice"
```

---

## 🤖 MCP Server Tools (Model Context Protocol)

When running as an MCP server (`gdrive mcp serve`), `gdrive-mcp` exposes 25 schema-safe tools to LLMs:
| Tool Name | Description | Key Arguments |
|:---|:---|:---|
| `gdrive_search` | Search files & folders (supports full-text, ordering & pagination) | `query` (string), `folder_id` (string), `page_size` (u32), `page_token` (string), `full_text` (bool), `order_by` (string) |
| `gdrive_read_file` | Read file (Docs to md, Sheets to CSV, Slides to text) | `file_id` (string) |
| `gdrive_get_file_info` | Inspect file/folder metadata without downloading full content | `file_id` (string) |
| `gdrive_list_folder` | List contents of a folder | `folder_id` (string), `page_size` (u32) |
| `gdrive_create_file` | Create a new file or native Google Doc | `name` (string), `content` (string), `parent_folder_id` (string), `as_google_doc` (bool) |
| `gdrive_update_file` | Update or append text to an existing file | `file_id` (string), `content` (string), `append` (bool) |
| `gdrive_copy_file` | Duplicate a file with optional new name or destination folder | `file_id` (string), `new_name` (string), `parent_folder_id` (string) |
| `gdrive_download_file` | Download a file to local disk by ID or name | `file_id_or_name` (string), `destination_path` (string) |
| `gdrive_rename_file` | Rename a file or folder | `file_id` (string), `new_name` (string) |
| `gdrive_move_file` | Move a file or folder to a target folder | `file_id` (string), `new_parent_id` (string), `old_parent_id` (string) |
| `gdrive_create_folder`| Create a folder in personal or Shared Drives | `name` (string), `parent_folder_id` (string) |
| `gdrive_trash_file` | Safely move a file or folder to the trash | `file_id` (string) |
| `gdrive_list_drives` | List accessible enterprise Shared Drives | `page_size` (u32) |
| `gdrive_list_permissions` | View who has access to a file or folder | `file_id` (string) |
| `gdrive_share_file` | Share a file or folder with users, groups, domains, or anyone with link | `file_id` (string), `role` (string), `permission_type` (string), `email_address` (string), `send_notification` (bool) |
| `gdrive_delete_permission` | Revoke a permission from a file or folder | `file_id` (string), `permission_id` (string) |
| `gdrive_get_shareable_link` | Get shareable web link with optional automatic public access | `file_id` (string), `make_public` (bool) |
| `gdrive_list_comments` | View comments and discussions on a document | `file_id` (string), `page_size` (u32) |
| `gdrive_add_comment` | Add feedback or review comments to a file | `file_id` (string), `content` (string) |
| `gdrive_resolve_comment` | Mark a comment as resolved | `file_id` (string), `comment_id` (string) |
| `gdrive_list_trash` | List files and folders currently in the trash bin | `page_size` (u32), `page_token` (string) |
| `gdrive_restore_file` | Restore a file or folder from trash (untrash) | `file_id` (string) |
| `gdrive_empty_trash` | Permanently empty all items in trash (requires confirmation) | `confirm` (bool) |
| `gdrive_list_revisions` | View previous versions and revision history of a file | `file_id` (string) |
| `gdrive_get_revision` | Retrieve content or metadata of a past revision | `file_id` (string), `revision_id` (string) |
---

## 🔌 Agent Configurations

### 1. Hermes Agent (`~/.hermes/config.yaml`)

```yaml
mcp_servers:
  gdrive:
    command: /usr/local/bin/gdrive
    args: ["mcp", "serve"]
```

### 2. Claude Desktop (`claude_desktop_config.json`)

**macOS:** `~/Library/Application Support/Claude/claude_desktop_config.json`  
**Windows:** `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "gdrive": {
      "command": "/usr/local/bin/gdrive",
      "args": ["mcp", "serve"]
    }
  }
}
```

### 3. Cursor / VS Code (`mcp.json` or `.cursor/mcp.json`)

```json
{
  "mcpServers": {
    "gdrive": {
      "command": "/usr/local/bin/gdrive",
      "args": ["mcp", "serve"]
    }
  }
}
```

### 4. OpenCode (`opencode.json`)

```json
{
  "mcp": {
    "servers": {
      "gdrive": {
        "command": "/usr/local/bin/gdrive",
        "args": ["mcp", "serve"]
      }
    }
  }
}
```

---

## 🛡️ License

This project is licensed under the [MIT License](LICENSE).
