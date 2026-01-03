#!/bin/bash

# Install script for fret-oz-syntax VS Code extension

EXTENSION_NAME="fret-oz-syntax-0.1.0"
VSCODE_EXTENSIONS_DIR="$HOME/.vscode/extensions"

echo "Installing Fret Configuration Syntax Highlighting..."

# Create extensions directory if it doesn't exist
mkdir -p "$VSCODE_EXTENSIONS_DIR"

# Remove old version if exists
if [ -d "$VSCODE_EXTENSIONS_DIR/$EXTENSION_NAME" ]; then
  echo "Removing old version..."
  rm -rf "$VSCODE_EXTENSIONS_DIR/$EXTENSION_NAME"
fi

# Copy extension files
echo "Copying extension files..."
cp -r "$(dirname "$0")" "$VSCODE_EXTENSIONS_DIR/$EXTENSION_NAME"

echo "✓ Installation complete!"
echo ""
echo "Please reload VS Code to activate the extension:"
echo "  1. Press Cmd+Shift+P (macOS) or Ctrl+Shift+P (Windows/Linux)"
echo "  2. Type 'Reload Window' and press Enter"
echo ""
echo "The extension will now provide syntax highlighting for .oz files."
