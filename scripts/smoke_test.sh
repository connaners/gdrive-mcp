#!/usr/bin/env bash
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${ROOT_DIR}"

# Color codes
GREEN='\033[0;32m'
RED='\033[0;31m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
NC='\033[0m' # No Color

FAILED_STEPS=()

run_step() {
    local step_num="$1"
    local step_name="$2"
    shift 2
    local cmd=("$@")

    echo ""
    echo -e "${BLUE}${BOLD}[${step_num}/7] ${step_name}...${NC}"
    echo -e "${YELLOW}Running: ${cmd[*]}${NC}"

    if "${cmd[@]}"; then
        echo -e "${GREEN}✓ ${step_name} PASSED${NC}"
    else
        echo -e "${RED}✗ ${step_name} FAILED${NC}"
        FAILED_STEPS+=("${step_name}")
    fi
}

echo -e "${BOLD}=======================================================${NC}"
echo -e "${BOLD}      🚀 gdrive-mcp Pre-PR Smoke Test Suite${NC}"
echo -e "${BOLD}=======================================================${NC}"

# 1. Format
run_step "1" "Rust Code Formatting" cargo fmt --all -- --check

# 2. Clippy
run_step "2" "Clippy Linter & Warnings" cargo clippy --workspace --all-targets -- -D warnings

# 3. Unit & Core tests
run_step "3" "Unit & Core Test Suite" cargo test --workspace

# 4. CLI E2E tests (Wizard, sandbox, profiles, doctor)
TEMP_CFG=$(mktemp -d)
trap 'rm -rf "${TEMP_CFG}"' EXIT
run_step "4" "CLI & Wizard Sandbox E2E Tests" env GDRIVE_CONFIG_DIR="${TEMP_CFG}" cargo test -p gdrive-cli --test cli_e2e

# 5. MCP Stdio Contract (Rule 1 & Rule 2)
run_step "5" "MCP Stdio JSON-RPC Contract (Rules 1 & 2)" cargo test -p gdrive-cli --test mcp_stdio_contract

# 6. TUI Headless Render Tests (Headless views & modals)
run_step "6" "TUI Headless Render Tests" cargo test -p gdrive-tui --test tui_render_test

# 7. Security & Secret Leaks (Rule 4)
run_step "7" "Security & Secret Leaks Guard (Rule 4)" cargo test -p gdrive-cli --test security_tests
if command -v cargo-audit >/dev/null 2>&1 || cargo audit --version >/dev/null 2>&1; then
    run_step "7b" "Dependency Vulnerability Audit" cargo audit --deny warnings
else
    echo -e "${YELLOW}ℹ cargo-audit tidak terpasang di PATH; melewati audit kerentanan dependensi opsional.${NC}"
fi
# Bonus: Installer test script
echo ""
echo -e "${BLUE}${BOLD}[Bonus] Installer Test Script (install.sh)...${NC}"
if "${ROOT_DIR}/scripts/test_install.sh"; then
    echo -e "${GREEN}✓ Installer tests PASSED${NC}"
else
    echo -e "${RED}✗ Installer tests FAILED${NC}"
    FAILED_STEPS+=("Installer Test Script")
fi

echo ""
echo -e "${BOLD}=======================================================${NC}"
if [ ${#FAILED_STEPS[@]} -eq 0 ]; then
    echo -e "${GREEN}${BOLD}  🎉 ALL PRE-PR CHECKS PASSED! READY FOR PR!${NC}"
    echo -e "${BOLD}=======================================================${NC}"
    exit 0
else
    echo -e "${RED}${BOLD}  ❌ PRE-PR VERIFICATION FAILED IN ${#FAILED_STEPS[@]} STEP(S):${NC}"
    for failed in "${FAILED_STEPS[@]}"; do
        echo -e "${RED}   - ${failed}${NC}"
    done
    echo -e "${BOLD}=======================================================${NC}"
    exit 1
fi
