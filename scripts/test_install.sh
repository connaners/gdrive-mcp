#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "=== Running Installer Script Verification Suite (scripts/test_install.sh) ==="

# 1. Syntax check
echo "[1/4] Checking bash syntax of install.sh..."
bash -n "${ROOT_DIR}/install.sh"
echo "  ✓ install.sh syntax is valid"

# 2. Unsupported OS rejection check
echo "[2/4] Testing unsupported OS rejection..."
MOCK_BIN_DIR="$(mktemp -d)"
cat << 'EOF' > "${MOCK_BIN_DIR}/uname"
#!/usr/bin/env bash
if [ "$1" = "-s" ]; then
    echo "Solaris"
else
    /usr/bin/uname "$@"
fi
EOF
chmod +x "${MOCK_BIN_DIR}/uname"

set +e
OS_TEST_OUTPUT=$(PATH="${MOCK_BIN_DIR}:${PATH}" bash "${ROOT_DIR}/install.sh" 2>&1)
OS_TEST_EXIT=$?
set -e
rm -rf "${MOCK_BIN_DIR}"

if [ "${OS_TEST_EXIT}" -eq 0 ]; then
    echo "  ✗ Expected installer to fail on unsupported OS 'Solaris', but it exited with 0"
    exit 1
fi
if [[ "${OS_TEST_OUTPUT}" != *"tidak didukung"* ]]; then
    echo "  ✗ Expected unsupported OS message, got: ${OS_TEST_OUTPUT}"
    exit 1
fi
echo "  ✓ Unsupported OS properly rejected"

# 3. Test installation with prebuilt binary placement (mocked download)
echo "[3/4] Testing simulated release binary download and placement..."

# Build target binary if not present
if [ ! -f "${ROOT_DIR}/target/debug/gdrive" ]; then
    echo "  Building local debug gdrive binary for test harness..."
    cargo build -p gdrive-cli
fi

SANDBOX_DIR="$(mktemp -d)"
MOCK_CURL_DIR="$(mktemp -d)"
TEMP_INSTALL_DIR="${SANDBOX_DIR}/install_dest"

# Prepare mock tar.gz
ARCHIVE_STAGING="$(mktemp -d)"
cp "${ROOT_DIR}/target/debug/gdrive" "${ARCHIVE_STAGING}/gdrive"
tar -czf "${SANDBOX_DIR}/mock_asset.tar.gz" -C "${ARCHIVE_STAGING}" gdrive
rm -rf "${ARCHIVE_STAGING}"

# Mock curl script that supplies the mock_asset.tar.gz when called with -o
cat << EOF > "${MOCK_CURL_DIR}/curl"
#!/usr/bin/env bash
output_file=""
while [ \$# -gt 0 ]; do
    if [ "\$1" = "-o" ]; then
        output_file="\$2"
        shift 2
    else
        shift
    fi
done

if [ -n "\${output_file}" ]; then
    cp "${SANDBOX_DIR}/mock_asset.tar.gz" "\${output_file}"
    exit 0
else
    exit 1
fi
EOF
chmod +x "${MOCK_CURL_DIR}/curl"

# Run install.sh with mocked curl and custom isolated INSTALL_DIR
INSTALL_OUTPUT=$(PATH="${MOCK_CURL_DIR}:${PATH}" INSTALL_DIR="${TEMP_INSTALL_DIR}" bash "${ROOT_DIR}/install.sh" 2>&1)

# Cleanup mocks
rm -rf "${MOCK_CURL_DIR}"

# 4. Assertions on installed artifact
echo "[4/4] Validating installed binary..."
INSTALLED_BIN="${TEMP_INSTALL_DIR}/gdrive"
if [ ! -f "${INSTALLED_BIN}" ]; then
    echo "  ✗ Installed binary '${INSTALLED_BIN}' does not exist!"
    echo "Installer output:"
    echo "${INSTALL_OUTPUT}"
    rm -rf "${SANDBOX_DIR}"
    exit 1
fi

if [ ! -x "${INSTALLED_BIN}" ]; then
    echo "  ✗ Installed binary '${INSTALLED_BIN}' is not executable!"
    rm -rf "${SANDBOX_DIR}"
    exit 1
fi

# Run version check
VERSION_OUTPUT=$("${INSTALLED_BIN}" --version)
if [[ "${VERSION_OUTPUT}" != *"gdrive"* ]]; then
    echo "  ✗ Installed binary version check failed: ${VERSION_OUTPUT}"
    rm -rf "${SANDBOX_DIR}"
    exit 1
fi
echo "  ✓ Installed binary runs successfully: ${VERSION_OUTPUT}"

# Check PATH warning was printed (since TEMP_INSTALL_DIR is not in PATH)
if [[ "${INSTALL_OUTPUT}" != *"belum ada di \$PATH"* ]]; then
    echo "  ✗ Expected PATH warning in installer output, but got:"
    echo "${INSTALL_OUTPUT}"
    rm -rf "${SANDBOX_DIR}"
    exit 1
fi
echo "  ✓ PATH warning properly reported for isolated directory"

rm -rf "${SANDBOX_DIR}"

echo ""
echo "======================================================="
echo "  🎉 All install.sh test harness checks passed!"
echo "======================================================="
