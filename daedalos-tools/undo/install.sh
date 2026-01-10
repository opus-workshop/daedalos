#!/usr/bin/env bash
#===============================================================================
# install.sh - Install undo tool
#===============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Installation paths
BIN_DIR="${HOME}/.local/bin"
LIB_DIR="${HOME}/.local/lib/daedalos/undo"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/daedalos/undo"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/daedalos/undo"

echo "Installing undo CLI..."

# Create directories
mkdir -p "$BIN_DIR" "$LIB_DIR" "$DATA_DIR" "$CONFIG_DIR"

# Copy library files
cp "${SCRIPT_DIR}/lib/"*.sh "$LIB_DIR/"

# Copy schema
mkdir -p "$LIB_DIR/schema"
cp "${SCRIPT_DIR}/schema/"*.sql "$LIB_DIR/schema/"

# Copy binaries
cp "${SCRIPT_DIR}/bin/undo" "${BIN_DIR}/undo"
cp "${SCRIPT_DIR}/bin/undod" "${BIN_DIR}/undod"
chmod +x "${BIN_DIR}/undo" "${BIN_DIR}/undod"

# Update paths in installed binaries
sed -i.bak "s|SCRIPT_DIR=.*|SCRIPT_DIR=\"${LIB_DIR}\"|" "${BIN_DIR}/undo"
sed -i.bak "s|LIB_DIR=.*|LIB_DIR=\"${LIB_DIR}\"|" "${BIN_DIR}/undo"
sed -i.bak "s|SCRIPT_DIR=.*|SCRIPT_DIR=\"${LIB_DIR}\"|" "${BIN_DIR}/undod"
sed -i.bak "s|LIB_DIR=.*|LIB_DIR=\"${LIB_DIR}\"|" "${BIN_DIR}/undod"
rm -f "${BIN_DIR}/undo.bak" "${BIN_DIR}/undod.bak"

# Create default config if not exists
if [[ ! -f "$CONFIG_DIR/config.yaml" ]]; then
    cat > "$CONFIG_DIR/config.yaml" << 'EOF'
# Undo configuration
enabled: true
storage_mode: auto  # auto, git, file
max_storage_mb: 1000

retention:
  entries_hours: 24
  hourly_checkpoints_days: 7
  daily_checkpoints_days: 30

ignore_patterns:
  - "*.log"
  - "*.tmp"
  - ".git/*"
  - "node_modules/*"
  - "__pycache__/*"
  - "*.pyc"
  - ".DS_Store"
EOF
fi

echo ""
echo "Installation complete!"
echo ""
echo "Installed:"
echo "  - ${BIN_DIR}/undo"
echo "  - ${BIN_DIR}/undod"
echo ""
echo "Make sure ${BIN_DIR} is in your PATH:"
echo '  export PATH="$HOME/.local/bin:$PATH"'
echo ""
echo "Quick start:"
echo "  undo                    # Show timeline"
echo "  undo record <file>      # Record a file change"
echo "  undo checkpoint 'name'  # Create checkpoint"
echo "  undo last               # Undo last change"
