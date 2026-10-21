# gdrive-mcp

Google Drive CLI & Model Context Protocol (MCP) Server written in Rust.

Dirancang untuk menghubungkan AI Coding Agent (Hermes, Claude Desktop, OpenCode) ke Google Drive pribadi maupun Shared Drives, dengan dukungan membaca file `.md`, teks, auto-export Google Docs, serta upload/create file langsung dari diskusi agen.

## 🚀 Fitur Utama
- **Dual Mode:** CLI mandiri (`gdrive search`, `gdrive read`, `gdrive create`) + MCP server (`gdrive mcp serve`).
- **Shared Drives Support:** Query menyertakan `supportsAllDrives=true` dan `includeItemsFromAllDrives=true`.
- **Auto-Export Google Docs:** Otomatis mengekspor Google Docs ke format teks/Markdown saat dibaca oleh LLM.
- **Create & Convert:** Agen dapat membuat file `.md` baru atau otomatis mengonversinya menjadi dokumen native Google Docs.
- **Strict Schema Safe:** Schema dijamin kompatibel dengan Google Gemini, Claude, dan OpenAI (bebas dari bug boolean schema).

## 📁 Struktur Workspace
```
gdrive-mcp/
├── Cargo.toml
├── crates/
│   ├── gdrive-core/      # Client Google Drive API v3 & token storage
│   ├── gdrive-mcp/       # Handler & tool definitions MCP via rmcp
│   └── gdrive-cli/       # CLI binary (entrypoint 'gdrive')
```

## 🛠️ Build & Install
```bash
cargo build --release
# Binari akan berada di: target/release/gdrive
```

## 🤖 Integrasi MCP ke AI Agent

### Hermes Agent (`~/.hermes/config.yaml`)
```yaml
mcp_servers:
  gdrive:
    command: /Users/rohmnsa/On My Own/workuspace/gdrive-mcp/target/release/gdrive
    args: ["mcp", "serve"]
```

### Claude Desktop (`claude_desktop_config.json`)
```json
{
  "mcpServers": {
    "gdrive": {
      "command": "/Users/rohmnsa/On My Own/workuspace/gdrive-mcp/target/release/gdrive",
      "args": ["mcp", "serve"]
    }
  }
}
```
