#!/usr/bin/env bash
#===============================================================================
#                         DAEDALOS UNINSTALLER
#===============================================================================
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

BIN_DIR="$HOME/.local/bin"
CONFIG_DIR="$HOME/.config/daedalos"
DATA_DIR="$HOME/.local/share/daedalos"
LAUNCHD_DIR="$HOME/Library/LaunchAgents"

log() { echo -e "${BLUE}[daedalos]${NC} $1"; }
warn() { echo -e "${YELLOW}[daedalos]${NC} $1"; }

echo ""
echo -e "${RED}Daedalos Uninstaller${NC}"
echo ""
read -p "This will remove Daedalos tools. Continue? [y/N] " -n 1 -r
echo ""

if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Cancelled."
    exit 0
fi

# Stop daemons
log "Stopping daemons..."
for daemon in loopd mcp-hub undod; do
    launchctl unload "$LAUNCHD_DIR/com.daedalos.$daemon.plist" 2>/dev/null || true
    rm -f "$LAUNCHD_DIR/com.daedalos.$daemon.plist"
done

# Remove symlinks
log "Removing tool symlinks..."
for tool in loop loopd sandbox mcp-hub verify undo undod codex scratch error-db lsp-pool context agent; do
    rm -f "$BIN_DIR/$tool"
done

# Remove MCP registration
if command -v claude &>/dev/null; then
    log "Removing MCP registration..."
    claude mcp remove daedalos 2>/dev/null || true
fi

# Ask about config/data
echo ""
read -p "Remove config ($CONFIG_DIR)? [y/N] " -n 1 -r
echo ""
if [[ $REPLY =~ ^[Yy]$ ]]; then
    rm -rf "$CONFIG_DIR"
    log "Config removed"
fi

read -p "Remove data ($DATA_DIR)? [y/N] " -n 1 -r
echo ""
if [[ $REPLY =~ ^[Yy]$ ]]; then
    rm -rf "$DATA_DIR"
    log "Data removed"
fi

# Note about CLAUDE.md
warn "Note: Daedalos section in ~/.claude/CLAUDE.md was NOT removed."
warn "Edit manually if desired."

echo ""
echo -e "${GREEN}Uninstall complete.${NC}"
echo "You may want to remove PATH entries from ~/.zshrc or ~/.bashrc"
