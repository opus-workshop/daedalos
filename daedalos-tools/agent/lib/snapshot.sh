#!/usr/bin/env bash
# snapshot.sh - Agent state snapshot and restore
#
# Saves and restores agent states including:
# - Agent metadata from agents.json
# - tmux scrollback buffer (conversation history)
# - Optional git stash of working changes

# Prevent double-sourcing
[[ -n "${_AGENT_SNAPSHOT_LOADED:-}" ]] && return 0
_AGENT_SNAPSHOT_LOADED=1

SNAPSHOTS_DIR="${DATA_DIR}/snapshots"
mkdir -p "$SNAPSHOTS_DIR"

# Create a snapshot of one or all agents
# Usage: snapshot_create [name] [--all] [--snapshot-name <name>]
snapshot_create() {
    local agent_name=""
    local all=false
    local snapshot_name=""

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --all|-a) all=true; shift ;;
            --name|-n) snapshot_name="$2"; shift 2 ;;
            -*) die "Unknown option: $1" ;;
            *) agent_name="$1"; shift ;;
        esac
    done

    # Generate snapshot name if not provided
    if [[ -z "$snapshot_name" ]]; then
        snapshot_name="snapshot-$(date +%Y%m%d-%H%M%S)"
    fi

    local snapshot_dir="${SNAPSHOTS_DIR}/${snapshot_name}"

    if [[ -d "$snapshot_dir" ]]; then
        die "Snapshot already exists: $snapshot_name"
    fi

    mkdir -p "$snapshot_dir"

    # Create snapshot metadata
    local meta_file="${snapshot_dir}/meta.json"
    cat > "$meta_file" << EOF
{
    "name": "$snapshot_name",
    "created": "$(iso_timestamp)",
    "agents": []
}
EOF

    # Snapshot agents
    if [[ "$all" == "true" ]]; then
        info "Creating snapshot of all agents: $snapshot_name"
        while IFS= read -r name; do
            [[ -z "$name" ]] && continue
            snapshot_agent "$name" "$snapshot_dir"
        done < <(agents_names)
    elif [[ -n "$agent_name" ]]; then
        local resolved
        resolved=$(agents_resolve "$agent_name")
        if [[ -z "$resolved" ]]; then
            rm -rf "$snapshot_dir"
            die "Agent not found: $agent_name"
        fi
        info "Creating snapshot of agent: $resolved"
        snapshot_agent "$resolved" "$snapshot_dir"
    else
        die "Usage: agent snapshot <name> or agent snapshot --all"
    fi

    success "Snapshot created: $snapshot_name"
    echo "Location: $snapshot_dir"
}

# Snapshot a single agent
# Usage: snapshot_agent <name> <snapshot_dir>
snapshot_agent() {
    local name="$1"
    local snapshot_dir="$2"

    local agent
    agent=$(agents_get "$name")
    if [[ -z "$agent" ]]; then
        warn "Agent not found: $name"
        return 1
    fi

    local agent_dir="${snapshot_dir}/${name}"
    mkdir -p "$agent_dir"

    debug "Snapshotting agent: $name to $agent_dir"

    # Save agent metadata
    echo "$agent" > "${agent_dir}/agent.json"

    # Get session and capture tmux scrollback
    local session
    session=$(echo "$agent" | jq -r '.tmux_session')

    if tmux_session_exists "$session"; then
        # Capture full scrollback buffer
        tmux capture-pane -t "$session" -p -S - > "${agent_dir}/scrollback.txt" 2>/dev/null || true

        # Capture current pane content
        tmux_get_pane_content "$session" 1000 > "${agent_dir}/pane_content.txt" 2>/dev/null || true
    else
        debug "Session not active: $session"
    fi

    # Get project and optionally stash git changes
    local project
    project=$(echo "$agent" | jq -r '.project')

    if [[ -d "$project/.git" ]]; then
        # Record git state
        (
            cd "$project" || exit 1
            echo "branch: $(git branch --show-current)" > "${agent_dir}/git_state.txt"
            echo "commit: $(git rev-parse HEAD)" >> "${agent_dir}/git_state.txt"

            # Check for uncommitted changes
            if ! git diff --quiet || ! git diff --cached --quiet; then
                # Create a diff of current changes
                git diff > "${agent_dir}/git_diff.patch" 2>/dev/null || true
                git diff --cached > "${agent_dir}/git_staged.patch" 2>/dev/null || true
            fi
        )
    fi

    # Update snapshot metadata
    local meta_file="${snapshot_dir}/meta.json"
    local tmp="${meta_file}.tmp.$$"
    jq --arg name "$name" \
       '.agents += [$name]' "$meta_file" > "$tmp" && mv "$tmp" "$meta_file"

    success "  Snapshotted: $name"
}

# List available snapshots
snapshot_list() {
    local as_json="${1:-false}"

    if [[ ! -d "$SNAPSHOTS_DIR" ]]; then
        if [[ "$as_json" == "true" ]]; then
            echo "[]"
        else
            echo "No snapshots found."
        fi
        return
    fi

    local -a snapshots=()

    for dir in "$SNAPSHOTS_DIR"/*; do
        [[ ! -d "$dir" ]] && continue
        local meta="${dir}/meta.json"
        [[ ! -f "$meta" ]] && continue

        local name created agent_count
        name=$(basename "$dir")
        created=$(jq -r '.created // "unknown"' "$meta")
        agent_count=$(jq -r '.agents | length' "$meta")

        if [[ "$as_json" == "true" ]]; then
            snapshots+=("$(jq -c --arg name "$name" --arg created "$created" --argjson count "$agent_count" '{name: $name, created: $created, agent_count: $count}' <<< '{}')")
        else
            snapshots+=("$name|$created|$agent_count agents")
        fi
    done

    if [[ ${#snapshots[@]} -eq 0 ]]; then
        if [[ "$as_json" == "true" ]]; then
            echo "[]"
        else
            echo "No snapshots found."
        fi
        return
    fi

    if [[ "$as_json" == "true" ]]; then
        printf '[%s]' "$(IFS=,; echo "${snapshots[*]}")"
    else
        format_table "NAME|CREATED|AGENTS" "${snapshots[@]}"
    fi
}

# Restore agents from a snapshot
# Usage: snapshot_restore <snapshot_name> [--agent <name>] [--no-focus]
snapshot_restore() {
    local snapshot_name=""
    local specific_agent=""
    local no_focus=false

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --agent|-a) specific_agent="$2"; shift 2 ;;
            --no-focus) no_focus=true; shift ;;
            -*) die "Unknown option: $1" ;;
            *) snapshot_name="$1"; shift ;;
        esac
    done

    if [[ -z "$snapshot_name" ]]; then
        # Interactive selection if fzf available
        if has_fzf && [[ -d "$SNAPSHOTS_DIR" ]]; then
            snapshot_name=$(ls "$SNAPSHOTS_DIR" 2>/dev/null | fzf --prompt="Select snapshot: ")
            if [[ -z "$snapshot_name" ]]; then
                return
            fi
        else
            die "Usage: agent restore <snapshot_name>"
        fi
    fi

    local snapshot_dir="${SNAPSHOTS_DIR}/${snapshot_name}"

    if [[ ! -d "$snapshot_dir" ]]; then
        die "Snapshot not found: $snapshot_name"
    fi

    local meta_file="${snapshot_dir}/meta.json"
    if [[ ! -f "$meta_file" ]]; then
        die "Invalid snapshot (no metadata): $snapshot_name"
    fi

    info "Restoring from snapshot: $snapshot_name"

    # Get agents to restore
    local -a agent_names
    if [[ -n "$specific_agent" ]]; then
        agent_names=("$specific_agent")
    else
        while IFS= read -r name; do
            [[ -n "$name" ]] && agent_names+=("$name")
        done < <(jq -r '.agents[]' "$meta_file")
    fi

    local first_restored=""

    for name in "${agent_names[@]}"; do
        local agent_dir="${snapshot_dir}/${name}"
        if [[ ! -d "$agent_dir" ]]; then
            warn "Agent data not found in snapshot: $name"
            continue
        fi

        if restore_agent "$name" "$agent_dir"; then
            [[ -z "$first_restored" ]] && first_restored="$name"
        fi
    done

    success "Restore complete"

    # Focus first restored agent unless --no-focus
    if [[ "$no_focus" != "true" ]] && [[ -n "$first_restored" ]]; then
        cmd_focus "$first_restored"
    fi
}

# Restore a single agent from snapshot data
# Usage: restore_agent <name> <agent_dir>
restore_agent() {
    local name="$1"
    local agent_dir="$2"

    local agent_json="${agent_dir}/agent.json"
    if [[ ! -f "$agent_json" ]]; then
        warn "No agent metadata: $name"
        return 1
    fi

    # Check if agent already exists
    if agents_exists "$name"; then
        warn "Agent already exists: $name (skipping, use --force to overwrite)"
        return 1
    fi

    # Read original agent data
    local project template sandbox
    project=$(jq -r '.project' "$agent_json")
    template=$(jq -r '.template // "default"' "$agent_json")
    sandbox=$(jq -r '.sandbox // "implement"' "$agent_json")

    # Check if project still exists
    if [[ ! -d "$project" ]]; then
        warn "Project directory no longer exists: $project"
        return 1
    fi

    # Get available slot
    local slot
    slot=$(agents_next_slot) || {
        warn "No available slots for: $name"
        return 1
    }

    info "Restoring agent: $name (slot $slot)"

    # Build claude command (same as spawn)
    local -a claude_cmd=("claude")

    # Add template args if available
    if [[ -n "$template" ]] && [[ "$template" != "default" ]]; then
        if templates_exists "$template"; then
            local template_args
            template_args=$(templates_get_claude_args "$template")
            if [[ -n "$template_args" ]]; then
                read -ra args <<< "$template_args"
                claude_cmd+=("${args[@]}")
            fi
        fi
    fi

    # Create tmux session
    local session
    session=$(tmux_session_name "$name")

    if ! tmux_create_session "$session" "$project" "${claude_cmd[@]}"; then
        die "Failed to create tmux session for: $name"
    fi

    # Record agent in database
    agents_create "$name" "$project" "${template:-default}" "$sandbox" "$slot"

    # Get PID and update
    sleep 0.5
    local pid
    pid=$(tmux_get_pane_pid "$session")
    if [[ -n "$pid" ]]; then
        agents_set_pid "$name" "$pid"
    fi

    # Optionally restore git state
    local git_diff="${agent_dir}/git_diff.patch"
    if [[ -f "$git_diff" ]] && [[ -s "$git_diff" ]]; then
        info "  Restoring uncommitted changes..."
        (cd "$project" && git apply --quiet "$git_diff" 2>/dev/null) || \
            warn "  Could not apply git diff (may have conflicts)"
    fi

    # Send context about previous session if scrollback exists
    local scrollback="${agent_dir}/scrollback.txt"
    if [[ -f "$scrollback" ]] && [[ -s "$scrollback" ]]; then
        # Wait for claude to initialize
        sleep 2

        # Send a summary prompt with previous context
        local context_msg="I'm restoring a previous session. Here's some context from where we left off:\n\n"
        context_msg+="[Previous scrollback available at: $scrollback]\n\n"
        context_msg+="Let's continue from where we were."

        # Note: We don't automatically send the full scrollback as it may be very long
        # The user can manually load it if needed
    fi

    success "  Restored: $name"
    return 0
}

# Delete a snapshot
# Usage: snapshot_delete <name> [--force]
snapshot_delete() {
    local snapshot_name=""
    local force=false

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --force|-f) force=true; shift ;;
            -*) die "Unknown option: $1" ;;
            *) snapshot_name="$1"; shift ;;
        esac
    done

    if [[ -z "$snapshot_name" ]]; then
        die "Usage: agent snapshot delete <name>"
    fi

    local snapshot_dir="${SNAPSHOTS_DIR}/${snapshot_name}"

    if [[ ! -d "$snapshot_dir" ]]; then
        die "Snapshot not found: $snapshot_name"
    fi

    if [[ "$force" != "true" ]]; then
        read -rp "Delete snapshot '$snapshot_name'? (y/N) " confirm
        if [[ "$confirm" != "y" && "$confirm" != "Y" ]]; then
            echo "Aborted."
            return
        fi
    fi

    rm -rf "$snapshot_dir"
    success "Deleted snapshot: $snapshot_name"
}

# Show snapshot details
# Usage: snapshot_show <name>
snapshot_show() {
    local snapshot_name="$1"

    if [[ -z "$snapshot_name" ]]; then
        die "Usage: agent snapshot show <name>"
    fi

    local snapshot_dir="${SNAPSHOTS_DIR}/${snapshot_name}"

    if [[ ! -d "$snapshot_dir" ]]; then
        die "Snapshot not found: $snapshot_name"
    fi

    local meta_file="${snapshot_dir}/meta.json"

    echo "${C_BOLD}Snapshot: ${C_CYAN}${snapshot_name}${C_RESET}"
    echo ""

    if [[ -f "$meta_file" ]]; then
        local created
        created=$(jq -r '.created // "unknown"' "$meta_file")
        echo "Created: $created"
        echo ""
    fi

    echo "${C_BOLD}Agents:${C_RESET}"
    for agent_dir in "$snapshot_dir"/*; do
        [[ ! -d "$agent_dir" ]] && continue
        local name
        name=$(basename "$agent_dir")
        [[ "$name" == "meta.json" ]] && continue

        local agent_json="${agent_dir}/agent.json"
        if [[ -f "$agent_json" ]]; then
            local project template
            project=$(jq -r '.project // "unknown"' "$agent_json")
            template=$(jq -r '.template // "default"' "$agent_json")

            echo "  ${C_CYAN}${name}${C_RESET}"
            echo "    Project: $project"
            echo "    Template: $template"

            # Show what's captured
            local captured=""
            [[ -f "${agent_dir}/scrollback.txt" ]] && captured+=" scrollback"
            [[ -f "${agent_dir}/git_diff.patch" ]] && captured+=" git-diff"
            [[ -f "${agent_dir}/git_state.txt" ]] && captured+=" git-state"
            [[ -n "$captured" ]] && echo "    Captured:$captured"
        fi
    done
}
