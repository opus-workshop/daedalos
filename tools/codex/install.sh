#!/usr/bin/env bash
# Install script for codex tool

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_PREFIX="${INSTALL_PREFIX:-$HOME/.local}"

echo "Installing codex tool..."

# Install Python package
cd "$SCRIPT_DIR"
pip install --user -e . --quiet 2>/dev/null || pip install -e . --quiet

# Make binary executable
chmod +x "$SCRIPT_DIR/bin/codex"

# Create symlink
mkdir -p "$INSTALL_PREFIX/bin"
ln -sf "$SCRIPT_DIR/bin/codex" "$INSTALL_PREFIX/bin/codex"

# Create data directory
mkdir -p "$HOME/.local/share/daedalos/codex"

# Verify installation
if command -v codex &>/dev/null; then
    echo "Installation complete!"
    echo ""
    codex --help | head -15
else
    echo "Installation complete!"
    echo ""
    echo "Note: Make sure $INSTALL_PREFIX/bin is in your PATH"
    echo "  export PATH=\"\$PATH:$INSTALL_PREFIX/bin\""
fi

# Check for Ollama
echo ""
if command -v ollama &>/dev/null; then
    echo "Ollama found - semantic search available"
    echo "Run 'codex index' to build the index"
else
    echo "Ollama not found - using TF-IDF fallback"
    echo "For better results, install Ollama: https://ollama.ai"
fi
