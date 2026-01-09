#!/usr/bin/env bash
#===============================================================================
# database.sh - SQLite operations for undo
#===============================================================================

DB_FILE="${UNDO_DATA_DIR}/timeline.db"

#-------------------------------------------------------------------------------
# Database Initialization
#-------------------------------------------------------------------------------

init_database() {
    if [[ ! -f "$DB_FILE" ]]; then
        local schema_file
        schema_file="$(dirname "${BASH_SOURCE[0]}")/../schema/init.sql"

        if [[ -f "$schema_file" ]]; then
            sqlite3 "$DB_FILE" < "$schema_file"
        else
            # Inline schema as fallback
            sqlite3 "$DB_FILE" << 'SQL'
CREATE TABLE IF NOT EXISTS entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp REAL NOT NULL,
    type TEXT NOT NULL,
    file_path TEXT,
    description TEXT,
    before_hash TEXT,
    after_hash TEXT,
    backup_ref TEXT,
    project_path TEXT NOT NULL,
    metadata TEXT
);
CREATE TABLE IF NOT EXISTS checkpoints (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    timestamp REAL NOT NULL,
    entry_id INTEGER REFERENCES entries(id),
    type TEXT,
    description TEXT
);
CREATE TABLE IF NOT EXISTS file_backups (
    hash TEXT PRIMARY KEY,
    content BLOB,
    compressed INTEGER DEFAULT 1,
    storage_type TEXT,
    storage_ref TEXT,
    size INTEGER,
    created REAL
);
CREATE INDEX IF NOT EXISTS idx_entries_timestamp ON entries(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_entries_file ON entries(file_path);
CREATE INDEX IF NOT EXISTS idx_entries_project ON entries(project_path);
SQL
        fi
        debug "Database initialized: $DB_FILE"
    fi
}

#-------------------------------------------------------------------------------
# Database Operations
#-------------------------------------------------------------------------------

db_query() {
    sqlite3 -separator $'\t' "$DB_FILE" "$@"
}

db_exec() {
    sqlite3 "$DB_FILE" "$@"
}

db_json() {
    sqlite3 -json "$DB_FILE" "$@" 2>/dev/null || \
        sqlite3 "$DB_FILE" "$@"  # Fallback for older sqlite3
}

#-------------------------------------------------------------------------------
# Entry Operations
#-------------------------------------------------------------------------------

add_entry() {
    local type="$1"
    local file_path="$2"
    local description="$3"
    local before_hash="${4:-}"
    local after_hash="${5:-}"
    local backup_ref="${6:-}"
    local project_path="${7:-$(pwd)}"

    local timestamp
    timestamp=$(now_timestamp)

    # Escape single quotes in description
    description="${description//\'/\'\'}"
    file_path="${file_path//\'/\'\'}"

    db_exec "INSERT INTO entries (timestamp, type, file_path, description, before_hash, after_hash, backup_ref, project_path)
             VALUES ($timestamp, '$type', '$file_path', '$description', '$before_hash', '$after_hash', '$backup_ref', '$project_path')"

    debug "Added entry: $type $file_path"
}

get_entries() {
    local limit="${1:-20}"
    local project="${2:-$(pwd)}"

    db_query "SELECT id, datetime(timestamp, 'unixepoch', 'localtime'), type, file_path, description
              FROM entries
              WHERE project_path = '$project'
              ORDER BY timestamp DESC
              LIMIT $limit"
}

get_entry_by_id() {
    local id="$1"
    db_query "SELECT id, timestamp, type, file_path, description, before_hash, after_hash, backup_ref, project_path
              FROM entries WHERE id = $id"
}

get_entries_since() {
    local entry_id="$1"
    local project="${2:-$(pwd)}"

    db_query "SELECT id, file_path, before_hash, backup_ref, type
              FROM entries
              WHERE project_path = '$project'
                AND id > $entry_id
                AND type IN ('edit', 'create', 'delete')
              ORDER BY id DESC"
}

get_last_entries() {
    local count="${1:-1}"
    local project="${2:-$(pwd)}"

    db_query "SELECT id, file_path, before_hash, backup_ref, type
              FROM entries
              WHERE project_path = '$project'
                AND type IN ('edit', 'create', 'delete')
              ORDER BY timestamp DESC
              LIMIT $count"
}

get_entries_for_file() {
    local file_path="$1"
    local limit="${2:-20}"
    local project="${3:-$(pwd)}"

    db_query "SELECT id, datetime(timestamp, 'unixepoch', 'localtime'), type, file_path, description
              FROM entries
              WHERE project_path = '$project' AND file_path = '$file_path'
              ORDER BY timestamp DESC
              LIMIT $limit"
}

#-------------------------------------------------------------------------------
# Checkpoint Operations
#-------------------------------------------------------------------------------

add_checkpoint() {
    local name="$1"
    local type="${2:-manual}"
    local description="${3:-User checkpoint}"

    local timestamp
    timestamp=$(now_timestamp)

    # Add checkpoint entry
    add_entry "checkpoint" "" "$name"

    local entry_id
    entry_id=$(db_query "SELECT last_insert_rowid()")

    # Ensure unique name by appending timestamp if needed
    local final_name="$name"
    if db_query "SELECT 1 FROM checkpoints WHERE name = '$name'" | grep -q 1; then
        final_name="${name}-$(date +%H%M%S)"
    fi

    db_exec "INSERT INTO checkpoints (name, timestamp, entry_id, type, description)
             VALUES ('$final_name', $timestamp, $entry_id, '$type', '$description')"

    echo "$final_name"
}

get_checkpoint_entry_id() {
    local name="$1"
    db_query "SELECT entry_id FROM checkpoints WHERE name = '$name'"
}

list_checkpoints() {
    local limit="${1:-10}"
    local project="${2:-$(pwd)}"

    db_query "SELECT c.name, datetime(c.timestamp, 'unixepoch', 'localtime'), c.type, c.description
              FROM checkpoints c
              JOIN entries e ON c.entry_id = e.id
              WHERE e.project_path = '$project'
              ORDER BY c.timestamp DESC
              LIMIT $limit"
}

#-------------------------------------------------------------------------------
# Cleanup Operations
#-------------------------------------------------------------------------------

cleanup_old_entries() {
    local hours="${1:-24}"
    local project="${2:-$(pwd)}"

    local cutoff
    cutoff=$(python3 -c "import time; print(time.time() - $hours * 3600)")

    # Delete old entries (but not checkpoints)
    local deleted
    deleted=$(db_exec "DELETE FROM entries
                       WHERE project_path = '$project'
                         AND timestamp < $cutoff
                         AND type != 'checkpoint';
                       SELECT changes();")

    debug "Cleaned up $deleted old entries"
}

cleanup_orphan_backups() {
    # Delete backups not referenced by any entry
    db_exec "DELETE FROM file_backups
             WHERE hash NOT IN (
                 SELECT DISTINCT before_hash FROM entries WHERE before_hash IS NOT NULL
                 UNION
                 SELECT DISTINCT after_hash FROM entries WHERE after_hash IS NOT NULL
             )"
}

get_database_stats() {
    local project="${1:-$(pwd)}"

    local entry_count
    entry_count=$(db_query "SELECT COUNT(*) FROM entries WHERE project_path = '$project'")

    local backup_count
    backup_count=$(db_query "SELECT COUNT(*) FROM file_backups")

    local backup_size
    backup_size=$(db_query "SELECT COALESCE(SUM(size), 0) FROM file_backups")

    echo "Entries: $entry_count"
    echo "Backups: $backup_count"
    echo "Storage: $(numfmt --to=iec $backup_size 2>/dev/null || echo "${backup_size} bytes")"
}
