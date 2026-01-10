#!/usr/bin/env bash
#===============================================================================
# common.sh - Shared utilities for verify tool
#===============================================================================

# Version
VERIFY_VERSION="1.0.0"

# Directories
VERIFY_CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/daedalos/verify"
VERIFY_STATE_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/daedalos/verify"

# Ensure directories exist
mkdir -p "$VERIFY_CONFIG_DIR" "$VERIFY_STATE_DIR" 2>/dev/null || true

#-------------------------------------------------------------------------------
# Logging
#-------------------------------------------------------------------------------

die() {
    echo -e "${RED:-}ERROR:${RESET:-} $*" >&2
    exit 1
}

warn() {
    echo -e "${YELLOW:-}WARNING:${RESET:-} $*" >&2
}

info() {
    if [[ "${VERBOSE:-false}" == "true" ]]; then
        echo -e "${BLUE:-}INFO:${RESET:-} $*"
    fi
}

debug() {
    if [[ "${DEBUG:-false}" == "true" ]]; then
        echo -e "${DIM:-}DEBUG: $*${RESET:-}" >&2
    fi
}

#-------------------------------------------------------------------------------
# YAML Parsing (simple implementation)
#-------------------------------------------------------------------------------

# Simple YAML parser for our pipeline format
parse_simple_yaml() {
    local yaml_file="$1"
    local prefix="${2:-}"
    local s='[[:space:]]*'
    local w='[a-zA-Z0-9_]*'

    sed -ne "s|^\($s\):|\1|" \
        -e "s|^\($s\)\($w\)$s:$s[\"']\(.*\)[\"']$s\$|\1$prefix\2=\"\3\"|p" \
        -e "s|^\($s\)\($w\)$s:$s\(.*\)$s\$|\1$prefix\2=\"\3\"|p" \
        "$yaml_file"
}

# Get a value from YAML (using yq if available, else grep)
yaml_get() {
    local file="$1"
    local path="$2"

    if command -v yq &>/dev/null; then
        yq -r "$path" "$file" 2>/dev/null
    else
        # Fallback: simple grep for top-level keys
        local key="${path#.}"
        grep "^${key}:" "$file" 2>/dev/null | sed 's/^[^:]*:[[:space:]]*//' | tr -d '"'"'"
    fi
}

# Get array from YAML
yaml_get_array() {
    local file="$1"
    local path="$2"

    if command -v yq &>/dev/null; then
        yq -r "$path | .[]?" "$file" 2>/dev/null
    else
        # Fallback not implemented for arrays
        echo ""
    fi
}

#-------------------------------------------------------------------------------
# Time utilities
#-------------------------------------------------------------------------------

# Get current time in milliseconds
now_ms() {
    if [[ "$(uname)" == "Darwin" ]]; then
        # macOS: use python for milliseconds
        python3 -c 'import time; print(int(time.time() * 1000))' 2>/dev/null || \
            echo "$(($(date +%s) * 1000))"
    else
        # Linux: date supports %N
        echo "$(($(date +%s%N) / 1000000))"
    fi
}

# Format milliseconds as human-readable
format_duration() {
    local ms="$1"

    if [[ $ms -lt 1000 ]]; then
        echo "${ms}ms"
    elif [[ $ms -lt 60000 ]]; then
        printf "%.1fs" "$(echo "scale=1; $ms / 1000" | bc)"
    else
        local mins=$((ms / 60000))
        local secs=$(((ms % 60000) / 1000))
        echo "${mins}m${secs}s"
    fi
}

#-------------------------------------------------------------------------------
# File utilities
#-------------------------------------------------------------------------------

# Check if we're in a git repo
in_git_repo() {
    git rev-parse --git-dir &>/dev/null 2>&1
}

# Get git root directory
git_root() {
    git rev-parse --show-toplevel 2>/dev/null
}

# Get staged files
staged_files() {
    if in_git_repo; then
        git diff --cached --name-only 2>/dev/null
    fi
}

#-------------------------------------------------------------------------------
# Tool checking
#-------------------------------------------------------------------------------

# Check if a command exists
has_command() {
    command -v "$1" &>/dev/null
}

# Require a command or die
require_command() {
    local cmd="$1"
    local install_hint="${2:-}"

    if ! has_command "$cmd"; then
        if [[ -n "$install_hint" ]]; then
            die "Required command '$cmd' not found. $install_hint"
        else
            die "Required command '$cmd' not found."
        fi
    fi
}

#-------------------------------------------------------------------------------
# Process management
#-------------------------------------------------------------------------------

# Run command with timeout
run_with_timeout() {
    local timeout_secs="$1"
    shift

    if has_command timeout; then
        timeout "${timeout_secs}s" "$@"
    elif has_command gtimeout; then
        gtimeout "${timeout_secs}s" "$@"
    else
        # Fallback: no timeout
        "$@"
    fi
}

#-------------------------------------------------------------------------------
# Help text
#-------------------------------------------------------------------------------

show_help() {
    cat << 'EOF'
verify - Universal project verification

USAGE:
    verify [OPTIONS] [path]

OPTIONS:
    -q, --quick       Fast checks only (lint + types)
    --staged          Only check git staged files
    -w, --watch       Continuous verification
    --fix             Auto-fix what's possible
    --pipeline NAME   Use specific pipeline
    --step STEP       Run only specific step
    --skip STEP       Skip specific step
    --json            Output as JSON
    --quiet           Minimal output (exit code only)
    -v, --verbose     Verbose output
    -h, --help        Show this help

COMMANDS:
    status            Show verification status
    pipelines         List available pipelines
    init              Create project config

EXAMPLES:
    verify                    # Run full verification
    verify --quick            # Fast checks only
    verify --fix              # Auto-fix issues
    verify --watch            # Watch mode
    verify --pipeline python  # Force Python pipeline

For more information: https://github.com/daedalos/tools
EOF
}
