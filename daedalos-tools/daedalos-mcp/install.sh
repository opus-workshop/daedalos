#!/usr/bin/env bash
# Install Daedalos MCP server

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "Installing Daedalos MCP server..."

# Install Python package
pip install --user -e "$SCRIPT_DIR" --quiet

# Get the path to the installed script
DAEDALOS_MCP_PATH=$(python3 -c "import shutil; print(shutil.which('daedalos-mcp') or '')")

if [[ -z "$DAEDALOS_MCP_PATH" ]]; then
    # Try common user bin locations
    for path in "$HOME/.local/bin/daedalos-mcp" "$HOME/Library/Python/3.*/bin/daedalos-mcp"; do
        if [[ -f $path ]]; then
            DAEDALOS_MCP_PATH="$path"
            break
        fi
    done
fi

echo ""
echo "Installation complete!"
echo ""
echo "MCP server installed at: $DAEDALOS_MCP_PATH"
echo ""
echo "To configure Claude Code, add to ~/.claude/settings.json:"
echo ""
cat << EOF
{
  "mcpServers": {
    "daedalos": {
      "command": "python3",
      "args": ["-m", "daedalos_mcp"]
    }
  }
}
EOF
echo ""
echo "Or if using the script directly:"
echo ""
cat << EOF
{
  "mcpServers": {
    "daedalos": {
      "command": "$DAEDALOS_MCP_PATH"
    }
  }
}
EOF
