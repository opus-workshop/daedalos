#!/usr/bin/env bash
# workflow.sh - Agent workflow/pipeline orchestration
#
# Defines and executes multi-agent workflows like:
# explorer -> planner -> implementer -> reviewer

# Prevent double-sourcing
[[ -n "${_AGENT_WORKFLOW_LOADED:-}" ]] && return 0
_AGENT_WORKFLOW_LOADED=1

WORKFLOWS_DIR="${CONFIG_DIR}/workflows"
WORKFLOW_STATE_DIR="${DATA_DIR}/workflow_state"
mkdir -p "$WORKFLOWS_DIR" "$WORKFLOW_STATE_DIR"

# ============================================================================
# BUILT-IN WORKFLOWS
# ============================================================================

# Initialize default workflows
workflow_init_defaults() {
    # Feature implementation workflow
    if [[ ! -f "${WORKFLOWS_DIR}/feature.yaml" ]]; then
        cat > "${WORKFLOWS_DIR}/feature.yaml" << 'EOF'
name: feature
description: Full feature implementation pipeline
stages:
  - name: explore
    template: explorer
    prompt: "Explore the codebase to understand the architecture and patterns relevant to: {task}"
    pass_to_next: "exploration_summary"

  - name: plan
    template: implementer
    prompt: "Based on this exploration:\n{exploration_summary}\n\nCreate a detailed implementation plan for: {task}"
    pass_to_next: "implementation_plan"

  - name: implement
    template: implementer
    prompt: "Following this plan:\n{implementation_plan}\n\nImplement the feature: {task}"
    pass_to_next: "implementation_summary"

  - name: review
    template: reviewer
    prompt: "Review the implementation:\n{implementation_summary}\n\nCheck for: correctness, edge cases, security issues, and code style."

parallel: false
EOF
    fi

    # Code review workflow
    if [[ ! -f "${WORKFLOWS_DIR}/review.yaml" ]]; then
        cat > "${WORKFLOWS_DIR}/review.yaml" << 'EOF'
name: review
description: Comprehensive code review pipeline
stages:
  - name: correctness
    template: reviewer
    prompt: "Review for correctness and logic errors in: {task}"

  - name: security
    template: debugger
    prompt: "Review for security vulnerabilities in: {task}"

  - name: style
    template: reviewer
    prompt: "Review for code style and best practices in: {task}"

parallel: true
EOF
    fi

    # Bug fix workflow
    if [[ ! -f "${WORKFLOWS_DIR}/bugfix.yaml" ]]; then
        cat > "${WORKFLOWS_DIR}/bugfix.yaml" << 'EOF'
name: bugfix
description: Bug investigation and fix pipeline
stages:
  - name: investigate
    template: debugger
    prompt: "Investigate the root cause of: {task}"
    pass_to_next: "investigation_report"

  - name: fix
    template: implementer
    prompt: "Based on this investigation:\n{investigation_report}\n\nImplement a fix for: {task}"
    pass_to_next: "fix_summary"

  - name: verify
    template: reviewer
    prompt: "Verify the fix:\n{fix_summary}\n\nEnsure the bug is properly resolved and no regressions introduced."

parallel: false
EOF
    fi
}

# ============================================================================
# WORKFLOW MANAGEMENT
# ============================================================================

# List available workflows
workflow_list() {
    local as_json="${1:-false}"

    workflow_init_defaults

    if [[ "$as_json" == "true" ]]; then
        local workflows="["
        local first=true
        for file in "$WORKFLOWS_DIR"/*.yaml; do
            [[ ! -f "$file" ]] && continue
            local name desc
            name=$(basename "$file" .yaml)
            desc=$(grep "^description:" "$file" | sed 's/description: *//')

            if [[ "$first" == "true" ]]; then
                first=false
            else
                workflows+=","
            fi
            workflows+="{\"name\":\"$name\",\"description\":\"$desc\"}"
        done
        workflows+="]"
        echo "$workflows"
    else
        echo "${C_BOLD}Available Workflows:${C_RESET}"
        for file in "$WORKFLOWS_DIR"/*.yaml; do
            [[ ! -f "$file" ]] && continue
            local name desc
            name=$(basename "$file" .yaml)
            desc=$(grep "^description:" "$file" | sed 's/description: *//')
            echo "  ${C_CYAN}${name}${C_RESET} - $desc"
        done
    fi
}

# Show workflow details
workflow_show() {
    local name="$1"
    local file="${WORKFLOWS_DIR}/${name}.yaml"

    if [[ ! -f "$file" ]]; then
        die "Workflow not found: $name"
    fi

    echo "${C_BOLD}Workflow: ${C_CYAN}${name}${C_RESET}"
    echo ""
    cat "$file"
}

# ============================================================================
# WORKFLOW EXECUTION
# ============================================================================

# Start a workflow
# Usage: workflow_start <workflow_name> <task_description> [--project <path>]
workflow_start() {
    local workflow_name=""
    local task=""
    local project=""

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --project|-p) project="$2"; shift 2 ;;
            -*) die "Unknown option: $1" ;;
            *)
                if [[ -z "$workflow_name" ]]; then
                    workflow_name="$1"
                else
                    task="$1"
                fi
                shift
                ;;
        esac
    done

    workflow_init_defaults

    if [[ -z "$workflow_name" ]] || [[ -z "$task" ]]; then
        die "Usage: agent workflow start <workflow_name> <task_description>"
    fi

    local workflow_file="${WORKFLOWS_DIR}/${workflow_name}.yaml"
    if [[ ! -f "$workflow_file" ]]; then
        die "Workflow not found: $workflow_name"
    fi

    project="${project:-$(pwd)}"

    # Generate workflow instance ID
    local instance_id="wf-$(date +%s%N | sha256sum | head -c 8)"
    local state_file="${WORKFLOW_STATE_DIR}/${instance_id}.json"

    # Parse workflow and create state
    local parallel
    parallel=$(grep "^parallel:" "$workflow_file" | awk '{print $2}')
    parallel="${parallel:-false}"

    # Extract stages
    local -a stages=()
    local current_stage=""
    local in_stages=false

    while IFS= read -r line; do
        if [[ "$line" =~ ^stages: ]]; then
            in_stages=true
            continue
        fi

        if [[ "$in_stages" == "true" ]]; then
            if [[ "$line" =~ ^[[:space:]]*-[[:space:]]*name: ]]; then
                if [[ -n "$current_stage" ]]; then
                    stages+=("$current_stage")
                fi
                current_stage=$(echo "$line" | sed 's/.*name: *//')
            fi
        fi
    done < "$workflow_file"
    [[ -n "$current_stage" ]] && stages+=("$current_stage")

    # Create workflow state
    cat > "$state_file" << EOF
{
    "id": "$instance_id",
    "workflow": "$workflow_name",
    "task": "$task",
    "project": "$project",
    "status": "running",
    "parallel": $parallel,
    "current_stage": 0,
    "stages": $(printf '%s\n' "${stages[@]}" | jq -R . | jq -s .),
    "stage_outputs": {},
    "agents": {},
    "started": "$(iso_timestamp)",
    "completed": null
}
EOF

    info "Starting workflow: $workflow_name"
    info "Instance ID: $instance_id"
    info "Task: $task"

    # Execute workflow
    if [[ "$parallel" == "true" ]]; then
        workflow_execute_parallel "$instance_id"
    else
        workflow_execute_sequential "$instance_id"
    fi

    echo "$instance_id"
}

# Execute workflow stages sequentially
workflow_execute_sequential() {
    local instance_id="$1"
    local state_file="${WORKFLOW_STATE_DIR}/${instance_id}.json"

    if [[ ! -f "$state_file" ]]; then
        die "Workflow state not found: $instance_id"
    fi

    local workflow_name task project
    workflow_name=$(jq -r '.workflow' "$state_file")
    task=$(jq -r '.task' "$state_file")
    project=$(jq -r '.project' "$state_file")

    local workflow_file="${WORKFLOWS_DIR}/${workflow_name}.yaml"

    local stage_count
    stage_count=$(jq '.stages | length' "$state_file")

    local context="{\"task\": \"$task\"}"

    for ((i=0; i<stage_count; i++)); do
        local stage_name
        stage_name=$(jq -r ".stages[$i]" "$state_file")

        # Update current stage
        local tmp="${state_file}.tmp.$$"
        jq --argjson stage "$i" '.current_stage = $stage' "$state_file" > "$tmp" && mv "$tmp" "$state_file"

        info "Stage $((i+1))/$stage_count: $stage_name"

        # Get stage config from workflow file
        local template prompt pass_to_next
        template=$(workflow_get_stage_field "$workflow_file" "$stage_name" "template")
        prompt=$(workflow_get_stage_field "$workflow_file" "$stage_name" "prompt")
        pass_to_next=$(workflow_get_stage_field "$workflow_file" "$stage_name" "pass_to_next")

        # Expand prompt with context
        prompt=$(echo "$prompt" | while IFS= read -r line; do
            echo "$line" | sed "s/{task}/$task/g"
        done)

        # Expand any variables from previous stages
        local -a keys
        mapfile -t keys < <(echo "$context" | jq -r 'keys[]')
        for key in "${keys[@]}"; do
            local value
            value=$(echo "$context" | jq -r ".[\"$key\"]")
            prompt=$(echo "$prompt" | sed "s/{$key}/$value/g")
        done

        # Spawn agent for this stage
        local agent_name="${instance_id}-${stage_name}"

        cmd_spawn -n "$agent_name" -p "$project" -t "${template:-implementer}" --no-focus --prompt "$prompt"

        # Record agent in state
        tmp="${state_file}.tmp.$$"
        jq --arg name "$agent_name" --arg stage "$stage_name" '.agents[$stage] = $name' "$state_file" > "$tmp" && mv "$tmp" "$state_file"

        # Wait for agent to complete (check status periodically)
        info "  Waiting for $stage_name to complete..."

        local timeout=600  # 10 minute timeout per stage
        local elapsed=0

        while [[ $elapsed -lt $timeout ]]; do
            sleep 10
            elapsed=$((elapsed + 10))

            # Check agent status
            local agent_status
            agent_status=$(agents_get "$agent_name" | jq -r '.status // "unknown"')

            if [[ "$agent_status" == "idle" || "$agent_status" == "waiting" ]]; then
                break
            fi

            if [[ "$agent_status" == "error" || "$agent_status" == "dead" ]]; then
                warn "  Stage $stage_name failed"
                break
            fi
        done

        # Capture output if pass_to_next is specified
        if [[ -n "$pass_to_next" ]]; then
            # For now, we'll use a placeholder - in production this would capture actual output
            local stage_output="Stage $stage_name completed successfully"

            tmp="${state_file}.tmp.$$"
            jq --arg key "$pass_to_next" --arg value "$stage_output" '.stage_outputs[$key] = $value' "$state_file" > "$tmp" && mv "$tmp" "$state_file"

            context=$(echo "$context" | jq --arg key "$pass_to_next" --arg value "$stage_output" '. + {($key): $value}')
        fi

        success "  Completed: $stage_name"
    done

    # Mark workflow complete
    tmp="${state_file}.tmp.$$"
    jq '.status = "completed" | .completed = "'"$(iso_timestamp)"'"' "$state_file" > "$tmp" && mv "$tmp" "$state_file"

    success "Workflow completed: $workflow_name"
}

# Execute workflow stages in parallel
workflow_execute_parallel() {
    local instance_id="$1"
    local state_file="${WORKFLOW_STATE_DIR}/${instance_id}.json"

    if [[ ! -f "$state_file" ]]; then
        die "Workflow state not found: $instance_id"
    fi

    local workflow_name task project
    workflow_name=$(jq -r '.workflow' "$state_file")
    task=$(jq -r '.task' "$state_file")
    project=$(jq -r '.project' "$state_file")

    local workflow_file="${WORKFLOWS_DIR}/${workflow_name}.yaml"

    local -a stage_names
    mapfile -t stage_names < <(jq -r '.stages[]' "$state_file")

    info "Spawning ${#stage_names[@]} agents in parallel"

    # Spawn all agents
    for stage_name in "${stage_names[@]}"; do
        local template prompt
        template=$(workflow_get_stage_field "$workflow_file" "$stage_name" "template")
        prompt=$(workflow_get_stage_field "$workflow_file" "$stage_name" "prompt")
        prompt=$(echo "$prompt" | sed "s/{task}/$task/g")

        local agent_name="${instance_id}-${stage_name}"

        cmd_spawn -n "$agent_name" -p "$project" -t "${template:-implementer}" --no-focus --prompt "$prompt"

        # Record agent in state
        local tmp="${state_file}.tmp.$$"
        jq --arg name "$agent_name" --arg stage "$stage_name" '.agents[$stage] = $name' "$state_file" > "$tmp" && mv "$tmp" "$state_file"
    done

    success "All stages spawned in parallel"
    info "Use 'agent workflow status $instance_id' to monitor progress"
}

# Get a field from a stage in workflow YAML
workflow_get_stage_field() {
    local file="$1"
    local stage_name="$2"
    local field="$3"

    # Simple YAML parsing - find the stage and extract field
    awk -v stage="$stage_name" -v field="$field" '
        /^[[:space:]]*- name:/ {
            in_stage = ($NF == stage)
        }
        in_stage && $1 == field":" {
            gsub(/^[^:]+: */, "")
            print
            exit
        }
    ' "$file"
}

# Check workflow status
workflow_status() {
    local instance_id="$1"

    if [[ -z "$instance_id" ]]; then
        # List all active workflows
        echo "${C_BOLD}Active Workflows:${C_RESET}"
        for state_file in "$WORKFLOW_STATE_DIR"/*.json; do
            [[ ! -f "$state_file" ]] && continue

            local id status workflow task
            id=$(jq -r '.id' "$state_file")
            status=$(jq -r '.status' "$state_file")
            workflow=$(jq -r '.workflow' "$state_file")
            task=$(jq -r '.task' "$state_file" | head -c 50)

            local status_color
            case "$status" in
                running)   status_color="${C_GREEN}" ;;
                completed) status_color="${C_BLUE}" ;;
                failed)    status_color="${C_RED}" ;;
                *)         status_color="${C_WHITE}" ;;
            esac

            echo "  ${C_CYAN}${id}${C_RESET} [${status_color}${status}${C_RESET}] $workflow: $task..."
        done
        return
    fi

    local state_file="${WORKFLOW_STATE_DIR}/${instance_id}.json"

    if [[ ! -f "$state_file" ]]; then
        die "Workflow not found: $instance_id"
    fi

    echo "${C_BOLD}Workflow Status: ${C_CYAN}${instance_id}${C_RESET}"
    echo ""

    local workflow status task started current_stage
    workflow=$(jq -r '.workflow' "$state_file")
    status=$(jq -r '.status' "$state_file")
    task=$(jq -r '.task' "$state_file")
    started=$(jq -r '.started' "$state_file")
    current_stage=$(jq -r '.current_stage' "$state_file")

    echo "Workflow: $workflow"
    echo "Status: $status"
    echo "Task: $task"
    echo "Started: $started"
    echo ""

    echo "${C_BOLD}Stages:${C_RESET}"
    local -a stages
    mapfile -t stages < <(jq -r '.stages[]' "$state_file")

    for i in "${!stages[@]}"; do
        local stage="${stages[$i]}"
        local agent_name
        agent_name=$(jq -r ".agents[\"$stage\"] // \"not started\"" "$state_file")

        local indicator
        if [[ $i -lt $current_stage ]]; then
            indicator="${C_GREEN}[done]${C_RESET}"
        elif [[ $i -eq $current_stage ]]; then
            indicator="${C_YELLOW}[running]${C_RESET}"
        else
            indicator="${C_DIM}[pending]${C_RESET}"
        fi

        echo "  $indicator $stage ($agent_name)"
    done
}

# Stop a workflow
workflow_stop() {
    local instance_id="$1"
    local force="${2:-false}"

    local state_file="${WORKFLOW_STATE_DIR}/${instance_id}.json"

    if [[ ! -f "$state_file" ]]; then
        die "Workflow not found: $instance_id"
    fi

    info "Stopping workflow: $instance_id"

    # Kill all associated agents
    local -a agent_names
    mapfile -t agent_names < <(jq -r '.agents[]' "$state_file")

    for agent_name in "${agent_names[@]}"; do
        [[ -z "$agent_name" ]] && continue
        if agents_exists "$agent_name"; then
            kill_agent "$agent_name" "$force"
        fi
    done

    # Update state
    local tmp="${state_file}.tmp.$$"
    jq '.status = "stopped"' "$state_file" > "$tmp" && mv "$tmp" "$state_file"

    success "Workflow stopped: $instance_id"
}
