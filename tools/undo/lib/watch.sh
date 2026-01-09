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
# Daemon Mode
#-------------------------------------------------------------------------------

start_daemon() {
    local project_path="${1:-.}"
    local pid_file="${UNDO_DATA_DIR}/undod.pid"

    # Check if already running
    if [[ -f "$pid_file" ]]; then
        local old_pid
        old_pid=$(cat "$pid_file")
        if kill -0 "$old_pid" 2>/dev/null; then
            warn "Daemon already running (PID: $old_pid)"
            return 1
        fi
    fi

    # Start in background
    cmd_watch "$project_path" &
    local pid=$!

    echo "$pid" > "$pid_file"
    info "Daemon started (PID: $pid)"
}

stop_daemon() {
    local pid_file="${UNDO_DATA_DIR}/undod.pid"

    if [[ ! -f "$pid_file" ]]; then
        info "Daemon not running"
        return 0
    fi

    local pid
    pid=$(cat "$pid_file")

    if kill -0 "$pid" 2>/dev/null; then
        kill "$pid"
        rm -f "$pid_file"
        success "Daemon stopped"
    else
        rm -f "$pid_file"
        info "Daemon was not running (stale pid file removed)"
    fi
}

daemon_status() {
    local pid_file="${UNDO_DATA_DIR}/undod.pid"

    if [[ ! -f "$pid_file" ]]; then
        echo "Daemon: not running"
        return 1
    fi

    local pid
    pid=$(cat "$pid_file")

    if kill -0 "$pid" 2>/dev/null; then
        echo "Daemon: running (PID: $pid)"
        return 0
    else
        echo "Daemon: not running (stale pid file)"
        return 1
    fi
}
