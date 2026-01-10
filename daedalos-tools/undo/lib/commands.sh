#!/usr/bin/env bash
#===============================================================================
# commands.sh - Command implementations for undo
#===============================================================================

#-------------------------------------------------------------------------------
# Last Command - Undo last N changes
#-------------------------------------------------------------------------------

cmd_last() {
    local count=1
    local dry_run=false
    local file_filter=""

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --dry-run)  dry_run=true; shift ;;
            --file)     file_filter="$2"; shift 2 ;;
            [0-9]*)     count="$1"; shift ;;
            *)          shift ;;
        esac
    done

    # Get last N entries
    local entries
    if [[ -n "$file_filter" ]]; then
        entries=$(db_query "SELECT id, file_path, before_hash, backup_ref, type
                           FROM entries
                           WHERE project_path = '$(pwd)'
                             AND file_path LIKE '%$file_filter%'
                             AND type IN ('edit', 'create', 'delete')
                           ORDER BY timestamp DESC
                           LIMIT $count")
    else
        entries=$(get_last_entries "$count")
    fi

    if [[ -z "$entries" ]]; then
        info "No entries to undo"
        exit 0
    fi

    if $dry_run; then
        echo "Would undo:"
        while IFS=$'\t' read -r id file_path before_hash backup_ref entry_type; do
            [[ -z "$id" ]] && continue
            echo "  #$id: [$entry_type] $file_path"
        done <<< "$entries"
        exit 0
    fi

    # Create checkpoint before undo
    local checkpoint_name
    checkpoint_name=$(add_checkpoint "pre-undo-$(date +%H%M%S)" "auto" "Before undo last $count")
    info "Created checkpoint: $checkpoint_name"

    # Restore each file
    local restored=0
    while IFS=$'\t' read -r id file_path before_hash backup_ref type; do
        [[ -z "$id" ]] && continue

        case "$type" in
            edit)
                if [[ -n "$before_hash" ]]; then
                    if restore_file "$before_hash" "$file_path"; then
                        success "Restored: $file_path"
                        ((restored++))
                    fi
                fi
                ;;
            create)
                # Undo create = delete the file
                if [[ -f "$file_path" ]]; then
                    rm -f "$file_path"
                    success "Deleted: $file_path (was newly created)"
                    ((restored++))
                fi
                ;;
            delete)
                # Undo delete = restore the file
                if [[ -n "$before_hash" ]]; then
                    if restore_file "$before_hash" "$file_path"; then
                        success "Restored: $file_path (was deleted)"
                        ((restored++))
                    fi
                fi
                ;;
        esac
    done <<< "$entries"

    # Add restore entry to timeline
    add_entry "restore" "" "Undo last $count changes ($restored files)"

    echo ""
    echo "Restored $restored file(s)"
}

#-------------------------------------------------------------------------------
# To Command - Restore to specific point
#-------------------------------------------------------------------------------

cmd_to() {
    local reference="${1:-}"
    local dry_run=false
    local file_filter=""

    if [[ -z "$reference" ]]; then
        die "Usage: undo to <reference>"
    fi

    shift
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --dry-run)  dry_run=true; shift ;;
            --file)     file_filter="$2"; shift 2 ;;
            *)          shift ;;
        esac
    done

    local target_entry_id

    # Parse reference
    if [[ "$reference" =~ ^#([0-9]+)$ ]]; then
        # Entry ID
        target_entry_id="${BASH_REMATCH[1]}"
    elif [[ "$reference" =~ ^[0-9]{1,2}:[0-9]{2}(:[0-9]{2})?$ ]]; then
        # Time reference (today)
        local target_ts
        target_ts=$(parse_time_reference "$reference")
        target_entry_id=$(db_query "SELECT id FROM entries
                                   WHERE project_path = '$(pwd)' AND timestamp <= $target_ts
                                   ORDER BY timestamp DESC LIMIT 1")
    else
        # Named checkpoint
        target_entry_id=$(get_checkpoint_entry_id "$reference")
    fi

    if [[ -z "$target_entry_id" ]]; then
        die "Could not find reference: $reference"
    fi

    # Get entries to undo (everything after target)
    local entries
    entries=$(get_entries_since "$target_entry_id")

    if [[ -z "$entries" ]]; then
        info "Already at that point (no changes since #$target_entry_id)"
        exit 0
    fi

    if $dry_run; then
        echo "Would restore to #$target_entry_id:"
        while IFS=$'\t' read -r id file_path before_hash backup_ref type; do
            [[ -z "$file_path" ]] && continue
            echo "  #$id: [$type] $file_path"
        done <<< "$entries"
        exit 0
    fi

    # Create checkpoint before restore
    local checkpoint_name
    checkpoint_name=$(add_checkpoint "pre-restore-$(date +%H%M%S)" "auto" "Before restore to #$target_entry_id")
    info "Created checkpoint: $checkpoint_name"

    # Restore files
    local restored=0
    while IFS=$'\t' read -r id file_path before_hash backup_ref type; do
        [[ -z "$file_path" ]] && continue

        case "$type" in
            edit|delete)
                if [[ -n "$before_hash" ]]; then
                    if restore_file "$before_hash" "$file_path"; then
                        success "Restored: $file_path"
                        ((restored++))
                    fi
                fi
                ;;
            create)
                if [[ -f "$file_path" ]]; then
                    rm -f "$file_path"
                    success "Deleted: $file_path (was created after target)"
                    ((restored++))
                fi
                ;;
        esac
    done <<< "$entries"

    add_entry "restore" "" "Restored to #$target_entry_id ($restored files)"

    echo ""
    echo "Restored $restored file(s) to state at #$target_entry_id"
}

#-------------------------------------------------------------------------------
# Preview Command
#-------------------------------------------------------------------------------

cmd_preview() {
    local subcmd="${1:-last}"
    shift || true

    case "$subcmd" in
        last)
            cmd_last --dry-run "$@"
            ;;
        to)
            cmd_to "${1:-}" --dry-run
            ;;
        *)
            die "Unknown preview command: $subcmd"
            ;;
    esac
}

#-------------------------------------------------------------------------------
# Diff Command
#-------------------------------------------------------------------------------

cmd_diff() {
    local reference="${1:-}"
    local file_filter=""
    local stat_only=false

    if [[ -z "$reference" ]]; then
        die "Usage: undo diff <reference>"
    fi

    shift
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --file)     file_filter="$2"; shift 2 ;;
            --stat)     stat_only=true; shift ;;
            *)          shift ;;
        esac
    done

    # Parse reference to get target entry
    local target_ts
    target_ts=$(parse_time_reference "$reference")

    if [[ "$target_ts" == "0" ]]; then
        die "Could not parse reference: $reference"
    fi

    # Get entries since that point
    local target_entry_id
    target_entry_id=$(db_query "SELECT id FROM entries
                               WHERE project_path = '$(pwd)' AND timestamp <= $target_ts
                               ORDER BY timestamp DESC LIMIT 1")

    local entries
    entries=$(get_entries_since "$target_entry_id")

    if [[ -z "$entries" ]]; then
        echo "No changes since $reference"
        exit 0
    fi

    echo "Changes since $reference:"
    echo ""

    while IFS=$'\t' read -r id file_path before_hash backup_ref type; do
        [[ -z "$file_path" ]] && continue
        [[ -n "$file_filter" ]] && [[ "$file_path" != *"$file_filter"* ]] && continue

        echo "--- $file_path [$type]"

        if ! $stat_only && [[ -n "$before_hash" ]] && [[ -f "$file_path" ]]; then
            # Create temp file with old content
            local temp_old
            temp_old=$(mktemp)
            if restore_file "$before_hash" "$temp_old" 2>/dev/null; then
                diff -u "$temp_old" "$file_path" 2>/dev/null || true
            fi
            rm -f "$temp_old"
        fi
        echo ""
    done <<< "$entries"
}

#-------------------------------------------------------------------------------
# Checkpoint Command
#-------------------------------------------------------------------------------

cmd_checkpoint() {
    local name="${1:-checkpoint-$(date +%Y%m%d-%H%M%S)}"

    local final_name
    final_name=$(add_checkpoint "$name" "manual" "User checkpoint")

    success "Checkpoint created: $final_name"
}

#-------------------------------------------------------------------------------
# Record Command - Manually record a file change
#-------------------------------------------------------------------------------

cmd_record() {
    local file_path="${1:-}"
    local description="${2:-manual edit}"

    if [[ -z "$file_path" ]]; then
        die "Usage: undo record <file> [description]"
    fi

    if [[ ! -f "$file_path" ]]; then
        die "File not found: $file_path"
    fi

    # Backup current state
    local hash
    hash=$(backup_file "$file_path")

    # Add entry
    add_entry "edit" "$file_path" "$description" "" "$hash" "$hash"

    success "Recorded: $file_path"
}

#-------------------------------------------------------------------------------
# Cleanup Command
#-------------------------------------------------------------------------------

cmd_cleanup() {
    local hours="${1:-24}"

    info "Cleaning up entries older than $hours hours..."
    cleanup_old_entries "$hours"

    info "Removing orphan backups..."
    cleanup_orphan_backups

    success "Cleanup complete"
    echo ""
    get_database_stats
}
