#!/usr/bin/env bash
# Install script for scratch tool

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_PREFIX="${INSTALL_PREFIX:-$HOME/.local}"

echo "Installing scratch tool..."

# Make binary executable
chmod +x "$SCRIPT_DIR/bin/scratch"

# Create symlink
mkdir -p "$INSTALL_PREFIX/bin"
ln -sf "$SCRIPT_DIR/bin/scratch" "$INSTALL_PREFIX/bin/scratch"

# Create required directories
mkdir -p "$HOME/.local/share/daedalos/scratch"

# Verify installation
if command -v scratch &>/dev/null; then
    echo "Installation complete!"
    echo ""
    scratch --help | head -15
else
    echo "Installation complete!"
    echo ""
    echo "Note: Make sure $INSTALL_PREFIX/bin is in your PATH"
    echo "  export PATH=\"\$PATH:$INSTALL_PREFIX/bin\""
fi
