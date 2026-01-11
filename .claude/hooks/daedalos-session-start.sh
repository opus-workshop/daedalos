#!/usr/bin/env bash
# Hook: Session start - create automatic checkpoint
#
# This hook runs when a Claude Code session starts.
# Creates an undo checkpoint for easy rollback if session goes wrong.

set -eo pipefail

# Only run if undo is available
command -v undo &>/dev/null || exit 0

# Create session checkpoint
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
undo checkpoint "claude-session-$TIMESTAMP" 2>/dev/null || true

# Log session start
if command -v journal &>/dev/null; then
    journal log "Claude Code session started" --source "claude-code" --category "session" 2>/dev/null || true
fi

exit 0
