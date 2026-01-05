#!/bin/bash
# Downloads pdfium library for the current platform.
# Uses bblanchon/pdfium-binaries releases.

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

# Detect platform
OS=$(uname -s)
ARCH=$(uname -m)

case "$OS" in
  Darwin)
    if [ "$ARCH" = "arm64" ]; then
      PLATFORM="mac-arm64"
    else
      PLATFORM="mac-x64"
    fi
    LIB_NAME="libpdfium.dylib"
    ;;
  Linux)
    if [ "$ARCH" = "aarch64" ]; then
      PLATFORM="linux-arm64"
    else
      PLATFORM="linux-x64"
    fi
    LIB_NAME="libpdfium.so"
    ;;
  MINGW*|MSYS*|CYGWIN*)
    if [ "$ARCH" = "x86_64" ]; then
      PLATFORM="win-x64"
    else
      PLATFORM="win-x86"
    fi
    LIB_NAME="pdfium.dll"
    ;;
  *)
    echo "Unsupported platform: $OS $ARCH"
    exit 1
    ;;
esac

DOWNLOAD_URL="https://github.com/bblanchon/pdfium-binaries/releases/latest/download/pdfium-${PLATFORM}.tgz"
TEMP_DIR=$(mktemp -d)

echo "Platform: $PLATFORM"
echo "Downloading pdfium from: $DOWNLOAD_URL"

# Download and extract
curl -L -o "$TEMP_DIR/pdfium.tgz" "$DOWNLOAD_URL"
tar -xzf "$TEMP_DIR/pdfium.tgz" -C "$TEMP_DIR"

# Platform-specific installation
case "$OS" in
  Darwin)
    # macOS: install to /usr/local/lib for development
    if [ -w /usr/local/lib ]; then
      cp "$TEMP_DIR/lib/$LIB_NAME" /usr/local/lib/
    else
      echo "Installing to /usr/local/lib (requires sudo)..."
      sudo cp "$TEMP_DIR/lib/$LIB_NAME" /usr/local/lib/
    fi
    echo "Installed to: /usr/local/lib/$LIB_NAME"

    # Bundle into ide.app
    APP_BUNDLE="$PROJECT_DIR/target/release/bundle/osx/ide.app"
    if [ -d "$APP_BUNDLE" ]; then
      FRAMEWORKS_DIR="$APP_BUNDLE/Contents/Frameworks"
      mkdir -p "$FRAMEWORKS_DIR"
      cp "$TEMP_DIR/lib/$LIB_NAME" "$FRAMEWORKS_DIR/"
      echo "Bundled into: $FRAMEWORKS_DIR/$LIB_NAME"
    fi
    ;;

  Linux)
    # Linux: install to /usr/local/lib
    if [ -w /usr/local/lib ]; then
      cp "$TEMP_DIR/lib/$LIB_NAME" /usr/local/lib/
    else
      echo "Installing to /usr/local/lib (requires sudo)..."
      sudo cp "$TEMP_DIR/lib/$LIB_NAME" /usr/local/lib/
      sudo ldconfig
    fi
    echo "Installed to: /usr/local/lib/$LIB_NAME"
    ;;

  MINGW*|MSYS*|CYGWIN*)
    # Windows: copy next to executables
    if [ -d "$PROJECT_DIR/target/release" ]; then
      cp "$TEMP_DIR/lib/$LIB_NAME" "$PROJECT_DIR/target/release/"
      echo "Copied to: $PROJECT_DIR/target/release/$LIB_NAME"
    fi
    ;;
esac

# Cleanup
rm -rf "$TEMP_DIR"

echo "Done. pdfium is ready to use."
