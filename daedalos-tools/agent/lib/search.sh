#!/usr/bin/env bash
# search.sh - Cross-agent search functionality
#
# Search across all agent outputs using tmux scrollback buffers.

# Prevent double-sourcing
[[ -n "${_AGENT_SEARCH_LOADED:-}" ]] && return 0
_AGENT_SEARCH_LOADED=1

# Search a single agent's output
# Usage: search_agent <name> <pattern> [options]
search_agent() {
    local name="$1"
    local pattern="$2"
    local ignore_case="${3:-false}"
    local context_lines="${4:-2}"

    local agent
    agent=$(agents_get "$name")
    if [[ -z "$agent" ]]; then
        return 1
    fi

    local session
    session=$(echo "$agent" | jq -r '.tmux_session')

    if ! tmux_session_exists "$session"; then
        return 1
    fi

    # Get full scrollback
    local content
    content=$(tmux_get_scrollback "$session")

    # Build grep options
    local grep_opts="-n"
    if [[ "$ignore_case" == "true" ]]; then
        grep_opts+=" -i"
    fi
    if [[ $context_lines -gt 0 ]]; then
        grep_opts+=" -C $context_lines"
    fi

    # Search and format results
    echo "$content" | grep $grep_opts -E "$pattern" 2>/dev/null
}

# Search across all agents
# Usage: search_all <pattern> [options]
search_all() {
    local pattern="$1"
    local target_agent="${2:-}"
    local ignore_case="${3:-false}"
    local context_lines="${4:-2}"
    local as_json="${5:-false}"

    local -a results

    # Get agents to search
    local -a agents_to_search
    if [[ -n "$target_agent" ]]; then
        local resolved
        resolved=$(agents_resolve "$target_agent")
        if [[ -n "$resolved" ]]; then
            agents_to_search+=("$resolved")
        else
            die "Agent not found: $target_agent"
        fi
    else
        while IFS= read -r name; do
            [[ -n "$name" ]] && agents_to_search+=("$name")
        done < <(agents_names)
    fi

    if [[ ${#agents_to_search[@]} -eq 0 ]]; then
        if [[ "$as_json" == "true" ]]; then
            echo "[]"
        else
            echo "No agents to search."
        fi
        return
    fi

    if [[ "$as_json" == "true" ]]; then
        local json_results="["
        local first=true

        for name in "${agents_to_search[@]}"; do
            local matches
            matches=$(search_agent "$name" "$pattern" "$ignore_case" "$context_lines")

            if [[ -n "$matches" ]]; then
                while IFS= read -r line; do
                    # Parse line number from grep output
                    local line_num match_text
                    if [[ "$line" =~ ^([0-9]+)[:-](.*)$ ]]; then
                        line_num="${BASH_REMATCH[1]}"
                        match_text="${BASH_REMATCH[2]}"
                    else
                        continue
                    fi

                    local entry
                    entry=$(jq -n \
                        --arg agent "$name" \
                        --argjson line "$line_num" \
                        --arg text "$match_text" \
                        '{agent: $agent, line: $line, text: $text}')

                    if [[ "$first" == "true" ]]; then
                        json_results+="$entry"
                        first=false
                    else
                        json_results+=",$entry"
                    fi
                done <<< "$matches"
            fi
        done

        json_results+="]"
        echo "$json_results" | jq '.'
        return
    fi

    # Text output
    local found=false

    for name in "${agents_to_search[@]}"; do
        local matches
        matches=$(search_agent "$name" "$pattern" "$ignore_case" "$context_lines")

        if [[ -n "$matches" ]]; then
            found=true
            echo "${C_BOLD}${C_CYAN}=== $name ===${C_RESET}"
            echo "$matches" | while IFS= read -r line; do
                # Highlight matches
                if [[ "$ignore_case" == "true" ]]; then
                    echo "$line" | grep --color=always -iE "$pattern"
                else
                    echo "$line" | grep --color=always -E "$pattern"
                fi
            done
            echo ""
        fi
    done

    if [[ "$found" == "false" ]]; then
        echo "No matches found."
    fi
}

# Interactive search with fzf
search_interactive() {
    local pattern="${1:-}"

    if ! has_fzf; then
        die "Interactive search requires fzf"
    fi

    # Collect all content with agent prefixes
    local all_content=""
    while IFS= read -r name; do
        [[ -z "$name" ]] && continue

        local agent
        agent=$(agents_get "$name")
        local session
        session=$(echo "$agent" | jq -r '.tmux_session')

        if tmux_session_exists "$session"; then
            local content
            content=$(tmux_get_scrollback "$session")
            while IFS= read -r line; do
                all_content+="${name}:${line}"$'\n'
            done <<< "$content"
        fi
    done < <(agents_names)

    if [[ -z "$all_content" ]]; then
        die "No agent content to search"
    fi

    # Use fzf for interactive search
    local query_arg=""
    if [[ -n "$pattern" ]]; then
        query_arg="--query=$pattern"
    fi

    local selected
    selected=$(echo "$all_content" | fzf --ansi $query_arg --preview-window=up:50% \
        --preview 'echo {} | cut -d: -f2-' \
        --header "Search across all agents (ESC to cancel)")

    if [[ -n "$selected" ]]; then
        local agent_name
        agent_name=$(echo "$selected" | cut -d: -f1)
        echo "Selected from agent: ${C_CYAN}${agent_name}${C_RESET}"
        echo "$selected" | cut -d: -f2-
    fi
}

# Get recent activity across all agents
get_recent_activity() {
    local lines="${1:-10}"

    echo "${C_BOLD}Recent Activity${C_RESET}"
    echo ""

    while IFS= read -r name; do
        [[ -z "$name" ]] && continue

        local agent
        agent=$(agents_get "$name")
        local session
        session=$(echo "$agent" | jq -r '.tmux_session')

        if tmux_session_exists "$session"; then
            local recent
            recent=$(tmux_get_pane_content "$session" "$lines" | tail -3)
            if [[ -n "$recent" ]]; then
                echo "${C_CYAN}${name}:${C_RESET}"
                echo "$recent" | sed 's/^/  /'
                echo ""
            fi
        fi
    done < <(agents_names)
}
