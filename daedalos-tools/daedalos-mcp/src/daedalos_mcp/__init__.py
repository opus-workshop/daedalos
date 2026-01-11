"""Daedalos MCP Server - Exposes Daedalos tools to Claude."""

import asyncio
import json
import subprocess
import os
import logging
from pathlib import Path
from typing import Any
from pydantic import AnyUrl

from mcp.server import Server
from mcp.server.stdio import stdio_server
from mcp.server.session import ServerSession
from mcp.types import (
    Tool,
    TextContent,
    CallToolResult,
    Resource,
    TextResourceContents,
    ServerNotification,
    ResourceUpdatedNotification,
    ResourceUpdatedNotificationParams,
)

logger = logging.getLogger(__name__)

# Subscription manager for resource updates
class SubscriptionManager:
    """Manages resource subscriptions and file watching."""

    def __init__(self):
        self.subscriptions: dict[str, set] = {}  # uri -> set of sessions
        self.sessions: dict[int, ServerSession] = {}  # id -> session
        self.watcher_task: asyncio.Task | None = None
        self.messages_dir = Path.home() / ".local/share/daedalos/agent/messages"
        self._last_mtime: float = 0

    def subscribe(self, uri: str, session: ServerSession) -> None:
        """Subscribe a session to a resource URI."""
        if uri not in self.subscriptions:
            self.subscriptions[uri] = set()
        session_id = id(session)
        self.subscriptions[uri].add(session_id)
        self.sessions[session_id] = session
        logger.info(f"Subscribed to {uri}")

        # Start watcher if this is an inbox subscription
        if "inbox" in uri and self.watcher_task is None:
            self.watcher_task = asyncio.create_task(self._watch_messages())

    def unsubscribe(self, uri: str, session: ServerSession) -> None:
        """Unsubscribe a session from a resource URI."""
        session_id = id(session)
        if uri in self.subscriptions:
            self.subscriptions[uri].discard(session_id)
            if not self.subscriptions[uri]:
                del self.subscriptions[uri]
        logger.info(f"Unsubscribed from {uri}")

    async def notify(self, uri: str) -> None:
        """Notify all subscribers that a resource has been updated."""
        if uri not in self.subscriptions:
            return

        for session_id in list(self.subscriptions[uri]):
            session = self.sessions.get(session_id)
            if session:
                try:
                    await session.send_resource_updated(AnyUrl(uri))
                    logger.info(f"Sent update notification for {uri}")
                except Exception as e:
                    logger.error(f"Failed to send notification: {e}")
                    # Remove dead session
                    self.subscriptions[uri].discard(session_id)

    async def _watch_messages(self) -> None:
        """Watch the messages directory for changes."""
        logger.info("Starting message watcher")
        while True:
            try:
                await asyncio.sleep(2)  # Check every 2 seconds

                if not self.messages_dir.exists():
                    continue

                # Check for any new/modified message files
                current_mtime = 0
                for f in self.messages_dir.iterdir():
                    if f.is_file():
                        mtime = f.stat().st_mtime
                        if mtime > current_mtime:
                            current_mtime = mtime

                if current_mtime > self._last_mtime:
                    self._last_mtime = current_mtime
                    await self.notify("daedalos://inbox")

            except asyncio.CancelledError:
                logger.info("Message watcher cancelled")
                break
            except Exception as e:
                logger.error(f"Watcher error: {e}")
                await asyncio.sleep(5)

# Global subscription manager
_subscriptions = SubscriptionManager()


# Tool definitions
TOOLS = [
    # Loop - iteration primitive
    Tool(
        name="loop_start",
        description="Start an iteration loop that runs until a promise (verification command) succeeds. Use this for iterative tasks like fixing tests or implementing features.",
        inputSchema={
            "type": "object",
            "properties": {
                "task": {"type": "string", "description": "Description of what you're trying to achieve"},
                "promise": {"type": "string", "description": "Shell command that should exit 0 when task is complete (e.g., 'pytest', 'npm test')"},
                "max_iterations": {"type": "integer", "description": "Maximum iterations before giving up", "default": 10},
            },
            "required": ["task", "promise"],
        },
    ),
    Tool(
        name="loop_status",
        description="Check the status of the current iteration loop",
        inputSchema={"type": "object", "properties": {}},
    ),
    Tool(
        name="loop_stop",
        description="Stop the current iteration loop",
        inputSchema={"type": "object", "properties": {}},
    ),

    # Verify - universal verification
    Tool(
        name="verify",
        description="Run verification checks (tests, linting, type checking) for the current project. Auto-detects project type.",
        inputSchema={
            "type": "object",
            "properties": {
                "quick": {"type": "boolean", "description": "Run only fast checks", "default": False},
                "path": {"type": "string", "description": "Path to verify (default: current directory)"},
            },
        },
    ),

    # Undo - file-level undo
    Tool(
        name="undo_checkpoint",
        description="Create a named checkpoint before making risky changes",
        inputSchema={
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Name for this checkpoint"},
            },
            "required": ["name"],
        },
    ),
    Tool(
        name="undo_last",
        description="Undo the last file change",
        inputSchema={"type": "object", "properties": {}},
    ),
    Tool(
        name="undo_timeline",
        description="Show the timeline of recent changes",
        inputSchema={
            "type": "object",
            "properties": {
                "limit": {"type": "integer", "description": "Number of entries to show", "default": 20},
            },
        },
    ),
    Tool(
        name="undo_restore",
        description="Restore to a specific checkpoint or change ID",
        inputSchema={
            "type": "object",
            "properties": {
                "id": {"type": "string", "description": "Checkpoint name or change ID to restore to"},
            },
            "required": ["id"],
        },
    ),

    # Project - codebase intelligence
    Tool(
        name="project_info",
        description="Get comprehensive information about the current project (type, structure, dependencies, conventions)",
        inputSchema={
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Path to project (default: current directory)"},
            },
        },
    ),
    Tool(
        name="project_symbols",
        description="List all symbols (functions, classes, types) in the project",
        inputSchema={
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Path to project"},
                "type": {"type": "string", "description": "Filter by symbol type (function, class, type, etc.)"},
            },
        },
    ),
    Tool(
        name="project_tree",
        description="Get the file tree structure of the project",
        inputSchema={
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Path to project"},
                "depth": {"type": "integer", "description": "Maximum depth", "default": 3},
            },
        },
    ),

    # Codex - semantic code search
    Tool(
        name="codex_search",
        description="Search code semantically by meaning, not just text. Find code related to concepts like 'authentication', 'error handling', etc.",
        inputSchema={
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Semantic search query"},
                "limit": {"type": "integer", "description": "Maximum results", "default": 10},
                "path": {"type": "string", "description": "Path to search in"},
            },
            "required": ["query"],
        },
    ),
    Tool(
        name="codex_index",
        description="Rebuild the semantic search index for the project",
        inputSchema={
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Path to index"},
            },
        },
    ),

    # Context - context window management
    Tool(
        name="context_estimate",
        description="Estimate current context window usage",
        inputSchema={"type": "object", "properties": {}},
    ),
    Tool(
        name="context_breakdown",
        description="Get detailed breakdown of what's consuming context",
        inputSchema={"type": "object", "properties": {}},
    ),

    # Error-db - error pattern database
    Tool(
        name="error_match",
        description="Find solutions for an error message from the error database",
        inputSchema={
            "type": "object",
            "properties": {
                "error": {"type": "string", "description": "The error message to look up"},
            },
            "required": ["error"],
        },
    ),
    Tool(
        name="error_add",
        description="Add a new error pattern and solution to the database",
        inputSchema={
            "type": "object",
            "properties": {
                "pattern": {"type": "string", "description": "Error pattern (regex)"},
                "solution": {"type": "string", "description": "How to fix this error"},
                "category": {"type": "string", "description": "Error category"},
            },
            "required": ["pattern", "solution"],
        },
    ),

    # Scratch - ephemeral environments
    Tool(
        name="scratch_new",
        description="Create a new ephemeral scratch environment for experimentation",
        inputSchema={
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Name for the scratch environment"},
            },
            "required": ["name"],
        },
    ),
    Tool(
        name="scratch_list",
        description="List all scratch environments",
        inputSchema={"type": "object", "properties": {}},
    ),
    Tool(
        name="scratch_destroy",
        description="Destroy a scratch environment",
        inputSchema={
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Name of scratch to destroy"},
            },
            "required": ["name"],
        },
    ),

    # Agent - multi-agent orchestration
    Tool(
        name="agent_spawn",
        description="Spawn a new Claude Code agent in a tmux session",
        inputSchema={
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Name for the agent"},
                "template": {"type": "string", "description": "Template: explorer, implementer, reviewer, debugger, planner, tester, watcher"},
                "project": {"type": "string", "description": "Project directory"},
                "no_focus": {"type": "boolean", "description": "Don't focus the new agent", "default": False},
            },
            "required": ["name"],
        },
    ),
    Tool(
        name="agent_list",
        description="List all running agents",
        inputSchema={"type": "object", "properties": {}},
    ),
    Tool(
        name="agent_focus",
        description="Focus (switch to) an agent",
        inputSchema={
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Agent name or slot number"},
            },
            "required": ["name"],
        },
    ),
    Tool(
        name="agent_search",
        description="Search across all agent outputs",
        inputSchema={
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Search query"},
                "agent": {"type": "string", "description": "Limit to specific agent"},
            },
            "required": ["query"],
        },
    ),
    Tool(
        name="agent_kill",
        description="Kill an agent",
        inputSchema={
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Agent name or slot number"},
                "force": {"type": "boolean", "description": "Force kill", "default": False},
            },
            "required": ["name"],
        },
    ),

    # Agent registration (for Claude Code terminals)
    Tool(
        name="agent_register",
        description="Register current terminal as an agent for inter-agent communication",
        inputSchema={
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Name for the agent"},
                "project": {"type": "string", "description": "Project directory (default: current)"},
            },
            "required": ["name"],
        },
    ),
    Tool(
        name="agent_unregister",
        description="Unregister an external agent",
        inputSchema={
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Agent name to unregister"},
            },
            "required": ["name"],
        },
    ),
    Tool(
        name="agent_whoami",
        description="Show current agent identity",
        inputSchema={"type": "object", "properties": {}},
    ),

    # Agent snapshots
    Tool(
        name="agent_snapshot",
        description="Create a snapshot of agent state(s) for later restoration",
        inputSchema={
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Agent name to snapshot (or omit for --all)"},
                "all": {"type": "boolean", "description": "Snapshot all agents", "default": False},
                "snapshot_name": {"type": "string", "description": "Name for this snapshot"},
            },
        },
    ),
    Tool(
        name="agent_snapshot_list",
        description="List available agent snapshots",
        inputSchema={"type": "object", "properties": {}},
    ),
    Tool(
        name="agent_restore",
        description="Restore agent(s) from a snapshot",
        inputSchema={
            "type": "object",
            "properties": {
                "snapshot": {"type": "string", "description": "Snapshot name to restore from"},
                "agent": {"type": "string", "description": "Restore only this agent from snapshot"},
            },
            "required": ["snapshot"],
        },
    ),

    # Inter-agent communication
    Tool(
        name="agent_send",
        description="Send a message to another agent",
        inputSchema={
            "type": "object",
            "properties": {
                "to": {"type": "string", "description": "Agent to send message to"},
                "message": {"type": "string", "description": "Message content"},
            },
            "required": ["to", "message"],
        },
    ),
    Tool(
        name="agent_inbox",
        description="Check messages for an agent",
        inputSchema={
            "type": "object",
            "properties": {
                "agent": {"type": "string", "description": "Agent name to check inbox (auto-registers if not specified)"},
                "all": {"type": "boolean", "description": "Show all messages including read", "default": False},
            },
        },
    ),
    Tool(
        name="agent_broadcast",
        description="Broadcast a message to all agents",
        inputSchema={
            "type": "object",
            "properties": {
                "message": {"type": "string", "description": "Message to broadcast"},
            },
            "required": ["message"],
        },
    ),
    Tool(
        name="agent_request_help",
        description="Request help from another agent (spawns helper if needed)",
        inputSchema={
            "type": "object",
            "properties": {
                "task": {"type": "string", "description": "Task description to get help with"},
                "template": {"type": "string", "description": "Preferred template for helper agent"},
            },
            "required": ["task"],
        },
    ),

    # Agent signals - coordination primitives
    Tool(
        name="agent_signal_complete",
        description="Signal that this agent's work is complete (for workflow coordination)",
        inputSchema={
            "type": "object",
            "properties": {
                "status": {"type": "string", "description": "Completion status: success, failure, blocked", "default": "success"},
                "data_file": {"type": "string", "description": "Path to file containing output data"},
            },
        },
    ),
    Tool(
        name="agent_signal_wait",
        description="Wait for another agent to signal completion",
        inputSchema={
            "type": "object",
            "properties": {
                "agent": {"type": "string", "description": "Agent name to wait for"},
                "timeout": {"type": "integer", "description": "Timeout in seconds", "default": 600},
            },
            "required": ["agent"],
        },
    ),
    Tool(
        name="agent_signal_check",
        description="Check if an agent has signaled completion (non-blocking)",
        inputSchema={
            "type": "object",
            "properties": {
                "agent": {"type": "string", "description": "Agent name to check"},
            },
            "required": ["agent"],
        },
    ),

    # Resource locks
    Tool(
        name="agent_lock_acquire",
        description="Acquire a lock on a shared resource",
        inputSchema={
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Lock name (resource identifier)"},
                "timeout": {"type": "integer", "description": "Timeout in seconds", "default": 30},
            },
            "required": ["name"],
        },
    ),
    Tool(
        name="agent_lock_release",
        description="Release a lock on a shared resource",
        inputSchema={
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Lock name to release"},
            },
            "required": ["name"],
        },
    ),
    Tool(
        name="agent_lock_list",
        description="List all active locks",
        inputSchema={"type": "object", "properties": {}},
    ),

    # Task claims
    Tool(
        name="agent_claim_create",
        description="Claim a task to prevent others from working on it",
        inputSchema={
            "type": "object",
            "properties": {
                "task_id": {"type": "string", "description": "Task identifier to claim"},
                "description": {"type": "string", "description": "Description of the task"},
            },
            "required": ["task_id"],
        },
    ),
    Tool(
        name="agent_claim_release",
        description="Release a task claim when done",
        inputSchema={
            "type": "object",
            "properties": {
                "task_id": {"type": "string", "description": "Task identifier to release"},
                "status": {"type": "string", "description": "Final status: completed, failed, abandoned", "default": "completed"},
            },
            "required": ["task_id"],
        },
    ),
    Tool(
        name="agent_claim_list",
        description="List all active task claims",
        inputSchema={"type": "object", "properties": {}},
    ),

    # Workflow tools - multi-agent pipelines
    Tool(
        name="workflow_list",
        description="List available workflows (feature, review, bugfix, tdd, refactor)",
        inputSchema={
            "type": "object",
            "properties": {
                "json": {"type": "boolean", "description": "Return as JSON", "default": False},
            },
        },
    ),
    Tool(
        name="workflow_start",
        description="Start a multi-agent workflow pipeline. Workflows coordinate multiple agents to complete complex tasks.",
        inputSchema={
            "type": "object",
            "properties": {
                "workflow": {"type": "string", "description": "Workflow name: feature, review, bugfix, tdd, refactor"},
                "task": {"type": "string", "description": "Task description for the workflow"},
                "project": {"type": "string", "description": "Project directory (default: current)"},
            },
            "required": ["workflow", "task"],
        },
    ),
    Tool(
        name="workflow_status",
        description="Check status of a workflow (or list all active workflows)",
        inputSchema={
            "type": "object",
            "properties": {
                "instance_id": {"type": "string", "description": "Workflow instance ID (omit to list all)"},
            },
        },
    ),
    Tool(
        name="workflow_stop",
        description="Stop a running workflow and kill its agents",
        inputSchema={
            "type": "object",
            "properties": {
                "instance_id": {"type": "string", "description": "Workflow instance ID to stop"},
                "force": {"type": "boolean", "description": "Force kill agents", "default": False},
            },
            "required": ["instance_id"],
        },
    ),

    # MCP Hub tools
    Tool(
        name="mcp_hub_status",
        description="Get status of the MCP hub daemon and running servers",
        inputSchema={
            "type": "object",
            "properties": {
                "json": {"type": "boolean", "description": "Return as JSON", "default": False},
            },
        },
    ),
    Tool(
        name="mcp_hub_warm",
        description="Pre-start MCP servers for fast response times",
        inputSchema={
            "type": "object",
            "properties": {
                "servers": {"type": "array", "items": {"type": "string"}, "description": "Server names to warm up"},
            },
            "required": ["servers"],
        },
    ),
    Tool(
        name="mcp_hub_list",
        description="List available MCP servers in the registry",
        inputSchema={
            "type": "object",
            "properties": {
                "category": {"type": "string", "description": "Filter by category"},
            },
        },
    ),
    Tool(
        name="mcp_hub_restart",
        description="Restart a running MCP server",
        inputSchema={
            "type": "object",
            "properties": {
                "server": {"type": "string", "description": "Server name to restart"},
            },
            "required": ["server"],
        },
    ),
    Tool(
        name="mcp_hub_logs",
        description="Get logs from an MCP server",
        inputSchema={
            "type": "object",
            "properties": {
                "server": {"type": "string", "description": "Server name"},
                "lines": {"type": "integer", "description": "Number of lines", "default": 50},
            },
            "required": ["server"],
        },
    ),
    Tool(
        name="mcp_hub_call",
        description="Call a tool through the MCP hub",
        inputSchema={
            "type": "object",
            "properties": {
                "tool": {"type": "string", "description": "Tool name to call"},
                "arguments": {"type": "object", "description": "Tool arguments as JSON"},
                "server": {"type": "string", "description": "Specific server to use (optional)"},
            },
            "required": ["tool"],
        },
    ),

    # LSP Pool tools - pre-warmed language servers
    Tool(
        name="lsp_pool_status",
        description="Get status of the LSP Pool daemon and running language servers",
        inputSchema={
            "type": "object",
            "properties": {
                "json": {"type": "boolean", "description": "Return as JSON", "default": False},
            },
        },
    ),
    Tool(
        name="lsp_pool_warm",
        description="Pre-warm a language server for faster code intelligence",
        inputSchema={
            "type": "object",
            "properties": {
                "language": {"type": "string", "description": "Language: typescript, python, rust, go, etc."},
                "path": {"type": "string", "description": "Project path to initialize server with"},
            },
            "required": ["language"],
        },
    ),
    Tool(
        name="lsp_pool_cool",
        description="Stop language servers for a language",
        inputSchema={
            "type": "object",
            "properties": {
                "language": {"type": "string", "description": "Language to stop servers for"},
            },
            "required": ["language"],
        },
    ),
    Tool(
        name="lsp_pool_list",
        description="List all running language servers in the pool",
        inputSchema={
            "type": "object",
            "properties": {
                "json": {"type": "boolean", "description": "Return as JSON", "default": False},
            },
        },
    ),
    Tool(
        name="lsp_pool_query",
        description="Send an LSP query for code intelligence (hover, definition, references, completion)",
        inputSchema={
            "type": "object",
            "properties": {
                "command": {"type": "string", "description": "Query type: hover, definition, references, completion, diagnostics"},
                "file": {"type": "string", "description": "File to query"},
                "line": {"type": "integer", "description": "Line number (1-based)"},
                "col": {"type": "integer", "description": "Column number (1-based)"},
            },
            "required": ["command", "file"],
        },
    ),
    Tool(
        name="lsp_pool_languages",
        description="List all supported languages and their server configurations",
        inputSchema={"type": "object", "properties": {}},
    ),
    Tool(
        name="lsp_pool_logs",
        description="Get stderr logs from a language server",
        inputSchema={
            "type": "object",
            "properties": {
                "key": {"type": "string", "description": "Server key in format language:project"},
            },
            "required": ["key"],
        },
    ),
    Tool(
        name="lsp_pool_restart",
        description="Restart a language server",
        inputSchema={
            "type": "object",
            "properties": {
                "key": {"type": "string", "description": "Server key in format language:project"},
            },
            "required": ["key"],
        },
    ),

    # Sandbox tools - ephemeral experiment environments
    Tool(
        name="sandbox_create",
        description="Create a new ephemeral sandbox environment for safe experimentation. Changes don't affect the original until promoted.",
        inputSchema={
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Name for the sandbox (auto-generated if not provided)"},
                "from_path": {"type": "string", "description": "Source directory to sandbox (default: current dir)"},
                "backend": {"type": "string", "description": "Copy strategy: btrfs, overlay, or rsync (auto-detected)"},
            },
        },
    ),
    Tool(
        name="sandbox_list",
        description="List all sandbox environments",
        inputSchema={
            "type": "object",
            "properties": {
                "json": {"type": "boolean", "description": "Return as JSON", "default": False},
            },
        },
    ),
    Tool(
        name="sandbox_enter",
        description="Enter a sandbox environment (changes directory to sandbox working copy)",
        inputSchema={
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Sandbox name"},
                "command": {"type": "string", "description": "Command to run instead of shell"},
            },
            "required": ["name"],
        },
    ),
    Tool(
        name="sandbox_diff",
        description="Show changes made in a sandbox compared to source",
        inputSchema={
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Sandbox name"},
                "files_only": {"type": "boolean", "description": "List changed files only", "default": False},
            },
            "required": ["name"],
        },
    ),
    Tool(
        name="sandbox_promote",
        description="Apply sandbox changes to the original source directory",
        inputSchema={
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Sandbox name"},
                "dry_run": {"type": "boolean", "description": "Show what would be promoted without doing it", "default": False},
                "backup": {"type": "boolean", "description": "Create backup of source first", "default": False},
            },
            "required": ["name"],
        },
    ),
    Tool(
        name="sandbox_discard",
        description="Delete a sandbox and all its changes",
        inputSchema={
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Sandbox name"},
            },
            "required": ["name"],
        },
    ),
    Tool(
        name="sandbox_info",
        description="Show detailed information about a sandbox",
        inputSchema={
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Sandbox name"},
                "json": {"type": "boolean", "description": "Return as JSON", "default": False},
            },
            "required": ["name"],
        },
    ),
    Tool(
        name="sandbox_run",
        description="Run a command in a sandbox without entering it",
        inputSchema={
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Sandbox name"},
                "command": {"type": "string", "description": "Command to run"},
            },
            "required": ["name", "command"],
        },
    ),

    # Gates tools - supervision and approval checkpoints
    Tool(
        name="gates_check",
        description="Check if an action is allowed through a supervision gate. Returns 'allowed' or 'denied' with reason.",
        inputSchema={
            "type": "object",
            "properties": {
                "gate": {"type": "string", "description": "Gate name: file_delete, file_create, file_modify, git_commit, git_push, git_force_push, loop_start, agent_spawn, shell_command, sensitive_file"},
                "context": {"type": "object", "description": "Additional context (e.g., {path: '/path/to/file'})"},
                "source": {"type": "string", "description": "Calling tool/agent name", "default": "mcp"},
            },
            "required": ["gate"],
        },
    ),
    Tool(
        name="gates_level",
        description="Get or set the supervision level (autonomous, supervised, collaborative, assisted, manual)",
        inputSchema={
            "type": "object",
            "properties": {
                "level": {"type": "string", "description": "New level to set (omit to get current level)"},
            },
        },
    ),
    Tool(
        name="gates_set",
        description="Set the action for a specific gate (allow, notify, approve, deny)",
        inputSchema={
            "type": "object",
            "properties": {
                "gate": {"type": "string", "description": "Gate name"},
                "action": {"type": "string", "description": "Action: allow, notify, approve, deny"},
            },
            "required": ["gate", "action"],
        },
    ),
    Tool(
        name="gates_config",
        description="Get the current supervision configuration",
        inputSchema={
            "type": "object",
            "properties": {
                "json": {"type": "boolean", "description": "Return as JSON", "default": True},
            },
        },
    ),
    Tool(
        name="gates_history",
        description="Get gate check history (audit trail)",
        inputSchema={
            "type": "object",
            "properties": {
                "gate": {"type": "string", "description": "Filter by gate name"},
                "days": {"type": "integer", "description": "Number of days to look back", "default": 7},
                "limit": {"type": "integer", "description": "Maximum entries", "default": 20},
            },
        },
    ),

    # Spec tools - rich specification management
    Tool(
        name="spec_show",
        description="Display the spec for a component. Specs contain intent (WHY), constraints, interface, examples, decisions, and anti-patterns.",
        inputSchema={
            "type": "object",
            "properties": {
                "component": {"type": "string", "description": "Component name (e.g., 'undo', 'loop')"},
                "section": {"type": "string", "description": "Specific section: intent, constraints, interface, examples, decisions, anti_patterns"},
            },
            "required": ["component"],
        },
    ),
    Tool(
        name="spec_query",
        description="Search across all specs semantically. Find answers to 'why' questions and design decisions.",
        inputSchema={
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Search query (e.g., 'why sqlite', 'what to avoid with undo')"},
            },
            "required": ["query"],
        },
    ),
    Tool(
        name="spec_list",
        description="List all specs in the project",
        inputSchema={
            "type": "object",
            "properties": {
                "missing": {"type": "boolean", "description": "Show components without specs", "default": False},
                "stale": {"type": "boolean", "description": "Show specs older than implementation", "default": False},
            },
        },
    ),
    Tool(
        name="spec_context",
        description="Get relevant spec sections for a task. Use before starting work to load intent and anti-patterns.",
        inputSchema={
            "type": "object",
            "properties": {
                "task": {"type": "string", "description": "Task description (e.g., 'fix undo restore command')"},
            },
            "required": ["task"],
        },
    ),
    Tool(
        name="spec_validate",
        description="Validate spec format and completeness",
        inputSchema={
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Path to validate (default: current directory)"},
            },
        },
    ),
    Tool(
        name="spec_new",
        description="Create a new spec from template",
        inputSchema={
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Component name"},
                "type": {"type": "string", "description": "Type: tool, library, service, doc", "default": "tool"},
            },
            "required": ["name"],
        },
    ),

    # Journal tools - narrative reconstruction
    Tool(
        name="journal_what",
        description="Answer 'what happened?' - get a narrative of recent events from all Daedalos tools",
        inputSchema={
            "type": "object",
            "properties": {
                "hours": {"type": "number", "description": "Hours to look back", "default": 1},
                "source": {"type": "string", "description": "Filter by source: gates, loop, agent, undo, mcp-hub"},
                "verbose": {"type": "boolean", "description": "Include more details", "default": False},
            },
        },
    ),
    Tool(
        name="journal_events",
        description="List raw events from the journal",
        inputSchema={
            "type": "object",
            "properties": {
                "hours": {"type": "number", "description": "Hours to look back", "default": 24},
                "source": {"type": "string", "description": "Filter by source"},
                "event_type": {"type": "string", "description": "Filter by event type"},
                "limit": {"type": "integer", "description": "Maximum events", "default": 100},
            },
        },
    ),
    Tool(
        name="journal_summary",
        description="Get a summary of recent events",
        inputSchema={
            "type": "object",
            "properties": {
                "hours": {"type": "number", "description": "Hours to look back", "default": 24},
            },
        },
    ),
    Tool(
        name="journal_log",
        description="Log a custom event to the journal",
        inputSchema={
            "type": "object",
            "properties": {
                "summary": {"type": "string", "description": "Event summary/description"},
                "source": {"type": "string", "description": "Source identifier", "default": "mcp"},
                "event_type": {"type": "string", "description": "Event type", "default": "custom"},
            },
            "required": ["summary"],
        },
    ),
]


def run_command(cmd: list[str], cwd: str | None = None) -> tuple[int, str, str]:
    """Run a shell command and return (returncode, stdout, stderr)."""
    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            cwd=cwd,
            timeout=60,
        )
        return result.returncode, result.stdout, result.stderr
    except subprocess.TimeoutExpired:
        return 1, "", "Command timed out"
    except Exception as e:
        return 1, "", str(e)


def run_tool(name: str, args: list[str], cwd: str | None = None) -> str:
    """Run a Daedalos tool and return output."""
    # Tools are in ~/.local/bin
    tool_path = Path.home() / ".local" / "bin" / name

    if not tool_path.exists():
        return f"Error: Tool '{name}' not found at {tool_path}"

    code, stdout, stderr = run_command([str(tool_path)] + args, cwd=cwd)

    output = stdout
    if stderr:
        output += f"\n[stderr]: {stderr}"
    if code != 0:
        output += f"\n[exit code: {code}]"

    return output.strip() or "(no output)"


# Tool handlers
async def handle_tool(name: str, arguments: dict[str, Any]) -> str:
    """Handle a tool call."""
    cwd = arguments.get("path") or os.getcwd()

    # Loop tools
    if name == "loop_start":
        args = ["start", arguments["task"], "--promise", arguments["promise"]]
        if arguments.get("max_iterations"):
            args.extend(["-n", str(arguments["max_iterations"])])
        return run_tool("loop", args, cwd)

    elif name == "loop_status":
        return run_tool("loop", ["status"], cwd)

    elif name == "loop_stop":
        return run_tool("loop", ["stop"], cwd)

    # Verify
    elif name == "verify":
        args = []
        if arguments.get("quick"):
            args.append("--quick")
        return run_tool("verify", args, cwd)

    # Undo tools
    elif name == "undo_checkpoint":
        return run_tool("undo", ["checkpoint", arguments["name"]], cwd)

    elif name == "undo_last":
        return run_tool("undo", ["last"], cwd)

    elif name == "undo_timeline":
        args = ["timeline"]
        if arguments.get("limit"):
            args.extend(["-n", str(arguments["limit"])])
        return run_tool("undo", args, cwd)

    elif name == "undo_restore":
        return run_tool("undo", ["to", arguments["id"]], cwd)

    # Project tools
    elif name == "project_info":
        return run_tool("project", ["summary"], cwd)

    elif name == "project_symbols":
        args = ["search"]
        if arguments.get("type"):
            args.extend(["--type", arguments["type"]])
        args.append("*")  # Search all symbols
        return run_tool("project", args, cwd)

    elif name == "project_tree":
        args = ["tree"]
        if arguments.get("depth"):
            args.extend(["--depth", str(arguments["depth"])])
        return run_tool("project", args, cwd)

    # Codex tools
    elif name == "codex_search":
        args = ["search", arguments["query"]]
        if arguments.get("limit"):
            args.extend(["--limit", str(arguments["limit"])])
        return run_tool("codex", args, cwd)

    elif name == "codex_index":
        return run_tool("codex", ["index"], cwd)

    # Context tools
    elif name == "context_estimate":
        return run_tool("context", ["estimate"])

    elif name == "context_breakdown":
        return run_tool("context", ["breakdown"])

    # Error-db tools
    elif name == "error_match":
        return run_tool("error-db", ["search", arguments["error"]])

    elif name == "error_add":
        args = ["add", arguments["pattern"], "--solution", arguments["solution"]]
        if arguments.get("category"):
            args.extend(["--category", arguments["category"]])
        return run_tool("error-db", args)

    # Scratch tools
    elif name == "scratch_new":
        return run_tool("scratch", ["new", arguments["name"]], cwd)

    elif name == "scratch_list":
        return run_tool("scratch", ["list"])

    elif name == "scratch_destroy":
        return run_tool("scratch", ["destroy", arguments["name"]])

    # Agent tools
    elif name == "agent_spawn":
        args = ["spawn", "-n", arguments["name"]]
        if arguments.get("template"):
            args.extend(["-t", arguments["template"]])
        if arguments.get("project"):
            args.extend(["-p", arguments["project"]])
        if arguments.get("no_focus"):
            args.append("--no-focus")
        return run_tool("agent", args)

    elif name == "agent_list":
        return run_tool("agent", ["list"])

    elif name == "agent_focus":
        return run_tool("agent", ["focus", arguments["name"]])

    elif name == "agent_search":
        args = ["search", arguments["query"]]
        if arguments.get("agent"):
            args.extend(["-a", arguments["agent"]])
        return run_tool("agent", args)

    elif name == "agent_kill":
        args = ["kill", arguments["name"]]
        if arguments.get("force"):
            args.append("--force")
        return run_tool("agent", args)

    # Agent registration tools
    elif name == "agent_register":
        args = ["register", arguments["name"]]
        if arguments.get("project"):
            args.extend(["-p", arguments["project"]])
        return run_tool("agent", args, cwd)

    elif name == "agent_unregister":
        return run_tool("agent", ["unregister", arguments["name"]])

    elif name == "agent_whoami":
        return run_tool("agent", ["whoami"])

    # Agent snapshot tools
    elif name == "agent_snapshot":
        args = ["snapshot"]
        if arguments.get("name"):
            args.append(arguments["name"])
        if arguments.get("all"):
            args.append("--all")
        if arguments.get("snapshot_name"):
            args.extend(["--name", arguments["snapshot_name"]])
        return run_tool("agent", args)

    elif name == "agent_snapshot_list":
        return run_tool("agent", ["snapshot", "list"])

    elif name == "agent_restore":
        args = ["restore", arguments["snapshot"]]
        if arguments.get("agent"):
            args.extend(["--agent", arguments["agent"]])
        return run_tool("agent", args)

    # Inter-agent communication
    elif name == "agent_send":
        return run_tool("agent", ["send", arguments["to"], arguments["message"]])

    elif name == "agent_inbox":
        args = ["inbox"]
        if arguments.get("agent"):
            args.append(arguments["agent"])
        if arguments.get("all"):
            args.append("--all")
        return run_tool("agent", args)

    elif name == "agent_broadcast":
        return run_tool("agent", ["broadcast", arguments["message"]])

    elif name == "agent_request_help":
        args = ["request-help", arguments["task"]]
        if arguments.get("template"):
            args.extend(["--template", arguments["template"]])
        return run_tool("agent", args)

    # Agent signals
    elif name == "agent_signal_complete":
        args = ["signal", "complete"]
        if arguments.get("status"):
            args.extend(["--status", arguments["status"]])
        if arguments.get("data_file"):
            args.extend(["--data", arguments["data_file"]])
        return run_tool("agent", args)

    elif name == "agent_signal_wait":
        args = ["signal", "wait", arguments["agent"]]
        if arguments.get("timeout"):
            args.append(str(arguments["timeout"]))
        return run_tool("agent", args)

    elif name == "agent_signal_check":
        return run_tool("agent", ["signal", "check", arguments["agent"]])

    # Resource locks
    elif name == "agent_lock_acquire":
        args = ["lock", "acquire", arguments["name"]]
        if arguments.get("timeout"):
            args.append(str(arguments["timeout"]))
        return run_tool("agent", args)

    elif name == "agent_lock_release":
        return run_tool("agent", ["lock", "release", arguments["name"]])

    elif name == "agent_lock_list":
        return run_tool("agent", ["lock", "list"])

    # Task claims
    elif name == "agent_claim_create":
        args = ["claim", "create", arguments["task_id"]]
        if arguments.get("description"):
            args.append(arguments["description"])
        return run_tool("agent", args)

    elif name == "agent_claim_release":
        args = ["claim", "release", arguments["task_id"]]
        if arguments.get("status"):
            args.append(arguments["status"])
        return run_tool("agent", args)

    elif name == "agent_claim_list":
        return run_tool("agent", ["claim", "list"])

    # Workflow tools
    elif name == "workflow_list":
        args = ["workflow", "list"]
        if arguments.get("json"):
            args.append("--json")
        return run_tool("agent", args)

    elif name == "workflow_start":
        args = ["workflow", "start", arguments["workflow"], arguments["task"]]
        if arguments.get("project"):
            args.extend(["--project", arguments["project"]])
        return run_tool("agent", args)

    elif name == "workflow_status":
        args = ["workflow", "status"]
        if arguments.get("instance_id"):
            args.append(arguments["instance_id"])
        return run_tool("agent", args)

    elif name == "workflow_stop":
        args = ["workflow", "stop", arguments["instance_id"]]
        if arguments.get("force"):
            args.append("--force")
        return run_tool("agent", args)

    # MCP Hub tools
    elif name == "mcp_hub_status":
        args = ["status"]
        if arguments.get("json"):
            args.append("--json")
        return run_tool("mcp-hub", args)

    elif name == "mcp_hub_warm":
        servers = arguments.get("servers", [])
        return run_tool("mcp-hub", ["warm"] + servers)

    elif name == "mcp_hub_list":
        args = ["list"]
        if arguments.get("category"):
            args.extend(["--category", arguments["category"]])
        return run_tool("mcp-hub", args)

    elif name == "mcp_hub_restart":
        return run_tool("mcp-hub", ["restart", arguments["server"]])

    elif name == "mcp_hub_logs":
        args = ["logs", arguments["server"]]
        if arguments.get("lines"):
            args.extend(["-n", str(arguments["lines"])])
        return run_tool("mcp-hub", args)

    elif name == "mcp_hub_call":
        args = ["call", arguments["tool"]]
        if arguments.get("server"):
            args.extend(["--server", arguments["server"]])
        # Convert arguments to command line args
        tool_args = arguments.get("arguments", {})
        for key, value in tool_args.items():
            args.extend([f"--{key}", str(value)])
        return run_tool("mcp-hub", args)

    # LSP Pool tools
    elif name == "lsp_pool_status":
        args = ["status"]
        if arguments.get("json"):
            args.append("--json")
        return run_tool("lsp-pool", args)

    elif name == "lsp_pool_warm":
        args = ["warm", arguments["language"]]
        if arguments.get("path"):
            args.append(arguments["path"])
        return run_tool("lsp-pool", args)

    elif name == "lsp_pool_cool":
        return run_tool("lsp-pool", ["cool", arguments["language"]])

    elif name == "lsp_pool_list":
        args = ["list"]
        if arguments.get("json"):
            args.append("--json")
        return run_tool("lsp-pool", args)

    elif name == "lsp_pool_query":
        args = ["query", arguments["command"], arguments["file"]]
        if arguments.get("line"):
            args.extend(["--line", str(arguments["line"])])
        if arguments.get("col"):
            args.extend(["--col", str(arguments["col"])])
        return run_tool("lsp-pool", args)

    elif name == "lsp_pool_languages":
        return run_tool("lsp-pool", ["languages"])

    elif name == "lsp_pool_logs":
        return run_tool("lsp-pool", ["logs", arguments["key"]])

    elif name == "lsp_pool_restart":
        return run_tool("lsp-pool", ["restart", arguments["key"]])

    # Sandbox tools
    elif name == "sandbox_create":
        args = ["create"]
        if arguments.get("name"):
            args.append(arguments["name"])
        if arguments.get("from_path"):
            args.extend(["--from", arguments["from_path"]])
        if arguments.get("backend"):
            args.extend(["--copy", arguments["backend"]])
        return run_tool("sandbox", args)

    elif name == "sandbox_list":
        args = ["list"]
        if arguments.get("json"):
            args.append("--json")
        return run_tool("sandbox", args)

    elif name == "sandbox_enter":
        args = ["enter", arguments["name"]]
        if arguments.get("command"):
            args.extend(["--command", arguments["command"]])
        return run_tool("sandbox", args)

    elif name == "sandbox_diff":
        args = ["diff", arguments["name"]]
        if arguments.get("files_only"):
            args.append("--files")
        return run_tool("sandbox", args)

    elif name == "sandbox_promote":
        args = ["promote", arguments["name"], "--yes"]  # Non-interactive
        if arguments.get("dry_run"):
            args.append("--dry-run")
        if arguments.get("backup"):
            args.append("--backup")
        return run_tool("sandbox", args)

    elif name == "sandbox_discard":
        return run_tool("sandbox", ["discard", arguments["name"], "--force"])

    elif name == "sandbox_info":
        args = ["info", arguments["name"]]
        if arguments.get("json"):
            args.append("--json")
        return run_tool("sandbox", args)

    elif name == "sandbox_run":
        return run_tool("sandbox", ["run", arguments["name"], arguments["command"]])

    # Gates tools
    elif name == "gates_check":
        import json as json_mod
        args = ["check", arguments["gate"]]
        if arguments.get("context"):
            args.append(json_mod.dumps(arguments["context"]))
        if arguments.get("source"):
            args.append(arguments["source"])
        return run_tool("gates", args)

    elif name == "gates_level":
        args = ["level"]
        if arguments.get("level"):
            args.append(arguments["level"])
        return run_tool("gates", args)

    elif name == "gates_set":
        return run_tool("gates", ["set", arguments["gate"], arguments["action"]])

    elif name == "gates_config":
        args = ["config"]
        if arguments.get("json", True):
            args.append("--json")
        return run_tool("gates", args)

    elif name == "gates_history":
        args = ["history", "--json"]
        if arguments.get("gate"):
            args.extend(["--gate", arguments["gate"]])
        if arguments.get("days"):
            args.extend(["--days", str(arguments["days"])])
        if arguments.get("limit"):
            args.extend(["--limit", str(arguments["limit"])])
        return run_tool("gates", args)

    # Spec tools
    elif name == "spec_show":
        args = ["show", arguments["component"]]
        if arguments.get("section"):
            args.extend(["--section", arguments["section"]])
        return run_tool("spec", args, cwd)

    elif name == "spec_query":
        return run_tool("spec", ["query", arguments["query"]], cwd)

    elif name == "spec_list":
        args = ["list"]
        if arguments.get("missing"):
            args.append("--missing")
        if arguments.get("stale"):
            args.append("--stale")
        return run_tool("spec", args, cwd)

    elif name == "spec_context":
        return run_tool("spec", ["context", arguments["task"]], cwd)

    elif name == "spec_validate":
        args = ["validate"]
        if arguments.get("path"):
            args.append(arguments["path"])
        return run_tool("spec", args, cwd)

    elif name == "spec_new":
        args = ["new", arguments["name"]]
        if arguments.get("type"):
            args.extend(["--type", arguments["type"]])
        return run_tool("spec", args, cwd)

    # Journal tools
    elif name == "journal_what":
        args = ["what"]
        if arguments.get("hours"):
            args.extend(["-h", str(arguments["hours"])])
        if arguments.get("source"):
            args.extend(["--source", arguments["source"]])
        if arguments.get("verbose"):
            args.append("-v")
        return run_tool("journal", args)

    elif name == "journal_events":
        args = ["events", "--json"]
        if arguments.get("hours"):
            args.extend(["-h", str(arguments["hours"])])
        if arguments.get("source"):
            args.extend(["--source", arguments["source"]])
        if arguments.get("event_type"):
            args.extend(["--type", arguments["event_type"]])
        if arguments.get("limit"):
            args.extend(["--limit", str(arguments["limit"])])
        return run_tool("journal", args)

    elif name == "journal_summary":
        args = ["summary", "--json"]
        if arguments.get("hours"):
            args.extend(["-h", str(arguments["hours"])])
        return run_tool("journal", args)

    elif name == "journal_log":
        return run_tool("journal", [
            "log",
            arguments["summary"],
            arguments.get("source", "mcp"),
            arguments.get("event_type", "custom"),
        ])

    else:
        return f"Unknown tool: {name}"


def create_server() -> Server:
    """Create and configure the MCP server."""
    server = Server("daedalos")

    @server.list_tools()
    async def list_tools() -> list[Tool]:
        return TOOLS

    @server.call_tool()
    async def call_tool(name: str, arguments: dict[str, Any]) -> CallToolResult:
        result = await handle_tool(name, arguments)
        return CallToolResult(
            content=[TextContent(type="text", text=result)]
        )

    @server.list_resources()
    async def list_resources() -> list[Resource]:
        """List available resources."""
        return [
            Resource(
                uri="daedalos://inbox",
                name="Agent Inbox",
                description="Unread messages for this agent. Check this resource to see messages from other agents.",
                mimeType="text/plain",
            ),
            Resource(
                uri="daedalos://agents",
                name="Active Agents",
                description="List of all active agents that can send/receive messages.",
                mimeType="application/json",
            ),
        ]

    @server.read_resource()
    async def read_resource(uri: str) -> TextResourceContents:
        """Read a resource by URI."""
        if uri == "daedalos://inbox":
            # Get inbox for current agent (auto-registers if needed)
            result = await asyncio.to_thread(
                subprocess.run,
                ["agent", "inbox", "--all"],
                capture_output=True,
                text=True,
                cwd=os.getcwd(),
            )
            content = result.stdout if result.returncode == 0 else f"Error: {result.stderr}"
            if not content.strip():
                content = "No messages."
            return TextResourceContents(
                uri=uri,
                mimeType="text/plain",
                text=content,
            )
        elif uri == "daedalos://agents":
            # List all agents
            result = await asyncio.to_thread(
                subprocess.run,
                ["agent", "list", "--json"],
                capture_output=True,
                text=True,
                cwd=os.getcwd(),
            )
            content = result.stdout if result.returncode == 0 else f"Error: {result.stderr}"
            return TextResourceContents(
                uri=uri,
                mimeType="application/json",
                text=content,
            )
        else:
            raise ValueError(f"Unknown resource: {uri}")

    @server.subscribe_resource()
    async def subscribe_resource(uri: AnyUrl) -> None:
        """Handle resource subscription requests."""
        session = server.request_context.session
        _subscriptions.subscribe(str(uri), session)

    @server.unsubscribe_resource()
    async def unsubscribe_resource(uri: AnyUrl) -> None:
        """Handle resource unsubscription requests."""
        session = server.request_context.session
        _subscriptions.unsubscribe(str(uri), session)

    return server


async def run_server():
    """Run the MCP server."""
    server = create_server()
    async with stdio_server() as (read_stream, write_stream):
        await server.run(read_stream, write_stream, server.create_initialization_options())


def main():
    """Entry point."""
    asyncio.run(run_server())


if __name__ == "__main__":
    main()
