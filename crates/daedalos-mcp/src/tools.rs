//! Daedalos Tool Definitions (Consolidated)
//!
//! Defines all available Daedalos tools for exposure via MCP.
//! Tools are consolidated by noun - each tool has an `action` parameter.
//! This reduces context overhead from ~57k tokens to ~15k tokens.

use serde_json::{json, Value};
use crate::protocol::{Tool, InputSchema};

/// Create a tool definition with the given name, description, and schema properties
fn tool(
    name: &str,
    description: &str,
    properties: Value,
    required: Vec<&str>,
) -> Tool {
    let props = properties.as_object().cloned().unwrap_or_default();
    Tool {
        name: name.to_string(),
        description: description.to_string(),
        input_schema: InputSchema {
            schema_type: "object".to_string(),
            properties: props,
            required: required.into_iter().map(|s| s.to_string()).collect(),
        },
    }
}

/// Get all available Daedalos tools (consolidated)
pub fn all_tools() -> Vec<Tool> {
    vec![
        // =========================================================================
        // AGENT - multi-agent orchestration (replaces 20+ tools)
        // =========================================================================
        tool(
            "agent",
            "Manage Claude Code agents in tmux. Actions: spawn, list, focus, kill, register, unregister, whoami, snapshot, snapshots, restore, send, inbox, broadcast, request_help, signal_complete, signal_wait, signal_check, lock_acquire, lock_release, lock_list, claim_create, claim_release, claim_list",
            json!({
                "action": {
                    "type": "string",
                    "description": "Action to perform",
                    "enum": ["spawn", "list", "focus", "kill", "search", "register", "unregister", "whoami", "snapshot", "snapshots", "restore", "send", "inbox", "broadcast", "request_help", "signal_complete", "signal_wait", "signal_check", "lock_acquire", "lock_release", "lock_list", "claim_create", "claim_release", "claim_list"]
                },
                "name": {"type": "string", "description": "Agent name (spawn, focus, kill, register, send, snapshot, restore, signal_wait, signal_check, lock_*, claim_*)"},
                "template": {"type": "string", "description": "Template: explorer, implementer, reviewer, debugger, planner, tester, watcher (spawn, request_help)"},
                "project": {"type": "string", "description": "Project directory (spawn, register)"},
                "query": {"type": "string", "description": "Search query (search)"},
                "message": {"type": "string", "description": "Message content (send, broadcast)"},
                "to": {"type": "string", "description": "Target agent (send)"},
                "task": {"type": "string", "description": "Task description (request_help, claim_create)"},
                "task_id": {"type": "string", "description": "Task identifier (claim_*)"},
                "status": {"type": "string", "description": "Status (signal_complete, claim_release)"},
                "timeout": {"type": "integer", "description": "Timeout seconds (signal_wait, lock_acquire)"},
                "force": {"type": "boolean", "description": "Force operation (kill)"},
                "all": {"type": "boolean", "description": "Apply to all (snapshot, inbox)"},
                "snapshot_name": {"type": "string", "description": "Snapshot name (snapshot, restore)"},
                "agent": {"type": "string", "description": "Agent name for filtering (search, inbox, restore, signal_wait, signal_check)"},
                "data_file": {"type": "string", "description": "Data file path (signal_complete)"},
                "description": {"type": "string", "description": "Task description (claim_create)"},
                "no_focus": {"type": "boolean", "description": "Don't focus new agent (spawn)"}
            }),
            vec!["action"],
        ),

        // =========================================================================
        // WORKFLOW - multi-agent pipelines (replaces 4 tools)
        // =========================================================================
        tool(
            "workflow",
            "Manage multi-agent workflow pipelines. Actions: list, start, status, stop",
            json!({
                "action": {
                    "type": "string",
                    "description": "Action to perform",
                    "enum": ["list", "start", "status", "stop"]
                },
                "workflow": {"type": "string", "description": "Workflow name: feature, review, bugfix, tdd, refactor (start)"},
                "task": {"type": "string", "description": "Task description (start)"},
                "project": {"type": "string", "description": "Project directory (start)"},
                "instance_id": {"type": "string", "description": "Workflow instance ID (status, stop)"},
                "force": {"type": "boolean", "description": "Force stop (stop)"},
                "json": {"type": "boolean", "description": "JSON output (list)"}
            }),
            vec!["action"],
        ),

        // =========================================================================
        // SANDBOX - ephemeral environments (replaces 8 tools)
        // =========================================================================
        tool(
            "sandbox",
            "Create and manage isolated development environments. Actions: create, list, enter, diff, promote, discard, info, run",
            json!({
                "action": {
                    "type": "string",
                    "description": "Action to perform",
                    "enum": ["create", "list", "enter", "diff", "promote", "discard", "info", "run"]
                },
                "name": {"type": "string", "description": "Sandbox name"},
                "from_path": {"type": "string", "description": "Source directory (create)"},
                "backend": {"type": "string", "description": "Copy strategy: btrfs, overlay, rsync (create)"},
                "command": {"type": "string", "description": "Command to run (enter, run)"},
                "dry_run": {"type": "boolean", "description": "Preview changes (promote)"},
                "backup": {"type": "boolean", "description": "Create backup (promote)"},
                "files_only": {"type": "boolean", "description": "List files only (diff)"},
                "json": {"type": "boolean", "description": "JSON output (list, info)"}
            }),
            vec!["action"],
        ),

        // =========================================================================
        // LSP_POOL - language servers (replaces 8 tools)
        // =========================================================================
        tool(
            "lsp_pool",
            "Manage pooled language servers. Actions: status, warm, cool, list, query, languages, logs, restart",
            json!({
                "action": {
                    "type": "string",
                    "description": "Action to perform",
                    "enum": ["status", "warm", "cool", "list", "query", "languages", "logs", "restart"]
                },
                "language": {"type": "string", "description": "Language (warm, cool)"},
                "path": {"type": "string", "description": "Project path (warm)"},
                "file": {"type": "string", "description": "File to query (query)"},
                "command": {"type": "string", "description": "Query type: hover, definition, references, completion, diagnostics (query)"},
                "line": {"type": "integer", "description": "Line number (query)"},
                "col": {"type": "integer", "description": "Column number (query)"},
                "key": {"type": "string", "description": "Server key language:project (logs, restart)"},
                "json": {"type": "boolean", "description": "JSON output (status, list)"}
            }),
            vec!["action"],
        ),

        // =========================================================================
        // MCP_HUB - MCP server management (replaces 6 tools)
        // =========================================================================
        tool(
            "mcp_hub",
            "Manage MCP server hub. Actions: status, warm, list, restart, logs, call",
            json!({
                "action": {
                    "type": "string",
                    "description": "Action to perform",
                    "enum": ["status", "warm", "list", "restart", "logs", "call"]
                },
                "servers": {"type": "array", "items": {"type": "string"}, "description": "Server names (warm)"},
                "server": {"type": "string", "description": "Server name (restart, logs, call)"},
                "category": {"type": "string", "description": "Filter category (list)"},
                "tool": {"type": "string", "description": "Tool name (call)"},
                "arguments": {"type": "object", "description": "Tool arguments (call)"},
                "lines": {"type": "integer", "description": "Log lines (logs)"},
                "json": {"type": "boolean", "description": "JSON output (status)"}
            }),
            vec!["action"],
        ),

        // =========================================================================
        // GATES - supervision (replaces 5 tools)
        // =========================================================================
        tool(
            "gates",
            "Check and configure supervision gates. Actions: check, level, set, config, history",
            json!({
                "action": {
                    "type": "string",
                    "description": "Action to perform",
                    "enum": ["check", "level", "set", "config", "history"]
                },
                "gate": {"type": "string", "description": "Gate name: file_delete, file_create, file_modify, git_commit, git_push, git_force_push, loop_start, agent_spawn, shell_command, sensitive_file"},
                "gate_action": {"type": "string", "description": "Gate action: allow, notify, approve, deny (set)"},
                "level": {"type": "string", "description": "Supervision level: autonomous, supervised, collaborative, assisted, manual (level)"},
                "context": {"type": "object", "description": "Additional context (check)"},
                "source": {"type": "string", "description": "Calling source (check)"},
                "days": {"type": "integer", "description": "Days to look back (history)"},
                "limit": {"type": "integer", "description": "Max entries (history)"},
                "json": {"type": "boolean", "description": "JSON output (config)"}
            }),
            vec!["action"],
        ),

        // =========================================================================
        // SPEC - specifications (replaces 6 tools)
        // =========================================================================
        tool(
            "spec",
            "Manage project specifications. Actions: show, query, list, context, validate, new",
            json!({
                "action": {
                    "type": "string",
                    "description": "Action to perform",
                    "enum": ["show", "query", "list", "context", "validate", "new"]
                },
                "component": {"type": "string", "description": "Component name (show)"},
                "section": {"type": "string", "description": "Section: intent, constraints, interface, examples, decisions, anti_patterns (show)"},
                "query": {"type": "string", "description": "Search query (query)"},
                "task": {"type": "string", "description": "Task description (context)"},
                "name": {"type": "string", "description": "Component name (new)"},
                "type": {"type": "string", "description": "Type: tool, library, service, doc (new)"},
                "path": {"type": "string", "description": "Path (validate)"},
                "missing": {"type": "boolean", "description": "Show missing specs (list)"},
                "stale": {"type": "boolean", "description": "Show stale specs (list)"}
            }),
            vec!["action"],
        ),

        // =========================================================================
        // UNDO - file changes (replaces 4 tools)
        // =========================================================================
        tool(
            "undo",
            "Track and revert file changes. Actions: checkpoint, last, timeline, restore",
            json!({
                "action": {
                    "type": "string",
                    "description": "Action to perform",
                    "enum": ["checkpoint", "last", "timeline", "restore"]
                },
                "name": {"type": "string", "description": "Checkpoint name (checkpoint)"},
                "id": {"type": "string", "description": "Checkpoint or change ID (restore)"},
                "limit": {"type": "integer", "description": "Entries to show (timeline)"}
            }),
            vec!["action"],
        ),

        // =========================================================================
        // JOURNAL - event logging (replaces 4 tools)
        // =========================================================================
        tool(
            "journal",
            "Query and log events. Actions: what, events, summary, log",
            json!({
                "action": {
                    "type": "string",
                    "description": "Action to perform",
                    "enum": ["what", "events", "summary", "log"]
                },
                "hours": {"type": "number", "description": "Hours to look back (what, events, summary)"},
                "source": {"type": "string", "description": "Filter by source (what, events)"},
                "event_type": {"type": "string", "description": "Filter by type (events, log)"},
                "verbose": {"type": "boolean", "description": "More details (what)"},
                "limit": {"type": "integer", "description": "Max events (events)"},
                "summary_text": {"type": "string", "description": "Event summary (log)"}
            }),
            vec!["action"],
        ),

        // =========================================================================
        // EVOLVE - code evolution (replaces 4 tools)
        // =========================================================================
        tool(
            "evolve",
            "Analyze code intent and evolution. Actions: analyze, intent, gaps, path",
            json!({
                "action": {
                    "type": "string",
                    "description": "Action to perform",
                    "enum": ["analyze", "intent", "gaps", "path"]
                },
                "target_path": {"type": "string", "description": "Path to analyze"},
                "json": {"type": "boolean", "description": "JSON output (analyze)"}
            }),
            vec!["action", "target_path"],
        ),

        // =========================================================================
        // RESOLVE - uncertainty resolution (replaces 4 tools)
        // =========================================================================
        tool(
            "resolve",
            "Resolve uncertainty through context. Actions: resolve, intent, gather, log",
            json!({
                "action": {
                    "type": "string",
                    "description": "Action to perform",
                    "enum": ["resolve", "intent", "gather", "log"]
                },
                "question": {"type": "string", "description": "Question to resolve (resolve, intent, gather)"},
                "decision": {"type": "string", "description": "Decision made (log)"},
                "reasoning": {"type": "string", "description": "Decision reasoning (log)"},
                "json": {"type": "boolean", "description": "JSON output (resolve)"}
            }),
            vec!["action"],
        ),

        // =========================================================================
        // LOOP - iteration (replaces 3 tools)
        // =========================================================================
        tool(
            "loop",
            "Iterate until verification succeeds. Actions: start, status, stop",
            json!({
                "action": {
                    "type": "string",
                    "description": "Action to perform",
                    "enum": ["start", "status", "stop"]
                },
                "task": {"type": "string", "description": "Task description (start)"},
                "promise": {"type": "string", "description": "Shell command that exits 0 when done (start)"},
                "max_iterations": {"type": "integer", "description": "Max iterations (start)"}
            }),
            vec!["action"],
        ),

        // =========================================================================
        // PROJECT - codebase intelligence (replaces 3 tools)
        // =========================================================================
        tool(
            "project",
            "Analyze project structure. Actions: info, symbols, tree",
            json!({
                "action": {
                    "type": "string",
                    "description": "Action to perform",
                    "enum": ["info", "symbols", "tree"]
                },
                "path": {"type": "string", "description": "Project path"},
                "symbol_type": {"type": "string", "description": "Filter symbol type (symbols)"},
                "depth": {"type": "integer", "description": "Tree depth (tree)"}
            }),
            vec!["action"],
        ),

        // =========================================================================
        // SCRATCH - ephemeral envs (replaces 3 tools)
        // =========================================================================
        tool(
            "scratch",
            "Manage ephemeral scratch environments. Actions: new, list, destroy",
            json!({
                "action": {
                    "type": "string",
                    "description": "Action to perform",
                    "enum": ["new", "list", "destroy"]
                },
                "name": {"type": "string", "description": "Scratch name (new, destroy)"}
            }),
            vec!["action"],
        ),

        // =========================================================================
        // CODEX - semantic search (replaces 2 tools)
        // =========================================================================
        tool(
            "codex",
            "Semantic code search. Actions: search, index",
            json!({
                "action": {
                    "type": "string",
                    "description": "Action to perform",
                    "enum": ["search", "index"]
                },
                "query": {"type": "string", "description": "Search query (search)"},
                "path": {"type": "string", "description": "Path to search/index"},
                "limit": {"type": "integer", "description": "Max results (search)"}
            }),
            vec!["action"],
        ),

        // =========================================================================
        // CONTEXT - context window (replaces 2 tools)
        // =========================================================================
        tool(
            "context",
            "Analyze context window usage. Actions: estimate, breakdown",
            json!({
                "action": {
                    "type": "string",
                    "description": "Action to perform",
                    "enum": ["estimate", "breakdown"]
                }
            }),
            vec!["action"],
        ),

        // =========================================================================
        // ERROR_DB - error patterns (replaces 2 tools)
        // =========================================================================
        tool(
            "error_db",
            "Match errors to known solutions. Actions: match, add",
            json!({
                "action": {
                    "type": "string",
                    "description": "Action to perform",
                    "enum": ["match", "add"]
                },
                "error": {"type": "string", "description": "Error message (match)"},
                "pattern": {"type": "string", "description": "Error regex pattern (add)"},
                "solution": {"type": "string", "description": "How to fix (add)"},
                "category": {"type": "string", "description": "Error category (add)"}
            }),
            vec!["action"],
        ),

        // =========================================================================
        // ANALYZE - test gaps (replaces 2 tools)
        // =========================================================================
        tool(
            "analyze",
            "Analyze and fill test gaps. Actions: tests, gaps",
            json!({
                "action": {
                    "type": "string",
                    "description": "Action to perform",
                    "enum": ["tests", "gaps"]
                },
                "path": {"type": "string", "description": "Path to analyze"},
                "dry_run": {"type": "boolean", "description": "Just show gaps (tests)"},
                "promise": {"type": "string", "description": "Verification command (tests)"},
                "max_iterations": {"type": "integer", "description": "Max iterations (tests)"}
            }),
            vec!["action"],
        ),

        // =========================================================================
        // VERIFY - single tool, unchanged
        // =========================================================================
        tool(
            "verify",
            "Run verification checks (lint, types, build, test). Auto-detects project type.",
            json!({
                "quick": {"type": "boolean", "description": "Run only fast checks"},
                "path": {"type": "string", "description": "Path to verify"}
            }),
            vec![],
        ),
    ]
}
