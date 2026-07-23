#!/usr/bin/env bash

# =====================================================================
# yt-dlp-tui Automated Multi-Platform Installation Script
# =====================================================================
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/idolikechemistry/yt-dlp-tui/main/install.sh | bash
#
# Supported Platforms:
#   - macOS (Apple Silicon: aarch64-apple-darwin)
#   - Linux (Intel/AMD 64-bit: x86_64-unknown-linux-gnu)
#
# =====================================================================

set -eo pipefail

# Define repository variables
REPO_OWNER="idolikechemistry"
REPO_NAME="yt-dlp-tui"
BINARY_NAME="yt-dlp-tui"

# Status loggers
info() {
    echo "[Info] $1"
}

warn() {
    echo "[Warning] $1"
}

error() {
    echo "[Error] $1" >&2
    exit 1
}

# 1. Detect operating system and CPU architecture
OS="$(uname -s)"
ARCH="$(uname -m)"
TARGET_TRIPLE=""

info "Detecting system environment..."
info "OS: ${OS}, Architecture: ${ARCH}"

case "${OS}" in
    Darwin)
        if [ "${ARCH}" = "arm64" ]; then
            TARGET_TRIPLE="aarch64-apple-darwin"
        else
            error "Unsupported macOS architecture: ${ARCH}. Only Apple Silicon (arm64) is currently supported."
        fi
        ;;
    Linux)
        if [ "${ARCH}" = "x86_64" ] || [ "${ARCH}" = "amd64" ]; then
            TARGET_TRIPLE="x86_64-unknown-linux-gnu"
        else
            error "Unsupported Linux architecture: ${ARCH}. Only x86_64 is currently supported."
        fi
        ;;
    *)
        error "Unsupported operating system: ${OS}. Only macOS (Apple Silicon) and Linux (x86_64) are supported."
        ;;
esac

info "Selected build target: ${TARGET_TRIPLE}"

# 2. Define download and temporary working directories
TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TEMP_DIR}"' EXIT

DOWNLOAD_URL="https://github.com/${REPO_OWNER}/${REPO_NAME}/releases/latest/download/${BINARY_NAME}-${TARGET_TRIPLE}.tar.gz"
ARCHIVE_PATH="${TEMP_DIR}/${BINARY_NAME}-${TARGET_TRIPLE}.tar.gz"

# 3. Download the target release asset
info "Downloading latest release from GitHub..."
if curl -fsSL "${DOWNLOAD_URL}" -o "${ARCHIVE_PATH}"; then
    info "Download completed successfully."
else
    error "Failed to download asset from URL: ${DOWNLOAD_URL}. Please verify your network connection or repository status."
fi

# 4. Decompress the archive
info "Decompressing release archive..."
if tar -xzf "${ARCHIVE_PATH}" -C "${TEMP_DIR}"; then
    info "Archive extraction completed."
else
    error "Failed to extract archive: ${ARCHIVE_PATH}"
fi

EXTRACTED_BINARY="${TEMP_DIR}/${BINARY_NAME}"
if [ ! -f "${EXTRACTED_BINARY}" ]; then
    error "Extracted executable not found at expected location: ${EXTRACTED_BINARY}"
fi

# Ensure executable permissions inside temp dir
chmod +x "${EXTRACTED_BINARY}"

# 5. Determine installation destination
INSTALL_DIR="/usr/local/bin"
DEST_PATH="${INSTALL_DIR}/${BINARY_NAME}"

info "Installing executable to ${DEST_PATH}..."

# Check if destination directory is writable without sudo
USE_SUDO=""
if [ ! -w "${INSTALL_DIR}" ]; then
    warn "Write permission denied for ${INSTALL_DIR}. Elevation (sudo) will be requested."
    USE_SUDO="sudo"
fi

# Move binary to target path
if ${USE_SUDO} mv "${EXTRACTED_BINARY}" "${DEST_PATH}"; then
    info "Executable successfully moved to ${DEST_PATH}."
else
    error "Failed to install executable to ${DEST_PATH}."
fi

# Set executable permission in target path
if ${USE_SUDO} chmod +x "${DEST_PATH}"; then
    info "Permissions configured correctly."
else
    error "Failed to configure executable permissions on ${DEST_PATH}."
fi

# 6. Post-install actions (macOS Gatekeeper Quarantine Removal)
if [ "${OS}" = "Darwin" ]; then
    info "Running macOS security policy configuration..."
    if ${USE_SUDO} xattr -d com.apple.quarantine "${DEST_PATH}" 2>/dev/null; then
        info "macOS quarantine attribute removed successfully."
    else
        info "Quarantine attribute removal skipped (already clean or not applicable)."
    fi
fi

# 7. Verify installation
info "Verifying installation..."
if command -v "${BINARY_NAME}" >/dev/null 2>&1; then
    INSTALLED_VERSION="$(${BINARY_NAME} -V 2>/dev/null || ${BINARY_NAME} --version 2>/dev/null || echo "Unknown")"
    info "Success! ${BINARY_NAME} has been installed."
    echo "====================================================================="
    echo "  Installation Complete!"
    echo "  Command: ${BINARY_NAME}"
    echo "  Version: ${INSTALLED_VERSION}"
    echo "  Location: ${DEST_PATH}"
    echo "====================================================================="
else
    warn "Installation finished, but the command is not present in your current PATH."
    warn "Please ensure ${INSTALL_DIR} is included in your PATH environment variable."
fi
