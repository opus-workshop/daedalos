#!/usr/bin/env bash
# hooks.sh - Agent lifecycle hooks
#
# Provides hooks that trigger on agent events:
# - on_spawn: When an agent is created
# - on_complete: When an agent signals completion
# - on_error: When an agent encounters an error
# - on_kill: When an agent is killed

# Prevent double-sourcing
[[ -n "${_AGENT_HOOKS_LOADED:-}" ]] && return 0
_AGENT_HOOKS_LOADED=1

HOOKS_DIR="${CONFIG_DIR}/hooks"
mkdir -p "$HOOKS_DIR"

# ============================================================================
# HOOK EXECUTION
# ============================================================================

# Execute hooks for an event
# Usage: hooks_run <event> <agent_name> [extra_data...]
hooks_run() {
    local event="$1"
    local agent_name="$2"
    shift 2
    local extra_data=("$@")

    local hook_dir="${HOOKS_DIR}/${event}"

    if [[ ! -d "$hook_dir" ]]; then
        debug "No hooks directory for event: $event"
        return 0
    fi

    # Find and run all executable hooks
    local found_hooks=false
    for hook in "$hook_dir"/*; do
        [[ ! -f "$hook" ]] && continue
        [[ ! -x "$hook" ]] && continue

        found_hooks=true
        debug "Running hook: $hook for $event on $agent_name"

        # Run hook with environment
        (
            export DAEDALOS_EVENT="$event"
            export DAEDALOS_AGENT_NAME="$agent_name"
            export DAEDALOS_HOOK_DATA="${extra_data[*]}"

            # Get agent info if available
            if agents_exists "$agent_name" 2>/dev/null; then
                local agent_json
                agent_json=$(agents_get "$agent_name")
                export DAEDALOS_AGENT_PROJECT=$(echo "$agent_json" | jq -r '.project // ""')
                export DAEDALOS_AGENT_TEMPLATE=$(echo "$agent_json" | jq -r '.template // ""')
                export DAEDALOS_AGENT_STATUS=$(echo "$agent_json" | jq -r '.status // ""')
            fi

            "$hook" 2>&1
        ) || warn "Hook failed: $(basename "$hook")"
    done

    if [[ "$found_hooks" == "false" ]]; then
        debug "No hooks found for event: $event"
    fi
}

# ============================================================================
# SPECIFIC EVENT HOOKS
# ============================================================================

# Called when an agent is spawned
hooks_on_spawn() {
    local agent_name="$1"
    local template="${2:-}"
    local project="${3:-}"

    hooks_run "on_spawn" "$agent_name" "template=$template" "project=$project"
}

# Called when an agent signals completion
hooks_on_complete() {
    local agent_name="$1"
    local status="${2:-success}"
    local data="${3:-}"

    hooks_run "on_complete" "$agent_name" "status=$status" "data=$data"
}

# Called when an agent encounters an error
hooks_on_error() {
    local agent_name="$1"
    local error_msg="${2:-}"

    hooks_run "on_error" "$agent_name" "error=$error_msg"
}

# Called when an agent is killed
hooks_on_kill() {
    local agent_name="$1"
    local reason="${2:-manual}"

    hooks_run "on_kill" "$agent_name" "reason=$reason"
}

# Called when a workflow completes
hooks_on_workflow_complete() {
    local workflow_id="$1"
    local status="${2:-completed}"

    hooks_run "on_workflow_complete" "$workflow_id" "status=$status"
}

# ============================================================================
# HOOK MANAGEMENT
# ============================================================================

# List configured hooks
# Usage: hooks_list [event]
hooks_list() {
    local event="${1:-}"

    if [[ -n "$event" ]]; then
        local hook_dir="${HOOKS_DIR}/${event}"
        if [[ -d "$hook_dir" ]]; then
            echo "${C_BOLD}Hooks for: ${event}${C_RESET}"
            for hook in "$hook_dir"/*; do
                [[ ! -f "$hook" ]] && continue
                local status="disabled"
                [[ -x "$hook" ]] && status="enabled"
                echo "  $(basename "$hook") [$status]"
            done
        else
            echo "No hooks for event: $event"
        fi
    else
        echo "${C_BOLD}Configured Hooks:${C_RESET}"
        echo ""
        for event_dir in "$HOOKS_DIR"/*; do
            [[ ! -d "$event_dir" ]] && continue
            local event_name=$(basename "$event_dir")
            local count=0
            for hook in "$event_dir"/*; do
                [[ -f "$hook" ]] && [[ -x "$hook" ]] && ((count++))
            done
            echo "  ${C_CYAN}${event_name}${C_RESET}: $count active hooks"
        done
    fi
}

# Add a hook
# Usage: hooks_add <event> <script_path> [name]
hooks_add() {
    local event="$1"
    local script="$2"
    local name="${3:-$(basename "$script")}"

    if [[ ! -f "$script" ]]; then
        die "Script not found: $script"
    fi

    local hook_dir="${HOOKS_DIR}/${event}"
    mkdir -p "$hook_dir"

    local hook_path="${hook_dir}/${name}"
    cp "$script" "$hook_path"
    chmod +x "$hook_path"

    success "Added hook: $name for event $event"
}

# Remove a hook
# Usage: hooks_remove <event> <name>
hooks_remove() {
    local event="$1"
    local name="$2"

    local hook_path="${HOOKS_DIR}/${event}/${name}"

    if [[ ! -f "$hook_path" ]]; then
        die "Hook not found: $name for event $event"
    fi

    rm "$hook_path"
    success "Removed hook: $name"
}

# Enable a hook
hooks_enable() {
    local event="$1"
    local name="$2"

    local hook_path="${HOOKS_DIR}/${event}/${name}"
    if [[ -f "$hook_path" ]]; then
        chmod +x "$hook_path"
        success "Enabled hook: $name"
    else
        die "Hook not found: $name"
    fi
}

# Disable a hook
hooks_disable() {
    local event="$1"
    local name="$2"

    local hook_path="${HOOKS_DIR}/${event}/${name}"
    if [[ -f "$hook_path" ]]; then
        chmod -x "$hook_path"
        success "Disabled hook: $name"
    else
        die "Hook not found: $name"
    fi
}

# Create a hook from template
# Usage: hooks_create <event> <name>
hooks_create() {
    local event="$1"
    local name="$2"

    local hook_dir="${HOOKS_DIR}/${event}"
    mkdir -p "$hook_dir"

    local hook_path="${hook_dir}/${name}"

    if [[ -f "$hook_path" ]]; then
        die "Hook already exists: $name"
    fi

    cat > "$hook_path" << 'EOF'
#!/usr/bin/env bash
# Daedalos Agent Hook
#
# Available environment variables:
#   DAEDALOS_EVENT        - The event that triggered this hook
#   DAEDALOS_AGENT_NAME   - Name of the agent
#   DAEDALOS_HOOK_DATA    - Additional event data
#   DAEDALOS_AGENT_PROJECT - Agent's project directory
#   DAEDALOS_AGENT_TEMPLATE - Agent's template
#   DAEDALOS_AGENT_STATUS - Agent's status

echo "Hook triggered: $DAEDALOS_EVENT for agent $DAEDALOS_AGENT_NAME"
echo "Data: $DAEDALOS_HOOK_DATA"

# Add your custom logic here
EOF

    chmod +x "$hook_path"
    success "Created hook: $hook_path"
    echo "Edit this file to add your hook logic"
}

# ============================================================================
# CLI INTEGRATION
# ============================================================================

cmd_hooks() {
    local action="${1:-list}"
    shift || true

    case "$action" in
        list)
            hooks_list "$@"
            ;;
        add)
            hooks_add "$@"
            ;;
        remove|rm)
            hooks_remove "$@"
            ;;
        enable)
            hooks_enable "$@"
            ;;
        disable)
            hooks_disable "$@"
            ;;
        create)
            hooks_create "$@"
            ;;
        *)
            echo "Usage: agent hooks <list|add|remove|enable|disable|create>"
            echo ""
            echo "Commands:"
            echo "  list [event]           List hooks (optionally for specific event)"
            echo "  add <event> <script>   Add a script as a hook"
            echo "  remove <event> <name>  Remove a hook"
            echo "  enable <event> <name>  Enable a disabled hook"
            echo "  disable <event> <name> Disable a hook"
            echo "  create <event> <name>  Create a new hook from template"
            echo ""
            echo "Events:"
            echo "  on_spawn      - Agent spawned"
            echo "  on_complete   - Agent signaled completion"
            echo "  on_error      - Agent encountered error"
            echo "  on_kill       - Agent was killed"
            echo "  on_workflow_complete - Workflow finished"
            ;;
    esac
}
