#!/bin/bash
# mcp-hwp Installer Script for macOS and Linux
# Usage: curl -sSL https://raw.githubusercontent.com/cypark/mcp-hwp/main/install.sh | bash

set -e

REPO="cypark/mcp-hwp"
BINARY_NAME="mcp-hwp"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

# Detect OS and Architecture
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
    linux)
        case "$ARCH" in
            x86_64) TARGET="x86_64-unknown-linux-gnu" ;;
            aarch64|arm64) TARGET="aarch64-unknown-linux-gnu" ;;
            *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
        esac
        ;;
    darwin)
        case "$ARCH" in
            x86_64) TARGET="x86_64-apple-darwin" ;;
            arm64) TARGET="aarch64-apple-darwin" ;;
            *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
        esac
        ;;
    *)
        echo "Unsupported OS: $OS"
        exit 1
        ;;
esac

echo "Detected: $OS ($ARCH) -> $TARGET"

# Get latest release version
echo "Fetching latest release..."
LATEST_RELEASE=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$LATEST_RELEASE" ]; then
    echo "Error: Could not fetch latest release"
    exit 1
fi

echo "Latest release: $LATEST_RELEASE"

# Download URL
DOWNLOAD_URL="https://github.com/$REPO/releases/download/$LATEST_RELEASE/${BINARY_NAME}-${TARGET}.tar.gz"

# Create temp directory
TEMP_DIR=$(mktemp -d)
trap "rm -rf $TEMP_DIR" EXIT

echo "Downloading from: $DOWNLOAD_URL"
curl -sSL "$DOWNLOAD_URL" -o "$TEMP_DIR/${BINARY_NAME}.tar.gz"

# Extract
echo "Extracting..."
tar -xzf "$TEMP_DIR/${BINARY_NAME}.tar.gz" -C "$TEMP_DIR"

# Install
if [ -w "$INSTALL_DIR" ]; then
    echo "Installing to $INSTALL_DIR..."
    mv "$TEMP_DIR/$BINARY_NAME" "$INSTALL_DIR/"
else
    echo "Installing to $INSTALL_DIR (requires sudo)..."
    sudo mv "$TEMP_DIR/$BINARY_NAME" "$INSTALL_DIR/"
fi

# Make executable
chmod +x "$INSTALL_DIR/$BINARY_NAME"

# Verify installation
if command -v "$BINARY_NAME" &> /dev/null; then
    echo "✅ Successfully installed $BINARY_NAME!"
    echo "Version: $($BINARY_NAME --version)"
    echo ""
    echo "Usage: $BINARY_NAME --help"
else
    echo "⚠️  Installation complete, but $BINARY_NAME not found in PATH"
    echo "Add $INSTALL_DIR to your PATH or run: export PATH=\"$INSTALL_DIR:\$PATH\""
fi
