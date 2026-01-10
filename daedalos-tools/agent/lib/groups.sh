#!/usr/bin/env bash
# groups.sh - Agent group management
#
# Named groups of agents for batch operations

# Prevent double-sourcing
[[ -n "${_AGENT_GROUPS_LOADED:-}" ]] && return 0
_AGENT_GROUPS_LOADED=1

GROUPS_FILE="${DATA_DIR}/groups.json"

# Initialize groups file
groups_init() {
    if [[ ! -f "$GROUPS_FILE" ]]; then
        echo '{"groups":{}}' > "$GROUPS_FILE"
    fi
}

# ============================================================================
# GROUP CRUD
# ============================================================================

# Create a new group
# Usage: groups_create <name> [description]
groups_create() {
    local name="$1"
    local description="${2:-}"

    groups_init

    if groups_exists "$name"; then
        die "Group already exists: $name"
    fi

    local timestamp
    timestamp=$(iso_timestamp)

    local tmp="${GROUPS_FILE}.tmp.$$"
    jq --arg name "$name" \
       --arg desc "$description" \
       --arg ts "$timestamp" \
       '.groups[$name] = {"name": $name, "description": $desc, "members": [], "created": $ts}' \
       "$GROUPS_FILE" > "$tmp" && mv "$tmp" "$GROUPS_FILE"

    success "Created group: $name"
}

# Check if group exists
groups_exists() {
    local name="$1"
    groups_init
    local group
    group=$(jq -r ".groups[\"$name\"] // empty" "$GROUPS_FILE")
    [[ -n "$group" ]]
}

# Delete a group
groups_delete() {
    local name="$1"
    local force="${2:-false}"

    if ! groups_exists "$name"; then
        die "Group not found: $name"
    fi

    if [[ "$force" != "true" ]]; then
        read -rp "Delete group '$name'? (y/N) " confirm
        if [[ "$confirm" != "y" && "$confirm" != "Y" ]]; then
            echo "Aborted."
            return
        fi
    fi

    local tmp="${GROUPS_FILE}.tmp.$$"
    jq --arg name "$name" 'del(.groups[$name])' "$GROUPS_FILE" > "$tmp" && mv "$tmp" "$GROUPS_FILE"

    success "Deleted group: $name"
}

# List all groups
groups_list() {
    local as_json="${1:-false}"

    groups_init

    if [[ "$as_json" == "true" ]]; then
        jq '.groups | to_entries | map(.value)' "$GROUPS_FILE"
    else
        local -a groups=()
        while IFS= read -r line; do
            local name desc count
            name=$(echo "$line" | jq -r '.name')
            desc=$(echo "$line" | jq -r '.description // ""')
            count=$(echo "$line" | jq -r '.members | length')

            groups+=("$name|$desc|$count members")
        done < <(jq -c '.groups[]' "$GROUPS_FILE" 2>/dev/null)

        if [[ ${#groups[@]} -eq 0 ]]; then
            echo "No groups defined. Create one with: agent group create <name>"
        else
            format_table "GROUP|DESCRIPTION|MEMBERS" "${groups[@]}"
        fi
    fi
}

# Show group details
groups_show() {
    local name="$1"

    if ! groups_exists "$name"; then
        die "Group not found: $name"
    fi

    local group
    group=$(jq ".groups[\"$name\"]" "$GROUPS_FILE")

    echo "${C_BOLD}Group: ${C_CYAN}${name}${C_RESET}"
    echo ""

    local desc
    desc=$(echo "$group" | jq -r '.description // ""')
    [[ -n "$desc" ]] && echo "Description: $desc"

    local created
    created=$(echo "$group" | jq -r '.created')
    echo "Created: $created"
    echo ""

    echo "${C_BOLD}Members:${C_RESET}"
    local -a members
    mapfile -t members < <(echo "$group" | jq -r '.members[]')

    if [[ ${#members[@]} -eq 0 ]]; then
        echo "  (no members)"
    else
        for member in "${members[@]}"; do
            if agents_exists "$member"; then
                local status
                status=$(agents_get "$member" | jq -r '.status // "unknown"')
                local status_color
                status_color=$(color_status "$status")
                echo "  ${C_CYAN}${member}${C_RESET} [${status_color}${status}${C_RESET}]"
            else
                echo "  ${C_DIM}${member} (not running)${C_RESET}"
            fi
        done
    fi
}

# ============================================================================
# MEMBERSHIP MANAGEMENT
# ============================================================================

# Add agent(s) to a group
# Usage: groups_add <group> <agent> [agent...]
groups_add() {
    local group_name="$1"
    shift

    if [[ $# -eq 0 ]]; then
        die "Usage: agent group add <group> <agent> [agent...]"
    fi

    if ! groups_exists "$group_name"; then
        die "Group not found: $group_name"
    fi

    for agent in "$@"; do
        local resolved
        resolved=$(agents_resolve "$agent" 2>/dev/null || echo "$agent")

        # Check if already in group
        local is_member
        is_member=$(jq -r --arg name "$resolved" ".groups[\"$group_name\"].members | index(\$name)" "$GROUPS_FILE")

        if [[ "$is_member" != "null" ]]; then
            warn "Already in group: $resolved"
            continue
        fi

        local tmp="${GROUPS_FILE}.tmp.$$"
        jq --arg group "$group_name" --arg agent "$resolved" \
           '.groups[$group].members += [$agent]' "$GROUPS_FILE" > "$tmp" && mv "$tmp" "$GROUPS_FILE"

        success "Added to $group_name: $resolved"
    done
}

# Remove agent(s) from a group
# Usage: groups_remove <group> <agent> [agent...]
groups_remove() {
    local group_name="$1"
    shift

    if [[ $# -eq 0 ]]; then
        die "Usage: agent group remove <group> <agent> [agent...]"
    fi

    if ! groups_exists "$group_name"; then
        die "Group not found: $group_name"
    fi

    for agent in "$@"; do
        local resolved
        resolved=$(agents_resolve "$agent" 2>/dev/null || echo "$agent")

        local tmp="${GROUPS_FILE}.tmp.$$"
        jq --arg group "$group_name" --arg agent "$resolved" \
           '.groups[$group].members -= [$agent]' "$GROUPS_FILE" > "$tmp" && mv "$tmp" "$GROUPS_FILE"

        success "Removed from $group_name: $resolved"
    done
}

# ============================================================================
# BATCH OPERATIONS
# ============================================================================

# Get members of a group
groups_members() {
    local name="$1"

    if ! groups_exists "$name"; then
        die "Group not found: $name"
    fi

    jq -r ".groups[\"$name\"].members[]" "$GROUPS_FILE"
}

# Kill all agents in a group
# Usage: groups_kill <group> [--force]
groups_kill() {
    local group_name="$1"
    local force="${2:-false}"

    if ! groups_exists "$group_name"; then
        die "Group not found: $group_name"
    fi

    local -a members
    mapfile -t members < <(groups_members "$group_name")

    if [[ ${#members[@]} -eq 0 ]]; then
        echo "No members in group: $group_name"
        return
    fi

    echo "This will kill ${#members[@]} agents in group '$group_name':"
    printf '  %s\n' "${members[@]}"

    if [[ "$force" != "true" ]]; then
        read -rp "Proceed? (y/N) " confirm
        if [[ "$confirm" != "y" && "$confirm" != "Y" ]]; then
            echo "Aborted."
            return
        fi
    fi

    for member in "${members[@]}"; do
        if agents_exists "$member"; then
            kill_agent "$member" "$force"
        fi
    done

    success "Killed all agents in: $group_name"
}

# Pause all agents in a group
groups_pause() {
    local group_name="$1"

    if ! groups_exists "$group_name"; then
        die "Group not found: $group_name"
    fi

    local -a members
    mapfile -t members < <(groups_members "$group_name")

    for member in "${members[@]}"; do
        if agents_exists "$member"; then
            cmd_pause "$member"
        fi
    done

    success "Paused all agents in: $group_name"
}

# Resume all agents in a group
groups_resume() {
    local group_name="$1"

    if ! groups_exists "$group_name"; then
        die "Group not found: $group_name"
    fi

    local -a members
    mapfile -t members < <(groups_members "$group_name")

    for member in "${members[@]}"; do
        if agents_exists "$member"; then
            cmd_resume "$member"
        fi
    done

    success "Resumed all agents in: $group_name"
}

# Send message to all agents in a group
groups_send() {
    local group_name="$1"
    local message="$2"
    local from="${DAEDALOS_AGENT_NAME:-user}"

    if ! groups_exists "$group_name"; then
        die "Group not found: $group_name"
    fi

    local -a members
    mapfile -t members < <(groups_members "$group_name")

    local count=0
    for member in "${members[@]}"; do
        if agents_exists "$member"; then
            comms_send "$member" "$from" "group_message" "$message"
            ((count++))
        fi
    done

    success "Sent message to $count agents in: $group_name"
}

# Spawn a team of agents from templates
# Usage: groups_spawn_team <group_name> <project> [templates...]
groups_spawn_team() {
    local group_name="$1"
    local project="$2"
    shift 2

    # Default team if no templates specified
    local -a templates=("$@")
    if [[ ${#templates[@]} -eq 0 ]]; then
        templates=("explorer" "implementer" "reviewer")
    fi

    # Create group if it doesn't exist
    if ! groups_exists "$group_name"; then
        groups_create "$group_name" "Team spawned at $(date)"
    fi

    project="${project:-$(pwd)}"

    info "Spawning team: $group_name"

    for template in "${templates[@]}"; do
        local agent_name="${group_name}-${template}"

        # Spawn agent
        cmd_spawn -n "$agent_name" -p "$project" -t "$template" --no-focus

        # Add to group
        groups_add "$group_name" "$agent_name"
    done

    success "Team spawned: $group_name with ${#templates[@]} agents"
}
