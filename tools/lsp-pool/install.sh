#!/usr/bin/env bash
# Install script for lsp-pool tool

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_PREFIX="${INSTALL_PREFIX:-$HOME/.local}"

echo "Installing lsp-pool tool..."

# Install Python dependencies
pip install --user click pyyaml --quiet 2>/dev/null || pip install click pyyaml --quiet

# Optional: psutil for memory tracking
pip install --user psutil --quiet 2>/dev/null || true

# Make binary executable
chmod +x "$SCRIPT_DIR/bin/lsp-pool"

# Create symlink
mkdir -p "$INSTALL_PREFIX/bin"
ln -sf "$SCRIPT_DIR/bin/lsp-pool" "$INSTALL_PREFIX/bin/lsp-pool"

# Create directories
mkdir -p "$HOME/.config/daedalos/lsp-pool"
mkdir -p "$HOME/.local/share/daedalos/lsp-pool"

# Create socket directory (may require sudo for /run)
if [[ -d /run/daedalos ]] || mkdir -p /run/daedalos 2>/dev/null; then
    echo "Socket directory ready: /run/daedalos"
else
    # Fallback to user directory
    mkdir -p "$HOME/.local/run/daedalos"
    export LSPPOOL_SOCKET="$HOME/.local/run/daedalos/lsp-pool.sock"
    echo "Using fallback socket: $LSPPOOL_SOCKET"
fi

# Verify installation
if command -v lsp-pool &>/dev/null; then
    echo "Installation complete!"
    echo ""
    lsp-pool --help | head -15
else
    echo "Installation complete!"
    echo ""
    echo "Note: Make sure $INSTALL_PREFIX/bin is in your PATH"
    echo "  export PATH=\"\$PATH:$INSTALL_PREFIX/bin\""
fi

# Check for language servers
echo ""
echo "Language servers needed for full functionality:"
for cmd in typescript-language-server pyright-langserver rust-analyzer gopls; do
    if command -v "$cmd" &>/dev/null; then
        echo "  [OK] $cmd"
    else
        echo "  [--] $cmd (not found)"
    fi
done
