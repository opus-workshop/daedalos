#!/usr/bin/env bash
# signals.sh - Agent coordination primitives
#
# Provides completion signals, locks, and task claims for multi-agent coordination.
# All state is stored in the filesystem for simplicity and debuggability.

# Prevent double-sourcing
[[ -n "${_AGENT_SIGNALS_LOADED:-}" ]] && return 0
_AGENT_SIGNALS_LOADED=1

SIGNALS_DIR="${DATA_DIR}/signals"
LOCKS_DIR="${DATA_DIR}/locks"
CLAIMS_DIR="${DATA_DIR}/claims"
mkdir -p "$SIGNALS_DIR" "$LOCKS_DIR" "$CLAIMS_DIR"

# ============================================================================
# COMPLETION SIGNALS
# ============================================================================

# Signal that work is complete with optional data
# Usage: signal_complete <agent_name> <status> [data_file]
# Status: success, failure, blocked
signal_complete() {
    local agent_name="$1"
    local status="${2:-success}"
    local data_file="${3:-}"

    local signal_dir="${SIGNALS_DIR}/${agent_name}"
    mkdir -p "$signal_dir"

    local timestamp
    timestamp=$(iso_timestamp)

    # Create signal file
    local signal_file="${signal_dir}/completion.json"
    local data_content=""
    if [[ -n "$data_file" ]] && [[ -f "$data_file" ]]; then
        data_content=$(cat "$data_file" | jq -Rs '.')
    else
        data_content='null'
    fi

    cat > "$signal_file" << EOF
{
    "agent": "$agent_name",
    "status": "$status",
    "timestamp": "$timestamp",
    "data": $data_content
}
EOF

    debug "Agent $agent_name signaled completion: $status"

    # Run on_complete hooks
    if type hooks_on_complete &>/dev/null; then
        hooks_on_complete "$agent_name" "$status" "$data_file"
    fi
}

# Check if an agent has signaled completion
# Usage: signal_check <agent_name>
# Returns: 0 if complete, 1 if still working
signal_check() {
    local agent_name="$1"
    local signal_file="${SIGNALS_DIR}/${agent_name}/completion.json"

    [[ -f "$signal_file" ]]
}

# Get completion signal data
# Usage: signal_get <agent_name>
signal_get() {
    local agent_name="$1"
    local signal_file="${SIGNALS_DIR}/${agent_name}/completion.json"

    if [[ -f "$signal_file" ]]; then
        cat "$signal_file"
    else
        echo 'null'
    fi
}

# Clear completion signal (for reuse)
# Usage: signal_clear <agent_name>
signal_clear() {
    local agent_name="$1"
    local signal_file="${SIGNALS_DIR}/${agent_name}/completion.json"

    if [[ -f "$signal_file" ]]; then
        rm -f "$signal_file"
    fi
}

# Wait for agent to complete
# Usage: signal_wait <agent_name> [timeout_seconds] [poll_interval]
# Returns: 0 on completion, 1 on timeout
signal_wait() {
    local agent_name="$1"
    local timeout="${2:-600}"  # Default 10 minutes
    local interval="${3:-5}"   # Default 5 seconds

    local elapsed=0
    while [[ $elapsed -lt $timeout ]]; do
        if signal_check "$agent_name"; then
            return 0
        fi
        sleep "$interval"
        elapsed=$((elapsed + interval))
    done

    warn "Timeout waiting for agent: $agent_name"
    return 1
}

# Wait for multiple agents to complete
# Usage: signal_wait_all <timeout> <agent1> <agent2> ...
signal_wait_all() {
    local timeout="$1"
    shift
    local agents=("$@")

    local interval=5
    local elapsed=0

    while [[ $elapsed -lt $timeout ]]; do
        local all_done=true
        for agent in "${agents[@]}"; do
            if ! signal_check "$agent"; then
                all_done=false
                break
            fi
        done

        if [[ "$all_done" == "true" ]]; then
            return 0
        fi

        sleep "$interval"
        elapsed=$((elapsed + interval))
    done

    warn "Timeout waiting for agents: ${agents[*]}"
    return 1
}

# ============================================================================
# RESOURCE LOCKS
# ============================================================================

# Acquire a lock on a resource
# Usage: lock_acquire <lock_name> <owner> [timeout_seconds]
# Returns: 0 if acquired, 1 if failed
lock_acquire() {
    local lock_name="$1"
    local owner="$2"
    local timeout="${3:-30}"

    local lock_file="${LOCKS_DIR}/${lock_name}.lock"
    local lock_meta="${LOCKS_DIR}/${lock_name}.json"

    local elapsed=0
    while [[ $elapsed -lt $timeout ]]; do
        # Try to create lock atomically
        if mkdir "${lock_file}.dir" 2>/dev/null; then
            # Got the lock
            local timestamp
            timestamp=$(iso_timestamp)

            cat > "$lock_meta" << EOF
{
    "lock": "$lock_name",
    "owner": "$owner",
    "acquired": "$timestamp",
    "pid": $$
}
EOF
            touch "$lock_file"
            debug "Lock acquired: $lock_name by $owner"
            return 0
        fi

        # Check if lock holder is still alive
        if [[ -f "$lock_meta" ]]; then
            local holder_pid
            holder_pid=$(jq -r '.pid' "$lock_meta")
            if [[ -n "$holder_pid" ]] && ! kill -0 "$holder_pid" 2>/dev/null; then
                # Lock holder is dead, clean up stale lock
                warn "Cleaning stale lock: $lock_name"
                rm -rf "${lock_file}.dir" "$lock_file" "$lock_meta"
                continue
            fi
        fi

        sleep 1
        elapsed=$((elapsed + 1))
    done

    warn "Failed to acquire lock: $lock_name (timeout)"
    return 1
}

# Release a lock
# Usage: lock_release <lock_name> <owner>
lock_release() {
    local lock_name="$1"
    local owner="$2"

    local lock_file="${LOCKS_DIR}/${lock_name}.lock"
    local lock_meta="${LOCKS_DIR}/${lock_name}.json"

    # Verify ownership
    if [[ -f "$lock_meta" ]]; then
        local current_owner
        current_owner=$(jq -r '.owner' "$lock_meta")
        if [[ "$current_owner" != "$owner" ]]; then
            warn "Cannot release lock $lock_name: not owner (owned by $current_owner)"
            return 1
        fi
    fi

    rm -rf "${lock_file}.dir" "$lock_file" "$lock_meta"
    debug "Lock released: $lock_name by $owner"
}

# Check if a lock is held
# Usage: lock_check <lock_name>
lock_check() {
    local lock_name="$1"
    local lock_file="${LOCKS_DIR}/${lock_name}.lock"

    [[ -f "$lock_file" ]]
}

# Get lock info
# Usage: lock_info <lock_name>
lock_info() {
    local lock_name="$1"
    local lock_meta="${LOCKS_DIR}/${lock_name}.json"

    if [[ -f "$lock_meta" ]]; then
        cat "$lock_meta"
    else
        echo 'null'
    fi
}

# List all active locks
lock_list() {
    local as_json="${1:-false}"

    if [[ "$as_json" == "true" ]]; then
        local result="["
        local first=true
        for meta in "$LOCKS_DIR"/*.json; do
            [[ ! -f "$meta" ]] && continue
            if [[ "$first" == "true" ]]; then
                first=false
            else
                result+=","
            fi
            result+=$(cat "$meta")
        done
        result+="]"
        echo "$result"
    else
        echo "${C_BOLD}Active Locks:${C_RESET}"
        for meta in "$LOCKS_DIR"/*.json; do
            [[ ! -f "$meta" ]] && continue
            local name owner acquired
            name=$(jq -r '.lock' "$meta")
            owner=$(jq -r '.owner' "$meta")
            acquired=$(jq -r '.acquired' "$meta")
            echo "  ${C_CYAN}${name}${C_RESET} - owned by $owner (since $acquired)"
        done
    fi
}

# ============================================================================
# TASK CLAIMS
# ============================================================================

# Claim a task (prevent others from working on it)
# Usage: claim_create <task_id> <agent_name> [description]
claim_create() {
    local task_id="$1"
    local agent_name="$2"
    local description="${3:-}"

    local claim_file="${CLAIMS_DIR}/${task_id}.json"

    if [[ -f "$claim_file" ]]; then
        local current_owner
        current_owner=$(jq -r '.agent' "$claim_file")
        if [[ "$current_owner" != "$agent_name" ]]; then
            die "Task already claimed by: $current_owner"
        fi
        return 0  # Already own it
    fi

    local timestamp
    timestamp=$(iso_timestamp)

    cat > "$claim_file" << EOF
{
    "task_id": "$task_id",
    "agent": "$agent_name",
    "description": "$description",
    "claimed": "$timestamp",
    "status": "active"
}
EOF

    debug "Task claimed: $task_id by $agent_name"
    success "Claimed task: $task_id"
}

# Release a task claim
# Usage: claim_release <task_id> <agent_name> [status]
claim_release() {
    local task_id="$1"
    local agent_name="$2"
    local status="${3:-completed}"

    local claim_file="${CLAIMS_DIR}/${task_id}.json"

    if [[ ! -f "$claim_file" ]]; then
        warn "No claim found for task: $task_id"
        return 1
    fi

    local current_owner
    current_owner=$(jq -r '.agent' "$claim_file")
    if [[ "$current_owner" != "$agent_name" ]]; then
        warn "Cannot release claim: not owner"
        return 1
    fi

    # Update status and archive
    local timestamp
    timestamp=$(iso_timestamp)
    local tmp="${claim_file}.tmp.$$"
    jq --arg status "$status" --arg ts "$timestamp" \
       '.status = $status | .released = $ts' "$claim_file" > "$tmp" && mv "$tmp" "$claim_file"

    # Move to archive
    mkdir -p "${CLAIMS_DIR}/archive"
    mv "$claim_file" "${CLAIMS_DIR}/archive/"

    debug "Task released: $task_id ($status)"
}

# Check if a task is claimed
# Usage: claim_check <task_id>
claim_check() {
    local task_id="$1"
    local claim_file="${CLAIMS_DIR}/${task_id}.json"

    [[ -f "$claim_file" ]]
}

# Get claim info
# Usage: claim_get <task_id>
claim_get() {
    local task_id="$1"
    local claim_file="${CLAIMS_DIR}/${task_id}.json"

    if [[ -f "$claim_file" ]]; then
        cat "$claim_file"
    else
        echo 'null'
    fi
}

# List all active claims
claim_list() {
    local as_json="${1:-false}"
    local agent_filter="${2:-}"

    if [[ "$as_json" == "true" ]]; then
        local result="["
        local first=true
        for claim_file in "$CLAIMS_DIR"/*.json; do
            [[ ! -f "$claim_file" ]] && continue
            [[ "$(basename "$claim_file")" == "*.json" ]] && continue

            if [[ -n "$agent_filter" ]]; then
                local agent
                agent=$(jq -r '.agent' "$claim_file")
                [[ "$agent" != "$agent_filter" ]] && continue
            fi

            if [[ "$first" == "true" ]]; then
                first=false
            else
                result+=","
            fi
            result+=$(cat "$claim_file")
        done
        result+="]"
        echo "$result"
    else
        echo "${C_BOLD}Active Claims:${C_RESET}"
        for claim_file in "$CLAIMS_DIR"/*.json; do
            [[ ! -f "$claim_file" ]] && continue
            [[ "$(basename "$claim_file")" == "*.json" ]] && continue

            local task_id agent description claimed
            task_id=$(jq -r '.task_id' "$claim_file")
            agent=$(jq -r '.agent' "$claim_file")
            description=$(jq -r '.description // ""' "$claim_file")
            claimed=$(jq -r '.claimed' "$claim_file")

            if [[ -n "$agent_filter" ]] && [[ "$agent" != "$agent_filter" ]]; then
                continue
            fi

            echo "  ${C_CYAN}${task_id}${C_RESET} - $agent"
            [[ -n "$description" ]] && echo "    $description"
            echo "    Claimed: $claimed"
        done
    fi
}

# ============================================================================
# HANDOFF PROTOCOL
# ============================================================================

# Create a handoff request (one agent asks another to continue work)
# Usage: handoff_create <from_agent> <to_agent> <context_file>
handoff_create() {
    local from="$1"
    local to="$2"
    local context_file="$3"

    local handoff_dir="${SIGNALS_DIR}/handoffs"
    mkdir -p "$handoff_dir"

    local handoff_id="handoff-$(date +%s%N | sha256sum | head -c 8)"
    local handoff_file="${handoff_dir}/${handoff_id}.json"

    local context_data='null'
    if [[ -f "$context_file" ]]; then
        context_data=$(cat "$context_file" | jq -Rs '.')
    fi

    local timestamp
    timestamp=$(iso_timestamp)

    cat > "$handoff_file" << EOF
{
    "id": "$handoff_id",
    "from": "$from",
    "to": "$to",
    "context": $context_data,
    "created": "$timestamp",
    "status": "pending"
}
EOF

    # Send message to target agent
    comms_send "$to" "$from" "handoff" "Handoff request: $handoff_id - check ${handoff_file}"

    echo "$handoff_id"
}

# Accept a handoff
# Usage: handoff_accept <handoff_id> <agent_name>
handoff_accept() {
    local handoff_id="$1"
    local agent_name="$2"

    local handoff_file="${SIGNALS_DIR}/handoffs/${handoff_id}.json"

    if [[ ! -f "$handoff_file" ]]; then
        die "Handoff not found: $handoff_id"
    fi

    local to
    to=$(jq -r '.to' "$handoff_file")
    if [[ "$to" != "$agent_name" ]]; then
        die "Handoff not addressed to you"
    fi

    local tmp="${handoff_file}.tmp.$$"
    jq '.status = "accepted"' "$handoff_file" > "$tmp" && mv "$tmp" "$handoff_file"

    # Get context
    jq -r '.context // ""' "$handoff_file"
}

# ============================================================================
# AGENT CLI INTEGRATION
# ============================================================================

# Add signal commands to main CLI
# Called from main agent script

cmd_signal() {
    local action="${1:-}"
    shift || true

    case "$action" in
        complete)
            local agent="${DAEDALOS_AGENT_NAME:-}"
            local status="success"
            local data_file=""

            while [[ $# -gt 0 ]]; do
                case "$1" in
                    --agent|-a) agent="$2"; shift 2 ;;
                    --status|-s) status="$2"; shift 2 ;;
                    --data|-d) data_file="$2"; shift 2 ;;
                    *) shift ;;
                esac
            done

            if [[ -z "$agent" ]]; then
                die "No agent specified. Set DAEDALOS_AGENT_NAME or use --agent"
            fi

            signal_complete "$agent" "$status" "$data_file"
            success "Signaled completion: $agent ($status)"
            ;;

        wait)
            local agent="$1"
            local timeout="${2:-600}"
            if [[ -z "$agent" ]]; then
                die "Usage: agent signal wait <agent_name> [timeout]"
            fi
            if signal_wait "$agent" "$timeout"; then
                signal_get "$agent"
            else
                exit 1
            fi
            ;;

        check)
            local agent="$1"
            if [[ -z "$agent" ]]; then
                die "Usage: agent signal check <agent_name>"
            fi
            if signal_check "$agent"; then
                signal_get "$agent"
            else
                echo "not complete"
                exit 1
            fi
            ;;

        clear)
            local agent="$1"
            if [[ -z "$agent" ]]; then
                die "Usage: agent signal clear <agent_name>"
            fi
            signal_clear "$agent"
            success "Cleared signal for: $agent"
            ;;

        *)
            echo "Usage: agent signal <complete|wait|check|clear>"
            echo ""
            echo "Commands:"
            echo "  complete [--status <status>] [--data <file>]  Signal work complete"
            echo "  wait <agent> [timeout]                         Wait for agent completion"
            echo "  check <agent>                                  Check if agent completed"
            echo "  clear <agent>                                  Clear completion signal"
            ;;
    esac
}

cmd_lock() {
    local action="${1:-}"
    shift || true

    case "$action" in
        acquire)
            local lock_name="$1"
            local owner="${DAEDALOS_AGENT_NAME:-$$}"
            local timeout="${2:-30}"

            if [[ -z "$lock_name" ]]; then
                die "Usage: agent lock acquire <name> [timeout]"
            fi

            if lock_acquire "$lock_name" "$owner" "$timeout"; then
                success "Lock acquired: $lock_name"
            else
                die "Failed to acquire lock: $lock_name"
            fi
            ;;

        release)
            local lock_name="$1"
            local owner="${DAEDALOS_AGENT_NAME:-$$}"

            if [[ -z "$lock_name" ]]; then
                die "Usage: agent lock release <name>"
            fi

            lock_release "$lock_name" "$owner"
            success "Lock released: $lock_name"
            ;;

        check)
            local lock_name="$1"
            if [[ -z "$lock_name" ]]; then
                die "Usage: agent lock check <name>"
            fi
            if lock_check "$lock_name"; then
                lock_info "$lock_name"
            else
                echo "unlocked"
            fi
            ;;

        list)
            lock_list "${1:-false}"
            ;;

        *)
            echo "Usage: agent lock <acquire|release|check|list>"
            ;;
    esac
}

cmd_claim() {
    local action="${1:-}"
    shift || true

    case "$action" in
        create|take)
            local task_id="$1"
            local agent="${DAEDALOS_AGENT_NAME:-}"
            local description="${2:-}"

            if [[ -z "$task_id" ]]; then
                die "Usage: agent claim create <task_id> [description]"
            fi
            if [[ -z "$agent" ]]; then
                agent="user-$$"
            fi

            claim_create "$task_id" "$agent" "$description"
            ;;

        release|done)
            local task_id="$1"
            local agent="${DAEDALOS_AGENT_NAME:-}"
            local status="${2:-completed}"

            if [[ -z "$task_id" ]]; then
                die "Usage: agent claim release <task_id> [status]"
            fi
            if [[ -z "$agent" ]]; then
                agent="user-$$"
            fi

            claim_release "$task_id" "$agent" "$status"
            success "Released claim: $task_id ($status)"
            ;;

        check)
            local task_id="$1"
            if [[ -z "$task_id" ]]; then
                die "Usage: agent claim check <task_id>"
            fi
            if claim_check "$task_id"; then
                claim_get "$task_id"
            else
                echo "unclaimed"
            fi
            ;;

        list)
            claim_list "${1:-false}" "${2:-}"
            ;;

        *)
            echo "Usage: agent claim <create|release|check|list>"
            ;;
    esac
}
