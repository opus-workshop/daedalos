#!/usr/bin/env bash
#===============================================================================
# watch.sh - File watching for undo
#===============================================================================

#-------------------------------------------------------------------------------
# Watch Command
#-------------------------------------------------------------------------------

cmd_watch() {
    local project_path="${1:-.}"

    cd "$project_path" || die "Cannot access: $project_path"
    project_path=$(pwd)

    # Check for fswatch
    if ! command -v fswatch &>/dev/null; then
        if [[ "$(uname)" == "Darwin" ]]; then
            die "Watch mode requires fswatch. Install with: brew install fswatch"
        else
            die "Watch mode requires fswatch. Install with your package manager."
        fi
    fi

    info "Starting undo watch daemon for: $project_path"
    info "Press Ctrl+C to stop"
    echo ""

    # Create session start checkpoint
    add_checkpoint "session-start-$(date +%H%M%S)" "session_start" "Watch session started"

    # Track file states
    declare -A file_hashes

    # Watch for changes
    fswatch -o \
        --event Created --event Updated --event Removed --event Renamed \
        --exclude '\.git' \
        --exclude 'node_modules' \
        --exclude '__pycache__' \
        --exclude '\.pytest_cache' \
        --exclude 'target' \
        --exclude 'build' \
        --exclude 'dist' \
        --exclude '\.DS_Store' \
        --exclude '\.undo' \
        --latency 0.3 \
        "$project_path" | while read -r event_count; do

        process_file_changes "$project_path"
    done
}

#-------------------------------------------------------------------------------
# Process Changes
#-------------------------------------------------------------------------------

process_file_changes() {
    local project_path="$1"

    # Use git to find changed files (more reliable than parsing fswatch)
    if command -v git &>/dev/null && git rev-parse --git-dir &>/dev/null 2>&1; then
        # Get modified files
        local changed_files
        changed_files=$(git status --porcelain 2>/dev/null | grep -E '^.M|^M' | awk '{print $2}')

        for file in $changed_files; do
            record_file_change "$file" "edit" "File modified"
        done

        # Get new files
        local new_files
        new_files=$(git status --porcelain 2>/dev/null | grep -E '^\?\?' | awk '{print $2}')

        for file in $new_files; do
            # Only record actual files, not directories
            if [[ -f "$file" ]]; then
                record_file_change "$file" "create" "File created"
            fi
        done
    fi
}

record_file_change() {
    local file_path="$1"
    local change_type="$2"
    local description="$3"

    # Skip binary files and large files
    local size
    size=$(file_size "$file_path")
    if [[ $size -gt 10485760 ]]; then  # 10MB
        debug "Skipping large file: $file_path"
        return
    fi

    # Backup current state
    local hash
    hash=$(backup_file "$file_path")

    # Add entry
    add_entry "$change_type" "$file_path" "$description" "" "$hash" "$hash"

    debug "Recorded $change_type: $file_path"
}

#-------------------------------------------------------------------------------
# Daemon Mode (delegates to Python daemon)
#-------------------------------------------------------------------------------

PYTHON_DAEMON="${LIB_DIR}/../undod.py"

start_daemon() {
    local project_path="${1:-.}"
    project_path=$(cd "$project_path" && pwd)

    # Start Python daemon in background
    nohup python3 "$PYTHON_DAEMON" start "$project_path" > "${UNDO_DATA_DIR}/undod.log" 2>&1 &
    local pid=$!

    echo "$pid" > "${UNDO_DATA_DIR}/undod.pid"
    info "Daemon started (PID: $pid)"
    info "Web UI: http://localhost:7778"
    info "Log: ${UNDO_DATA_DIR}/undod.log"
}

stop_daemon() {
    python3 "$PYTHON_DAEMON" stop 2>/dev/null || true

    local pid_file="${UNDO_DATA_DIR}/undod.pid"
    if [[ -f "$pid_file" ]]; then
        local pid
        pid=$(cat "$pid_file")
        kill "$pid" 2>/dev/null || true
        rm -f "$pid_file"
    fi
    success "Daemon stopped"
}

daemon_status() {
    python3 "$PYTHON_DAEMON" status
}
