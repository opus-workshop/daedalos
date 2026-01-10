#!/usr/bin/env bash
#===============================================================================
# runner.sh - Step execution for verify
#===============================================================================

#-------------------------------------------------------------------------------
# Variable Substitution
#-------------------------------------------------------------------------------

substitute_vars() {
    local cmd="$1"

    # Substitute <scheme> with detected Xcode scheme
    if [[ "$cmd" == *"<scheme>"* ]]; then
        local scheme
        scheme=$(detect_xcode_scheme)
        if [[ -z "$scheme" ]]; then
            warn "Could not detect Xcode scheme"
            scheme="Unknown"
        fi
        cmd="${cmd//<scheme>/$scheme}"
    fi

    # Substitute <pm> with detected package manager
    if [[ "$cmd" == *"<pm>"* ]]; then
        local pm
        pm=$(detect_package_manager)
        pm="${pm:-npm}"
        cmd="${cmd//<pm>/$pm}"
    fi

    # Substitute <project> with project name
    if [[ "$cmd" == *"<project>"* ]]; then
        local project
        project=$(basename "$(pwd)")
        cmd="${cmd//<project>/$project}"
    fi

    echo "$cmd"
}

#-------------------------------------------------------------------------------
# Staged File Adaptation
#-------------------------------------------------------------------------------

adapt_for_staged() {
    local cmd="$1"

    # Get staged files
    local staged
    staged=$(git diff --cached --name-only 2>/dev/null | tr '\n' ' ')

    if [[ -z "${staged// }" ]]; then
        # No staged files - return a passing command
        echo "true"
        return
    fi

    # Adapt command based on tool
    case "$cmd" in
        *eslint*)
            # Filter to .js/.ts/.jsx/.tsx files
            local js_files
            js_files=$(echo "$staged" | tr ' ' '\n' | grep -E '\.(js|jsx|ts|tsx)$' | tr '\n' ' ')
            if [[ -n "${js_files// }" ]]; then
                echo "npx eslint $js_files"
            else
                echo "true"
            fi
            ;;
        *ruff*)
            # Filter to .py files
            local py_files
            py_files=$(echo "$staged" | tr ' ' '\n' | grep -E '\.py$' | tr '\n' ' ')
            if [[ -n "${py_files// }" ]]; then
                echo "ruff check $py_files"
            else
                echo "true"
            fi
            ;;
        *swiftlint*)
            # Filter to .swift files
            local swift_files
            swift_files=$(echo "$staged" | tr ' ' '\n' | grep -E '\.swift$' | tr '\n' ' ')
            if [[ -n "${swift_files// }" ]]; then
                echo "swiftlint lint $swift_files"
            else
                echo "true"
            fi
            ;;
        *cargo\ clippy*)
            # Clippy doesn't support file list, run on all
            echo "$cmd"
            ;;
        *)
            echo "$cmd"
            ;;
    esac
}

#-------------------------------------------------------------------------------
# Step Execution
#-------------------------------------------------------------------------------

run_step() {
    local pipeline_file="$1"
    local step_name="$2"
    local fix_mode="${FIX:-false}"
    local staged_mode="${STAGED:-false}"

    # Get step command
    local cmd
    if [[ "$fix_mode" == "true" ]]; then
        cmd=$(get_step_config "$pipeline_file" "$step_name" "fix_command")
    fi
    if [[ -z "$cmd" ]]; then
        cmd=$(get_step_config "$pipeline_file" "$step_name" "command")
    fi

    if [[ -z "$cmd" ]]; then
        warn "No command found for step: $step_name"
        return 1
    fi

    # Get timeout
    local timeout
    timeout=$(get_step_config "$pipeline_file" "$step_name" "timeout")
    timeout="${timeout:-60}"

    # Substitute variables
    cmd=$(substitute_vars "$cmd")

    # Adapt for staged mode
    if [[ "$staged_mode" == "true" ]]; then
        cmd=$(adapt_for_staged "$cmd")
    fi

    debug "Running: $cmd"

    # Run command
    local output exit_code
    output=$(run_with_timeout "$timeout" bash -c "$cmd" 2>&1) || exit_code=$?
    exit_code=${exit_code:-0}

    # Return results via global variables (bash limitation)
    STEP_OUTPUT="$output"
    STEP_EXIT_CODE=$exit_code

    return $exit_code
}

#-------------------------------------------------------------------------------
# Main Verification
#-------------------------------------------------------------------------------

run_verification() {
    local start_time
    start_time=$(now_ms)

    # Detect or use specified pipeline
    local project_type
    if [[ -n "${PIPELINE:-}" ]]; then
        project_type="$PIPELINE"
    else
        project_type=$(detect_project_type)
    fi

    if [[ "$project_type" == "unknown" ]]; then
        die "Could not detect project type. Use --pipeline to specify."
    fi

    info "Detected project type: $project_type"

    # Load pipeline
    local pipeline_file
    pipeline_file=$(load_pipeline "$project_type")
    info "Using pipeline: $pipeline_file"

    # Get steps
    local steps
    steps=$(get_pipeline_steps "$pipeline_file")

    if [[ -z "$steps" ]]; then
        die "No steps found in pipeline"
    fi

    # Initialize JSON if needed
    if [[ "${JSON:-false}" == "true" ]]; then
        json_init
    fi

    # Run steps
    local all_passed=true
    local error_count=0
    local step_count=0

    while IFS= read -r step_name; do
        [[ -z "$step_name" ]] && continue

        # Check if we should skip this step
        if [[ -n "${ONLY_STEP:-}" ]] && [[ "$step_name" != "$ONLY_STEP" ]]; then
            continue
        fi

        # Check if step is in skip list
        local should_skip=false
        for skip in "${SKIP_STEPS[@]:-}"; do
            if [[ "$step_name" == "$skip" ]]; then
                should_skip=true
                break
            fi
        done

        if [[ "$should_skip" == "true" ]]; then
            print_step_skipped "$step_name" "user skip"
            continue
        fi

        # Check quick mode
        if [[ "${QUICK:-false}" == "true" ]]; then
            if ! is_quick_step "$pipeline_file" "$step_name"; then
                print_step_skipped "$step_name" "not quick"
                continue
            fi
        fi

        ((step_count++))

        # Show progress
        print_step_start "$step_name"

        # Run step
        local step_start
        step_start=$(now_ms)

        if run_step "$pipeline_file" "$step_name"; then
            local step_end
            step_end=$(now_ms)
            local step_duration=$((step_end - step_start))

            print_step_result "$step_name" 0 "$step_duration"

            if [[ "${JSON:-false}" == "true" ]]; then
                json_add_step "$step_name" "true" "$step_duration" "$STEP_OUTPUT"
            fi
        else
            local step_end
            step_end=$(now_ms)
            local step_duration=$((step_end - step_start))

            print_step_result "$step_name" "$STEP_EXIT_CODE" "$step_duration"
            print_step_errors "$STEP_OUTPUT"

            if [[ "${JSON:-false}" == "true" ]]; then
                json_add_step "$step_name" "false" "$step_duration" "$STEP_OUTPUT"
            fi

            all_passed=false
            ((error_count++))
        fi
    done <<< "$steps"

    # Calculate total duration
    local end_time total_duration
    end_time=$(now_ms)
    total_duration=$((end_time - start_time))

    # Output summary
    if [[ "${JSON:-false}" == "true" ]]; then
        json_print "$total_duration" "$all_passed"
    else
        print_summary "$total_duration" "$all_passed" "$step_count" "$error_count"
    fi

    # Save status
    save_verification_status "$project_type" "$all_passed"

    # Return with appropriate code (don't exit, let caller handle)
    if [[ "$all_passed" == "true" ]]; then
        return 0
    else
        return 1
    fi
}

#-------------------------------------------------------------------------------
# Status Persistence
#-------------------------------------------------------------------------------

save_verification_status() {
    local project_type="$1"
    local passed="$2"
    local timestamp
    timestamp=$(date -Iseconds)

    local status_file="${VERIFY_STATE_DIR}/last_status.json"

    cat > "$status_file" << EOF
{
  "project_path": "$(pwd)",
  "project_type": "$project_type",
  "passed": $passed,
  "timestamp": "$timestamp"
}
EOF
}

load_verification_status() {
    local status_file="${VERIFY_STATE_DIR}/last_status.json"

    if [[ -f "$status_file" ]]; then
        cat "$status_file"
    else
        echo "{}"
    fi
}
