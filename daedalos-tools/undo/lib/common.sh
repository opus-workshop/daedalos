#!/usr/bin/env bash
#===============================================================================
# common.sh - Shared utilities for undo tool
#===============================================================================

# Version
UNDO_VERSION="1.0.0"

# Directories
UNDO_DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/daedalos/undo"
UNDO_CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/daedalos/undo"
UNDO_BACKUP_DIR="${UNDO_DATA_DIR}/backups"

# Ensure directories exist
mkdir -p "$UNDO_DATA_DIR" "$UNDO_CONFIG_DIR" "$UNDO_BACKUP_DIR" 2>/dev/null || true

#-------------------------------------------------------------------------------
# Colors
#-------------------------------------------------------------------------------

setup_colors() {
    if [[ -t 1 ]] && [[ -z "${NO_COLOR:-}" ]]; then
        RED='\033[0;31m'
        GREEN='\033[0;32m'
        YELLOW='\033[0;33m'
        BLUE='\033[0;34m'
        CYAN='\033[0;36m'
        BOLD='\033[1m'
        DIM='\033[2m'
        RESET='\033[0m'
    else
        RED='' GREEN='' YELLOW='' BLUE='' CYAN='' BOLD='' DIM='' RESET=''
    fi
}

setup_colors

#-------------------------------------------------------------------------------
# Logging
#-------------------------------------------------------------------------------

die() {
    echo -e "${RED}ERROR:${RESET} $*" >&2
    exit 1
}

warn() {
    echo -e "${YELLOW}WARNING:${RESET} $*" >&2
}

info() {
    echo -e "${BLUE}INFO:${RESET} $*"
}

success() {
    echo -e "${GREEN}OK:${RESET} $*"
}

debug() {
    if [[ "${DEBUG:-false}" == "true" ]]; then
        echo -e "${DIM}DEBUG: $*${RESET}" >&2
    fi
}

#-------------------------------------------------------------------------------
# Time Utilities
#-------------------------------------------------------------------------------

now_timestamp() {
    python3 -c 'import time; print(f"{time.time():.3f}")' 2>/dev/null || \
        echo "$(date +%s).000"
}

format_timestamp() {
    local ts="$1"
    local int_ts="${ts%%.*}"
    date -r "$int_ts" "+%H:%M:%S" 2>/dev/null || \
        date -d "@$int_ts" "+%H:%M:%S" 2>/dev/null || \
        echo "$ts"
}

format_timestamp_full() {
    local ts="$1"
    local int_ts="${ts%%.*}"
    date -r "$int_ts" "+%Y-%m-%d %H:%M:%S" 2>/dev/null || \
        date -d "@$int_ts" "+%Y-%m-%d %H:%M:%S" 2>/dev/null || \
        echo "$ts"
}

#-------------------------------------------------------------------------------
# File Utilities
#-------------------------------------------------------------------------------

get_project_root() {
    local dir="${1:-.}"

    # Try git root first
    if git -C "$dir" rev-parse --show-toplevel 2>/dev/null; then
        return
    fi

    # Otherwise use current directory
    cd "$dir" && pwd
}

get_relative_path() {
    local file="$1"
    local root="${2:-$(pwd)}"

    # Make path relative to root
    echo "${file#$root/}"
}

file_hash() {
    local file="$1"
    if [[ -f "$file" ]]; then
        sha256sum "$file" 2>/dev/null | cut -d' ' -f1 | head -c 16
    fi
}

file_size() {
    local file="$1"
    if [[ -f "$file" ]]; then
        stat -f%z "$file" 2>/dev/null || stat -c%s "$file" 2>/dev/null || echo 0
    else
        echo 0
    fi
}

#-------------------------------------------------------------------------------
# Help
#-------------------------------------------------------------------------------

show_help() {
    cat << 'EOF'
undo - File-level undo with timeline navigation

USAGE:
    undo [COMMAND] [OPTIONS]

COMMANDS:
    timeline        Show chronological list of changes (default)
    last [N]        Undo the last N edits
    to <ref>        Restore to a specific point
    preview <cmd>   Show what would change without doing it
    diff <ref>      Show diff from reference to current
    checkpoint [n]  Create a named checkpoint
    watch           Start watching for file changes
    status          Show current status
    cleanup         Clean up old entries
    record <file>   Manually record a file change
    help            Show this help

OPTIONS:
    -n <count>      Number of entries to show
    --file <path>   Filter to specific file
    --dry-run       Show what would happen without doing it
    --json          Output as JSON
    -v, --verbose   Verbose output
    -h, --help      Show this help

EXAMPLES:
    undo                        # Show timeline
    undo last                   # Undo last change
    undo last 3                 # Undo last 3 changes
    undo to 12:42:00            # Restore to timestamp
    undo to #5                  # Restore to entry #5
    undo to "pre-refactor"      # Restore to checkpoint
    undo checkpoint "before-big-change"
    undo diff #5                # Show diff since entry #5

For more information: https://github.com/daedalos/tools
EOF
}
