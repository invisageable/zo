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

cp "$SRC/package.json" "$DST/"
cp "$SRC/language-configuration.json" "$DST/"
cp "$SRC/syntaxes/zo.tmLanguage.json" "$DST/syntaxes/"

echo "zo-vscode installed to $DST"
echo "reload VS Code to pick up changes."
