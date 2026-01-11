#!/usr/bin/env bash
# Hook: Automatic undo tracking for file changes
#
# This hook runs AFTER file edits to record them in the undo timeline.
# Enables "undo last" to revert Claude's changes.

set -eo pipefail

# Only run if undo is available
command -v undo &>/dev/null || exit 0

# Parse hook input
INPUT=$(cat)

HOOK_TYPE=$(echo "$INPUT" | jq -r '.hook_type // empty')
TOOL_NAME=$(echo "$INPUT" | jq -r '.tool_name // empty')
TOOL_INPUT=$(echo "$INPUT" | jq -r '.tool_input // empty')
TOOL_OUTPUT=$(echo "$INPUT" | jq -r '.tool_output // empty')
SUCCESS=$(echo "$INPUT" | jq -r '.success // true')

# Only track successful file operations
if [[ "$SUCCESS" != "true" ]]; then
    exit 0
fi

case "$TOOL_NAME" in
    Edit|Write|NotebookEdit)
        FILE=$(echo "$TOOL_INPUT" | jq -r '.file_path // empty')
        if [[ -n "$FILE" ]]; then
            # The undo daemon should already be tracking via inotify
            # But we can log the change source
            journal log "File changed by Claude: $FILE" --source "claude-code" --category "file_change" 2>/dev/null || true
        fi
        ;;
esac

exit 0
