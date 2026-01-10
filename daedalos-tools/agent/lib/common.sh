#!/usr/bin/env bash
# common.sh - Shared utilities for agent CLI
#
# Provides configuration, colors, formatting, JSON helpers, and validation.

# Prevent double-sourcing
[[ -n "${_AGENT_COMMON_LOADED:-}" ]] && return 0
_AGENT_COMMON_LOADED=1

# Configuration directories
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/daedalos/agent"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/daedalos/agent"
AGENTS_FILE="${DATA_DIR}/agents.json"
TEMPLATES_DIR="${CONFIG_DIR}/templates"
CONFIG_FILE="${CONFIG_DIR}/config.yaml"

# Ensure directories exist
mkdir -p "$CONFIG_DIR" "$DATA_DIR" "$TEMPLATES_DIR"

# Color support
NO_COLOR="${NO_COLOR:-}"
TERM_COLORS=0

# Initialize color support
setup_colors() {
    if [[ -z "$NO_COLOR" ]] && [[ -t 1 ]]; then
        TERM_COLORS=$(tput colors 2>/dev/null || echo 0)
    fi

    if [[ $TERM_COLORS -ge 8 ]]; then
        C_RESET=$(tput sgr0)
        C_BOLD=$(tput bold)
        C_RED=$(tput setaf 1)
        C_GREEN=$(tput setaf 2)
        C_YELLOW=$(tput setaf 3)
        C_BLUE=$(tput setaf 4)
        C_MAGENTA=$(tput setaf 5)
        C_CYAN=$(tput setaf 6)
        C_WHITE=$(tput setaf 7)
        C_DIM=$(tput dim 2>/dev/null || echo "")
    else
        C_RESET=""
        C_BOLD=""
        C_RED=""
        C_GREEN=""
        C_YELLOW=""
        C_BLUE=""
        C_MAGENTA=""
        C_CYAN=""
        C_WHITE=""
        C_DIM=""
    fi
}

# Get color for agent status
color_status() {
    local status="$1"
    case "$status" in
        active|running)  echo "${C_GREEN}" ;;
        thinking)        echo "${C_BLUE}" ;;
        waiting)         echo "${C_YELLOW}" ;;
        idle)            echo "${C_WHITE}" ;;
        paused)          echo "${C_MAGENTA}" ;;
        error|dead)      echo "${C_RED}" ;;
        *)               echo "${C_WHITE}" ;;
    esac
}

# Format seconds as human-readable duration
format_duration() {
    local seconds="$1"
    local days=$((seconds / 86400))
    local hours=$(( (seconds % 86400) / 3600 ))
    local minutes=$(( (seconds % 3600) / 60 ))

    if [[ $days -gt 0 ]]; then
        echo "${days}d ${hours}h"
    elif [[ $hours -gt 0 ]]; then
        echo "${hours}h ${minutes}m"
    elif [[ $minutes -gt 0 ]]; then
        echo "${minutes}m"
    else
        echo "<1m"
    fi
}

# Format data as an aligned table
# Usage: format_table "HEADER1|HEADER2|HEADER3" "row1col1|row1col2|row1col3" ...
format_table() {
    local -a lines=("$@")
    local -a widths=()
    local IFS='|'

    # Calculate column widths
    for line in "${lines[@]}"; do
        read -ra cols <<< "$line"
        for i in "${!cols[@]}"; do
            local len=${#cols[i]}
            if [[ -z "${widths[i]:-}" ]] || [[ $len -gt ${widths[i]} ]]; then
                widths[i]=$len
            fi
        done
    done

    # Print lines
    local first=1
    for line in "${lines[@]}"; do
        read -ra cols <<< "$line"
        local output=""
        for i in "${!cols[@]}"; do
            local col="${cols[i]}"
            local width=${widths[i]:-0}
            if [[ -n "$output" ]]; then
                output+="  "
            fi
            output+=$(printf "%-${width}s" "$col")
        done
        if [[ $first -eq 1 ]]; then
            echo "${C_BOLD}${output}${C_RESET}"
            first=0
        else
            echo "$output"
        fi
    done
}

# JSON helpers using jq

# Get value from JSON file
# Usage: json_get <file> <path>
json_get() {
    local file="$1"
    local path="$2"
    if [[ -f "$file" ]]; then
        jq -r "$path // empty" "$file" 2>/dev/null
    fi
}

# Set value in JSON file
# Usage: json_set <file> <path> <value>
json_set() {
    local file="$1"
    local path="$2"
    local value="$3"
    local tmp="${file}.tmp.$$"

    if [[ -f "$file" ]]; then
        jq "$path = $value" "$file" > "$tmp" && mv "$tmp" "$file"
    else
        echo "{}" | jq "$path = $value" > "$file"
    fi
}

# Delete key from JSON file
# Usage: json_delete <file> <path>
json_delete() {
    local file="$1"
    local path="$2"
    local tmp="${file}.tmp.$$"

    if [[ -f "$file" ]]; then
        jq "del($path)" "$file" > "$tmp" && mv "$tmp" "$file"
    fi
}

# Validate agent name
validate_name() {
    local name="$1"
    if [[ -z "$name" ]]; then
        die "Agent name cannot be empty"
    fi
    if [[ ! "$name" =~ ^[a-zA-Z][a-zA-Z0-9_-]*$ ]]; then
        die "Agent name must start with a letter and contain only letters, numbers, hyphens, and underscores"
    fi
    if [[ ${#name} -gt 32 ]]; then
        die "Agent name must be 32 characters or less"
    fi
    return 0
}

# Validate project directory
validate_project() {
    local path="$1"
    if [[ -z "$path" ]]; then
        die "Project path cannot be empty"
    fi
    if [[ ! -d "$path" ]]; then
        warn "Project directory does not exist: $path"
    fi
    return 0
}

# Error handling

# Print error and exit
die() {
    echo "${C_RED}error:${C_RESET} $*" >&2
    exit 1
}

# Print warning
warn() {
    echo "${C_YELLOW}warning:${C_RESET} $*" >&2
}

# Print info
info() {
    echo "${C_CYAN}info:${C_RESET} $*"
}

# Print success
success() {
    echo "${C_GREEN}success:${C_RESET} $*"
}

# Print debug (only if AGENT_DEBUG is set)
debug() {
    if [[ -n "${AGENT_DEBUG:-}" ]]; then
        echo "${C_DIM}debug: $*${C_RESET}" >&2
    fi
}

# Check required commands
check_requirements() {
    local missing=()

    for cmd in tmux jq; do
        if ! command -v "$cmd" &>/dev/null; then
            missing+=("$cmd")
        fi
    done

    if [[ ${#missing[@]} -gt 0 ]]; then
        die "Missing required commands: ${missing[*]}"
    fi
}

# Check if fzf is available (optional)
has_fzf() {
    command -v fzf &>/dev/null
}

# Get config value with default
config_get() {
    local key="$1"
    local default="${2:-}"

    if [[ -f "$CONFIG_FILE" ]]; then
        # Simple YAML parsing for key: value format
        local value
        value=$(grep "^${key}:" "$CONFIG_FILE" 2>/dev/null | cut -d':' -f2- | sed 's/^ *//')
        if [[ -n "$value" ]]; then
            echo "$value"
            return
        fi
    fi
    echo "$default"
}

# Generate ISO timestamp
iso_timestamp() {
    date -u +"%Y-%m-%dT%H:%M:%SZ"
}

# Parse ISO timestamp to epoch seconds
parse_timestamp() {
    local ts="$1"
    if [[ "$OSTYPE" == "darwin"* ]]; then
        date -j -f "%Y-%m-%dT%H:%M:%SZ" "$ts" +%s 2>/dev/null || echo 0
    else
        date -d "$ts" +%s 2>/dev/null || echo 0
    fi
}

# Calculate uptime from ISO timestamp
uptime_from_timestamp() {
    local ts="$1"
    local created
    created=$(parse_timestamp "$ts")
    local now
    now=$(date +%s)
    echo $((now - created))
}

# Initialize colors on source
setup_colors
