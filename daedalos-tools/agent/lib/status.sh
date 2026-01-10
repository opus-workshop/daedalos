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

# Detect status from pane content
detect_status_from_content() {
    local content="$1"

    # Check for common status indicators (last 30 lines)
    local recent
    recent=$(echo "$content" | tail -30)

    # Check for thinking indicators
    if echo "$recent" | grep -qiE '(thinking|analyzing|processing|reading|searching)'; then
        echo "$STATUS_THINKING"
        return
    fi

    # Check for active tool execution
    if echo "$recent" | grep -qE '(Running|Executing|Writing|Editing)'; then
        echo "$STATUS_ACTIVE"
        return
    fi

    # Check for error states
    if echo "$recent" | grep -qiE '(error:|failed:|exception|traceback)'; then
        echo "$STATUS_ERROR"
        return
    fi

    # Check for waiting/prompt state (common Claude Code prompt patterns)
    if echo "$recent" | grep -qE '(^>|^claude>|\$ $|❯ $)'; then
        echo "$STATUS_IDLE"
        return
    fi

    # Check for waiting for input
    if echo "$recent" | grep -qiE '(waiting|pending|press enter|continue\?)'; then
        echo "$STATUS_WAITING"
        return
    fi

    # Default to active if there's recent content
    if [[ -n "$recent" ]]; then
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
