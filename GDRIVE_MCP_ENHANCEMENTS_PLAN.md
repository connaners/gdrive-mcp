# gdrive-mcp Enhancements Plan

## Context
Repository `/Users/rohmnsa/On My Own/workuspace/gdrive-mcp` is an active, production-grade Google Drive CLI and Model Context Protocol (MCP) server written in Rust (`gdrive-core`, `gdrive-mcp`, `gdrive-cli`, `gdrive-tui`).
This plan implements 4 targeted enhancements inspired by maintainer-grade practices: resolving release packaging and installer asset naming discrepancies, decoupling fast pre-commit checks from a comprehensive pre-push audit hook, adding an automated one-click client MCP registration CLI subcommand (`gdrive mcp install`), and activating dependency audit checks.

### Advisor & Maintainer Review Resolution
All 3 advisory points have been evaluated, validated, and directly incorporated into this plan:
1. **Linux Fallback Gating (Step 1)**: The installer's fallback to `gdrive-linux-${ARCH_NAME}.tar.gz` is strictly gated behind `[[ "${OS}" == Linux* ]]` so macOS (`Darwin`) systems never download incompatible Linux ELF binaries.
2. **CLI Parsing Unit Test (Step 3)**: Adhering to `AGENTS.md` (Layer 2), `crates/gdrive-cli/src/main.rs` includes `test_mcp_install_command_parsing` alongside the integration tests in `cli_e2e.rs`.
3. **Zed JSONC & Safety Handling (Step 3)**: Client configuration parsing handles JSONC comments/trailing commas gracefully and employs atomic backup (`.bak`) and staged temporary write (`.tmp` -> rename) to avoid corrupting user configs.

---

## Approach

### Step 1: Release Packaging & Installer Asset Alignment
- **Problem**: `install.sh` queries GitHub Releases for target triple names (`gdrive-${ARCH_NAME}-${PLATFORM}.tar.gz`, e.g. `gdrive-x86_64-unknown-linux-gnu.tar.gz`), whereas `.circleci/config.yml` packages `dist/gdrive-linux-x86_64.tar.gz`. On Linux, downloads 404 and fall back to compiling via `cargo install`.
- **Edit**:
  - In `/Users/rohmnsa/On My Own/workuspace/gdrive-mcp/.circleci/config.yml`:
    - In `build-release` job, package the binary using target triple naming:
      `tar -czvf dist/gdrive-x86_64-unknown-linux-gnu.tar.gz -C dist gdrive`
      and generate `dist/gdrive-x86_64-unknown-linux-gnu.tar.gz.sha256`.
    - Also create the legacy copy `dist/gdrive-linux-x86_64.tar.gz` and its `.sha256` so older scripts don't break.
    - Update the `gh release upload` and `gh release create` steps to upload all `dist/gdrive-*.tar.gz` and `dist/gdrive-*.sha256` assets.
  - In `/Users/rohmnsa/On My Own/workuspace/gdrive-mcp/install.sh`:
    - In the asset resolution section, add fallback asset names: download to fixed archive staging `"${TMP_DIR}/archive.tar.gz"` to decouple download filename from extraction.
    - **OS Gating (Advisor 3)**: If primary `${ASSET_NAME}` fails with HTTP 404, check fallback naming `gdrive-linux-${ARCH_NAME}.tar.gz` **strictly when `OS` is Linux** (`[[ "${OS}" == Linux* ]]`). NEVER attempt Linux fallback binaries on macOS (`Darwin`), preventing download of incompatible ELF binaries.
    - If both fail, fall back to `cargo install`.
  - In `/Users/rohmnsa/On My Own/workuspace/gdrive-mcp/scripts/test_install.sh`:
    - Update step count banners from `[1/4]` to `[1/5]`.
    - Add test case `[4/5]` asserting that fallback archive naming is recognized and correctly unpacked by the installer on Linux, followed by binary validation as `[5/5]`.
### Step 2: Optimized Git Hooks (Lightweight `pre-commit` + Maintainer `pre-push`)
- **Problem**: `.githooks/pre-commit` currently executes `cargo test --workspace` on every commit, introducing multi-second delays for minor changes. There is no `pre-push` gate to prevent pushing bad branches or committed secrets.
- **Edit**:
  - In `/Users/rohmnsa/On My Own/workuspace/gdrive-mcp/.githooks/pre-commit`:
    - Streamline to fast formatting check (`cargo fmt --all -- --check`) and fast clippy on workspace targets (`cargo clippy --workspace --all-targets -- -D warnings`). Execution: <1s.
    - Remove the slow full `cargo test --workspace` invocation from `pre-commit`.
  - Create `/Users/rohmnsa/On My Own/workuspace/gdrive-mcp/.githooks/pre-push`:
    - Read Git's stdin stream: `while read -r local_ref local_oid remote_ref remote_oid; do ... done`.
    - **Branch Deletion Guard**: If all pushed refs are branch deletions (`local_oid` all zeros or `local_ref` is `(delete)`), exit 0 immediately to skip running the smoke test suite on branch deletion.
    - **Branch Protection Guard**: Reject pushing to playground branches (`preview-all`, `local*`) and direct push to `refs/heads/main` or `refs/heads/master` on the remote.
    - **Safe Diff Resolution**: Compute base revision safely: if pushing a new branch (`@{upstream}` unset), determine base via `git merge-base "$local_oid" origin/main 2>/dev/null || git merge-base "$local_oid" main 2>/dev/null || echo "$local_oid^"`. Inspect diff for sensitive token patterns (`client_secret`, `refresh_token`, `BEGIN PRIVATE KEY`, `"type": "service_account"`).
    - **Verification Suite**: Execute `scripts/smoke_test.sh` (which already runs `scripts/test_install.sh` as bonus check; do NOT call `test_install.sh` twice).
  - In `/Users/rohmnsa/On My Own/workuspace/gdrive-mcp/scripts/setup_git_hooks.sh`:
    - Ensure both `chmod +x "${ROOT_DIR}/.githooks/pre-commit"` and `chmod +x "${ROOT_DIR}/.githooks/pre-push"` are executed.
    - Add notification message verifying that both hooks are installed and active.
### Step 3: MCP Client Auto-Registration Subcommand (`gdrive mcp install`)
- **Problem**: Users and AI assistants currently have to manually locate and edit client JSON config files (`claude_desktop_config.json`, `~/.cursor/mcp.json`) to register `gdrive`.
- **Edit**:
  - In `/Users/rohmnsa/On My Own/workuspace/gdrive-mcp/crates/gdrive-cli/Cargo.toml`:
    - Add `dirs.workspace = true` under `[dependencies]`.
  - In `/Users/rohmnsa/On My Own/workuspace/gdrive-mcp/crates/gdrive-cli/src/main.rs`:
    - Define `McpClient` with `#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]`:
      `Claude`, `Cursor`, `Windsurf`, `Zed`, `Json`.
    - Extend `McpCommands` enum with `Install`:
      ```rust
      /// Daftarkan gdrive MCP server secara otomatis ke client (Claude Desktop, Cursor, Windsurf, Zed)
      Install {
          /// Target client: claude, cursor, windsurf, zed, json
          #[arg(short, long, default_value = "claude", value_enum)]
          client: McpClient,
          /// Nama server MCP dalam konfigurasi (default: "gdrive")
          #[arg(short, long, default_value = "gdrive")]
          name: String,
          /// Profil Google Drive yang digunakan server (opsional, misal: "work")
          #[arg(short, long)]
          profile: Option<String>,
          /// Path file konfigurasi kustom
          #[arg(long)]
          config_path: Option<PathBuf>,
          /// Timpa konfigurasi server jika sudah ada
          #[arg(long)]
          force: bool,
          /// Tampilkan preview JSON konfigurasi tanpa menulis ke disk
          #[arg(long)]
          dry_run: bool,
      },
      ```
    - Implement `handle_mcp_install(...) -> Result<()>`:
      - Resolve current executable path using `std::env::current_exe()?`.
      - Build server args vector: if `let Some(prof) = profile`, `vec!["--profile".to_string(), prof, "mcp".to_string(), "serve".to_string()]`; otherwise `vec!["mcp".to_string(), "serve".to_string()]`.
      - Locate config path per client:
        - `Claude`: `dirs::config_dir()/Claude/claude_desktop_config.json` (on macOS resolves to `~/Library/Application Support/Claude/claude_desktop_config.json`, Windows `%APPDATA%\Claude`, Linux `~/.config/Claude`).
        - `Cursor`: `dirs::home_dir()/.cursor/mcp.json`.
        - `Windsurf`: `dirs::home_dir()/.codeium/windsurf/mcp_config.json`.
        - `Zed`: macOS (`~/Library/Application Support/Zed/settings.json` or `~/.config/zed/settings.json`), Linux (`~/.config/zed/settings.json`).
      - Expected injected JSON structure:
        - For `Claude`, `Cursor`, `Windsurf`:
          ```json
          "mcpServers": {
            "<name>": {
              "command": "<absolute-binary-path>",
              "args": ["mcp", "serve"]
            }
          }
          ```
        - For `Zed`:
          ```json
          "context_servers": {
            "<name>": {
              "command": {
                "path": "<absolute-binary-path>",
                "args": ["mcp", "serve"]
              }
            }
          }
          ```
      - If `dry_run` or `client == McpClient::Json`, print the formatted JSON snippet to stdout and return `Ok(())`.
      - **JSONC & Safety Handling (Advisor 1)**:
        - If file exists, read content. Strip leading/trailing whitespaces and check for comments (`//` or `/*`). If plain `serde_json::from_str` fails due to comments (JSONC), strip line comments or provide explicit error instructing the user to use `--dry-run` or manual editing for commented configs.
        - If file absent, start with `serde_json::json!({})`.
        - Target root object: `"mcpServers"` (or `"context_servers"` for Zed).
        - Check if key `<name>` already exists: if present and `!force`, return `bail!("Server '{}' already exists in {:?}. Use --force to overwrite.", name, path)`.
        - Create parent directories with `fs::create_dir_all`.
        - **Atomic Write & Backup**: If target file already exists, create `<path>.bak` before modifying. Write new content to a temporary file (`<path>.tmp`) and rename atomically over `<path>` with `fs::rename`.
    - **Testing (Layer 2 & Layer 3 per AGENTS.md / Advisor 2)**:
      1. **Layer 2 Unit Test in `crates/gdrive-cli/src/main.rs`**:
         - Add `test_mcp_install_command_parsing` in `#[cfg(test)] mod tests`: verify clap correctly parses `mcp install`, default `client = Claude`, custom `--client zed`, `--name my-drive`, `--profile work`, and `--force`.
      2. **Layer 2 E2E Integration Tests in `crates/gdrive-cli/tests/cli_e2e.rs`**:
         - `test_cli_mcp_install_dry_run_claude`: verify `--dry-run` outputs valid JSON containing `mcpServers.gdrive`.
         - `test_cli_mcp_install_dry_run_zed`: verify `--dry-run --client zed` outputs valid JSON containing `context_servers.gdrive`.
         - `test_cli_mcp_install_custom_path`: verify installing to a temporary config file creates valid JSON and preserves existing third-party servers.
         - `test_cli_mcp_install_profile_and_force`: verify `--profile work` adds `--profile work` to args, and existing server errors without `--force`.
### Step 4: Supply-Chain & Security Guarding
- **Problem**: `.forgeguard/config.toml` has `dependency-audit` set to `enabled = false`.
- **Edit**:
  - In `/Users/rohmnsa/On My Own/workuspace/gdrive-mcp/.forgeguard/config.toml`:
    - Set `dependency-audit` command to `enabled = true` and `required = false`.
    - Use shell fallback guard in `command` so `forgeguard gate` passes cleanly across developer machines without `cargo-audit` installed:
      `command = "sh -c 'if command -v cargo-audit >/dev/null 2>&1 || cargo audit --version >/dev/null 2>&1; then cargo audit --deny warnings; else echo \"cargo-audit not installed; skipping audit\"; fi'"`
  - In `/Users/rohmnsa/On My Own/workuspace/gdrive-mcp/scripts/smoke_test.sh`:
    - Keep Step 7 `cargo test -p gdrive-cli --test security_tests` intact.
    - Append conditional audit check in Step 7: if `command -v cargo-audit >/dev/null 2>&1`, execute `cargo audit --deny warnings`; otherwise output yellow informational notice without failing the test suite.
---

## Critical Files & Anchors
- `/Users/rohmnsa/On My Own/workuspace/gdrive-mcp/.circleci/config.yml`: Line 78 (`build-release`), packaging asset names.
- `/Users/rohmnsa/On My Own/workuspace/gdrive-mcp/install.sh`: Line 39 (`ASSET_NAME`), download URL resolution and fallback.
- `/Users/rohmnsa/On My Own/workuspace/gdrive-mcp/crates/gdrive-cli/src/main.rs`: Line 417 (`enum McpCommands`) and line 1579 (`handle_mcp`), adding `Install` action.
- `/Users/rohmnsa/On My Own/workuspace/gdrive-mcp/.githooks/pre-commit`: Line 1-15, remove slow workspace unit tests.
- `/Users/rohmnsa/On My Own/workuspace/gdrive-mcp/.githooks/pre-push`: New file, pre-push hook for smoke test & leak guard.

---

## Verification
1. **CLI Unit & MCP Install Tests**:
   - Run `cargo test -p gdrive-cli test_mcp_install_command_parsing` -> asserts CLI flags parse properly into typed struct.
   - Run `cargo test -p gdrive-cli --test cli_e2e test_cli_mcp_install` -> asserts successful dry-run, zed formatting, key merging, and profile injection.
   - Run `cargo run -p gdrive-cli -- mcp install --dry-run --client claude` -> outputs valid JSON with `mcpServers.gdrive`.
2. **Installer Verification**:
   - Run `./scripts/test_install.sh` -> all 5/5 tests pass including Linux-gated fallback archive detection.
3. **Pre-Push & Pre-Commit Hook**:
   - Run `./scripts/setup_git_hooks.sh` -> sets up hooks.
   - Execute `.githooks/pre-commit` -> completes fast (<1s).
   - Execute `.githooks/pre-push` -> runs `scripts/smoke_test.sh` and exits 0; correctly handles branch deletions and new branches.
4. **Full Workspace Smoke Test & Gate**:
   - Run `./scripts/smoke_test.sh` -> all 7 steps + installer bonus check pass green.
   - Run `cargo check --workspace` and `cargo test --workspace` -> zero errors.

## Assumptions & Contingencies
- **Config directory detection**: If `dirs::config_dir()` returns `None` on exotic operating systems, fall back to `$HOME/.config`.
- **Pre-push hook in headless environments**: If `git push` is executed in an environment without git credentials or upstream configured, the hook falls back to diffing against `HEAD~1`.
