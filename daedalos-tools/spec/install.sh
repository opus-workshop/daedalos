#!/usr/bin/env bash
#
# Install the spec tool
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"

echo "Installing spec tool..."

# Create directories
mkdir -p "$PREFIX/bin"
mkdir -p "$PREFIX/lib/daedalos/spec"

# Copy binary
cp "$SCRIPT_DIR/bin/spec" "$PREFIX/bin/spec"
chmod +x "$PREFIX/bin/spec"

# Copy libraries
cp "$SCRIPT_DIR/lib/"*.sh "$PREFIX/lib/daedalos/spec/"

# Copy templates
mkdir -p "$PREFIX/lib/daedalos/spec/templates"
cp "$SCRIPT_DIR/templates/"*.yaml "$PREFIX/lib/daedalos/spec/templates/" 2>/dev/null || true

# Update paths in the installed script
sed -i.bak "s|SCRIPT_DIR=\"\$(cd.*|SCRIPT_DIR=\"$PREFIX/lib/daedalos/spec\"|" "$PREFIX/bin/spec"
sed -i.bak "s|LIB_DIR=.*|LIB_DIR=\"$PREFIX/lib/daedalos/spec\"|" "$PREFIX/bin/spec"
sed -i.bak "s|TEMPLATE_DIR=.*|TEMPLATE_DIR=\"$PREFIX/lib/daedalos/spec/templates\"|" "$PREFIX/bin/spec"
rm -f "$PREFIX/bin/spec.bak"

echo "Installed to $PREFIX/bin/spec"
echo
echo "Ensure $PREFIX/bin is in your PATH:"
echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
