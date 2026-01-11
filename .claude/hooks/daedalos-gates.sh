#!/usr/bin/env bash
# Hook: Check Daedalos gates before significant actions
#
# This hook runs BEFORE certain tool calls to check if they're allowed
# by the current supervision configuration.

set -eo pipefail

# Only run if gates is available
command -v gates &>/dev/null || exit 0

# Parse hook input
INPUT=$(cat)

HOOK_TYPE=$(echo "$INPUT" | jq -r '.hook_type // empty')
TOOL_NAME=$(echo "$INPUT" | jq -r '.tool_name // empty')
TOOL_INPUT=$(echo "$INPUT" | jq -r '.tool_input // empty')

# Map tool names to gates
case "$TOOL_NAME" in
    Edit|Write|NotebookEdit)
        FILE=$(echo "$TOOL_INPUT" | jq -r '.file_path // empty')

        # Check for sensitive files
        if [[ "$FILE" == *.env* ]] || [[ "$FILE" == *secret* ]] || [[ "$FILE" == *credential* ]] || [[ "$FILE" == *.key ]]; then
            if ! gates check sensitive_file 2>/dev/null; then
                echo "Gate denied: sensitive_file modification requires approval"
                exit 1
            fi
        fi

        # Check general file modification
        if ! gates check file_modify 2>/dev/null; then
            echo "Gate denied: file_modify not allowed at current supervision level"
            exit 1
        fi
        ;;
    Bash)
        CMD=$(echo "$TOOL_INPUT" | jq -r '.command // empty')

        # Check for git push
        if [[ "$CMD" == *"git push"* ]]; then
            if [[ "$CMD" == *"--force"* ]] || [[ "$CMD" == *"-f "* ]]; then
                if ! gates check git_force_push 2>/dev/null; then
                    echo "Gate denied: git_force_push not allowed"
                    exit 1
                fi
            else
                if ! gates check git_push 2>/dev/null; then
                    echo "Gate denied: git_push requires approval"
                    exit 1
                fi
            fi
        fi

        # Check for destructive commands
        if [[ "$CMD" == *"rm -rf"* ]] || [[ "$CMD" == *"rm -r"* ]]; then
            if ! gates check file_delete 2>/dev/null; then
                echo "Gate denied: file_delete not allowed"
                exit 1
            fi
        fi
        ;;
esac

exit 0
