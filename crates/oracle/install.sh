#!/usr/bin/env bash
# Install oracle and create symlinks for ora and ask

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Build release binary
echo "Building oracle..."
cd "$WORKSPACE_ROOT"
cargo build --release -p oracle

# Determine install location
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
mkdir -p "$INSTALL_DIR"

# Install the binary
BINARY="$WORKSPACE_ROOT/target/release/oracle"
if [[ ! -f "$BINARY" ]]; then
    echo "Error: Binary not found at $BINARY"
    exit 1
fi

echo "Installing to $INSTALL_DIR..."
cp "$BINARY" "$INSTALL_DIR/oracle"
chmod +x "$INSTALL_DIR/oracle"

# Create symlinks
echo "Creating symlinks..."
ln -sf "$INSTALL_DIR/oracle" "$INSTALL_DIR/ora"
ln -sf "$INSTALL_DIR/oracle" "$INSTALL_DIR/ask"

echo ""
echo "Installed:"
echo "  $INSTALL_DIR/oracle"
echo "  $INSTALL_DIR/ora -> oracle"
echo "  $INSTALL_DIR/ask -> oracle"
echo ""
echo "Make sure $INSTALL_DIR is in your PATH."
echo ""
echo "Usage:"
echo "  ask \"what does this function do?\""
echo "  git diff | ask \"review this\""
echo "  ora      # REPL mode"
