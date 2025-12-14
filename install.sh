#!/bin/sh
# Parsnip installer script
# Usage: curl -fsSL https://raw.githubusercontent.com/omar16100/parsnip/main/install.sh | sh

set -e

REPO="omar16100/parsnip"
BINARY_NAME="parsnip"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

info() {
    printf "${GREEN}[INFO]${NC} %s\n" "$1"
}

warn() {
    printf "${YELLOW}[WARN]${NC} %s\n" "$1"
}

error() {
    printf "${RED}[ERROR]${NC} %s\n" "$1"
    exit 1
}

# Detect OS
detect_os() {
    case "$(uname -s)" in
        Linux*)  echo "linux" ;;
        Darwin*) echo "macos" ;;
        MINGW*|MSYS*|CYGWIN*) echo "windows" ;;
        *) error "Unsupported OS: $(uname -s)" ;;
    esac
}

# Detect architecture
detect_arch() {
    case "$(uname -m)" in
        x86_64|amd64) echo "amd64" ;;
        arm64|aarch64) echo "arm64" ;;
        *) error "Unsupported architecture: $(uname -m)" ;;
    esac
}

# Get latest release version
get_latest_version() {
    curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | \
        grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/'
}

# Download and install
install() {
    OS=$(detect_os)
    ARCH=$(detect_arch)

    info "Detected OS: $OS, Arch: $ARCH"

    # Get latest version
    VERSION=$(get_latest_version)
    if [ -z "$VERSION" ]; then
        error "Could not determine latest version. Check https://github.com/${REPO}/releases"
    fi
    info "Latest version: $VERSION"

    # Construct download URL
    ASSET_NAME="parsnip-${OS}-${ARCH}.tar.gz"
    DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${ASSET_NAME}"

    info "Downloading from: $DOWNLOAD_URL"

    # Create temp directory
    TMP_DIR=$(mktemp -d)
    trap "rm -rf $TMP_DIR" EXIT

    # Download binary
    if ! curl -fsSL "$DOWNLOAD_URL" -o "$TMP_DIR/$ASSET_NAME"; then
        error "Failed to download $ASSET_NAME. Make sure release exists at $DOWNLOAD_URL"
    fi

    # Download checksums and verify
    CHECKSUM_URL="https://github.com/${REPO}/releases/download/${VERSION}/checksums.sha256"
    if curl -fsSL "$CHECKSUM_URL" -o "$TMP_DIR/checksums.sha256" 2>/dev/null; then
        info "Verifying checksum..."
        EXPECTED_SUM=$(grep "$ASSET_NAME" "$TMP_DIR/checksums.sha256" | awk '{print $1}')
        if [ -n "$EXPECTED_SUM" ]; then
            if command -v sha256sum >/dev/null 2>&1; then
                ACTUAL_SUM=$(sha256sum "$TMP_DIR/$ASSET_NAME" | awk '{print $1}')
            elif command -v shasum >/dev/null 2>&1; then
                ACTUAL_SUM=$(shasum -a 256 "$TMP_DIR/$ASSET_NAME" | awk '{print $1}')
            else
                warn "No sha256sum or shasum found, skipping checksum verification"
                ACTUAL_SUM="$EXPECTED_SUM"
            fi

            if [ "$EXPECTED_SUM" != "$ACTUAL_SUM" ]; then
                error "Checksum verification failed!
Expected: $EXPECTED_SUM
Got:      $ACTUAL_SUM
The downloaded file may be corrupted or tampered with."
            fi
            info "Checksum verified"
        else
            warn "No checksum found for $ASSET_NAME, skipping verification"
        fi
    else
        warn "Checksum file not found, skipping verification"
    fi

    # Extract
    info "Extracting..."
    tar -xzf "$TMP_DIR/$ASSET_NAME" -C "$TMP_DIR"

    # Create install directory
    mkdir -p "$INSTALL_DIR"

    # Install binary
    mv "$TMP_DIR/$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME"
    chmod +x "$INSTALL_DIR/$BINARY_NAME"

    info "Installed to: $INSTALL_DIR/$BINARY_NAME"

    # Check if in PATH
    if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
        warn "$INSTALL_DIR is not in your PATH"
        echo ""
        echo "Add this to your shell profile (~/.bashrc, ~/.zshrc, etc.):"
        echo ""
        echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
        echo ""
    fi

    info "Installation complete! Run 'parsnip --version' to verify."
}

# Run installation
install
