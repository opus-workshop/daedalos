#!/usr/bin/env bash
#===============================================================================
# install.sh - Install verify tool
#===============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Installation paths
BIN_DIR="${HOME}/.local/bin"
LIB_DIR="${HOME}/.local/lib/daedalos/verify"

echo "Installing verify..."

# Create directories
mkdir -p "$BIN_DIR" "$LIB_DIR"

# Copy library files
cp -r "${SCRIPT_DIR}/lib/"* "$LIB_DIR/"
cp -r "${SCRIPT_DIR}/pipelines" "$LIB_DIR/"

# Create wrapper script that sources from installed location
cat > "${BIN_DIR}/verify" << 'WRAPPER'
#!/usr/bin/env bash
set -euo pipefail

LIB_DIR="${HOME}/.local/lib/daedalos/verify"
PIPELINES_DIR="${LIB_DIR}/pipelines"
export PIPELINES_DIR

source "${LIB_DIR}/common.sh"
source "${LIB_DIR}/output.sh"
source "${LIB_DIR}/detect.sh"
source "${LIB_DIR}/runner.sh"
source "${LIB_DIR}/watch.sh"

WRAPPER

# Append the main logic (minus the library loading parts)
sed -n '/^# Defaults$/,$p' "${SCRIPT_DIR}/bin/verify" >> "${BIN_DIR}/verify"

chmod +x "${BIN_DIR}/verify"

echo "Installed verify to ${BIN_DIR}/verify"
echo ""
echo "Make sure ${BIN_DIR} is in your PATH:"
echo '  export PATH="$HOME/.local/bin:$PATH"'
