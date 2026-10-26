#!/usr/bin/env bash
set -euo pipefail

REPO="connaners/gdrive-mcp"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# Warna terminal
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}==>${NC} Installing gdrive-mcp..."

# 1. Deteksi OS
OS="$(uname -s)"
case "${OS}" in
    Darwin*)  PLATFORM="apple-darwin" ;;
    Linux*)   PLATFORM="unknown-linux-gnu" ;;
    *)
        echo -e "${RED}Error: Sistem operasi '${OS}' tidak didukung oleh installer ini.${NC}"
        echo "Silakan unduh binary secara manual dari: https://github.com/${REPO}/releases"
        exit 1
        ;;
esac

# 2. Deteksi Arsitektur
ARCH="$(uname -m)"
case "${ARCH}" in
    x86_64|amd64)   ARCH_NAME="x86_64" ;;
    arm64|aarch64)  ARCH_NAME="aarch64" ;;
    *)
        echo -e "${RED}Error: Arsitektur '${ARCH}' tidak didukung.${NC}"
        exit 1
        ;;
esac

ASSET_NAME="gdrive-${ARCH_NAME}-${PLATFORM}.tar.gz"
DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/${ASSET_NAME}"

# Fallback asset naming strictly for Linux (e.g. gdrive-linux-x86_64.tar.gz)
FALLBACK_ASSET_NAME=""
FALLBACK_DOWNLOAD_URL=""
if [[ "${OS}" == Linux* ]]; then
    FALLBACK_ASSET_NAME="gdrive-linux-${ARCH_NAME}.tar.gz"
    FALLBACK_DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/${FALLBACK_ASSET_NAME}"
fi

# Buat direktori tujuan jika belum ada
mkdir -p "${INSTALL_DIR}"

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT
ARCHIVE_PATH="${TMP_DIR}/archive.tar.gz"

DOWNLOADED=false
echo -e "${BLUE}==>${NC} Downloading ${ASSET_NAME} from GitHub Releases..."
if curl -fsSL "${DOWNLOAD_URL}" -o "${ARCHIVE_PATH}"; then
    DOWNLOADED=true
elif [ -n "${FALLBACK_ASSET_NAME:-}" ]; then
    echo -e "${YELLOW}==>${NC} Primary asset ${ASSET_NAME} tidak ditemukan. Mencoba fallback ${FALLBACK_ASSET_NAME}..."
    if curl -fsSL "${FALLBACK_DOWNLOAD_URL}" -o "${ARCHIVE_PATH}"; then
        DOWNLOADED=true
    fi
fi

if [ "${DOWNLOADED}" = "true" ]; then
    tar -xzf "${ARCHIVE_PATH}" -C "${TMP_DIR}"
    
    # Pasang binary
    TARGET_BIN="${INSTALL_DIR}/gdrive"
    rm -f "${TARGET_BIN}"
    cp "${TMP_DIR}/gdrive" "${TARGET_BIN}"
    chmod +x "${TARGET_BIN}"

    # Codesign ad-hoc jika di macOS untuk menghindari AMFI kill
    if [ "${OS}" = "Darwin" ]; then
        if command -v codesign >/dev/null 2>&1; then
            codesign --force --sign - "${TARGET_BIN}" >/dev/null 2>&1 || true
        fi
    fi

    echo -e "${GREEN}==> Sukses!${NC} gdrive berhasil dipasang di: ${TARGET_BIN}"
else
    echo -e "${YELLOW}==>${NC} Binary prebuilt belum tersedia untuk ${ASSET_NAME}."
    echo -e "${BLUE}==>${NC} Mencoba instalasi via 'cargo install'..."
    if command -v cargo >/dev/null 2>&1; then
        cargo install --git "https://github.com/${REPO}.git" gdrive-cli
        echo -e "${GREEN}==> Sukses!${NC} gdrive berhasil dipasang via Cargo."
    else
        echo -e "${RED}Gagal: Binary tidak ditemukan dan Cargo tidak terpasang.${NC}"
        echo "Silakan install Rust & Cargo (https://rustup.rs) atau unduh manual dari:"
        echo "https://github.com/${REPO}/releases"
        exit 1
    fi
fi

# Cek apakah direktori install ada di PATH
if [[ ":$PATH:" != *":${INSTALL_DIR}:"* ]]; then
    echo ""
    echo -e "${YELLOW}Catatan:${NC} '${INSTALL_DIR}' belum ada di \$PATH Anda."
    echo "Tambahkan baris berikut ke ~/.zshrc atau ~/.bashrc:"
    echo -e "  ${BLUE}export PATH=\"\$HOME/.local/bin:\$PATH\"${NC}"
fi

echo ""
echo -e "${GREEN}🎉 Siap digunakan!${NC}"
echo "Jalankan 'gdrive auth login' untuk menghubungkan Google Drive Anda."
