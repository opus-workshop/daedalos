#!/usr/bin/env bash
#
# spec/lib/common.sh - Common utilities for spec tool
#

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
DIM='\033[2m'
NC='\033[0m'

# Logging
info()    { echo -e "${BLUE}info${NC}: $*"; }
success() { echo -e "${GREEN}success${NC}: $*"; }
warn()    { echo -e "${YELLOW}warn${NC}: $*" >&2; }
error()   { echo -e "${RED}error${NC}: $*" >&2; }
die()     { error "$*"; exit 1; }

# Check if yq is available, fall back to python
# Returns empty string on parse failure (never exits non-zero)
parse_yaml() {
    local file="$1"
    local query="${2:-.}"

    if command -v yq &>/dev/null; then
        yq -r "$query" "$file" 2>/dev/null || true
    else
        python3 -c "
import sys, yaml
try:
    with open('$file') as f:
        data = yaml.safe_load(f)
    query = '$query'.strip('.')
    if query:
        for key in query.split('.'):
            if isinstance(data, dict):
                data = data.get(key)
            else:
                data = None
                break
    if data is not None:
        if isinstance(data, (dict, list)):
            print(yaml.dump(data, default_flow_style=False))
        else:
            print(data)
except:
    pass  # Return empty on any error
" 2>/dev/null || true
    fi
}

# Find spec file for a component
find_spec() {
    local component="$1"

    # First, check index
    local index_file="${PROJECT_ROOT}/.daedalos/specs/index.yaml"
    if [[ -f "$index_file" ]]; then
        local indexed_path
        indexed_path=$(parse_yaml "$index_file" ".components.$component")
        if [[ -n "$indexed_path" ]] && [[ -f "${PROJECT_ROOT}/${indexed_path}" ]]; then
            echo "${PROJECT_ROOT}/${indexed_path}"
            return
        fi
    fi

    # Search common locations
    local search_paths=(
        "${PROJECT_ROOT}/daedalos-tools/${component}/${component}.spec.yaml"
        "${PROJECT_ROOT}/tools/${component}/${component}.spec.yaml"
        "${PROJECT_ROOT}/${component}/${component}.spec.yaml"
        "${PROJECT_ROOT}/${component}.spec.yaml"
    )

    for path in "${search_paths[@]}"; do
        if [[ -f "$path" ]]; then
            echo "$path"
            return
        fi
    done

    # Glob search
    local found
    found=$(find "$PROJECT_ROOT" -name "${component}.spec.yaml" -type f 2>/dev/null | head -1)
    if [[ -n "$found" ]]; then
        echo "$found"
    fi
}

# Show entire spec
show_spec() {
    local spec_path="$1"
    local format="${2:-yaml}"

    if [[ "$format" == "json" ]]; then
        if command -v yq &>/dev/null; then
            yq -o json "$spec_path"
        else
            python3 -c "
import yaml, json
with open('$spec_path') as f:
    print(json.dumps(yaml.safe_load(f), indent=2))
"
        fi
    else
        # Pretty print YAML with syntax highlighting if bat available
        if command -v bat &>/dev/null; then
            bat --style=plain --language=yaml "$spec_path"
        else
            cat "$spec_path"
        fi
    fi
}

# Show specific section
show_section() {
    local spec_path="$1"
    local section="$2"
    local format="${3:-yaml}"

    local content
    content=$(parse_yaml "$spec_path" ".$section")

    if [[ -z "$content" ]] || [[ "$content" == "null" ]]; then
        die "Section not found: $section"
    fi

    echo -e "${BOLD}# $section${NC}"
    echo "$content"
}

# List all specs
list_specs() {
    local show_missing="${1:-false}"
    local show_stale="${2:-false}"

    echo -e "${BOLD}Specs in ${PROJECT_ROOT}${NC}"
    echo

    # Find all spec files
    local specs
    specs=$(find "$PROJECT_ROOT" -name "*.spec.yaml" -type f 2>/dev/null | sort)

    if [[ -z "$specs" ]]; then
        warn "No specs found"
        return
    fi

    while IFS= read -r spec_path; do
        local name
        name=$(parse_yaml "$spec_path" ".name")
        local rel_path="${spec_path#$PROJECT_ROOT/}"

        # Check staleness (compare to implementation)
        local impl_dir
        impl_dir=$(dirname "$spec_path")
        local impl_file="${impl_dir}/bin/${name}"

        local status_icon="  "
        if $show_stale; then
            if [[ -f "$impl_file" ]] && [[ "$impl_file" -nt "$spec_path" ]]; then
                status_icon="${YELLOW}!${NC} "
            fi
        fi

        printf "%s${CYAN}%-20s${NC} %s\n" "$status_icon" "$name" "${DIM}${rel_path}${NC}"
    done <<< "$specs"

    # Show missing if requested
    if $show_missing; then
        echo
        echo -e "${BOLD}Components without specs:${NC}"
        find "$PROJECT_ROOT/daedalos-tools" -maxdepth 1 -type d 2>/dev/null | while read -r dir; do
            local name
            name=$(basename "$dir")
            if [[ "$name" != "daedalos-tools" ]] && [[ ! -f "${dir}/${name}.spec.yaml" ]]; then
                echo -e "  ${DIM}${name}${NC}"
            fi
        done
    fi
}

# Create new spec from template
create_spec() {
    local name="$1"
    local type="${2:-tool}"

    local template_file="${TEMPLATE_DIR}/${type}.spec.yaml"
    if [[ ! -f "$template_file" ]]; then
        template_file="${TEMPLATE_DIR}/tool.spec.yaml"
    fi

    # Determine output path
    local output_dir="${PROJECT_ROOT}/daedalos-tools/${name}"
    local output_file="${output_dir}/${name}.spec.yaml"

    if [[ -f "$output_file" ]]; then
        die "Spec already exists: $output_file"
    fi

    mkdir -p "$output_dir"

    # Copy and substitute
    sed "s/__NAME__/${name}/g; s/__DATE__/$(date +%Y-%m-%d)/g" \
        "$template_file" > "$output_file"

    success "Created: $output_file"
    info "Edit the spec to fill in intent, constraints, and decisions"
}

# Rebuild index
rebuild_index() {
    local index_dir="${PROJECT_ROOT}/.daedalos/specs"
    local index_file="${index_dir}/index.yaml"

    mkdir -p "$index_dir"

    echo "version: 1.0" > "$index_file"
    echo "project: $(basename "$PROJECT_ROOT")" >> "$index_file"
    echo "generated: $(date -Iseconds)" >> "$index_file"
    echo "" >> "$index_file"
    echo "components:" >> "$index_file"

    # Find all specs
    find "$PROJECT_ROOT" -name "*.spec.yaml" -type f 2>/dev/null | sort | while read -r spec_path; do
        local name
        name=$(parse_yaml "$spec_path" ".name" 2>/dev/null)
        if [[ -n "$name" ]] && [[ "$name" != "null" ]]; then
            local rel_path="${spec_path#$PROJECT_ROOT/}"
            echo "  ${name}: ${rel_path}" >> "$index_file"
        fi
    done

    success "Index rebuilt: $index_file"
}

# Show index
show_index() {
    local index_file="${PROJECT_ROOT}/.daedalos/specs/index.yaml"

    if [[ ! -f "$index_file" ]]; then
        warn "No index found. Run 'spec index rebuild'"
        return 1
    fi

    cat "$index_file"
}

# Inject spec into context format
inject_spec() {
    local component="$1"
    local format="${2:-markdown}"

    local spec_path
    spec_path=$(find_spec "$component")

    if [[ -z "$spec_path" ]]; then
        die "Spec not found: $component"
    fi

    local intent constraints anti_patterns

    intent=$(parse_yaml "$spec_path" ".intent")
    constraints=$(parse_yaml "$spec_path" ".constraints")
    anti_patterns=$(parse_yaml "$spec_path" ".anti_patterns")

    if [[ "$format" == "markdown" ]]; then
        echo "## ${component}"
        echo
        echo "### Intent"
        echo "$intent"
        echo
        if [[ -n "$constraints" ]] && [[ "$constraints" != "null" ]]; then
            echo "### Constraints"
            # Constraints already have - prefix from YAML list format
            echo "$constraints"
            echo
        fi
        if [[ -n "$anti_patterns" ]] && [[ "$anti_patterns" != "null" ]]; then
            echo "### Anti-patterns (AVOID)"
            # Extract just the pattern names for cleaner output
            echo "$anti_patterns" | grep "^- pattern:" | sed 's/^- pattern: /- /'
        fi
    else
        echo "---"
        echo "component: $component"
        echo "intent: |"
        echo "$intent" | sed 's/^/  /'
        if [[ -n "$constraints" ]] && [[ "$constraints" != "null" ]]; then
            echo "constraints:"
            echo "$constraints" | sed 's/^/  /'
        fi
    fi
}

# Inject all specs
inject_all_specs() {
    local format="${1:-markdown}"

    echo "# Component Specifications"
    echo

    find "$PROJECT_ROOT" -name "*.spec.yaml" -type f 2>/dev/null | sort | while read -r spec_path; do
        local name
        name=$(parse_yaml "$spec_path" ".name" 2>/dev/null)
        if [[ -n "$name" ]] && [[ "$name" != "null" ]] && [[ "$name" != "__NAME__" ]]; then
            inject_spec "$name" "$format"
            echo
        fi
    done
}

# Inject compact summary (just intents)
inject_summary() {
    echo "# Daedalos Tools - Quick Reference"
    echo
    echo "| Tool | Purpose |"
    echo "|------|---------|"

    find "$PROJECT_ROOT" -name "*.spec.yaml" -type f 2>/dev/null | sort | while read -r spec_path; do
        local name intent_first_line
        name=$(parse_yaml "$spec_path" ".name" 2>/dev/null)
        if [[ -n "$name" ]] && [[ "$name" != "null" ]] && [[ "$name" != "__NAME__" ]]; then
            # Get first sentence of intent
            intent_first_line=$(parse_yaml "$spec_path" ".intent" 2>/dev/null | head -1 | sed 's/\.$//')
            printf "| \`%s\` | %s |\n" "$name" "$intent_first_line"
        fi
    done
}
