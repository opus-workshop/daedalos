#!/usr/bin/env bash
#===============================================================================
# watch.sh - Watch mode for verify
#===============================================================================

#-------------------------------------------------------------------------------
# Watch Mode
#-------------------------------------------------------------------------------

run_watch_mode() {
    local project_name
    project_name=$(basename "$(pwd)")

    # Check for fswatch
    if ! has_command fswatch; then
        if [[ "$(uname)" == "Darwin" ]]; then
            die "Watch mode requires fswatch. Install with: brew install fswatch"
        else
            die "Watch mode requires fswatch. Install with your package manager."
        fi
    fi

    # Print header
    print_watch_header "$project_name"

    # Initial run in quick mode
    echo ""
    QUICK=true
    run_verification_quiet || true

    echo ""
    print_watch_status "watching"

    # Set up key handling in background
    handle_watch_keys &
    local key_handler_pid=$!

    # Watch for changes
    fswatch -o . \
        --exclude '\.git' \
        --exclude 'node_modules' \
        --exclude 'target' \
        --exclude 'build' \
        --exclude '__pycache__' \
        --exclude '\.pytest_cache' \
        --exclude '\.mypy_cache' \
        --exclude 'dist' \
        --exclude '\.DS_Store' \
        --latency 0.5 | while read -r _; do
        clear_watch_line
        echo "Changes detected, verifying..."
        echo ""
        QUICK=true
        run_verification_quiet || true
        echo ""
        print_watch_status "watching"
    done

    # Clean up
    kill "$key_handler_pid" 2>/dev/null || true
}

#-------------------------------------------------------------------------------
# Watch UI
#-------------------------------------------------------------------------------

print_watch_header() {
    local name="$1"

    clear
    cat << EOF
+------------------------------------------------------------------+
| ${BOLD}VERIFY WATCH${RESET}: $name
| ${DIM}[Enter] Full verify | [Q] Quit | [F] Fix mode${RESET}
+------------------------------------------------------------------+
EOF
}

print_watch_status() {
    local status="$1"
    local timestamp
    timestamp=$(date +%H:%M:%S)

    case "$status" in
        watching)
            printf "${DIM}[%s] Watching for changes...${RESET}" "$timestamp"
            ;;
        running)
            printf "${CYAN}[%s] Running verification...${RESET}" "$timestamp"
            ;;
        passed)
            printf "${GREEN}[%s] All checks passed${RESET}" "$timestamp"
            ;;
        failed)
            printf "${RED}[%s] Verification failed${RESET}" "$timestamp"
            ;;
    esac
}

clear_watch_line() {
    printf "\r\033[K"  # Clear current line
}

#-------------------------------------------------------------------------------
# Key Handling
#-------------------------------------------------------------------------------

handle_watch_keys() {
    while true; do
        read -rsn1 key

        case "$key" in
            q|Q)
                echo ""
                echo "Stopping watch mode..."
                # Kill parent process group
                kill 0 2>/dev/null
                exit 0
                ;;
            "")  # Enter key
                clear_watch_line
                echo "Running full verification..."
                echo ""
                QUICK=false
                run_verification_quiet || true
                echo ""
                print_watch_status "watching"
                ;;
            f|F)
                clear_watch_line
                echo "Running with --fix..."
                echo ""
                FIX=true QUICK=true
                run_verification_quiet || true
                echo ""
                print_watch_status "watching"
                ;;
        esac
    done
}

#-------------------------------------------------------------------------------
# Quiet Verification (for watch mode)
#-------------------------------------------------------------------------------

run_verification_quiet() {
    # Run verification (it handles its own output)
    run_verification
    local result=$?
    return $result
}
