#!/usr/bin/env bash
#===============================================================================
# detect.sh - Project and tool detection for verify
#===============================================================================

#-------------------------------------------------------------------------------
# Project Type Detection
#-------------------------------------------------------------------------------

detect_project_type() {
    local path="${1:-.}"

    # Check for project indicator files (in priority order)

    # Swift/Xcode
    if compgen -G "$path/*.xcodeproj" >/dev/null 2>&1 || \
       compgen -G "$path/*.xcworkspace" >/dev/null 2>&1 || \
       [[ -f "$path/Package.swift" ]]; then
        echo "swift"
        return
    fi

    # Rust
    if [[ -f "$path/Cargo.toml" ]]; then
        echo "rust"
        return
    fi

    # Go
    if [[ -f "$path/go.mod" ]]; then
        echo "go"
        return
    fi

    # TypeScript/JavaScript
    if [[ -f "$path/package.json" ]]; then
        if [[ -f "$path/tsconfig.json" ]]; then
            echo "typescript"
        else
            echo "javascript"
        fi
        return
    fi

    # Python
    if [[ -f "$path/pyproject.toml" ]] || \
       [[ -f "$path/setup.py" ]] || \
       [[ -f "$path/requirements.txt" ]]; then
        echo "python"
        return
    fi

    # Elixir
    if [[ -f "$path/mix.exs" ]]; then
        echo "elixir"
        return
    fi

    # Ruby
    if [[ -f "$path/Gemfile" ]]; then
        echo "ruby"
        return
    fi

    # Java/Kotlin (Gradle)
    if [[ -f "$path/build.gradle" ]] || [[ -f "$path/build.gradle.kts" ]]; then
        echo "java"
        return
    fi

    # Java (Maven)
    if [[ -f "$path/pom.xml" ]]; then
        echo "java"
        return
    fi

    # Shell scripts
    if compgen -G "$path/*.sh" >/dev/null 2>&1; then
        echo "shell"
        return
    fi

    echo "unknown"
}

#-------------------------------------------------------------------------------
# Package Manager Detection
#-------------------------------------------------------------------------------

detect_package_manager() {
    local path="${1:-.}"

    # Node.js package managers
    if [[ -f "$path/pnpm-lock.yaml" ]]; then
        echo "pnpm"
    elif [[ -f "$path/yarn.lock" ]]; then
        echo "yarn"
    elif [[ -f "$path/bun.lockb" ]]; then
        echo "bun"
    elif [[ -f "$path/package-lock.json" ]]; then
        echo "npm"
    elif [[ -f "$path/package.json" ]]; then
        echo "npm"  # Default for Node projects
    else
        echo ""
    fi
}

#-------------------------------------------------------------------------------
# Tool Detection
#-------------------------------------------------------------------------------

detect_tool() {
    local tool="$1"
    command -v "$tool" &>/dev/null
}

# Check if a tool is available locally (npx, cargo, etc.)
detect_local_tool() {
    local tool="$1"
    local path="${2:-.}"

    case "$tool" in
        eslint|prettier|tsc|jest|vitest)
            [[ -f "$path/node_modules/.bin/$tool" ]] && return 0
            ;;
    esac

    detect_tool "$tool"
}

#-------------------------------------------------------------------------------
# Xcode Scheme Detection
#-------------------------------------------------------------------------------

detect_xcode_scheme() {
    local path="${1:-.}"

    # Find xcodeproj or xcworkspace
    local project
    project=$(find "$path" -maxdepth 1 -name "*.xcworkspace" -print -quit 2>/dev/null)
    if [[ -z "$project" ]]; then
        project=$(find "$path" -maxdepth 1 -name "*.xcodeproj" -print -quit 2>/dev/null)
    fi

    if [[ -z "$project" ]]; then
        echo ""
        return
    fi

    # List schemes and get first one
    local schemes
    if [[ "$project" == *.xcworkspace ]]; then
        schemes=$(xcodebuild -workspace "$project" -list 2>/dev/null | \
                  awk '/Schemes:/{found=1; next} found && /^[[:space:]]+/{gsub(/^[[:space:]]+|[[:space:]]+$/, ""); print; next} found{exit}')
    else
        schemes=$(xcodebuild -project "$project" -list 2>/dev/null | \
                  awk '/Schemes:/{found=1; next} found && /^[[:space:]]+/{gsub(/^[[:space:]]+|[[:space:]]+$/, ""); print; next} found{exit}')
    fi

    echo "$schemes" | head -1
}

#-------------------------------------------------------------------------------
# Pipeline Loading
#-------------------------------------------------------------------------------

load_pipeline() {
    local type="$1"
    local pipelines_dir="${PIPELINES_DIR:-$(dirname "${BASH_SOURCE[0]}")/../pipelines}"

    # Check for project-local config first
    if [[ -f ".daedalos/verify.yaml" ]]; then
        debug "Using project-local verify.yaml"
        echo ".daedalos/verify.yaml"
        return
    fi

    if [[ -f ".claude-os/verify.yaml" ]]; then
        debug "Using project-local verify.yaml"
        echo ".claude-os/verify.yaml"
        return
    fi

    # Use built-in pipeline
    local pipeline_file="${pipelines_dir}/${type}.yaml"

    if [[ ! -f "$pipeline_file" ]]; then
        die "No pipeline found for project type: $type (looked in $pipeline_file)"
    fi

    echo "$pipeline_file"
}

#-------------------------------------------------------------------------------
# Step Extraction
#-------------------------------------------------------------------------------

# Get steps from pipeline file
get_pipeline_steps() {
    local pipeline_file="$1"

    if has_command yq; then
        yq -r '.steps[].name' "$pipeline_file" 2>/dev/null
    else
        # Fallback: grep for step names
        grep -E "^\s+-\s*name:" "$pipeline_file" 2>/dev/null | \
            sed 's/.*name:[[:space:]]*//' | tr -d '"'"'"
    fi
}

# Get step configuration
get_step_config() {
    local pipeline_file="$1"
    local step_name="$2"
    local field="$3"

    if has_command yq; then
        yq -r ".steps[] | select(.name == \"$step_name\") | .$field // \"\"" "$pipeline_file" 2>/dev/null
    else
        # Fallback: basic parsing (limited)
        local in_step=false
        local found_step=false

        while IFS= read -r line; do
            if [[ "$line" =~ ^[[:space:]]*-[[:space:]]*name:[[:space:]]*(.*)$ ]]; then
                local current_name="${BASH_REMATCH[1]//\"/}"
                current_name="${current_name//\'/}"
                if [[ "$current_name" == "$step_name" ]]; then
                    found_step=true
                    in_step=true
                else
                    in_step=false
                fi
            elif $in_step && [[ "$line" =~ ^[[:space:]]+${field}:[[:space:]]*(.*)$ ]]; then
                local value="${BASH_REMATCH[1]//\"/}"
                value="${value//\'/}"
                echo "$value"
                return
            fi
        done < "$pipeline_file"
    fi
}

# Check if step is quick (runs in quick mode)
is_quick_step() {
    local pipeline_file="$1"
    local step_name="$2"

    local quick
    quick=$(get_step_config "$pipeline_file" "$step_name" "quick")

    [[ "$quick" == "true" ]]
}
