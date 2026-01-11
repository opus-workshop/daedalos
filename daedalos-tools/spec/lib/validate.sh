#!/usr/bin/env bash
#
# spec/lib/validate.sh - Spec validation
#

# Required sections for a valid spec
REQUIRED_SECTIONS=("name" "intent" "constraints" "interface")

# Validate all specs in path
validate_specs() {
    local path="$1"
    local errors=0
    local warnings=0

    echo -e "${BOLD}Validating specs in: ${path}${NC}"
    echo

    find "$path" -name "*.spec.yaml" -type f 2>/dev/null | while read -r spec_path; do
        validate_single_spec "$spec_path"
        local result=$?
        if [[ $result -eq 1 ]]; then
            ((errors++))
        elif [[ $result -eq 2 ]]; then
            ((warnings++))
        fi
    done

    echo
    if [[ $errors -gt 0 ]]; then
        error "$errors spec(s) have errors"
        return 1
    elif [[ $warnings -gt 0 ]]; then
        warn "$warnings spec(s) have warnings"
        return 0
    else
        success "All specs valid"
        return 0
    fi
}

# Validate a single spec file
validate_single_spec() {
    local spec_path="$1"
    local rel_path="${spec_path#$PROJECT_ROOT/}"
    local has_error=false
    local has_warning=false

    # Check YAML syntax
    if ! python3 -c "import yaml; yaml.safe_load(open('$spec_path'))" 2>/dev/null; then
        echo -e "${RED}INVALID${NC} $rel_path"
        echo "  - Invalid YAML syntax"
        return 1
    fi

    local name
    name=$(parse_yaml "$spec_path" ".name" 2>/dev/null)

    # Check required sections
    for section in "${REQUIRED_SECTIONS[@]}"; do
        local content
        content=$(parse_yaml "$spec_path" ".$section" 2>/dev/null)
        if [[ -z "$content" ]] || [[ "$content" == "null" ]]; then
            if ! $has_error; then
                echo -e "${RED}ERROR${NC} $rel_path"
            fi
            echo "  - Missing required section: $section"
            has_error=true
        fi
    done

    # Check recommended sections
    local recommended=("examples" "decisions" "anti_patterns")
    for section in "${recommended[@]}"; do
        local content
        content=$(parse_yaml "$spec_path" ".$section" 2>/dev/null)
        if [[ -z "$content" ]] || [[ "$content" == "null" ]]; then
            if ! $has_error && ! $has_warning; then
                echo -e "${YELLOW}WARN${NC} $rel_path"
            fi
            echo "  - Missing recommended section: $section"
            has_warning=true
        fi
    done

    # Check intent quality (should be > 50 chars)
    local intent
    intent=$(parse_yaml "$spec_path" ".intent" 2>/dev/null)
    if [[ -n "$intent" ]] && [[ ${#intent} -lt 50 ]]; then
        if ! $has_error && ! $has_warning; then
            echo -e "${YELLOW}WARN${NC} $rel_path"
        fi
        echo "  - Intent seems too short (should explain WHY)"
        has_warning=true
    fi

    # Check for connects_to references
    local connects
    connects=$(parse_yaml "$spec_path" ".connects_to" 2>/dev/null)
    if [[ -n "$connects" ]] && [[ "$connects" != "null" ]]; then
        # Validate that referenced components exist
        echo "$connects" | grep -oE 'component: [a-zA-Z0-9_-]+' | while read -r line; do
            local ref_component="${line#component: }"
            local ref_spec
            ref_spec=$(find_spec "$ref_component" 2>/dev/null)
            if [[ -z "$ref_spec" ]]; then
                if ! $has_warning; then
                    echo -e "${YELLOW}WARN${NC} $rel_path"
                fi
                echo "  - Referenced component not found: $ref_component"
                has_warning=true
            fi
        done
    fi

    if $has_error; then
        return 1
    elif $has_warning; then
        return 2
    else
        echo -e "${GREEN}OK${NC} $rel_path"
        return 0
    fi
}

# Compare spec to implementation
diff_spec() {
    local component="$1"

    local spec_path
    spec_path=$(find_spec "$component")

    if [[ -z "$spec_path" ]]; then
        die "Spec not found: $component"
    fi

    local impl_dir
    impl_dir=$(dirname "$spec_path")
    local impl_file="${impl_dir}/bin/${component}"

    if [[ ! -f "$impl_file" ]]; then
        warn "Implementation not found: $impl_file"
        return 1
    fi

    echo -e "${BOLD}Comparing spec to implementation: ${component}${NC}"
    echo

    # Extract commands from spec
    local spec_commands
    spec_commands=$(parse_yaml "$spec_path" ".interface.commands" 2>/dev/null | grep -oE '^[a-zA-Z_]+:' | tr -d ':' | sort)

    # Extract commands from implementation (look for case statements)
    local impl_commands
    impl_commands=$(grep -oE '\b(cmd_[a-zA-Z_]+|[a-zA-Z_]+)\)' "$impl_file" 2>/dev/null | tr -d ')' | sed 's/cmd_//' | sort -u)

    echo "Commands in spec:"
    echo "$spec_commands" | sed 's/^/  /'
    echo

    echo "Commands in implementation:"
    echo "$impl_commands" | sed 's/^/  /'
    echo

    # Find differences
    local only_spec only_impl

    only_spec=$(comm -23 <(echo "$spec_commands") <(echo "$impl_commands") 2>/dev/null)
    only_impl=$(comm -13 <(echo "$spec_commands") <(echo "$impl_commands") 2>/dev/null)

    if [[ -n "$only_spec" ]]; then
        echo -e "${YELLOW}In spec but not implemented:${NC}"
        echo "$only_spec" | sed 's/^/  - /'
    fi

    if [[ -n "$only_impl" ]]; then
        echo -e "${YELLOW}Implemented but not in spec:${NC}"
        echo "$only_impl" | sed 's/^/  - /'
    fi

    if [[ -z "$only_spec" ]] && [[ -z "$only_impl" ]]; then
        success "Spec and implementation match"
    fi
}
