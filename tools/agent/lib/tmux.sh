#!/usr/bin/env bash
# tmux.sh - tmux session management for agent CLI
#
# Provides functions for creating, managing, and interacting with tmux sessions.

# Prevent double-sourcing
[[ -n "${_AGENT_TMUX_LOADED:-}" ]] && return 0
_AGENT_TMUX_LOADED=1

# Session name prefix
TMUX_SESSION_PREFIX="claude-agent-"

# Get full session name for an agent
tmux_session_name() {
    local name="$1"
    echo "${TMUX_SESSION_PREFIX}${name}"
}

# Check if a tmux session exists
tmux_session_exists() {
    local session="$1"
    tmux has-session -t "$session" 2>/dev/null
}

# Create a new tmux session
# Usage: tmux_create_session <session_name> <working_dir> [command...]
tmux_create_session() {
    local session="$1"
    local working_dir="$2"
    shift 2
    local cmd=("$@")

    if tmux_session_exists "$session"; then
        warn "Session already exists: $session"
        return 1
    fi

    # Create detached session
    tmux new-session -d -s "$session" -c "$working_dir"

    # Set session options
    tmux set-option -t "$session" remain-on-exit off

    # If command provided, send it to the session
    if [[ ${#cmd[@]} -gt 0 ]]; then
        # Build command string
        local cmd_str="${cmd[*]}"
        tmux send-keys -t "$session" "$cmd_str" Enter
    fi

    return 0
}

# Kill a tmux session
tmux_kill_session() {
    local session="$1"
    local force="${2:-false}"

    if ! tmux_session_exists "$session"; then
        debug "Session does not exist: $session"
        return 0
    fi

    if [[ "$force" != "true" ]]; then
        # Try graceful shutdown first
        tmux send-keys -t "$session" C-c
        sleep 0.5
    fi

    tmux kill-session -t "$session" 2>/dev/null
}

# Focus (attach to) a tmux session
tmux_focus_session() {
    local session="$1"

    if ! tmux_session_exists "$session"; then
        die "Session does not exist: $session"
    fi

    # If already in tmux, switch client
    if [[ -n "${TMUX:-}" ]]; then
        tmux switch-client -t "$session"
    else
        tmux attach-session -t "$session"
    fi
}

# Get the content of a tmux pane
# Usage: tmux_get_pane_content <session> [lines]
tmux_get_pane_content() {
    local session="$1"
    local lines="${2:-100}"

    if ! tmux_session_exists "$session"; then
        return 1
    fi

    tmux capture-pane -t "$session" -p -S "-${lines}"
}

# Get full scrollback buffer
tmux_get_scrollback() {
    local session="$1"

    if ! tmux_session_exists "$session"; then
        return 1
    fi

    tmux capture-pane -t "$session" -p -S -
}

# Send keys to a tmux session
tmux_send_keys() {
    local session="$1"
    shift
    local keys="$*"

    if ! tmux_session_exists "$session"; then
        return 1
    fi

    tmux send-keys -t "$session" $keys
}

# Get the PID of the main process in the tmux pane
tmux_get_pane_pid() {
    local session="$1"

    if ! tmux_session_exists "$session"; then
        return 1
    fi

    # Get the pane's shell PID
    local pane_pid
    pane_pid=$(tmux display-message -t "$session" -p '#{pane_pid}')

    if [[ -n "$pane_pid" ]]; then
        # Get child process (the actual command running)
        local child_pid
        if [[ "$OSTYPE" == "darwin"* ]]; then
            child_pid=$(pgrep -P "$pane_pid" 2>/dev/null | head -1)
        else
            child_pid=$(pgrep --parent "$pane_pid" 2>/dev/null | head -1)
        fi

        if [[ -n "$child_pid" ]]; then
            echo "$child_pid"
        else
            echo "$pane_pid"
        fi
    fi
}

# Get all child PIDs of a process
get_child_pids() {
    local parent_pid="$1"
    local pids=()

    if [[ "$OSTYPE" == "darwin"* ]]; then
        while IFS= read -r pid; do
            [[ -n "$pid" ]] && pids+=("$pid")
        done < <(pgrep -P "$parent_pid" 2>/dev/null)
    else
        while IFS= read -r pid; do
            [[ -n "$pid" ]] && pids+=("$pid")
        done < <(pgrep --parent "$parent_pid" 2>/dev/null)
    fi

    echo "${pids[*]}"
}

# Send signal to process in tmux pane
tmux_signal_process() {
    local session="$1"
    local signal="$2"

    local pid
    pid=$(tmux_get_pane_pid "$session")

    if [[ -n "$pid" ]]; then
        kill -"$signal" "$pid" 2>/dev/null
        return $?
    fi
    return 1
}

# Pause process in tmux pane (SIGSTOP)
tmux_pause_process() {
    local session="$1"
    tmux_signal_process "$session" "STOP"
}

# Resume process in tmux pane (SIGCONT)
tmux_resume_process() {
    local session="$1"
    tmux_signal_process "$session" "CONT"
}

# Check if process is stopped
process_is_stopped() {
    local pid="$1"

    if [[ ! -d "/proc/$pid" ]] && [[ "$OSTYPE" != "darwin"* ]]; then
        return 1
    fi

    local state
    if [[ "$OSTYPE" == "darwin"* ]]; then
        state=$(ps -o state= -p "$pid" 2>/dev/null)
    else
        state=$(cat "/proc/$pid/stat" 2>/dev/null | awk '{print $3}')
    fi

    [[ "$state" == "T" ]] || [[ "$state" == "t" ]]
}

# List all agent tmux sessions
tmux_list_agent_sessions() {
    tmux list-sessions -F '#{session_name}' 2>/dev/null | grep "^${TMUX_SESSION_PREFIX}" | sed "s/^${TMUX_SESSION_PREFIX}//"
}

# Get window dimensions
tmux_get_dimensions() {
    local session="$1"

    if ! tmux_session_exists "$session"; then
        return 1
    fi

    local width height
    width=$(tmux display-message -t "$session" -p '#{window_width}')
    height=$(tmux display-message -t "$session" -p '#{window_height}')
    echo "${width}x${height}"
}

# Set window title
tmux_set_title() {
    local session="$1"
    local title="$2"

    if tmux_session_exists "$session"; then
        tmux rename-window -t "$session" "$title"
    fi
}
