#!/usr/bin/env bash
#===============================================================================
# output.sh - Output formatting for verify
#===============================================================================

#-------------------------------------------------------------------------------
# Color Setup
#-------------------------------------------------------------------------------

setup_colors() {
    if [[ -t 1 ]] && [[ -z "${NO_COLOR:-}" ]]; then
        RED='\033[0;31m'
        GREEN='\033[0;32m'
        YELLOW='\033[0;33m'
        BLUE='\033[0;34m'
        CYAN='\033[0;36m'
        BOLD='\033[1m'
        DIM='\033[2m'
        RESET='\033[0m'
    else
        RED=''
        GREEN=''
        YELLOW=''
        BLUE=''
        CYAN=''
        BOLD=''
        DIM=''
        RESET=''
    fi
}

# Call setup immediately
setup_colors

#-------------------------------------------------------------------------------
# Step Output
#-------------------------------------------------------------------------------

print_step_start() {
    local name="$1"

    if [[ "${QUIET:-false}" != "true" ]] && [[ "${JSON:-false}" != "true" ]]; then
        printf "${DIM}[     ]${RESET} %s..." "$name"
    fi
}

print_step_result() {
    local name="$1"
    local exit_code="$2"
    local duration_ms="$3"
    local extra="${4:-}"

    if [[ "${QUIET:-false}" == "true" ]] || [[ "${JSON:-false}" == "true" ]]; then
        return
    fi

    # Format duration
    local duration_str
    if [[ $duration_ms -lt 1000 ]]; then
        duration_str="${duration_ms}ms"
    else
        duration_str=$(printf "%.1fs" "$(echo "scale=1; $duration_ms / 1000" | bc 2>/dev/null || echo "$((duration_ms / 1000))")")
    fi

    # Clear the "running" line and print result
    printf "\r"

    if [[ $exit_code -eq 0 ]]; then
        printf "[${DIM}%5s${RESET}] ${GREEN}ok${RESET} %s" "$duration_str" "$name"
        if [[ -n "$extra" ]]; then
            printf " ${DIM}%s${RESET}" "$extra"
        fi
        printf "\n"
    else
        printf "[${DIM}%5s${RESET}] ${RED}FAIL${RESET} %s\n" "$duration_str" "$name"
    fi
}

print_step_errors() {
    local output="$1"
    local max_lines="${2:-10}"

    if [[ "${QUIET:-false}" == "true" ]] || [[ "${JSON:-false}" == "true" ]]; then
        return
    fi

    if [[ -z "$output" ]]; then
        return
    fi

    # Indent and dim the error output
    local line_count=0
    while IFS= read -r line && [[ $line_count -lt $max_lines ]]; do
        printf "     ${DIM}%s${RESET}\n" "$line"
        ((line_count++))
    done <<< "$output"

    local total_lines
    total_lines=$(echo "$output" | wc -l | tr -d ' ')
    if [[ $total_lines -gt $max_lines ]]; then
        printf "     ${DIM}... and %d more lines${RESET}\n" "$((total_lines - max_lines))"
    fi
}

print_step_skipped() {
    local name="$1"
    local reason="${2:-skipped}"

    if [[ "${QUIET:-false}" == "true" ]] || [[ "${JSON:-false}" == "true" ]]; then
        return
    fi

    printf "[${DIM}  -  ${RESET}] ${YELLOW}skip${RESET} %s ${DIM}(%s)${RESET}\n" "$name" "$reason"
}

#-------------------------------------------------------------------------------
# Summary Output
#-------------------------------------------------------------------------------

print_separator() {
    if [[ "${QUIET:-false}" != "true" ]] && [[ "${JSON:-false}" != "true" ]]; then
        echo "----------------------------"
    fi
}

print_summary() {
    local duration_ms="$1"
    local passed="$2"
    local step_count="${3:-0}"
    local error_count="${4:-0}"

    if [[ "${QUIET:-false}" == "true" ]] || [[ "${JSON:-false}" == "true" ]]; then
        return
    fi

    # Format duration
    local duration_str
    if [[ $duration_ms -lt 1000 ]]; then
        duration_str="${duration_ms}ms"
    else
        duration_str=$(printf "%.1fs" "$(echo "scale=1; $duration_ms / 1000" | bc 2>/dev/null || echo "$((duration_ms / 1000))")")
    fi

    print_separator

    if [[ "$passed" == "true" ]]; then
        printf "Total: ${BOLD}%s${RESET} ${GREEN}All checks passed${RESET}\n" "$duration_str"
    else
        printf "Total: ${BOLD}%s${RESET} ${RED}%d error(s)${RESET}\n" "$duration_str" "$error_count"
    fi
}

#-------------------------------------------------------------------------------
# JSON Output
#-------------------------------------------------------------------------------

# Initialize JSON results array
json_init() {
    JSON_STEPS=()
}

# Add step result to JSON
json_add_step() {
    local name="$1"
    local success="$2"
    local duration_ms="$3"
    local output="${4:-}"

    # Escape output for JSON
    local escaped_output
    escaped_output=$(echo "$output" | jq -Rs '.' 2>/dev/null || echo '""')

    JSON_STEPS+=("{\"name\":\"$name\",\"success\":$success,\"duration_ms\":$duration_ms,\"output\":$escaped_output}")
}

# Print final JSON result
json_print() {
    local total_duration="$1"
    local success="$2"

    local steps_json
    steps_json=$(IFS=,; echo "${JSON_STEPS[*]}")

    cat << EOF
{
  "success": $success,
  "duration_ms": $total_duration,
  "steps": [$steps_json]
}
EOF
}

#-------------------------------------------------------------------------------
# Status Display
#-------------------------------------------------------------------------------

print_status() {
    local project_type="$1"
    local pipeline_file="$2"
    local last_run="${3:-never}"
    local last_result="${4:-unknown}"

    echo "Verify Status"
    echo "----------------------------"
    echo "Project type: $project_type"
    echo "Pipeline: $pipeline_file"
    echo "Last run: $last_run"
    echo "Last result: $last_result"
    echo ""
    echo "Available steps:"

    if [[ -f "$pipeline_file" ]]; then
        get_pipeline_steps "$pipeline_file" | while read -r step; do
            local is_quick
            if is_quick_step "$pipeline_file" "$step"; then
                echo "  - $step (quick)"
            else
                echo "  - $step"
            fi
        done
    fi
}

#-------------------------------------------------------------------------------
# Pipeline List
#-------------------------------------------------------------------------------

print_pipelines() {
    local pipelines_dir="$1"

    echo "Available Pipelines"
    echo "----------------------------"

    for pipeline in "$pipelines_dir"/*.yaml; do
        if [[ -f "$pipeline" ]]; then
            local name
            name=$(basename "$pipeline" .yaml)

            local desc
            desc=$(yaml_get "$pipeline" ".description" 2>/dev/null || echo "")

            printf "  ${BOLD}%-12s${RESET} %s\n" "$name" "$desc"
        fi
    done
}
