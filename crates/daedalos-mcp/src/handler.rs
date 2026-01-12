//! Tool Handler
//!
//! Handles tool calls by invoking the appropriate Daedalos CLI tools.

use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use serde_json::{Map, Value};
use tracing::{debug, warn};

use crate::protocol::ToolResult;

/// Get the path to a Daedalos tool binary
fn tool_path(name: &str) -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local")
        .join("bin")
        .join(name)
}

/// Run a tool command and return the output
async fn run_tool(name: &str, args: Vec<String>, cwd: Option<&str>) -> ToolResult {
    let path = tool_path(name);

    if !path.exists() {
        return ToolResult::error(format!("Tool '{}' not found at {:?}", name, path));
    }

    debug!("Running tool: {} {:?}", name, args);

    let mut cmd = Command::new(&path);
    cmd.args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    match tokio::time::timeout(
        std::time::Duration::from_secs(60),
        cmd.output()
    ).await {
        Ok(Ok(output)) => {
            let mut result = String::from_utf8_lossy(&output.stdout).to_string();

            if !output.stderr.is_empty() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if !stderr.trim().is_empty() {
                    result.push_str(&format!("\n[stderr]: {}", stderr));
                }
            }

            if !output.status.success() {
                result.push_str(&format!("\n[exit code: {}]", output.status.code().unwrap_or(-1)));
                ToolResult::error(if result.trim().is_empty() { "(no output)".to_string() } else { result })
            } else {
                ToolResult::success(if result.trim().is_empty() { "(no output)".to_string() } else { result })
            }
        }
        Ok(Err(e)) => ToolResult::error(format!("Failed to execute tool: {}", e)),
        Err(_) => ToolResult::error("Command timed out after 60 seconds"),
    }
}

/// Helper to get a string argument
fn get_str(args: &Map<String, Value>, key: &str) -> Option<String> {
    args.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

/// Helper to get an integer argument
fn get_int(args: &Map<String, Value>, key: &str) -> Option<i64> {
    args.get(key).and_then(|v| v.as_i64())
}

/// Helper to get a boolean argument
fn get_bool(args: &Map<String, Value>, key: &str) -> Option<bool> {
    args.get(key).and_then(|v| v.as_bool())
}

/// Handle a tool call
pub async fn handle_tool(name: &str, arguments: Map<String, Value>) -> ToolResult {
    let cwd = get_str(&arguments, "path");
    let cwd_ref = cwd.as_deref();

    match name {
        // =========================================================================
        // Loop tools
        // =========================================================================
        "loop_start" => {
            let task = match get_str(&arguments, "task") {
                Some(t) => t,
                None => return ToolResult::error("Missing required argument: task"),
            };
            let promise = match get_str(&arguments, "promise") {
                Some(p) => p,
                None => return ToolResult::error("Missing required argument: promise"),
            };
            let mut args = vec!["start".to_string(), task, "--promise".to_string(), promise];
            if let Some(n) = get_int(&arguments, "max_iterations") {
                args.extend(["-n".to_string(), n.to_string()]);
            }
            run_tool("loop", args, cwd_ref).await
        }
        "loop_status" => run_tool("loop", vec!["status".to_string()], cwd_ref).await,
        "loop_stop" => run_tool("loop", vec!["stop".to_string()], cwd_ref).await,

        // =========================================================================
        // Verify
        // =========================================================================
        "verify" => {
            let mut args = vec![];
            if get_bool(&arguments, "quick").unwrap_or(false) {
                args.push("--quick".to_string());
            }
            run_tool("verify", args, cwd_ref).await
        }

        // =========================================================================
        // Undo tools
        // =========================================================================
        "undo_checkpoint" => {
            let name = match get_str(&arguments, "name") {
                Some(n) => n,
                None => return ToolResult::error("Missing required argument: name"),
            };
            run_tool("undo", vec!["checkpoint".to_string(), name], cwd_ref).await
        }
        "undo_last" => run_tool("undo", vec!["last".to_string()], cwd_ref).await,
        "undo_timeline" => {
            let mut args = vec!["timeline".to_string()];
            if let Some(n) = get_int(&arguments, "limit") {
                args.extend(["-n".to_string(), n.to_string()]);
            }
            run_tool("undo", args, cwd_ref).await
        }
        "undo_restore" => {
            let id = match get_str(&arguments, "id") {
                Some(i) => i,
                None => return ToolResult::error("Missing required argument: id"),
            };
            run_tool("undo", vec!["to".to_string(), id], cwd_ref).await
        }

        // =========================================================================
        // Project tools
        // =========================================================================
        "project_info" => run_tool("project", vec!["summary".to_string()], cwd_ref).await,
        "project_symbols" => {
            let mut args = vec!["search".to_string()];
            if let Some(t) = get_str(&arguments, "type") {
                args.extend(["--type".to_string(), t]);
            }
            args.push("*".to_string());
            run_tool("project", args, cwd_ref).await
        }
        "project_tree" => {
            let mut args = vec!["tree".to_string()];
            if let Some(d) = get_int(&arguments, "depth") {
                args.extend(["--depth".to_string(), d.to_string()]);
            }
            run_tool("project", args, cwd_ref).await
        }

        // =========================================================================
        // Codex tools
        // =========================================================================
        "codex_search" => {
            let query = match get_str(&arguments, "query") {
                Some(q) => q,
                None => return ToolResult::error("Missing required argument: query"),
            };
            let mut args = vec!["search".to_string(), query];
            if let Some(limit) = get_int(&arguments, "limit") {
                args.extend(["--limit".to_string(), limit.to_string()]);
            }
            run_tool("codex", args, cwd_ref).await
        }
        "codex_index" => run_tool("codex", vec!["index".to_string()], cwd_ref).await,

        // =========================================================================
        // Context tools
        // =========================================================================
        "context_estimate" => run_tool("context", vec!["estimate".to_string()], None).await,
        "context_breakdown" => run_tool("context", vec!["breakdown".to_string()], None).await,

        // =========================================================================
        // Error-db tools
        // =========================================================================
        "error_match" => {
            let error = match get_str(&arguments, "error") {
                Some(e) => e,
                None => return ToolResult::error("Missing required argument: error"),
            };
            run_tool("error-db", vec!["search".to_string(), error], None).await
        }
        "error_add" => {
            let pattern = match get_str(&arguments, "pattern") {
                Some(p) => p,
                None => return ToolResult::error("Missing required argument: pattern"),
            };
            let solution = match get_str(&arguments, "solution") {
                Some(s) => s,
                None => return ToolResult::error("Missing required argument: solution"),
            };
            let mut args = vec!["add".to_string(), pattern, "--solution".to_string(), solution];
            if let Some(cat) = get_str(&arguments, "category") {
                args.extend(["--category".to_string(), cat]);
            }
            run_tool("error-db", args, None).await
        }

        // =========================================================================
        // Scratch tools
        // =========================================================================
        "scratch_new" => {
            let name = match get_str(&arguments, "name") {
                Some(n) => n,
                None => return ToolResult::error("Missing required argument: name"),
            };
            run_tool("scratch", vec!["new".to_string(), name], cwd_ref).await
        }
        "scratch_list" => run_tool("scratch", vec!["list".to_string()], None).await,
        "scratch_destroy" => {
            let name = match get_str(&arguments, "name") {
                Some(n) => n,
                None => return ToolResult::error("Missing required argument: name"),
            };
            run_tool("scratch", vec!["destroy".to_string(), name], None).await
        }

        // =========================================================================
        // Agent tools
        // =========================================================================
        "agent_spawn" => {
            let name = match get_str(&arguments, "name") {
                Some(n) => n,
                None => return ToolResult::error("Missing required argument: name"),
            };
            let mut args = vec!["spawn".to_string(), "-n".to_string(), name];
            if let Some(template) = get_str(&arguments, "template") {
                args.extend(["-t".to_string(), template]);
            }
            if let Some(project) = get_str(&arguments, "project") {
                args.extend(["-p".to_string(), project]);
            }
            if get_bool(&arguments, "no_focus").unwrap_or(false) {
                args.push("--no-focus".to_string());
            }
            run_tool("agent", args, None).await
        }
        "agent_list" => run_tool("agent", vec!["list".to_string()], None).await,
        "agent_focus" => {
            let name = match get_str(&arguments, "name") {
                Some(n) => n,
                None => return ToolResult::error("Missing required argument: name"),
            };
            run_tool("agent", vec!["focus".to_string(), name], None).await
        }
        "agent_search" => {
            let query = match get_str(&arguments, "query") {
                Some(q) => q,
                None => return ToolResult::error("Missing required argument: query"),
            };
            let mut args = vec!["search".to_string(), query];
            if let Some(agent) = get_str(&arguments, "agent") {
                args.extend(["-a".to_string(), agent]);
            }
            run_tool("agent", args, None).await
        }
        "agent_kill" => {
            let name = match get_str(&arguments, "name") {
                Some(n) => n,
                None => return ToolResult::error("Missing required argument: name"),
            };
            let mut args = vec!["kill".to_string(), name];
            if get_bool(&arguments, "force").unwrap_or(false) {
                args.push("--force".to_string());
            }
            run_tool("agent", args, None).await
        }
        "agent_register" => {
            let name = match get_str(&arguments, "name") {
                Some(n) => n,
                None => return ToolResult::error("Missing required argument: name"),
            };
            let mut args = vec!["register".to_string(), name];
            if let Some(project) = get_str(&arguments, "project") {
                args.extend(["-p".to_string(), project]);
            }
            run_tool("agent", args, cwd_ref).await
        }
        "agent_unregister" => {
            let name = match get_str(&arguments, "name") {
                Some(n) => n,
                None => return ToolResult::error("Missing required argument: name"),
            };
            run_tool("agent", vec!["unregister".to_string(), name], None).await
        }
        "agent_whoami" => run_tool("agent", vec!["whoami".to_string()], None).await,
        "agent_snapshot" => {
            let mut args = vec!["snapshot".to_string()];
            if let Some(name) = get_str(&arguments, "name") {
                args.push(name);
            }
            if get_bool(&arguments, "all").unwrap_or(false) {
                args.push("--all".to_string());
            }
            if let Some(snapshot_name) = get_str(&arguments, "snapshot_name") {
                args.extend(["--name".to_string(), snapshot_name]);
            }
            run_tool("agent", args, None).await
        }
        "agent_snapshot_list" => {
            run_tool("agent", vec!["snapshot".to_string(), "list".to_string()], None).await
        }
        "agent_restore" => {
            let snapshot = match get_str(&arguments, "snapshot") {
                Some(s) => s,
                None => return ToolResult::error("Missing required argument: snapshot"),
            };
            let mut args = vec!["restore".to_string(), snapshot];
            if let Some(agent) = get_str(&arguments, "agent") {
                args.extend(["--agent".to_string(), agent]);
            }
            run_tool("agent", args, None).await
        }
        "agent_send" => {
            let to = match get_str(&arguments, "to") {
                Some(t) => t,
                None => return ToolResult::error("Missing required argument: to"),
            };
            let message = match get_str(&arguments, "message") {
                Some(m) => m,
                None => return ToolResult::error("Missing required argument: message"),
            };
            run_tool("agent", vec!["send".to_string(), to, message], None).await
        }
        "agent_inbox" => {
            let mut args = vec!["inbox".to_string()];
            if let Some(agent) = get_str(&arguments, "agent") {
                args.push(agent);
            }
            if get_bool(&arguments, "all").unwrap_or(false) {
                args.push("--all".to_string());
            }
            run_tool("agent", args, None).await
        }
        "agent_broadcast" => {
            let message = match get_str(&arguments, "message") {
                Some(m) => m,
                None => return ToolResult::error("Missing required argument: message"),
            };
            run_tool("agent", vec!["broadcast".to_string(), message], None).await
        }
        "agent_request_help" => {
            let task = match get_str(&arguments, "task") {
                Some(t) => t,
                None => return ToolResult::error("Missing required argument: task"),
            };
            let mut args = vec!["request-help".to_string(), task];
            if let Some(template) = get_str(&arguments, "template") {
                args.extend(["--template".to_string(), template]);
            }
            run_tool("agent", args, None).await
        }
        "agent_signal_complete" => {
            let mut args = vec!["signal".to_string(), "complete".to_string()];
            if let Some(status) = get_str(&arguments, "status") {
                args.extend(["--status".to_string(), status]);
            }
            if let Some(data_file) = get_str(&arguments, "data_file") {
                args.extend(["--data".to_string(), data_file]);
            }
            run_tool("agent", args, None).await
        }
        "agent_signal_wait" => {
            let agent = match get_str(&arguments, "agent") {
                Some(a) => a,
                None => return ToolResult::error("Missing required argument: agent"),
            };
            let mut args = vec!["signal".to_string(), "wait".to_string(), agent];
            if let Some(timeout) = get_int(&arguments, "timeout") {
                args.push(timeout.to_string());
            }
            run_tool("agent", args, None).await
        }
        "agent_signal_check" => {
            let agent = match get_str(&arguments, "agent") {
                Some(a) => a,
                None => return ToolResult::error("Missing required argument: agent"),
            };
            run_tool("agent", vec!["signal".to_string(), "check".to_string(), agent], None).await
        }
        "agent_lock_acquire" => {
            let name = match get_str(&arguments, "name") {
                Some(n) => n,
                None => return ToolResult::error("Missing required argument: name"),
            };
            let mut args = vec!["lock".to_string(), "acquire".to_string(), name];
            if let Some(timeout) = get_int(&arguments, "timeout") {
                args.push(timeout.to_string());
            }
            run_tool("agent", args, None).await
        }
        "agent_lock_release" => {
            let name = match get_str(&arguments, "name") {
                Some(n) => n,
                None => return ToolResult::error("Missing required argument: name"),
            };
            run_tool("agent", vec!["lock".to_string(), "release".to_string(), name], None).await
        }
        "agent_lock_list" => {
            run_tool("agent", vec!["lock".to_string(), "list".to_string()], None).await
        }
        "agent_claim_create" => {
            let task_id = match get_str(&arguments, "task_id") {
                Some(t) => t,
                None => return ToolResult::error("Missing required argument: task_id"),
            };
            let mut args = vec!["claim".to_string(), "create".to_string(), task_id];
            if let Some(desc) = get_str(&arguments, "description") {
                args.push(desc);
            }
            run_tool("agent", args, None).await
        }
        "agent_claim_release" => {
            let task_id = match get_str(&arguments, "task_id") {
                Some(t) => t,
                None => return ToolResult::error("Missing required argument: task_id"),
            };
            let mut args = vec!["claim".to_string(), "release".to_string(), task_id];
            if let Some(status) = get_str(&arguments, "status") {
                args.push(status);
            }
            run_tool("agent", args, None).await
        }
        "agent_claim_list" => {
            run_tool("agent", vec!["claim".to_string(), "list".to_string()], None).await
        }

        // =========================================================================
        // Workflow tools
        // =========================================================================
        "workflow_list" => {
            let mut args = vec!["workflow".to_string(), "list".to_string()];
            if get_bool(&arguments, "json").unwrap_or(false) {
                args.push("--json".to_string());
            }
            run_tool("agent", args, None).await
        }
        "workflow_start" => {
            let workflow = match get_str(&arguments, "workflow") {
                Some(w) => w,
                None => return ToolResult::error("Missing required argument: workflow"),
            };
            let task = match get_str(&arguments, "task") {
                Some(t) => t,
                None => return ToolResult::error("Missing required argument: task"),
            };
            let mut args = vec!["workflow".to_string(), "start".to_string(), workflow, task];
            if let Some(project) = get_str(&arguments, "project") {
                args.extend(["--project".to_string(), project]);
            }
            run_tool("agent", args, None).await
        }
        "workflow_status" => {
            let mut args = vec!["workflow".to_string(), "status".to_string()];
            if let Some(instance_id) = get_str(&arguments, "instance_id") {
                args.push(instance_id);
            }
            run_tool("agent", args, None).await
        }
        "workflow_stop" => {
            let instance_id = match get_str(&arguments, "instance_id") {
                Some(i) => i,
                None => return ToolResult::error("Missing required argument: instance_id"),
            };
            let mut args = vec!["workflow".to_string(), "stop".to_string(), instance_id];
            if get_bool(&arguments, "force").unwrap_or(false) {
                args.push("--force".to_string());
            }
            run_tool("agent", args, None).await
        }

        // =========================================================================
        // MCP Hub tools
        // =========================================================================
        "mcp_hub_status" => {
            let mut args = vec!["status".to_string()];
            if get_bool(&arguments, "json").unwrap_or(false) {
                args.push("--json".to_string());
            }
            run_tool("mcp-hub", args, None).await
        }
        "mcp_hub_warm" => {
            let servers = arguments.get("servers")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<Vec<_>>())
                .unwrap_or_default();
            let mut args = vec!["warm".to_string()];
            args.extend(servers);
            run_tool("mcp-hub", args, None).await
        }
        "mcp_hub_list" => {
            let mut args = vec!["list".to_string()];
            if let Some(cat) = get_str(&arguments, "category") {
                args.extend(["--category".to_string(), cat]);
            }
            run_tool("mcp-hub", args, None).await
        }
        "mcp_hub_restart" => {
            let server = match get_str(&arguments, "server") {
                Some(s) => s,
                None => return ToolResult::error("Missing required argument: server"),
            };
            run_tool("mcp-hub", vec!["restart".to_string(), server], None).await
        }
        "mcp_hub_logs" => {
            let server = match get_str(&arguments, "server") {
                Some(s) => s,
                None => return ToolResult::error("Missing required argument: server"),
            };
            let mut args = vec!["logs".to_string(), server];
            if let Some(lines) = get_int(&arguments, "lines") {
                args.extend(["-n".to_string(), lines.to_string()]);
            }
            run_tool("mcp-hub", args, None).await
        }
        "mcp_hub_call" => {
            let tool_name = match get_str(&arguments, "tool") {
                Some(t) => t,
                None => return ToolResult::error("Missing required argument: tool"),
            };
            let mut args = vec!["call".to_string(), tool_name];
            if let Some(server) = get_str(&arguments, "server") {
                args.extend(["--server".to_string(), server]);
            }
            if let Some(tool_args) = arguments.get("arguments").and_then(|v| v.as_object()) {
                for (k, v) in tool_args {
                    args.extend([format!("--{}", k), v.to_string()]);
                }
            }
            run_tool("mcp-hub", args, None).await
        }

        // =========================================================================
        // LSP Pool tools
        // =========================================================================
        "lsp_pool_status" => {
            let mut args = vec!["status".to_string()];
            if get_bool(&arguments, "json").unwrap_or(false) {
                args.push("--json".to_string());
            }
            run_tool("lsp-pool", args, None).await
        }
        "lsp_pool_warm" => {
            let language = match get_str(&arguments, "language") {
                Some(l) => l,
                None => return ToolResult::error("Missing required argument: language"),
            };
            let mut args = vec!["warm".to_string(), language];
            if let Some(path) = get_str(&arguments, "path") {
                args.push(path);
            }
            run_tool("lsp-pool", args, None).await
        }
        "lsp_pool_cool" => {
            let language = match get_str(&arguments, "language") {
                Some(l) => l,
                None => return ToolResult::error("Missing required argument: language"),
            };
            run_tool("lsp-pool", vec!["cool".to_string(), language], None).await
        }
        "lsp_pool_list" => {
            let mut args = vec!["list".to_string()];
            if get_bool(&arguments, "json").unwrap_or(false) {
                args.push("--json".to_string());
            }
            run_tool("lsp-pool", args, None).await
        }
        "lsp_pool_query" => {
            let command = match get_str(&arguments, "command") {
                Some(c) => c,
                None => return ToolResult::error("Missing required argument: command"),
            };
            let file = match get_str(&arguments, "file") {
                Some(f) => f,
                None => return ToolResult::error("Missing required argument: file"),
            };
            let mut args = vec!["query".to_string(), command, file];
            if let Some(line) = get_int(&arguments, "line") {
                args.extend(["--line".to_string(), line.to_string()]);
            }
            if let Some(col) = get_int(&arguments, "col") {
                args.extend(["--col".to_string(), col.to_string()]);
            }
            run_tool("lsp-pool", args, None).await
        }
        "lsp_pool_languages" => {
            run_tool("lsp-pool", vec!["languages".to_string()], None).await
        }
        "lsp_pool_logs" => {
            let key = match get_str(&arguments, "key") {
                Some(k) => k,
                None => return ToolResult::error("Missing required argument: key"),
            };
            run_tool("lsp-pool", vec!["logs".to_string(), key], None).await
        }
        "lsp_pool_restart" => {
            let key = match get_str(&arguments, "key") {
                Some(k) => k,
                None => return ToolResult::error("Missing required argument: key"),
            };
            run_tool("lsp-pool", vec!["restart".to_string(), key], None).await
        }

        // =========================================================================
        // Sandbox tools
        // =========================================================================
        "sandbox_create" => {
            let mut args = vec!["create".to_string()];
            if let Some(name) = get_str(&arguments, "name") {
                args.push(name);
            }
            if let Some(from_path) = get_str(&arguments, "from_path") {
                args.extend(["--from".to_string(), from_path]);
            }
            if let Some(backend) = get_str(&arguments, "backend") {
                args.extend(["--copy".to_string(), backend]);
            }
            run_tool("sandbox", args, None).await
        }
        "sandbox_list" => {
            let mut args = vec!["list".to_string()];
            if get_bool(&arguments, "json").unwrap_or(false) {
                args.push("--json".to_string());
            }
            run_tool("sandbox", args, None).await
        }
        "sandbox_enter" => {
            let name = match get_str(&arguments, "name") {
                Some(n) => n,
                None => return ToolResult::error("Missing required argument: name"),
            };
            let mut args = vec!["enter".to_string(), name];
            if let Some(command) = get_str(&arguments, "command") {
                args.extend(["--command".to_string(), command]);
            }
            run_tool("sandbox", args, None).await
        }
        "sandbox_diff" => {
            let name = match get_str(&arguments, "name") {
                Some(n) => n,
                None => return ToolResult::error("Missing required argument: name"),
            };
            let mut args = vec!["diff".to_string(), name];
            if get_bool(&arguments, "files_only").unwrap_or(false) {
                args.push("--files".to_string());
            }
            run_tool("sandbox", args, None).await
        }
        "sandbox_promote" => {
            let name = match get_str(&arguments, "name") {
                Some(n) => n,
                None => return ToolResult::error("Missing required argument: name"),
            };
            let mut args = vec!["promote".to_string(), name, "--yes".to_string()];
            if get_bool(&arguments, "dry_run").unwrap_or(false) {
                args.push("--dry-run".to_string());
            }
            if get_bool(&arguments, "backup").unwrap_or(false) {
                args.push("--backup".to_string());
            }
            run_tool("sandbox", args, None).await
        }
        "sandbox_discard" => {
            let name = match get_str(&arguments, "name") {
                Some(n) => n,
                None => return ToolResult::error("Missing required argument: name"),
            };
            run_tool("sandbox", vec!["discard".to_string(), name, "--force".to_string()], None).await
        }
        "sandbox_info" => {
            let name = match get_str(&arguments, "name") {
                Some(n) => n,
                None => return ToolResult::error("Missing required argument: name"),
            };
            let mut args = vec!["info".to_string(), name];
            if get_bool(&arguments, "json").unwrap_or(false) {
                args.push("--json".to_string());
            }
            run_tool("sandbox", args, None).await
        }
        "sandbox_run" => {
            let name = match get_str(&arguments, "name") {
                Some(n) => n,
                None => return ToolResult::error("Missing required argument: name"),
            };
            let command = match get_str(&arguments, "command") {
                Some(c) => c,
                None => return ToolResult::error("Missing required argument: command"),
            };
            run_tool("sandbox", vec!["run".to_string(), name, command], None).await
        }

        // =========================================================================
        // Gates tools
        // =========================================================================
        "gates_check" => {
            let gate = match get_str(&arguments, "gate") {
                Some(g) => g,
                None => return ToolResult::error("Missing required argument: gate"),
            };
            let mut args = vec!["check".to_string(), gate];
            if let Some(context) = arguments.get("context") {
                args.push(context.to_string());
            }
            if let Some(source) = get_str(&arguments, "source") {
                args.push(source);
            }
            run_tool("gates", args, None).await
        }
        "gates_level" => {
            let mut args = vec!["level".to_string()];
            if let Some(level) = get_str(&arguments, "level") {
                args.push(level);
            }
            run_tool("gates", args, None).await
        }
        "gates_set" => {
            let gate = match get_str(&arguments, "gate") {
                Some(g) => g,
                None => return ToolResult::error("Missing required argument: gate"),
            };
            let action = match get_str(&arguments, "action") {
                Some(a) => a,
                None => return ToolResult::error("Missing required argument: action"),
            };
            run_tool("gates", vec!["set".to_string(), gate, action], None).await
        }
        "gates_config" => {
            let mut args = vec!["config".to_string()];
            if get_bool(&arguments, "json").unwrap_or(true) {
                args.push("--json".to_string());
            }
            run_tool("gates", args, None).await
        }
        "gates_history" => {
            let mut args = vec!["history".to_string(), "--json".to_string()];
            if let Some(gate) = get_str(&arguments, "gate") {
                args.extend(["--gate".to_string(), gate]);
            }
            if let Some(days) = get_int(&arguments, "days") {
                args.extend(["--days".to_string(), days.to_string()]);
            }
            if let Some(limit) = get_int(&arguments, "limit") {
                args.extend(["--limit".to_string(), limit.to_string()]);
            }
            run_tool("gates", args, None).await
        }

        // =========================================================================
        // Spec tools
        // =========================================================================
        "spec_show" => {
            let component = match get_str(&arguments, "component") {
                Some(c) => c,
                None => return ToolResult::error("Missing required argument: component"),
            };
            let mut args = vec!["show".to_string(), component];
            if let Some(section) = get_str(&arguments, "section") {
                args.extend(["--section".to_string(), section]);
            }
            run_tool("spec", args, cwd_ref).await
        }
        "spec_query" => {
            let query = match get_str(&arguments, "query") {
                Some(q) => q,
                None => return ToolResult::error("Missing required argument: query"),
            };
            run_tool("spec", vec!["query".to_string(), query], cwd_ref).await
        }
        "spec_list" => {
            let mut args = vec!["list".to_string()];
            if get_bool(&arguments, "missing").unwrap_or(false) {
                args.push("--missing".to_string());
            }
            if get_bool(&arguments, "stale").unwrap_or(false) {
                args.push("--stale".to_string());
            }
            run_tool("spec", args, cwd_ref).await
        }
        "spec_context" => {
            let task = match get_str(&arguments, "task") {
                Some(t) => t,
                None => return ToolResult::error("Missing required argument: task"),
            };
            run_tool("spec", vec!["context".to_string(), task], cwd_ref).await
        }
        "spec_validate" => {
            let mut args = vec!["validate".to_string()];
            if let Some(path) = get_str(&arguments, "path") {
                args.push(path);
            }
            run_tool("spec", args, cwd_ref).await
        }
        "spec_new" => {
            let name = match get_str(&arguments, "name") {
                Some(n) => n,
                None => return ToolResult::error("Missing required argument: name"),
            };
            let mut args = vec!["new".to_string(), name];
            if let Some(spec_type) = get_str(&arguments, "type") {
                args.extend(["--type".to_string(), spec_type]);
            }
            run_tool("spec", args, cwd_ref).await
        }

        // =========================================================================
        // Evolve tools
        // =========================================================================
        "evolve" => {
            let path = match get_str(&arguments, "path") {
                Some(p) => p,
                None => return ToolResult::error("Missing required argument: path"),
            };
            let mut args = vec![];
            if get_bool(&arguments, "json").unwrap_or(true) {
                args.push("--json".to_string());
            }
            args.push(path);
            run_tool("evolve", args, cwd_ref).await
        }
        "evolve_intent" => {
            let path = match get_str(&arguments, "path") {
                Some(p) => p,
                None => return ToolResult::error("Missing required argument: path"),
            };
            run_tool("evolve", vec!["intent".to_string(), path], cwd_ref).await
        }
        "evolve_gaps" => {
            let path = match get_str(&arguments, "path") {
                Some(p) => p,
                None => return ToolResult::error("Missing required argument: path"),
            };
            run_tool("evolve", vec!["gaps".to_string(), path], cwd_ref).await
        }
        "evolve_path" => {
            let path = match get_str(&arguments, "path") {
                Some(p) => p,
                None => return ToolResult::error("Missing required argument: path"),
            };
            run_tool("evolve", vec!["path".to_string(), path], cwd_ref).await
        }

        // =========================================================================
        // Analyze tools
        // =========================================================================
        "analyze_tests" => {
            let mut args = vec!["tests".to_string()];
            if let Some(path) = get_str(&arguments, "path") {
                args.push(path);
            }
            if get_bool(&arguments, "dry_run").unwrap_or(false) {
                args.push("--dry-run".to_string());
            }
            if let Some(n) = get_int(&arguments, "max_iterations") {
                args.push("-n".to_string());
                args.push(n.to_string());
            }
            if let Some(promise) = get_str(&arguments, "promise") {
                args.push("--promise".to_string());
                args.push(promise);
            }
            args.push("--json".to_string());
            run_tool("analyze", args, cwd_ref).await
        }
        "analyze_gaps" => {
            let mut args = vec!["gaps".to_string()];
            if let Some(path) = get_str(&arguments, "path") {
                args.push(path);
            }
            args.push("--json".to_string());
            run_tool("analyze", args, cwd_ref).await
        }

        // =========================================================================
        // Resolve tools
        // =========================================================================
        "resolve" => {
            let question = match get_str(&arguments, "question") {
                Some(q) => q,
                None => return ToolResult::error("Missing required argument: question"),
            };
            let mut args = vec![];
            if get_bool(&arguments, "json").unwrap_or(true) {
                args.push("--json".to_string());
            }
            args.push(question);
            run_tool("resolve", args, cwd_ref).await
        }
        "resolve_intent" => {
            let question = match get_str(&arguments, "question") {
                Some(q) => q,
                None => return ToolResult::error("Missing required argument: question"),
            };
            run_tool("resolve", vec!["--intent".to_string(), question], cwd_ref).await
        }
        "resolve_gather" => {
            let question = match get_str(&arguments, "question") {
                Some(q) => q,
                None => return ToolResult::error("Missing required argument: question"),
            };
            run_tool("resolve", vec!["--gather".to_string(), question], cwd_ref).await
        }
        "resolve_log" => {
            let decision = match get_str(&arguments, "decision") {
                Some(d) => d,
                None => return ToolResult::error("Missing required argument: decision"),
            };
            let mut args = vec!["--log".to_string(), decision];
            if let Some(reasoning) = get_str(&arguments, "reasoning") {
                args.push(reasoning);
            }
            run_tool("resolve", args, cwd_ref).await
        }

        // =========================================================================
        // Journal tools
        // =========================================================================
        "journal_what" => {
            let mut args = vec!["what".to_string()];
            if let Some(hours) = arguments.get("hours").and_then(|v| v.as_f64()) {
                args.extend(["-h".to_string(), hours.to_string()]);
            }
            if let Some(source) = get_str(&arguments, "source") {
                args.extend(["--source".to_string(), source]);
            }
            if get_bool(&arguments, "verbose").unwrap_or(false) {
                args.push("-v".to_string());
            }
            run_tool("journal", args, None).await
        }
        "journal_events" => {
            let mut args = vec!["events".to_string(), "--json".to_string()];
            if let Some(hours) = arguments.get("hours").and_then(|v| v.as_f64()) {
                args.extend(["-h".to_string(), hours.to_string()]);
            }
            if let Some(source) = get_str(&arguments, "source") {
                args.extend(["--source".to_string(), source]);
            }
            if let Some(event_type) = get_str(&arguments, "event_type") {
                args.extend(["--type".to_string(), event_type]);
            }
            if let Some(limit) = get_int(&arguments, "limit") {
                args.extend(["--limit".to_string(), limit.to_string()]);
            }
            run_tool("journal", args, None).await
        }
        "journal_summary" => {
            let mut args = vec!["summary".to_string(), "--json".to_string()];
            if let Some(hours) = arguments.get("hours").and_then(|v| v.as_f64()) {
                args.extend(["-h".to_string(), hours.to_string()]);
            }
            run_tool("journal", args, None).await
        }
        "journal_log" => {
            let summary = match get_str(&arguments, "summary") {
                Some(s) => s,
                None => return ToolResult::error("Missing required argument: summary"),
            };
            let source = get_str(&arguments, "source").unwrap_or_else(|| "mcp".to_string());
            let event_type = get_str(&arguments, "event_type").unwrap_or_else(|| "custom".to_string());
            run_tool("journal", vec!["log".to_string(), summary, source, event_type], None).await
        }

        // =========================================================================
        // Unknown tool
        // =========================================================================
        _ => {
            warn!("Unknown tool: {}", name);
            ToolResult::error(format!("Unknown tool: {}", name))
        }
    }
}
