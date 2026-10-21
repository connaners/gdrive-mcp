# gdrive-mcp

[![CI](https://github.com/connaners/gdrive-mcp/actions/workflows/ci.yml/badge.svg)](https://github.com/connaners/gdrive-mcp/actions/workflows/ci.yml)
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
  search    Cari file atau folder di Google Drive (support full-text & order-by)
  rename    Ganti nama file atau folder di Google Drive
  move      Pindahkan file atau folder ke folder tujuan
  read      Baca isi file dari Google Drive (teks / markdown / docs / sheets / slides)
  create    Buat dokumen baru dari teks langsung
  update    Perbarui isi file yang sudah ada di Google Drive
  upload    Upload file lokal dari disk ke Google Drive
  download  Download file dari Google Drive ke disk lokal
  mkdir     Buat folder baru di Google Drive
  trash     Pindahkan file atau folder ke tempat sampah (Safe Trash)
  drives    Tampilkan daftar Shared Drives (Team Drives) yang dapat diakses
  mcp       Jalankan MCP server untuk AI agents (Hermes, Claude, OpenCode)
```

### Examples

```bash
# Search files with pretty table output (shows ID, Type icon, Name, Size, Modified Date)
gdrive search "Roadmap" --limit 10 -s "modifiedTime desc"

# Full-text search inside document contents
gdrive search "API Authentication" --full-text

# Output in JSON or plain format for scripts and piping
gdrive search "Roadmap" --json
gdrive search "Roadmap" --plain

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
```

---

## 🤖 MCP Server Tools (Model Context Protocol)

When running as an MCP server (`gdrive mcp serve`), `gdrive-mcp` exposes 11 tools to LLMs:

| Tool Name | Description | Key Arguments |
|:---|:---|:---|
| `gdrive_search` | Search files & folders (supports full-text, ordering & pagination) | `query` (string), `folder_id` (string), `page_size` (u32), `page_token` (string), `full_text` (bool), `order_by` (string) |
| `gdrive_read_file` | Read file (Docs to md, Sheets to CSV, Slides to text) | `file_id` (string) |
| `gdrive_list_folder` | List contents of a folder | `folder_id` (string), `page_size` (u32) |
| `gdrive_create_file` | Create a new file or native Google Doc | `name` (string), `content` (string), `parent_folder_id` (string), `as_google_doc` (bool) |
| `gdrive_update_file` | Update or append text to an existing file | `file_id` (string), `content` (string), `append` (bool) |
| `gdrive_download_file` | Download a file to local disk by ID or name | `file_id_or_name` (string), `destination_path` (string) |
| `gdrive_rename_file` | Rename a file or folder | `file_id` (string), `new_name` (string) |
| `gdrive_move_file` | Move a file or folder to a target folder | `file_id` (string), `new_parent_id` (string), `old_parent_id` (string) |
| `gdrive_create_folder`| Create a folder in personal or Shared Drives | `name` (string), `parent_folder_id` (string) |
| `gdrive_trash_file` | Safely move a file or folder to the trash | `file_id` (string) |
| `gdrive_list_drives` | List accessible enterprise Shared Drives | `page_size` (u32) |

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
