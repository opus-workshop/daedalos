#!/usr/bin/env bash
# Install script for mcp-hub tool

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_PREFIX="${INSTALL_PREFIX:-$HOME/.local}"

echo "Installing mcp-hub tool..."

# Install Python dependencies
pip install --user click pyyaml --quiet 2>/dev/null || pip install click pyyaml --quiet

# Make binary executable
chmod +x "$SCRIPT_DIR/bin/mcp-hub"

# Create symlink
mkdir -p "$INSTALL_PREFIX/bin"
ln -sf "$SCRIPT_DIR/bin/mcp-hub" "$INSTALL_PREFIX/bin/mcp-hub"

# Create directories
mkdir -p "$HOME/.config/daedalos/mcp-hub"
mkdir -p "$HOME/.local/share/daedalos/mcp-hub"

# Create socket directory (may require sudo for /run)
if [[ -d /run/daedalos ]] || mkdir -p /run/daedalos 2>/dev/null; then
    echo "Socket directory ready: /run/daedalos"
else
    # Fallback to user directory
    mkdir -p "$HOME/.local/run/daedalos"
    export MCPHUB_SOCKET="$HOME/.local/run/daedalos/mcp-hub.sock"
    echo "Using fallback socket: $MCPHUB_SOCKET"
fi

# Verify installation
if command -v mcp-hub &>/dev/null; then
    echo "Installation complete!"
    echo ""
    mcp-hub --help | head -15
else
    echo "Installation complete!"
    echo ""
    echo "Note: Make sure $INSTALL_PREFIX/bin is in your PATH"
    echo "  export PATH=\"\$PATH:$INSTALL_PREFIX/bin\""
fi

# Check for npm (needed for built-in servers)
echo ""
if command -v npm &>/dev/null; then
    echo "[OK] npm found - can install MCP servers"
else
    echo "[--] npm not found - some features require npm"
fi
