#!/usr/bin/env bash
# Install script for project tool

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_PREFIX="${INSTALL_PREFIX:-$HOME/.local}"

echo "Installing project tool..."

# Check Python version
if ! command -v python3 &>/dev/null; then
    echo "Error: python3 is required but not found"
    exit 1
fi

PYTHON_VERSION=$(python3 -c 'import sys; print(f"{sys.version_info.major}.{sys.version_info.minor}")')
REQUIRED_VERSION="3.8"

if [[ "$(printf '%s\n' "$REQUIRED_VERSION" "$PYTHON_VERSION" | sort -V | head -n1)" != "$REQUIRED_VERSION" ]]; then
    echo "Error: Python $REQUIRED_VERSION or higher is required (found $PYTHON_VERSION)"
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

cat > "$INSTALL_PREFIX/bin/project" << 'WRAPPER'
#!/usr/bin/env bash
# Wrapper for project tool
exec python3 -m project "$@"
WRAPPER

chmod +x "$INSTALL_PREFIX/bin/project"

# Verify installation
if command -v project &>/dev/null; then
    echo "Installation complete!"
    echo ""
    echo "Usage:"
    echo "  project summary [path]   - Show project summary"
    echo "  project map [path]       - Show dependency map"
    echo "  project deps <file>      - Show file dependencies"
    echo "  project dependents <file>- Show reverse dependencies"
    echo "  project search <query>   - Search symbols"
    echo "  project stats [path]     - Show statistics"
    echo "  project tree [path]      - Show file tree"
    echo "  project index [path]     - Re-index project"
    echo ""
    echo "Run 'project --help' for more information."
else
    echo "Installation complete!"
    echo ""
    echo "Note: Make sure $INSTALL_PREFIX/bin is in your PATH"
    echo "  export PATH=\"\$PATH:$INSTALL_PREFIX/bin\""
fi
