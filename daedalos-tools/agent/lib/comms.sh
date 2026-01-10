#!/usr/bin/env bash
# comms.sh - Inter-agent communication
#
# Provides message passing, shared workspace, and help requests between agents.
# Messages are stored in a file-based queue per agent.

# Prevent double-sourcing
[[ -n "${_AGENT_COMMS_LOADED:-}" ]] && return 0
_AGENT_COMMS_LOADED=1

MESSAGES_DIR="${DATA_DIR}/messages"
SHARED_DIR="${DATA_DIR}/shared"
mkdir -p "$MESSAGES_DIR" "$SHARED_DIR"

# ============================================================================
# MESSAGE QUEUE
# ============================================================================

# Get message queue path for an agent
_msg_queue() {
    local agent="$1"
    echo "${MESSAGES_DIR}/${agent}.jsonl"
}

# Send a message to an agent
# Usage: comms_send <to_agent> <from_agent> <type> <content>
comms_send() {
    local to="$1"
    local from="$2"
    local msg_type="$3"
    local content="$4"

    # Validate target agent exists
    if ! agents_exists "$to"; then
        die "Target agent not found: $to"
    fi

    local queue
    queue=$(_msg_queue "$to")

    local timestamp
    timestamp=$(iso_timestamp)

    local msg_id
    msg_id="msg-$(date +%s%N | sha256sum | head -c 8)"

    # Append message to queue (JSON Lines format)
    jq -c -n \
        --arg id "$msg_id" \
        --arg from "$from" \
        --arg to "$to" \
        --arg type "$msg_type" \
        --arg content "$content" \
        --arg timestamp "$timestamp" \
        --arg status "pending" \
        '{id: $id, from: $from, to: $to, type: $type, content: $content, timestamp: $timestamp, status: $status}' \
        >> "$queue"

    # Notify the target agent via tmux (non-blocking)
    local agent
    agent=$(agents_get "$to")
    if [[ -n "$agent" ]]; then
        local session
        session=$(echo "$agent" | jq -r '.tmux_session')
        if tmux_session_exists "$session"; then
            # Display notification in the tmux status
            tmux set-option -t "$session" status-right "#[fg=yellow]MSG from $from#[default]" 2>/dev/null || true
        fi
    fi

    echo "$msg_id"
}

# Get pending messages for an agent
# Usage: comms_inbox <agent> [--all] [--json]
comms_inbox() {
    local agent="$1"
    local show_all=false
    local as_json=false
    shift

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --all|-a) show_all=true; shift ;;
            --json) as_json=true; shift ;;
            *) shift ;;
        esac
    done

    local queue
    queue=$(_msg_queue "$agent")

    if [[ ! -f "$queue" ]]; then
        if [[ "$as_json" == "true" ]]; then
            echo "[]"
        else
            echo "No messages."
        fi
        return
    fi

    if [[ "$as_json" == "true" ]]; then
        if [[ "$show_all" == "true" ]]; then
            jq -s '.' "$queue"
        else
            jq -s '[.[] | select(.status == "pending")]' "$queue"
        fi
    else
        local count=0
        while IFS= read -r line; do
            local status from msg_type content timestamp
            status=$(echo "$line" | jq -r '.status')
            [[ "$show_all" != "true" ]] && [[ "$status" != "pending" ]] && continue

            from=$(echo "$line" | jq -r '.from')
            msg_type=$(echo "$line" | jq -r '.type')
            content=$(echo "$line" | jq -r '.content')
            timestamp=$(echo "$line" | jq -r '.timestamp')

            ((count++))
            echo "${C_CYAN}[$msg_type]${C_RESET} from ${C_BOLD}$from${C_RESET} ($timestamp)"
            echo "  $content"
            echo ""
        done < "$queue"

        if [[ $count -eq 0 ]]; then
            echo "No messages."
        fi
    fi
}

# Mark messages as read
# Usage: comms_read <agent> [message_id|--all]
comms_read() {
    local agent="$1"
    local msg_id="${2:---all}"

    local queue
    queue=$(_msg_queue "$agent")

    if [[ ! -f "$queue" ]]; then
        return
    fi

    local tmp="${queue}.tmp.$$"

    if [[ "$msg_id" == "--all" ]]; then
        # Mark all pending as read
        while IFS= read -r line; do
            echo "$line" | jq -c '.status = "read"'
        done < "$queue" > "$tmp" && mv "$tmp" "$queue"
    else
        # Mark specific message
        while IFS= read -r line; do
            local id
            id=$(echo "$line" | jq -r '.id')
            if [[ "$id" == "$msg_id" ]]; then
                echo "$line" | jq -c '.status = "read"'
            else
                echo "$line"
            fi
        done < "$queue" > "$tmp" && mv "$tmp" "$queue"
    fi
}

# Clear read messages
# Usage: comms_clear <agent>
comms_clear() {
    local agent="$1"
    local queue
    queue=$(_msg_queue "$agent")

    if [[ ! -f "$queue" ]]; then
        return
    fi

    local tmp="${queue}.tmp.$$"
    jq -c 'select(.status == "pending")' "$queue" > "$tmp" 2>/dev/null
    mv "$tmp" "$queue"
}

# ============================================================================
# HELP REQUESTS
# ============================================================================

# Request help from another agent
# Usage: comms_help <from_agent> <task_description> [--template <template>]
comms_help() {
    local from="$1"
    local task="$2"
    local template=""
    shift 2

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --template|-t) template="$2"; shift 2 ;;
            *) shift ;;
        esac
    done

    # Find a suitable idle agent or spawn a new one
    local helper=""

    # First, look for an idle agent with matching template
    while IFS= read -r agent_json; do
        local name status agent_template
        name=$(echo "$agent_json" | jq -r '.name')
        status=$(echo "$agent_json" | jq -r '.status')
        agent_template=$(echo "$agent_json" | jq -r '.template')

        # Skip the requesting agent
        [[ "$name" == "$from" ]] && continue

        # Check if agent is idle
        if [[ "$status" == "idle" || "$status" == "waiting" ]]; then
            if [[ -z "$template" ]] || [[ "$agent_template" == "$template" ]]; then
                helper="$name"
                break
            fi
        fi
    done < <(agents_all)

    # If no idle agent found, spawn a new one
    if [[ -z "$helper" ]]; then
        local helper_name="helper-$(date +%s%N | sha256sum | head -c 4)"

        # Get project from requesting agent
        local requester
        requester=$(agents_get "$from")
        local project
        project=$(echo "$requester" | jq -r '.project')

        local spawn_args=("-n" "$helper_name" "-p" "$project" "--no-focus")
        if [[ -n "$template" ]]; then
            spawn_args+=("-t" "$template")
        fi

        info "Spawning helper agent: $helper_name"
        cmd_spawn "${spawn_args[@]}"
        helper="$helper_name"

        # Wait for agent to initialize
        sleep 2
    fi

    # Send the help request
    local request_id
    request_id=$(comms_send "$helper" "$from" "help_request" "$task")

    # Also send the task as input to the helper agent
    local helper_agent
    helper_agent=$(agents_get "$helper")
    local session
    session=$(echo "$helper_agent" | jq -r '.tmux_session')

    if tmux_session_exists "$session"; then
        local prompt="A colleague ($from) needs help with: $task\n\nPlease assist them. When done, send your response."
        tmux_send_keys "$session" "$prompt" Enter
    fi

    success "Help request sent to: $helper"
    echo "Request ID: $request_id"
}

# Reply to a help request
# Usage: comms_reply <from_agent> <to_agent> <response>
comms_reply() {
    local from="$1"
    local to="$2"
    local response="$3"

    comms_send "$to" "$from" "help_response" "$response"
    success "Reply sent to: $to"
}

# ============================================================================
# SHARED WORKSPACE
# ============================================================================

# Share a file/artifact with all agents or specific ones
# Usage: comms_share <from_agent> <file_path> [--to <agent>] [--name <artifact_name>]
comms_share() {
    local from="$1"
    local file_path="$2"
    local to=""
    local artifact_name=""
    shift 2

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --to) to="$2"; shift 2 ;;
            --name|-n) artifact_name="$2"; shift 2 ;;
            *) shift ;;
        esac
    done

    if [[ ! -f "$file_path" ]]; then
        die "File not found: $file_path"
    fi

    # Generate artifact name if not provided
    if [[ -z "$artifact_name" ]]; then
        artifact_name="$(basename "$file_path")-$(date +%s%N | sha256sum | head -c 6)"
    fi

    local artifact_dir="${SHARED_DIR}/${artifact_name}"
    mkdir -p "$artifact_dir"

    # Copy file
    cp "$file_path" "${artifact_dir}/content"

    # Create metadata
    cat > "${artifact_dir}/meta.json" << EOF
{
    "name": "$artifact_name",
    "original_path": "$file_path",
    "shared_by": "$from",
    "shared_at": "$(iso_timestamp)",
    "recipients": $(if [[ -n "$to" ]]; then echo "[\"$to\"]"; else echo "[]"; fi)
}
EOF

    # Notify recipients
    if [[ -n "$to" ]]; then
        comms_send "$to" "$from" "shared_artifact" "New artifact shared: $artifact_name (from $file_path)"
    else
        # Notify all agents
        while IFS= read -r name; do
            [[ -z "$name" ]] && continue
            [[ "$name" == "$from" ]] && continue
            comms_send "$name" "$from" "shared_artifact" "New artifact shared: $artifact_name (from $file_path)"
        done < <(agents_names)
    fi

    success "Shared: $artifact_name"
    echo "Location: ${artifact_dir}/content"
}

# List shared artifacts
# Usage: comms_artifacts [--json]
comms_artifacts() {
    local as_json="${1:-false}"

    if [[ ! -d "$SHARED_DIR" ]]; then
        if [[ "$as_json" == "true" ]]; then
            echo "[]"
        else
            echo "No shared artifacts."
        fi
        return
    fi

    if [[ "$as_json" == "true" ]]; then
        local artifacts="["
        local first=true
        for dir in "$SHARED_DIR"/*; do
            [[ ! -d "$dir" ]] && continue
            local meta="${dir}/meta.json"
            [[ ! -f "$meta" ]] && continue

            if [[ "$first" == "true" ]]; then
                first=false
            else
                artifacts+=","
            fi
            artifacts+="$(cat "$meta")"
        done
        artifacts+="]"
        echo "$artifacts"
    else
        echo "${C_BOLD}Shared Artifacts:${C_RESET}"
        for dir in "$SHARED_DIR"/*; do
            [[ ! -d "$dir" ]] && continue
            local meta="${dir}/meta.json"
            [[ ! -f "$meta" ]] && continue

            local name shared_by shared_at original
            name=$(jq -r '.name' "$meta")
            shared_by=$(jq -r '.shared_by' "$meta")
            shared_at=$(jq -r '.shared_at' "$meta")
            original=$(jq -r '.original_path' "$meta")

            echo "  ${C_CYAN}${name}${C_RESET}"
            echo "    From: $original"
            echo "    Shared by: $shared_by at $shared_at"
        done
    fi
}

# Get a shared artifact
# Usage: comms_get_artifact <name>
comms_get_artifact() {
    local name="$1"
    local artifact_dir="${SHARED_DIR}/${name}"

    if [[ ! -d "$artifact_dir" ]]; then
        die "Artifact not found: $name"
    fi

    local content="${artifact_dir}/content"
    if [[ -f "$content" ]]; then
        cat "$content"
    fi
}

# ============================================================================
# BROADCAST
# ============================================================================

# Broadcast a message to all agents
# Usage: comms_broadcast <from_agent> <message>
comms_broadcast() {
    local from="$1"
    local message="$2"

    local count=0
    while IFS= read -r name; do
        [[ -z "$name" ]] && continue
        [[ "$name" == "$from" ]] && continue
        comms_send "$name" "$from" "broadcast" "$message"
        ((count++))
    done < <(agents_names)

    success "Broadcast sent to $count agents"
}
