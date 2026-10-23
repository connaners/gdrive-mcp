#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "Setting up git hooks for gdrive-mcp..."
mkdir -p "${ROOT_DIR}/.githooks"
chmod +x "${ROOT_DIR}/.githooks/pre-commit"
git config core.hooksPath .githooks
echo "✓ Git hooks configured to .githooks (pre-commit active)."
