#!/usr/bin/env bash
# Install script for context tool

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_PREFIX="${INSTALL_PREFIX:-$HOME/.local}"

echo "Installing context tool..."

# Check Python version
if ! command -v python3 &>/dev/null; then
    echo "Error: python3 is required but not found"
    exit 1
fi

# Install with pip
echo "Installing Python package..."
cd "$SCRIPT_DIR"

# Check if we're in a virtual environment or use --user
if [[ -n "${VIRTUAL_ENV:-}" ]]; then
    pip install -e .
else
    pip install --user -e .
fi

# Create wrapper script
echo "Creating wrapper script..."
mkdir -p "$INSTALL_PREFIX/bin"

cat > "$INSTALL_PREFIX/bin/context" << 'WRAPPER'
#!/usr/bin/env bash
# Wrapper for context tool
exec python3 -m context "$@"
WRAPPER

chmod +x "$INSTALL_PREFIX/bin/context"

# Verify installation
if command -v context &>/dev/null; then
    echo "Installation complete!"
    echo ""
    echo "Usage:"
    echo "  context status       - Show context budget"
    echo "  context breakdown    - Detailed breakdown"
    echo "  context files        - Files in context"
    echo "  context compact      - Compaction suggestions"
    echo ""
    echo "Run 'context --help' for more information."
else
    echo "Installation complete!"
    echo ""
    echo "Note: Make sure $INSTALL_PREFIX/bin is in your PATH"
    echo "  export PATH=\"\$PATH:$INSTALL_PREFIX/bin\""
fi
