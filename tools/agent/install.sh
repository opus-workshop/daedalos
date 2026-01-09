#!/usr/bin/env bash
# Install script for agent CLI
# Part of Daedalos toolsuite

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"
BIN_DIR="${PREFIX}/bin"
LIB_DIR="${PREFIX}/lib/daedalos/agent"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/daedalos/agent"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/daedalos/agent"
COMPLETION_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/zsh/site-functions"

echo "Installing agent CLI..."

# Check dependencies
check_dep() {
    if ! command -v "$1" &>/dev/null; then
        echo "Warning: $1 not found. Some features may not work."
    fi
}

check_dep tmux
check_dep jq
check_dep fzf  # Optional but recommended

# Create directories
mkdir -p "$BIN_DIR" "$LIB_DIR" "$CONFIG_DIR/templates" "$DATA_DIR" "$COMPLETION_DIR"

# Copy library files
cp "$SCRIPT_DIR"/lib/*.sh "$LIB_DIR/"

# Copy and symlink main executable
chmod +x "$SCRIPT_DIR/bin/agent"
ln -sf "$SCRIPT_DIR/bin/agent" "$BIN_DIR/agent"

# Copy templates
cp "$SCRIPT_DIR"/templates/*.json "$CONFIG_DIR/templates/"

# Copy completions
cp "$SCRIPT_DIR/completions/_agent" "$COMPLETION_DIR/"

# Initialize agents.json if not exists
if [[ ! -f "$DATA_DIR/agents.json" ]]; then
    echo '{"agents":{},"next_slot":1,"max_slots":9}' > "$DATA_DIR/agents.json"
fi

# Create default config if not exists
if [[ ! -f "$CONFIG_DIR/config.yaml" ]]; then
    cat > "$CONFIG_DIR/config.yaml" << 'EOF'
# Agent CLI configuration
max_agents: 9
default_template: implementer
default_sandbox: implement
auto_focus: true
log_retention_days: 7
EOF
fi

# Verify installation
echo ""
if command -v agent &>/dev/null; then
    echo "Installation complete!"
    echo ""
    agent --help | head -20
else
    echo "Installation complete!"
    echo ""
    echo "Note: Make sure $BIN_DIR is in your PATH"
    echo "  export PATH=\"\$PATH:$BIN_DIR\""
fi

echo ""
echo "For zsh completions, add to .zshrc:"
echo "  fpath=($COMPLETION_DIR \$fpath)"
echo "  autoload -Uz compinit && compinit"
