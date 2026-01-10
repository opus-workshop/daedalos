#!/usr/bin/env bash
# status.sh - Status detection and display
#
# Detects agent status from tmux pane content and formats status output.

# Prevent double-sourcing
[[ -n "${_AGENT_STATUS_LOADED:-}" ]] && return 0
_AGENT_STATUS_LOADED=1

# Status constants
STATUS_ACTIVE="active"
STATUS_THINKING="thinking"
STATUS_WAITING="waiting"
STATUS_IDLE="idle"
STATUS_PAUSED="paused"
STATUS_ERROR="error"
STATUS_DEAD="dead"

# Strip ANSI escape codes from text
strip_ansi() {
    # Remove ANSI escape sequences for proper pattern matching
    sed 's/\x1b\[[0-9;]*m//g; s/\x1b\[[0-9;]*[A-Za-z]//g'
}

# Detect status from pane content
# Claude Code has specific output patterns we can detect
detect_status_from_content() {
    local content="$1"

    # Strip ANSI codes for clean matching
    local clean_content
    clean_content=$(echo "$content" | strip_ansi)

    # Check for common status indicators (last 50 lines for context)
    local recent
    recent=$(echo "$clean_content" | tail -50)

    # Check the very last few lines for prompt state (most reliable)
    local last_lines
    last_lines=$(echo "$clean_content" | tail -5)

    # Claude Code specific patterns

    # Pattern: Spinner or "Thinking" indicator
    if echo "$recent" | grep -qE '(⠋|⠙|⠹|⠸|⠼|⠴|⠦|⠧|⠇|⠏)'; then
        echo "$STATUS_THINKING"
        return
    fi

    # Pattern: Tool call in progress (Claude Code shows tool names in brackets)
    if echo "$recent" | grep -qE '\[(Read|Write|Edit|Bash|Glob|Grep|Task|WebFetch|WebSearch|LSP)\]'; then
        # Check if it's the most recent activity
        if echo "$last_lines" | grep -qE '\[(Read|Write|Edit|Bash|Glob|Grep|Task|WebFetch|WebSearch|LSP)\]'; then
            echo "$STATUS_ACTIVE"
            return
        fi
    fi

    # Pattern: Cost display (appears after each response) - usually means waiting for input
    if echo "$last_lines" | grep -qE '(Cost:|Tokens:|tokens|input/output)'; then
        echo "$STATUS_IDLE"
        return
    fi

    # Pattern: Question or awaiting response
    if echo "$last_lines" | grep -qE '(\? $|\?$|y/n|Y/N|\[y/N\]|\[Y/n\])'; then
        echo "$STATUS_WAITING"
        return
    fi

    # Pattern: Claude Code prompt (various forms)
    if echo "$last_lines" | grep -qE '(^> $|^❯ $|^claude> |^claude-code> |User:)'; then
        echo "$STATUS_IDLE"
        return
    fi

    # Pattern: Error messages
    if echo "$recent" | grep -qiE '(^error:|^Error:|failed:|FAILED|exception:|Exception:|panic:|Panic:)'; then
        echo "$STATUS_ERROR"
        return
    fi

    # Pattern: Permission denied or blocked
    if echo "$recent" | grep -qiE '(permission denied|access denied|blocked|rejected)'; then
        echo "$STATUS_ERROR"
        return
    fi

    # Pattern: Completion signal was sent (by the agent itself)
    if echo "$recent" | grep -qE 'agent signal complete|Signaled completion'; then
        echo "$STATUS_IDLE"
        return
    fi

    # Pattern: Active processing indicators
    if echo "$recent" | grep -qiE '(processing|analyzing|reading|writing|searching|generating|creating|updating)'; then
        echo "$STATUS_THINKING"
        return
    fi

    # Pattern: Test running
    if echo "$recent" | grep -qE '(PASS|FAIL|running test|test.*\.\.\.|\d+ passed|\d+ failed)'; then
        echo "$STATUS_ACTIVE"
        return
    fi

    # Pattern: Compiling/Building
    if echo "$recent" | grep -qiE '(compiling|building|bundling|linking|compiled|built)'; then
        echo "$STATUS_ACTIVE"
        return
    fi

    # Check if there's very recent output (within last 5 lines)
    local line_count
    line_count=$(echo "$last_lines" | wc -l)

    # If we have output and no clear status, assume active
    if [[ $line_count -gt 2 ]] && [[ -n "$(echo "$last_lines" | tr -d '[:space:]')" ]]; then
        echo "$STATUS_ACTIVE"
    else
        echo "$STATUS_IDLE"
    fi
}

# Get full status for an agent
get_agent_status() {
    local name="$1"

    # Get agent data
    local agent
    agent=$(agents_get "$name")
    if [[ -z "$agent" ]]; then
        echo "$STATUS_DEAD"
        return
    fi

    local session
    session=$(echo "$agent" | jq -r '.tmux_session')

    # Check if tmux session exists
    if ! tmux_session_exists "$session"; then
        echo "$STATUS_DEAD"
        return
    fi

    # Check if process is stopped (paused)
    local pid
    pid=$(tmux_get_pane_pid "$session")
    if [[ -n "$pid" ]] && process_is_stopped "$pid"; then
        echo "$STATUS_PAUSED"
        return
    fi

    # Get pane content and analyze
    local content
    content=$(tmux_get_pane_content "$session" 50)
    detect_status_from_content "$content"
}

# Update agent status in database
update_agent_status() {
    local name="$1"
    local status
    status=$(get_agent_status "$name")
    agents_update "$name" "status" "$status"

    if [[ "$status" != "$STATUS_DEAD" ]]; then
        agents_touch "$name"
    fi

    echo "$status"
}

# Format single agent status for display
format_agent_status() {
    local name="$1"
    local as_json="${2:-false}"

    local agent
    agent=$(agents_get "$name")
    if [[ -z "$agent" ]]; then
        die "Agent not found: $name"
    fi

    local slot project template status created
    slot=$(echo "$agent" | jq -r '.slot')
    project=$(echo "$agent" | jq -r '.project')
    template=$(echo "$agent" | jq -r '.template // "default"')
    created=$(echo "$agent" | jq -r '.created')

    # Get live status
    status=$(get_agent_status "$name")

    # Calculate uptime
    local uptime_secs uptime_str
    uptime_secs=$(uptime_from_timestamp "$created")
    uptime_str=$(format_duration "$uptime_secs")

    if [[ "$as_json" == "true" ]]; then
        jq -n \
            --arg name "$name" \
            --argjson slot "$slot" \
            --arg project "$project" \
            --arg template "$template" \
            --arg status "$status" \
            --arg uptime "$uptime_str" \
            --argjson uptime_secs "$uptime_secs" \
            --arg created "$created" \
            '{
                name: $name,
                slot: $slot,
                project: $project,
                template: $template,
                status: $status,
                uptime: $uptime,
                uptime_seconds: $uptime_secs,
                created: $created
            }'
    else
        local color
        color=$(color_status "$status")

        echo "${C_BOLD}Agent:${C_RESET}    ${C_CYAN}${name}${C_RESET} (slot ${slot})"
        echo "${C_BOLD}Project:${C_RESET}  ${project}"
        echo "${C_BOLD}Template:${C_RESET} ${template}"
        echo "${C_BOLD}Status:${C_RESET}   ${color}${status}${C_RESET}"
        echo "${C_BOLD}Uptime:${C_RESET}   ${uptime_str}"
        echo "${C_BOLD}Created:${C_RESET}  ${created}"
    fi
}

# Format agent list for display
format_agent_list() {
    local as_json="${1:-false}"
    local quiet="${2:-false}"

    agents_init

    if [[ "$as_json" == "true" ]]; then
        local result="["
        local first=true
        while IFS= read -r agent_json; do
            [[ -z "$agent_json" ]] && continue

            local name
            name=$(echo "$agent_json" | jq -r '.name')
            local status
            status=$(get_agent_status "$name")

            local slot project template created
            slot=$(echo "$agent_json" | jq -r '.slot')
            project=$(echo "$agent_json" | jq -r '.project')
            template=$(echo "$agent_json" | jq -r '.template // "default"')
            created=$(echo "$agent_json" | jq -r '.created')

            local uptime_secs uptime_str
            uptime_secs=$(uptime_from_timestamp "$created")
            uptime_str=$(format_duration "$uptime_secs")

            local entry
            entry=$(jq -n \
                --arg name "$name" \
                --argjson slot "$slot" \
                --arg project "$project" \
                --arg template "$template" \
                --arg status "$status" \
                --arg uptime "$uptime_str" \
                '{name: $name, slot: $slot, project: $project, template: $template, status: $status, uptime: $uptime}')

            if [[ "$first" == "true" ]]; then
                result+="$entry"
                first=false
            else
                result+=",$entry"
            fi
        done < <(agents_all)
        result+="]"
        echo "$result" | jq '.'
        return
    fi

    if [[ "$quiet" == "true" ]]; then
        agents_names
        return
    fi

    # Build table rows
    local -a rows
    rows+=("SLOT|NAME|PROJECT|STATUS|UPTIME")

    while IFS= read -r agent_json; do
        [[ -z "$agent_json" ]] && continue

        local name slot project status created
        name=$(echo "$agent_json" | jq -r '.name')
        slot=$(echo "$agent_json" | jq -r '.slot')
        project=$(echo "$agent_json" | jq -r '.project' | xargs basename)
        created=$(echo "$agent_json" | jq -r '.created')

        status=$(get_agent_status "$name")

        local uptime_secs uptime_str
        uptime_secs=$(uptime_from_timestamp "$created")
        uptime_str=$(format_duration "$uptime_secs")

        local color
        color=$(color_status "$status")

        rows+=("${slot}|${name}|${project}|${color}${status}${C_RESET}|${uptime_str}")
    done < <(agents_all)

    if [[ ${#rows[@]} -eq 1 ]]; then
        echo "No agents running."
        return
    fi

    format_table "${rows[@]}"
}

# Watch mode - continuously update status
watch_status() {
    local name="${1:-}"
    local interval="${2:-2}"

    while true; do
        clear
        echo "${C_BOLD}Agent Status${C_RESET} (updating every ${interval}s, Ctrl+C to exit)"
        echo ""

        if [[ -n "$name" ]]; then
            format_agent_status "$name"
        else
            format_agent_list
        fi

        sleep "$interval"
    done
}

# Get resource usage for agent
get_agent_resources() {
    local name="$1"

    local agent
    agent=$(agents_get "$name")
    if [[ -z "$agent" ]]; then
        return 1
    fi

    local session
    session=$(echo "$agent" | jq -r '.tmux_session')

    local pid
    pid=$(tmux_get_pane_pid "$session")

    if [[ -z "$pid" ]] || [[ "$pid" == "0" ]]; then
        echo '{"cpu": 0, "memory": 0}'
        return
    fi

    # Get CPU and memory usage
    local cpu mem
    if [[ "$OSTYPE" == "darwin"* ]]; then
        read -r cpu mem < <(ps -o %cpu=,%mem= -p "$pid" 2>/dev/null || echo "0 0")
    else
        read -r cpu mem < <(ps -o %cpu=,%mem= -p "$pid" 2>/dev/null || echo "0 0")
    fi

    jq -n --arg cpu "${cpu:-0}" --arg mem "${mem:-0}" '{cpu: ($cpu | tonumber), memory: ($mem | tonumber)}'
}
