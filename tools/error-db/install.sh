#!/usr/bin/env bash
# Install script for error-db tool

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_PREFIX="${INSTALL_PREFIX:-$HOME/.local}"

echo "Installing error-db tool..."

# Make binary executable
chmod +x "$SCRIPT_DIR/bin/error-db"

# Create symlink
mkdir -p "$INSTALL_PREFIX/bin"
ln -sf "$SCRIPT_DIR/bin/error-db" "$INSTALL_PREFIX/bin/error-db"

# Create data directory
mkdir -p "$HOME/.local/share/daedalos/error-db"

# Verify installation
if command -v error-db &>/dev/null; then
    echo "Installation complete!"
    echo ""
    error-db --help | head -20
else
    echo "Installation complete!"
    echo ""
    echo "Note: Make sure $INSTALL_PREFIX/bin is in your PATH"
    echo "  export PATH=\"\$PATH:$INSTALL_PREFIX/bin\""
fi
