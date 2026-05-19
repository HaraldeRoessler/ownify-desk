#!/usr/bin/env bash
# Download and install the ownify microclaw agent binary.
# Usage: ./install-microclaw.sh [version]
# Default: latest release from GitHub

set -euo pipefail

VERSION="${1:-latest}"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
VERSION_FILE="$INSTALL_DIR/.microclaw-version"

mkdir -p "$INSTALL_DIR"

OS=$(uname -s)
ARCH=$(uname -m)

case "$OS" in
  Darwin)  PLATFORM="apple-darwin" ;;
  Linux)   PLATFORM="unknown-linux-gnu" ;;
  *)       echo "Unsupported OS: $OS"; exit 1 ;;
esac

case "$ARCH" in
  arm64|aarch64) ARCH="aarch64" ;;
  x86_64)        ARCH="x86_64" ;;
  *)             echo "Unsupported arch: $ARCH"; exit 1 ;;
esac

if [ "$VERSION" = "latest" ]; then
  echo "Fetching latest release version..."
  VERSION=$(curl -s https://api.github.com/repos/HaraldeRoessler/ownify-microclaw/releases/latest | grep '"tag_name"' | sed 's/.*"tag_name": "\(.*\)".*/\1/')
  if [ -z "$VERSION" ]; then
    echo "No GitHub releases found yet."
    echo ""
    echo "Releases are created automatically when code is merged to main."
    echo "For now, build from source:"
    echo "  git clone https://github.com/HaraldeRoessler/ownify-microclaw.git"
    echo "  cd ownify-microclaw && cargo build --release"
    echo "  cp target/release/microclaw ~/.local/bin/"
    exit 1
  fi
fi

echo "Installing microclaw $VERSION for $PLATFORM/$ARCH..."

# GitHub release asset filename pattern
ASSET="microclaw-${VERSION}-${ARCH}-${PLATFORM}.tar.gz"
DOWNLOAD_URL="https://github.com/HaraldeRoessler/ownify-microclaw/releases/download/${VERSION}/${ASSET}"

echo "Downloading $DOWNLOAD_URL..."
TMPFILE=$(mktemp)
curl -L -o "$TMPFILE" "$DOWNLOAD_URL" || {
  echo "Download failed. The release may not have prebuilt binaries yet."
  echo "Build from source instead:"
  echo "  git clone https://github.com/HaraldeRoessler/ownify-microclaw.git"
  echo "  cd ownify-microclaw && cargo build --release"
  echo "  cp target/release/microclaw $INSTALL_DIR/"
  exit 1
}

echo "Extracting..."
tar xzf "$TMPFILE" -C "$INSTALL_DIR" microclaw
chmod +x "$INSTALL_DIR/microclaw"
rm "$TMPFILE"

echo "$VERSION" > "$VERSION_FILE"

echo ""
echo "✓ microclaw $VERSION installed to $INSTALL_DIR/microclaw"
echo ""
echo "Make sure $INSTALL_DIR is in your PATH:"
echo "  export PATH=\"$INSTALL_DIR:\$PATH\"  # add to ~/.bashrc or ~/.zshrc"
echo ""
echo "Now start ownify-desk:"
echo "  ownify-desk start"
