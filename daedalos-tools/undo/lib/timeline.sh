#!/usr/bin/env bash
#===============================================================================
# timeline.sh - Timeline display for undo
#===============================================================================

#-------------------------------------------------------------------------------
# Timeline Command
#-------------------------------------------------------------------------------

cmd_timeline() {
    local limit=20
    local file_filter=""
    local json=false
    local since=""

    while [[ $# -gt 0 ]]; do
        case "$1" in
            -n)         limit="$2"; shift 2 ;;
            --file)     file_filter="$2"; shift 2 ;;
            --since)    since="$2"; shift 2 ;;
            --json)     json=true; shift ;;
            *)          shift ;;
        esac
    done

    local entries

    if [[ -n "$file_filter" ]]; then
        entries=$(get_entries_for_file "$file_filter" "$limit")
    elif [[ -n "$since" ]]; then
        # Parse since time
        local since_ts
        since_ts=$(parse_time_reference "$since")
        entries=$(db_query "SELECT id, datetime(timestamp, 'unixepoch', 'localtime'), type, file_path, description
                           FROM entries
                           WHERE project_path = '$(pwd)' AND timestamp >= $since_ts
                           ORDER BY timestamp DESC
                           LIMIT $limit")
    else
        entries=$(get_entries "$limit")
    fi

    if $json; then
        format_timeline_json "$entries"
    else
        format_timeline_table "$entries"
    fi
}

#-------------------------------------------------------------------------------
# Timeline Formatting
#-------------------------------------------------------------------------------

format_timeline_table() {
    local entries="$1"

    echo "+------+----------+--------+----------------------+----------------------+"
    echo "| ID   | Time     | Type   | File                 | Description          |"
    echo "+------+----------+--------+----------------------+----------------------+"

    if [[ -z "$entries" ]]; then
        echo "| No entries found                                                        |"
    else
        while IFS=$'\t' read -r id timestamp type file_path description; do
            [[ -z "$id" ]] && continue

            # Extract just the time part
            local time_part="${timestamp##* }"

            # Truncate long strings
            local short_file="${file_path:0:20}"
            local short_desc="${description:0:20}"

            # Type-specific formatting
            if [[ "$type" == "checkpoint" ]]; then
                printf "| ${CYAN}%-4s${RESET} | ${CYAN}%8s${RESET} | ${CYAN}%-6s${RESET} | ${CYAN}%-20s${RESET} | ${CYAN}%-20s${RESET} |\n" \
                    "#$id" "$time_part" "$type" "---" "$short_desc"
            else
                local type_color=""
                case "$type" in
                    edit)    type_color="$YELLOW" ;;
                    create)  type_color="$GREEN" ;;
                    delete)  type_color="$RED" ;;
                    restore) type_color="$BLUE" ;;
                esac
                printf "| %-4s | %8s | ${type_color}%-6s${RESET} | %-20s | %-20s |\n" \
                    "#$id" "$time_part" "$type" "$short_file" "$short_desc"
            fi
        done <<< "$entries"
    fi

    echo "+------+----------+--------+----------------------+----------------------+"
}

format_timeline_json() {
    local entries="$1"

    echo "["
    local first=true
    while IFS=$'\t' read -r id timestamp type file_path description; do
        [[ -z "$id" ]] && continue

        if ! $first; then
            echo ","
        fi
        first=false

        cat << EOF
  {
    "id": $id,
    "timestamp": "$timestamp",
    "type": "$type",
    "file_path": "$file_path",
    "description": "$description"
  }
EOF
    done <<< "$entries"
    echo "]"
}

#-------------------------------------------------------------------------------
# Time Parsing
#-------------------------------------------------------------------------------

parse_time_reference() {
    local ref="$1"

    # Entry ID: #5
    if [[ "$ref" =~ ^#([0-9]+)$ ]]; then
        local entry_id="${BASH_REMATCH[1]}"
        db_query "SELECT timestamp FROM entries WHERE id = $entry_id"
        return
    fi

    # Time today: 12:42:00 or 12:42
    if [[ "$ref" =~ ^[0-9]{1,2}:[0-9]{2}(:[0-9]{2})?$ ]]; then
        # Construct full datetime
        local today
        today=$(date +%Y-%m-%d)
        local datetime="$today $ref"

        # Convert to timestamp
        if [[ "$(uname)" == "Darwin" ]]; then
            date -j -f "%Y-%m-%d %H:%M:%S" "${datetime}:00" +%s 2>/dev/null || \
            date -j -f "%Y-%m-%d %H:%M" "$datetime" +%s 2>/dev/null || \
            echo "0"
        else
            date -d "$datetime" +%s 2>/dev/null || echo "0"
        fi
        return
    fi

    # Named checkpoint
    local entry_id
    entry_id=$(get_checkpoint_entry_id "$ref")
    if [[ -n "$entry_id" ]]; then
        db_query "SELECT timestamp FROM entries WHERE id = $entry_id"
        return
    fi

    echo "0"
}

#-------------------------------------------------------------------------------
# Status Command
#-------------------------------------------------------------------------------

cmd_status() {
    local project
    project=$(pwd)

    echo "Undo Status"
    echo "----------------------------"
    echo "Project: $project"
    echo ""

    get_database_stats "$project"

    echo ""
    echo "Recent checkpoints:"
    list_checkpoints 5 "$project" | while IFS=$'\t' read -r name timestamp type desc; do
        [[ -z "$name" ]] && continue
        echo "  - $name ($timestamp)"
    done
}
