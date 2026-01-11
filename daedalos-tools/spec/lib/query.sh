#!/usr/bin/env bash
#
# spec/lib/query.sh - Semantic querying of specs
#

# Lowercase helper (works on bash 3)
to_lower() {
    echo "$1" | tr '[:upper:]' '[:lower:]'
}

# Check if haystack contains needle (case-insensitive)
contains_ci() {
    local haystack needle
    haystack=$(to_lower "$1")
    needle=$(to_lower "$2")
    [[ "$haystack" == *"$needle"* ]]
}

# Query specs for relevant content
query_specs() {
    local query="$1"
    local query_lower
    query_lower=$(to_lower "$query")

    echo -e "${BOLD}Searching specs for: ${CYAN}${query}${NC}"
    echo

    local found=false

    # Find all specs (use process substitution to keep while loop in parent shell)
    while read -r spec_path; do
        local name
        name=$(parse_yaml "$spec_path" ".name" 2>/dev/null)
        [[ -z "$name" ]] && continue

        local matches=""

        # Search in different sections with context
        # Intent
        local intent
        intent=$(parse_yaml "$spec_path" ".intent" 2>/dev/null)
        if contains_ci "$intent" "$query"; then
            matches+="intent "
        fi

        # Decisions (search in choice and why)
        local decisions
        decisions=$(parse_yaml "$spec_path" ".decisions" 2>/dev/null)
        if contains_ci "$decisions" "$query"; then
            matches+="decisions "
        fi

        # Anti-patterns
        local anti
        anti=$(parse_yaml "$spec_path" ".anti_patterns" 2>/dev/null)
        if contains_ci "$anti" "$query"; then
            matches+="anti_patterns "
        fi

        # Constraints
        local constraints
        constraints=$(parse_yaml "$spec_path" ".constraints" 2>/dev/null)
        if contains_ci "$constraints" "$query"; then
            matches+="constraints "
        fi

        # Examples
        local examples
        examples=$(parse_yaml "$spec_path" ".examples" 2>/dev/null)
        if contains_ci "$examples" "$query"; then
            matches+="examples "
        fi

        if [[ -n "$matches" ]]; then
            found=true
            echo -e "${GREEN}${name}${NC} ${DIM}(${matches% })${NC}"

            # Show relevant excerpts
            for section in $matches; do
                local content
                content=$(parse_yaml "$spec_path" ".$section" 2>/dev/null)
                # Show first few lines containing the query
                echo "$content" | grep -i -m 3 -C 1 "$query_lower" 2>/dev/null | /usr/bin/head -10 | sed 's/^/  /'
            done
            echo
        fi
    done < <(find "$PROJECT_ROOT" -name "*.spec.yaml" -type f 2>/dev/null)

    if ! $found; then
        warn "No matches found for: $query"
    fi
}

# Get context for a specific task
get_context_for_task() {
    local task="$1"
    local task_lower
    task_lower=$(to_lower "$task")

    # Extract likely component names from task
    local components=()

    # Find all specs and check relevance
    find "$PROJECT_ROOT" -name "*.spec.yaml" -type f 2>/dev/null | while read -r spec_path; do
        local name
        name=$(parse_yaml "$spec_path" ".name" 2>/dev/null)
        [[ -z "$name" ]] && continue

        local name_lower
        name_lower=$(to_lower "$name")
        local relevance=0

        # Check if component name appears in task
        if [[ "$task_lower" == *"$name_lower"* ]]; then
            relevance=100
        fi

        # Check if task keywords appear in spec
        local intent
        intent=$(parse_yaml "$spec_path" ".intent" 2>/dev/null)
        local intent_lower
        intent_lower=$(to_lower "$intent")

        # Simple keyword matching (could be enhanced with embeddings)
        for word in $task_lower; do
            if [[ ${#word} -gt 3 ]] && [[ "$intent_lower" == *"$word"* ]]; then
                ((relevance += 10)) || true
            fi
        done

        if [[ $relevance -gt 20 ]]; then
            echo "---"
            echo "# $name (relevance: $relevance)"
            echo

            # Output focused context
            echo "## Intent"
            echo "$intent"
            echo

            # Get interface for the likely command
            local interface
            interface=$(parse_yaml "$spec_path" ".interface" 2>/dev/null)
            if [[ -n "$interface" ]] && [[ "$interface" != "null" ]]; then
                echo "## Interface"
                echo "$interface" | /usr/bin/head -30
                echo
            fi

            # Always include anti-patterns (prevent mistakes)
            local anti
            anti=$(parse_yaml "$spec_path" ".anti_patterns" 2>/dev/null)
            if [[ -n "$anti" ]] && [[ "$anti" != "null" ]]; then
                echo "## Anti-patterns (AVOID)"
                echo "$anti"
                echo
            fi

            # Include relevant decisions
            local decisions
            decisions=$(parse_yaml "$spec_path" ".decisions" 2>/dev/null)
            if [[ -n "$decisions" ]] && [[ "$decisions" != "null" ]]; then
                # Only show decisions related to task keywords
                echo "## Relevant Decisions"
                echo "$decisions" | /usr/bin/head -20
            fi
        fi
    done
}
