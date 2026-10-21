# AGENTS.md — Development & Architecture Guidelines for gdrive-mcp

Welcome, AI Coding Agent. This document outlines the architectural patterns, rules, and workflows for developing and extending `gdrive-mcp`.

---

## 1. Project Overview & Mission

`gdrive-mcp` is an unofficial, high-performance **Google Drive CLI and Model Context Protocol (MCP) server** written in Rust.
It provides AI agents (such as Hermes, Claude, OpenCode) and human developers with fast, seamless access to personal Google Drives and **Shared Drives (Team Drives)**.

### Key Capabilities:
- Search files and folders across personal and shared drives (`supportsAllDrives=true`).
- Read `.md` / text files directly and auto-export Google Workspace Docs to Markdown/Text.
- Create/upload files directly from AI discussions, with optional conversion to native Google Docs.
- Dual execution: Standalone CLI + MCP server over stdio.

---

## 2. Workspace Architecture

The project follows a modular Cargo workspace pattern (similar to `trelloc`):

```
gdrive-mcp/
├── Cargo.toml                  # Workspace manifest
├── crates/
│   ├── gdrive-core/            # Pure Google Drive API client & OAuth token storage
│   │   ├── src/auth.rs         # Token persistence & OAuth flow
│   │   ├── src/client.rs       # HTTP client calling Google Drive v3 REST API
│   │   ├── src/models.rs       # Data models (DriveFile, DriveFileList, etc.)
│   │   └── src/lib.rs
│   │
│   ├── gdrive-mcp/             # MCP server implementation using `rmcp`
│   │   ├── src/models.rs       # Tool parameter schemas with #[derive(JsonSchema)]
│   │   ├── src/server.rs       # MCP ToolRouter, ServerHandler, and stdio runner
│   │   └── src/lib.rs
│   │
│   └── gdrive-cli/             # Main executable binary named `gdrive`
│       └── src/main.rs         # CLI argument parsing (clap) & subcommand dispatcher
```

---

## 3. Critical Rules for Agents

### Rule 1: NEVER Print Debug Logs to `stdout`!
MCP stdio transport strictly uses `stdin` and `stdout` for newline-delimited JSON-RPC messages.
- Any stray `println!` or unredirected log will corrupt the JSON-RPC frame and crash the client connection.
- **Always log to `stderr`**: Use `eprintln!`, `tracing::info!`, `tracing::warn!`, or `tracing::error!`.
- In `gdrive-cli`, `tracing_subscriber` is already configured with `.with_writer(std::io::stderr)`. Keep it that way.

### Rule 2: Strict Schema Safety (NO Raw Boolean Schemas)
- Strict backends (like Google Gemini API / Vertex AI Protobuf) reject tool parameters where property schemas are bare boolean literals (`"prop": true`).
- **Every struct field** in `crates/gdrive-mcp/src/models.rs` MUST have an explicit Rust doc-comment (`/// ...`).
- Never use untyped `Option<serde_json::Value>` without doc comments or custom schema attributes, as `schemars` will generate `Schema::Bool(true)` and cause HTTP 400 errors.

### Rule 3: Always Support Shared Drives
When adding or modifying Google Drive API calls in `gdrive-core`:
- Always include query parameters: `supportsAllDrives=true` and `includeItemsFromAllDrives=true`.
- For file listings and searches, include `corpora=allDrives`.

### Rule 4: Zero Secret Leaks
- Never commit `token.json`, `credentials.json`, `client_secret*.json`, or `.env` files.
- Tokens must always be stored in the user config directory (`~/.config/gdrive-mcp/token.json`).

---

## 4. Development & Build Commands

```bash
# Check syntax & types across all workspace crates
cargo check

# Run tests
cargo test

# Build debug binary (output: target/debug/gdrive)
cargo build

# Build release binary (output: target/release/gdrive)
cargo build --release

# Run CLI directly via cargo
cargo run -p gdrive-cli -- search "RFC"
cargo run -p gdrive-cli -- auth status
cargo run -p gdrive-cli -- mcp serve
```

---

## 5. Adding New MCP Tools

When adding a new tool to `gdrive-mcp`:
1. Define the argument struct in `crates/gdrive-mcp/src/models.rs` with `#[derive(Debug, Deserialize, JsonSchema)]` and doc-comments on **every field**.
2. Add the client method in `crates/gdrive-core/src/client.rs`.
3. Add the `#[tool(...)]` method in `crates/gdrive-mcp/src/server.rs` under `impl GDriveMcpServer`.
4. Update `get_info().with_instructions(...)` in `server.rs` so LLMs understand when to call the new tool.
