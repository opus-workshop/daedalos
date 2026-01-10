"""Daedalos MCP Server - Exposes Daedalos tools to Claude."""

import asyncio
import json
import subprocess
import os
from pathlib import Path
from typing import Any

from mcp.server import Server
from mcp.server.stdio import stdio_server
from mcp.types import (
    Tool,
    TextContent,
    CallToolResult,
)


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
                "template": {"type": "string", "description": "Template: explorer, implementer, reviewer, debugger, watcher"},
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
                "agent": {"type": "string", "description": "Agent name to check inbox"},
                "all": {"type": "boolean", "description": "Show all messages including read", "default": False},
            },
            "required": ["agent"],
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
        return run_tool("error-db", ["match", arguments["error"]])

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
        args = ["inbox", arguments["agent"]]
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
