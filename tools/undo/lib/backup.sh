#!/usr/bin/env bash
#===============================================================================
# backup.sh - Backup and restore operations for undo
#===============================================================================

# Maximum size for inline storage (100KB)
MAX_INLINE_SIZE=102400

#-------------------------------------------------------------------------------
# Backup Operations
#-------------------------------------------------------------------------------

backup_file() {
    local file_path="$1"
    local project_path="${2:-$(pwd)}"

    if [[ ! -f "$file_path" ]]; then
        echo ""
        return
    fi

    # Calculate hash
    local full_hash
    full_hash=$(sha256sum "$file_path" | cut -d' ' -f1)
    local hash="${full_hash:0:16}"

    # Check if already backed up
    local existing
    existing=$(db_query "SELECT hash FROM file_backups WHERE hash = '$hash'")
    if [[ -n "$existing" ]]; then
        debug "Backup already exists: $hash"
        echo "$hash"
        return
    fi

    local size
    size=$(file_size "$file_path")

    if [[ $size -lt $MAX_INLINE_SIZE ]]; then
        # Small file: store inline (compressed)
        local content
        content=$(gzip -c "$file_path" | base64 | tr -d '\n')

        db_exec "INSERT INTO file_backups (hash, content, compressed, storage_type, size, created)
                 VALUES ('$hash', '$content', 1, 'inline', $size, $(now_timestamp))"
        debug "Backed up inline: $file_path ($size bytes)"
    else
        # Large file: store as separate file
        mkdir -p "$UNDO_BACKUP_DIR"
        local backup_path="${UNDO_BACKUP_DIR}/${hash}.gz"
        gzip -c "$file_path" > "$backup_path"

        db_exec "INSERT INTO file_backups (hash, storage_type, storage_ref, size, created)
                 VALUES ('$hash', 'file', '$backup_path', $size, $(now_timestamp))"
        debug "Backed up to file: $file_path ($size bytes)"
    fi

    echo "$hash"
}

#-------------------------------------------------------------------------------
# Restore Operations
#-------------------------------------------------------------------------------

restore_file() {
    local hash="$1"
    local target_path="$2"

    if [[ -z "$hash" ]]; then
        warn "No hash provided for restore"
        return 1
    fi

    local row
    row=$(db_query "SELECT content, storage_type, storage_ref FROM file_backups WHERE hash = '$hash'")

    if [[ -z "$row" ]]; then
        warn "Backup not found: $hash"
        return 1
    fi

    local content storage_type storage_ref
    IFS=$'\t' read -r content storage_type storage_ref <<< "$row"

    # Create temp file for atomic write
    local temp_file
    temp_file=$(mktemp)

    # Clean up on exit
    local cleanup_needed=true
    cleanup_temp() {
        if [[ "$cleanup_needed" == "true" ]] && [[ -f "$temp_file" ]]; then
            rm -f "$temp_file"
        fi
    }
    trap cleanup_temp EXIT

    case "$storage_type" in
        inline)
            echo "$content" | base64 -d | gunzip > "$temp_file"
            ;;
        file)
            if [[ ! -f "$storage_ref" ]]; then
                warn "Backup file missing: $storage_ref"
                return 1
            fi
            gunzip -c "$storage_ref" > "$temp_file"
            ;;
        git)
            if ! git show "$storage_ref" > "$temp_file" 2>/dev/null; then
                warn "Git ref not found: $storage_ref"
                return 1
            fi
            ;;
        *)
            die "Unknown storage type: $storage_type"
            ;;
    esac

    # Ensure target directory exists
    mkdir -p "$(dirname "$target_path")"

    # Preserve permissions if file exists
    if [[ -f "$target_path" ]]; then
        chmod --reference="$target_path" "$temp_file" 2>/dev/null || true
    fi

    # Atomic move
    mv "$temp_file" "$target_path"
    cleanup_needed=false

    debug "Restored: $target_path from $hash"
    return 0
}

#-------------------------------------------------------------------------------
# Delete Backup for File
#-------------------------------------------------------------------------------

delete_file_if_restored() {
    local file_path="$1"

    # For 'create' entries being undone, delete the file
    if [[ -f "$file_path" ]]; then
        rm -f "$file_path"
        debug "Deleted: $file_path"
    fi
}

#-------------------------------------------------------------------------------
# Verify Backup Integrity
#-------------------------------------------------------------------------------

verify_backup() {
    local hash="$1"

    local exists
    exists=$(db_query "SELECT 1 FROM file_backups WHERE hash = '$hash'")

    if [[ -z "$exists" ]]; then
        return 1
    fi

    local storage_type storage_ref
    read -r storage_type storage_ref <<< "$(db_query "SELECT storage_type, storage_ref FROM file_backups WHERE hash = '$hash'")"

    case "$storage_type" in
        inline)
            return 0  # Always valid if in database
            ;;
        file)
            [[ -f "$storage_ref" ]]
            ;;
        git)
            git cat-file -e "$storage_ref" 2>/dev/null
            ;;
    esac
}

#-------------------------------------------------------------------------------
# Get Backup Size
#-------------------------------------------------------------------------------

get_total_backup_size() {
    db_query "SELECT COALESCE(SUM(size), 0) FROM file_backups"
}
