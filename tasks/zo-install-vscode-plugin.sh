#!/bin/sh

# Installs the zo-vscode extension from the source copy
# in crates/compiler/zo-vscode to the VS Code extensions
# directory.
#
# Usage: sh tasks/install-vscode.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SRC="$PROJECT_ROOT/crates/compiler/zo-vscode"
DST="$HOME/.vscode/extensions/zo-vscode"

if [ ! -d "$SRC" ]; then
  echo "error: source not found at $SRC"
  exit 1
fi

rm -rf "$DST"
mkdir -p "$DST/syntaxes"
mkdir -p "$DST/out"

cp "$SRC/package.json" "$DST/"
cp "$SRC/language-configuration.json" "$DST/"
cp "$SRC/syntaxes/zo.tmLanguage.json" "$DST/syntaxes/"

# LSP client entrypoint.
if [ -d "$SRC/out" ]; then
  cp -r "$SRC/out/." "$DST/out/"
fi

# Runtime dependency for vscode-languageclient.
if [ -d "$SRC/node_modules" ]; then
  cp -r "$SRC/node_modules" "$DST/node_modules"
fi

# Place zo-lsp next to zo at ~/.zo/bin/ so zo_host_paths
# resolves ~/.zo/lib/core automatically.
ZO_LSP="$PROJECT_ROOT/target/debug/zo-lsp"
ZO_HOME="$HOME/.zo"
ZO_BIN="$ZO_HOME/bin"
ZO_LIB="$ZO_HOME/lib"

mkdir -p "$ZO_BIN"
mkdir -p "$ZO_LIB"

if [ -f "$ZO_LSP" ]; then
  ln -sf "$ZO_LSP" "$ZO_BIN/zo-lsp"
  echo "zo-lsp symlinked to $ZO_BIN/zo-lsp"
fi

# Dev: symlink core library so zo_host_paths finds it
# at <exe>/../lib/core (the installed layout).
CORE_SRC="$PROJECT_ROOT/crates/compiler-lib/core"
if [ -d "$CORE_SRC" ] && [ ! -e "$ZO_LIB/core" ]; then
  ln -sf "$CORE_SRC" "$ZO_LIB/core"
  echo "core library symlinked to $ZO_LIB/core"
fi


echo "zo-vscode installed to $DST"
echo "reload VS Code to pick up changes."
