#!/usr/bin/env bash
# Install script for sandbox tool

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_PREFIX="${INSTALL_PREFIX:-$HOME/.local}"

echo "Installing sandbox tool..."

# Make binary executable
chmod +x "$SCRIPT_DIR/bin/sandbox"

# Create symlink
mkdir -p "$INSTALL_PREFIX/bin"
ln -sf "$SCRIPT_DIR/bin/sandbox" "$INSTALL_PREFIX/bin/sandbox"

# Create required directories
mkdir -p "$HOME/.local/share/daedalos/sandbox"
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/daedalos/sandbox"

# Verify installation
if command -v sandbox &>/dev/null; then
    echo "Installation complete!"
    echo ""
    sandbox --help | head -20
else
    echo "Installation complete!"
    echo ""
    echo "Note: Make sure $INSTALL_PREFIX/bin is in your PATH"
    echo "  export PATH=\"\$PATH:$INSTALL_PREFIX/bin\""
fi
