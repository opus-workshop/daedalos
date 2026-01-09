#!/usr/bin/env bash
# agents.sh - Agent CRUD operations
#
# Manages agent metadata in agents.json

# Prevent double-sourcing
[[ -n "${_AGENT_AGENTS_LOADED:-}" ]] && return 0
_AGENT_AGENTS_LOADED=1

# Initialize agents.json if it doesn't exist
agents_init() {
    if [[ ! -f "$AGENTS_FILE" ]]; then
        echo '{"agents":{},"next_slot":1,"max_slots":9}' > "$AGENTS_FILE"
    fi
}

# Get all agents as JSON
agents_list() {
    agents_init
    jq '.agents' "$AGENTS_FILE"
}

# Get list of agent names
agents_names() {
    agents_init
    jq -r '.agents | keys[]' "$AGENTS_FILE"
}

# Get agent by name
agents_get() {
    local name="$1"
    agents_init
    jq -r ".agents[\"$name\"] // empty" "$AGENTS_FILE"
}

# Check if agent exists
agents_exists() {
    local name="$1"
    local agent
    agent=$(agents_get "$name")
    [[ -n "$agent" ]]
}

# Create a new agent entry
# Usage: agents_create <name> <project> <template> <sandbox> <slot>
agents_create() {
    local name="$1"
    local project="$2"
    local template="$3"
    local sandbox="$4"
    local slot="$5"

    agents_init

    if agents_exists "$name"; then
        die "Agent already exists: $name"
    fi

    local session
    session=$(tmux_session_name "$name")
    local timestamp
    timestamp=$(iso_timestamp)

    local tmp="${AGENTS_FILE}.tmp.$$"
    jq --arg name "$name" \
       --arg project "$project" \
       --arg template "$template" \
       --arg sandbox "$sandbox" \
       --argjson slot "$slot" \
       --arg session "$session" \
       --arg timestamp "$timestamp" \
       '.agents[$name] = {
           "name": $name,
           "slot": $slot,
           "project": $project,
           "template": $template,
           "sandbox": $sandbox,
           "tmux_session": $session,
           "pid": 0,
           "created": $timestamp,
           "status": "starting",
           "last_activity": $timestamp
       }' "$AGENTS_FILE" > "$tmp" && mv "$tmp" "$AGENTS_FILE"
}

# Update agent entry
# Usage: agents_update <name> <field> <value>
agents_update() {
    local name="$1"
    local field="$2"
    local value="$3"

    if ! agents_exists "$name"; then
        return 1
    fi

    local tmp="${AGENTS_FILE}.tmp.$$"

    # Handle numeric vs string values
    if [[ "$value" =~ ^[0-9]+$ ]]; then
        jq --arg name "$name" \
           --arg field "$field" \
           --argjson value "$value" \
           '.agents[$name][$field] = $value' "$AGENTS_FILE" > "$tmp" && mv "$tmp" "$AGENTS_FILE"
    else
        jq --arg name "$name" \
           --arg field "$field" \
           --arg value "$value" \
           '.agents[$name][$field] = $value' "$AGENTS_FILE" > "$tmp" && mv "$tmp" "$AGENTS_FILE"
    fi
}

# Update last activity timestamp
agents_touch() {
    local name="$1"
    local timestamp
    timestamp=$(iso_timestamp)
    agents_update "$name" "last_activity" "$timestamp"
}

# Delete agent entry
agents_delete() {
    local name="$1"

    if ! agents_exists "$name"; then
        return 0
    fi

    # Get the slot before deletion
    local slot
    slot=$(agents_get "$name" | jq -r '.slot')

    local tmp="${AGENTS_FILE}.tmp.$$"
    jq --arg name "$name" 'del(.agents[$name])' "$AGENTS_FILE" > "$tmp" && mv "$tmp" "$AGENTS_FILE"

    debug "Deleted agent: $name (slot $slot)"
}

# Get next available slot
agents_next_slot() {
    agents_init

    local max_slots
    max_slots=$(jq -r '.max_slots // 9' "$AGENTS_FILE")

    # Get used slots
    local -a used_slots
    while IFS= read -r slot; do
        [[ -n "$slot" ]] && used_slots+=("$slot")
    done < <(jq -r '.agents[].slot' "$AGENTS_FILE" 2>/dev/null)

    # Find first available slot
    for ((i=1; i<=max_slots; i++)); do
        local found=0
        for used in "${used_slots[@]}"; do
            if [[ "$used" == "$i" ]]; then
                found=1
                break
            fi
        done
        if [[ $found -eq 0 ]]; then
            echo "$i"
            return 0
        fi
    done

    # No slots available
    return 1
}

# Get agent by slot number
agents_by_slot() {
    local slot="$1"
    agents_init
    jq -r --argjson slot "$slot" '.agents | to_entries[] | select(.value.slot == $slot) | .value.name' "$AGENTS_FILE"
}

# Fuzzy match agent name
agents_fuzzy_match() {
    local query="$1"
    local -a names

    # Get all agent names
    while IFS= read -r name; do
        [[ -n "$name" ]] && names+=("$name")
    done < <(agents_names)

    if [[ ${#names[@]} -eq 0 ]]; then
        return 1
    fi

    # Exact match first
    for name in "${names[@]}"; do
        if [[ "$name" == "$query" ]]; then
            echo "$name"
            return 0
        fi
    done

    # Prefix match
    for name in "${names[@]}"; do
        if [[ "$name" == "$query"* ]]; then
            echo "$name"
            return 0
        fi
    done

    # Contains match
    for name in "${names[@]}"; do
        if [[ "$name" == *"$query"* ]]; then
            echo "$name"
            return 0
        fi
    done

    # Use fzf if available and multiple matches possible
    if has_fzf; then
        local match
        match=$(printf '%s\n' "${names[@]}" | fzf --filter="$query" --no-sort | head -1)
        if [[ -n "$match" ]]; then
            echo "$match"
            return 0
        fi
    fi

    return 1
}

# Resolve agent identifier (name or slot number) to name
agents_resolve() {
    local identifier="$1"

    # If it's a number, look up by slot
    if [[ "$identifier" =~ ^[0-9]+$ ]]; then
        local name
        name=$(agents_by_slot "$identifier")
        if [[ -n "$name" ]]; then
            echo "$name"
            return 0
        fi
        return 1
    fi

    # Try exact match first
    if agents_exists "$identifier"; then
        echo "$identifier"
        return 0
    fi

    # Try fuzzy match
    agents_fuzzy_match "$identifier"
}

# Count active agents
agents_count() {
    agents_init
    jq '.agents | length' "$AGENTS_FILE"
}

# Get all agents as array for iteration
agents_all() {
    agents_init
    jq -r '.agents | to_entries[] | .value | @json' "$AGENTS_FILE"
}

# Set agent PID
agents_set_pid() {
    local name="$1"
    local pid="$2"
    agents_update "$name" "pid" "$pid"
}

# Get agent PID
agents_get_pid() {
    local name="$1"
    local agent
    agent=$(agents_get "$name")
    if [[ -n "$agent" ]]; then
        echo "$agent" | jq -r '.pid // 0'
    else
        echo 0
    fi
}

# Check if we've hit the agent limit
agents_at_limit() {
    agents_init
    local count max
    count=$(agents_count)
    max=$(jq -r '.max_slots // 9' "$AGENTS_FILE")
    [[ $count -ge $max ]]
}
