#!/usr/bin/env bash
# Daedalos Tools Installer
# Builds all tools and installs them to ~/.local/bin

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="${DAEDALOS_BIN:-$HOME/.local/bin}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Daedalos Tools Installer${NC}"
echo "=========================="
echo ""
echo "Source: $SCRIPT_DIR"
echo "Install to: $INSTALL_DIR"
echo ""

# Ensure install directory exists
mkdir -p "$INSTALL_DIR"

# Build in release mode for better performance
echo -e "${YELLOW}Building all tools (release mode)...${NC}"
cd "$SCRIPT_DIR"
cargo build --release 2>&1 | grep -E "(Compiling|Finished|error)" || true

echo ""
echo -e "${YELLOW}Installing binaries...${NC}"

INSTALLED=0
SKIPPED=0

# Install function: install_bin <source_name> [install_name]
install_bin() {
    local bin="$1"
    local install_name="${2:-$1}"
    local src="$SCRIPT_DIR/target/release/$bin"
    local dst="$INSTALL_DIR/$install_name"

    if [ -f "$src" ]; then
        rm -f "$dst"  # Remove old file/symlink
        cp "$src" "$dst"
        chmod +x "$dst"
        if [ "$bin" = "$install_name" ]; then
            echo -e "  ${GREEN}✓${NC} $bin"
        else
            echo -e "  ${GREEN}✓${NC} $bin -> $install_name"
        fi
        INSTALLED=$((INSTALLED + 1))
    else
        echo -e "  ${YELLOW}○${NC} $bin (not built)"
        SKIPPED=$((SKIPPED + 1))
    fi
}

# Install all binaries
install_bin loop
install_bin trust
install_bin undo
install_bin undod
install_bin verify
install_bin observe
install_bin agent
install_bin codex
install_bin gates
install_bin scratch
install_bin journal
install_bin context
install_bin project
install_bin error-db
install_bin sandbox
install_bin spec
install_bin mcp-hub
install_bin lsp-pool
install_bin evolve
install_bin resolve
install_bin daedalos-env env
install_bin daedalos-notify notify
install_bin focus
install_bin metrics
install_bin session
install_bin secrets
install_bin handoff
install_bin pair
install_bin review
install_bin backup
install_bin container
install_bin template
install_bin remote
install_bin daedalos
install_bin daedalos-mcp

echo ""
echo -e "${GREEN}Done!${NC}"
echo "  Installed: $INSTALLED"
echo "  Skipped: $SKIPPED"
echo ""

# Check if install dir is in PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo -e "${YELLOW}Warning:${NC} $INSTALL_DIR is not in your PATH"
    echo ""
    echo "Add this to your shell config (~/.bashrc, ~/.zshrc, etc.):"
    echo ""
    echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
    echo ""
fi

# Verify installation
echo "Verifying installation..."
if [ -f "$INSTALL_DIR/loop" ]; then
    echo -e "  ${GREEN}✓${NC} loop installed"
    "$INSTALL_DIR/loop" --version 2>/dev/null || true
fi
if [ -f "$INSTALL_DIR/trust" ]; then
    echo -e "  ${GREEN}✓${NC} trust installed"
fi
