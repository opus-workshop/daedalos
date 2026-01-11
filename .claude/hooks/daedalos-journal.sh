#!/usr/bin/env bash
# Hook: Log significant Claude Code actions to Daedalos journal
#
# This hook runs after tool calls and logs them to the journal
# for visibility and audit purposes.

set -eo pipefail

# Only run if journal is available
command -v journal &>/dev/null || exit 0

# Parse hook input (JSON from stdin)
INPUT=$(cat)

HOOK_TYPE=$(echo "$INPUT" | jq -r '.hook_type // empty')
TOOL_NAME=$(echo "$INPUT" | jq -r '.tool_name // empty')
TOOL_INPUT=$(echo "$INPUT" | jq -r '.tool_input // empty')
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // empty')

# Only log certain tools
case "$TOOL_NAME" in
    Edit|Write|NotebookEdit)
        FILE=$(echo "$TOOL_INPUT" | jq -r '.file_path // empty')
        journal log "Claude edited: $FILE" --source "claude-code" --category "file_change"
        ;;
    Bash)
        CMD=$(echo "$TOOL_INPUT" | jq -r '.command // empty' | head -c 100)
        journal log "Claude ran: $CMD" --source "claude-code" --category "shell"
        ;;
    Task)
        DESC=$(echo "$TOOL_INPUT" | jq -r '.description // empty')
        journal log "Claude spawned agent: $DESC" --source "claude-code" --category "agent"
        ;;
esac

exit 0
