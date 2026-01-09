#!/usr/bin/env bash
# templates.sh - Template handling for agent CLI
#
# Manages agent templates for different use cases.

# Prevent double-sourcing
[[ -n "${_AGENT_TEMPLATES_LOADED:-}" ]] && return 0
_AGENT_TEMPLATES_LOADED=1

# List available templates
templates_list() {
    local as_json="${1:-false}"

    local -a templates
    while IFS= read -r file; do
        [[ -n "$file" ]] && templates+=("$file")
    done < <(find "$TEMPLATES_DIR" -name "*.json" -type f 2>/dev/null | sort)

    if [[ "$as_json" == "true" ]]; then
        local result="["
        local first=true
        for file in "${templates[@]}"; do
            local content
            content=$(cat "$file")
            if [[ "$first" == "true" ]]; then
                result+="$content"
                first=false
            else
                result+=",$content"
            fi
        done
        result+="]"
        echo "$result" | jq '.'
        return
    fi

    if [[ ${#templates[@]} -eq 0 ]]; then
        echo "No templates found."
        echo "Templates directory: $TEMPLATES_DIR"
        return
    fi

    echo "${C_BOLD}Available Templates${C_RESET}"
    echo ""
    for file in "${templates[@]}"; do
        local name description
        name=$(jq -r '.name // empty' "$file")
        description=$(jq -r '.description // "No description"' "$file")
        printf "  ${C_CYAN}%-15s${C_RESET} %s\n" "$name" "$description"
    done
}

# Get template by name
templates_get() {
    local name="$1"

    local template_file="${TEMPLATES_DIR}/${name}.json"
    if [[ -f "$template_file" ]]; then
        cat "$template_file"
        return 0
    fi

    # Try without .json extension
    if [[ -f "${TEMPLATES_DIR}/${name}" ]]; then
        cat "${TEMPLATES_DIR}/${name}"
        return 0
    fi

    return 1
}

# Check if template exists
templates_exists() {
    local name="$1"
    [[ -f "${TEMPLATES_DIR}/${name}.json" ]] || [[ -f "${TEMPLATES_DIR}/${name}" ]]
}

# Get template field
templates_get_field() {
    local name="$1"
    local field="$2"
    local default="${3:-}"

    local template
    template=$(templates_get "$name")
    if [[ -z "$template" ]]; then
        echo "$default"
        return
    fi

    local value
    value=$(echo "$template" | jq -r ".$field // empty")
    if [[ -n "$value" ]]; then
        echo "$value"
    else
        echo "$default"
    fi
}

# Get claude arguments from template
templates_get_claude_args() {
    local name="$1"

    local template
    template=$(templates_get "$name")
    if [[ -z "$template" ]]; then
        return
    fi

    # Get claude_args array and output space-separated
    echo "$template" | jq -r '.claude_args // [] | .[]' | tr '\n' ' '
}

# Get environment variables from template
templates_get_env() {
    local name="$1"

    local template
    template=$(templates_get "$name")
    if [[ -z "$template" ]]; then
        return
    fi

    # Output as KEY=VALUE lines
    echo "$template" | jq -r '.env // {} | to_entries[] | "\(.key)=\(.value)"'
}

# Get sandbox preset from template
templates_get_sandbox() {
    local name="$1"
    templates_get_field "$name" "sandbox" "implement"
}

# Get prompt prefix from template
templates_get_prompt_prefix() {
    local name="$1"
    templates_get_field "$name" "prompt_prefix" ""
}

# Create a new template
templates_create() {
    local name="$1"
    local description="${2:-Custom template}"
    local sandbox="${3:-implement}"

    local template_file="${TEMPLATES_DIR}/${name}.json"

    if [[ -f "$template_file" ]]; then
        die "Template already exists: $name"
    fi

    jq -n \
        --arg name "$name" \
        --arg description "$description" \
        --arg sandbox "$sandbox" \
        '{
            name: $name,
            description: $description,
            sandbox: $sandbox,
            claude_args: [],
            env: {},
            prompt_prefix: ""
        }' > "$template_file"

    success "Created template: $name"
    echo "Edit at: $template_file"
}

# Delete a template
templates_delete() {
    local name="$1"

    local template_file="${TEMPLATES_DIR}/${name}.json"

    if [[ ! -f "$template_file" ]]; then
        die "Template not found: $name"
    fi

    rm "$template_file"
    success "Deleted template: $name"
}

# Show template details
templates_show() {
    local name="$1"

    local template
    template=$(templates_get "$name")
    if [[ -z "$template" ]]; then
        die "Template not found: $name"
    fi

    echo "$template" | jq '.'
}

# Apply template to get full command arguments
# Returns: environment exports and claude command parts
templates_apply() {
    local name="$1"
    local project="$2"

    if ! templates_exists "$name"; then
        die "Template not found: $name"
    fi

    # Get sandbox
    local sandbox
    sandbox=$(templates_get_sandbox "$name")

    # Get claude args
    local claude_args
    claude_args=$(templates_get_claude_args "$name")

    # Get prompt prefix
    local prompt_prefix
    prompt_prefix=$(templates_get_prompt_prefix "$name")

    # Output as structured data
    jq -n \
        --arg sandbox "$sandbox" \
        --arg claude_args "$claude_args" \
        --arg prompt_prefix "$prompt_prefix" \
        '{sandbox: $sandbox, claude_args: $claude_args, prompt_prefix: $prompt_prefix}'
}

# Initialize default templates if they don't exist
templates_init_defaults() {
    mkdir -p "$TEMPLATES_DIR"

    # Only create if no templates exist
    local count
    count=$(find "$TEMPLATES_DIR" -name "*.json" -type f 2>/dev/null | wc -l)
    if [[ $count -gt 0 ]]; then
        return
    fi

    debug "Initializing default templates..."

    # These templates will be created by install.sh
}
