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

  ZO_HOME="${HOME}/.zo"
  INSTALL_DIR="${ZO_HOME}/bin"
  LIB_DIR="${ZO_HOME}/lib"
  mkdir -p "$INSTALL_DIR"
  mkdir -p "$LIB_DIR"

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

  # Download and install the stdlib alongside the binary so
  # the compiler can auto-import the prelude (io, assert, math).
  # The runtime looks for it at `<exe>/../lib/std`.
  STD_URL="https://github.com/${REPO}/archive/refs/tags/${VERSION}.tar.gz"
  TMP_DIR="$(mktemp -d 2>/dev/null || mktemp -d -t zo-std)"

  echo "Downloading stdlib from $STD_URL..."

  if ! curl -sL "$STD_URL" | tar xz -C "$TMP_DIR"; then
    echo ""
    echo "error: failed to download stdlib archive"
    rm -rf "$TMP_DIR"
    exit 1
  fi

  SRC_STD="$(find "$TMP_DIR" -type d -path '*/crates/compiler-lib/std' | head -n 1)"

  if [ -z "$SRC_STD" ] || [ ! -d "$SRC_STD" ]; then
    echo "error: stdlib not found in archive at crates/compiler-lib/std"
    rm -rf "$TMP_DIR"
    exit 1
  fi

  rm -rf "$LIB_DIR/std"
  mkdir -p "$LIB_DIR/std"
  # Copy contents (portable across BSD/GNU cp; no -T).
  (cd "$SRC_STD" && tar cf - .) | (cd "$LIB_DIR/std" && tar xf -)
  rm -rf "$TMP_DIR"

  echo "stdlib installed to $LIB_DIR/std"
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

  # Add to PATH permanently.
  SHELL_PROFILE=""

  case "$SHELL" in
    */zsh)  SHELL_PROFILE="$HOME/.zshrc" ;;
    */bash) SHELL_PROFILE="$HOME/.bashrc" ;;
    */fish) SHELL_PROFILE="$HOME/.config/fish/config.fish" ;;
  esac

  if [ -n "$SHELL_PROFILE" ] && ! grep -q '.zo/bin' "$SHELL_PROFILE" 2>/dev/null; then
    echo 'export PATH="$HOME/.zo/bin:$PATH"' >> "$SHELL_PROFILE"
    echo "Added zo to PATH in $SHELL_PROFILE"
    echo ""
    echo "To start using zo, run:"
    echo ""
    echo "  source $SHELL_PROFILE"
    echo ""
    echo "Or open a new terminal window."
  else
    echo "zo is already in your PATH."
  fi

  echo ""
}

install
