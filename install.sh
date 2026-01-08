#!/bin/sh
set -e

REPO="ericzakariasson/duster"
INSTALL_DIR="/usr/local/bin"

# Detect OS and architecture
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
  darwin) OS="macos" ;;
  linux) OS="linux" ;;
  *) echo "Unsupported OS: $OS"; exit 1 ;;
esac

case "$ARCH" in
  x86_64) ARCH="x86_64" ;;
  arm64|aarch64) ARCH="arm64" ;;
  *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

# Linux only has x86_64 builds
if [ "$OS" = "linux" ]; then
  ARCH="x86_64"
fi

ASSET="duster-${OS}-${ARCH}.tar.gz"
URL="https://github.com/${REPO}/releases/latest/download/${ASSET}"

echo "Downloading duster for ${OS}-${ARCH}..."
curl -fsSL "$URL" -o /tmp/duster.tar.gz

echo "Installing to ${INSTALL_DIR}..."
tar -xzf /tmp/duster.tar.gz -C /tmp
sudo mv /tmp/duster "$INSTALL_DIR/duster"
rm /tmp/duster.tar.gz

echo "✓ duster installed successfully!"
echo "Run 'duster --help' to get started."
