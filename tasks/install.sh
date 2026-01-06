#!/bin/sh

# An installer for the graphical programming language zo.
# Usage: curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/invisageable/zo/main/tasks/install.sh | sh

set -e

REPO="invisageable/zo"
BINARY_NAME="zo"

# Detect OS and architecture
detect_platform() {
  OS="$(uname -s)"
  ARCH="$(uname -m)"

  case "$OS" in
    Linux)   OS="unknown-linux-gnu" ;;
    Darwin)  OS="apple-darwin" ;;
    MINGW*|MSYS*|CYGWIN*) OS="pc-windows-msvc" ;;
    *)
      echo "error: unsupported operating system: $OS"
      exit 1
      ;;
  esac

  case "$ARCH" in
    x86_64|amd64)  ARCH="x86_64" ;;
    aarch64|arm64) ARCH="aarch64" ;;
    *)
      echo "error: unsupported architecture: $ARCH"
      exit 1
      ;;
  esac

  PLATFORM="${ARCH}-${OS}"
}

# Get latest release tag
get_latest_release() {
  curl -sL "https://api.github.com/repos/${REPO}/releases/latest" | \
    grep '"tag_name":' | \
    sed -E 's/.*"([^"]+)".*/\1/'
}

# Download and install
install() {
  detect_platform

  echo "Detected platform: $PLATFORM"

  VERSION=$(get_latest_release)
  if [ -z "$VERSION" ]; then
    echo "error: could not determine latest version"
    echo "tip: you can build from source with: cargo install --git https://github.com/${REPO}"
    exit 1
  fi

  echo "Installing zo $VERSION..."

  # Release format: zo-VERSION-PLATFORM.tar.gz (e.g., zo-0.1.0-x86_64-apple-darwin.tar.gz)
  DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${VERSION}-${PLATFORM}.tar.gz"

  INSTALL_DIR="${HOME}/.zo/bin"
  mkdir -p "$INSTALL_DIR"

  echo "Downloading from $DOWNLOAD_URL..."

  if ! curl -sL "$DOWNLOAD_URL" | tar xz -C "$INSTALL_DIR"; then
    echo ""
    echo "error: failed to download pre-built binary"
    echo ""
    echo "You can install from source instead:"
    echo "  cargo install --git https://github.com/${REPO}"
    exit 1
  fi

  echo ""
  echo "zo installed to $INSTALL_DIR/$BINARY_NAME"
  echo ""

  # Check Linux dependencies
  if [ "$(uname -s)" = "Linux" ]; then
    echo "Linux detected. zo requires these runtime libraries:"
    echo "  - GTK3, WebKit2GTK, libsoup3"
    echo ""
    echo "Install with:"
    echo "  Ubuntu/Debian: sudo apt-get install libgtk-3-0 libwebkit2gtk-4.1-0 libsoup-3.0-0"
    echo "  Fedora:        sudo dnf install gtk3 webkit2gtk4.1 libsoup3"
    echo "  Arch:          sudo pacman -S gtk3 webkit2gtk-4.1 libsoup3"
    echo ""
  fi

  echo "Add this to your shell profile (.bashrc, .zshrc, etc.):"
  echo ""
  echo "  export PATH=\"\$HOME/.zo/bin:\$PATH\""
  echo ""
}

install
