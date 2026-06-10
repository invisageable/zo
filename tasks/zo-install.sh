#!/bin/sh

# An installer for the programming language zo.
# Usage: curl --proto '=https' --tlsv1.2 -sSf https://zo.compilords.house/install.sh | sh

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

# Download a tarball and extract it into a destination
# directory. Returns the curl|tar pipeline's exit code so
# the caller decides whether the failure is fatal. `-f`
# turns 4xx/5xx HTTP responses into curl errors so a
# missing release artifact doesn't silently succeed with
# an empty extract.
download_tarball() {
  url="$1"
  dest="$2"
  mkdir -p "$dest"
  curl -sLf "$url" | tar xz -C "$dest"
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

  if ! download_tarball "$DOWNLOAD_URL" "$INSTALL_DIR"; then
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

  # Download and install the shipped packages alongside the
  # binary so the compiler can resolve `load core::…` and
  # `load provider::…`. It looks for each at `<exe>/../lib/<pack>`
  # (see `zo-host-paths`); `core` carries the auto-imported
  # prelude (io, assert, math).
  PACKS_URL="https://github.com/${REPO}/archive/refs/tags/${VERSION}.tar.gz"
  TMP_DIR="$(mktemp -d 2>/dev/null || mktemp -d -t zo-packs)"

  echo "Downloading standard packages from $PACKS_URL..."

  if ! download_tarball "$PACKS_URL" "$TMP_DIR"; then
    echo ""
    echo "error: failed to download standard-package archive"
    rm -rf "$TMP_DIR"
    exit 1
  fi

  # Keep in lockstep with `SYSTEM_PACK_ROOTS` in
  # `crates/compiler/zo-host-paths/src/lib.rs`.
  for pack in core provider; do
    src="$(find "$TMP_DIR" -type d \
      -path "*/crates/compiler-lib/${pack}" | head -n 1)"

    if [ -z "$src" ] || [ ! -d "$src" ]; then
      echo "error: pack '${pack}' not found in archive at" \
        "crates/compiler-lib/${pack}"
      rm -rf "$TMP_DIR"
      exit 1
    fi

    rm -rf "$LIB_DIR/${pack}"
    mkdir -p "$LIB_DIR/${pack}"
    # Copy contents (portable across BSD/GNU cp; no -T).
    (cd "$src" && tar cf - .) | (cd "$LIB_DIR/${pack}" && tar xf -)

    echo "pack '${pack}' installed to $LIB_DIR/${pack}"
  done

  rm -rf "$TMP_DIR"
  echo ""

  # Vendored prebuilt dylibs (raylib today; more libs land
  # here as the FFI surface grows). Shipped in a separate
  # release artifact so binary-only installs of `zo` itself
  # stay small for users who already have a system raylib
  # (`brew install raylib` / `apt install libraylib-dev`).
  # Resolved at codegen time as the F7 fallback for any
  # `#link { macos: { vendor: "name" }, ... }` entry —
  # `<exe-dir>/../lib/vendor/name` is the lookup path.
  VENDOR_URL="https://github.com/${REPO}/releases/download/${VERSION}/zo-vendor-${VERSION}-${PLATFORM}.tar.gz"
  VENDOR_DIR="${LIB_DIR}/vendor"

  echo "Downloading vendored libraries from $VENDOR_URL..."

  if download_tarball "$VENDOR_URL" "$VENDOR_DIR" 2>/dev/null; then
    echo "vendored libraries installed to $VENDOR_DIR"
  else
    # Non-fatal — programs that use only system-installed
    # libraries still build and run. Programs depending on
    # a vendored fallback (no system copy of the lib) will
    # fail at runtime with "Symbol not found".
    echo "no vendored libraries available for this release (skipping)"
    echo "  install system raylib for graphics demos:"
    echo "    macOS:  brew install raylib"
    echo "    Linux:  apt install libraylib-dev"
  fi
  echo ""

  # iOS Simulator runtime. The `aarch64-apple-ios-sim` cross-build of
  # libzo_runtime, staged so `zo build`/`run --target ios` can embed it
  # in the generated `.app` — `bundle_ios` resolves it from
  # `${ZO_HOME}/lib/runtime/<triple>/`. Apple-Silicon macOS only: the
  # Simulator runtime is arm64 and iOS codegen is arm64-only. Non-fatal,
  # like the vendor libraries — hosts without it just can't target iOS.
  if [ "$OS" = "apple-darwin" ] && [ "$ARCH" = "aarch64" ]; then
    IOS_TRIPLE="aarch64-apple-ios-sim"
    IOS_URL="https://github.com/${REPO}/releases/download/${VERSION}/zo-runtime-${VERSION}-${IOS_TRIPLE}.tar.gz"
    IOS_DIR="${LIB_DIR}/runtime/${IOS_TRIPLE}"

    echo "Downloading iOS Simulator runtime from $IOS_URL..."

    if download_tarball "$IOS_URL" "$IOS_DIR" 2>/dev/null; then
      echo "iOS Simulator runtime installed to $IOS_DIR"
    else
      echo "no iOS Simulator runtime for this release (skipping)"
    fi
    echo ""
  fi

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
