//! Tool Handler (Consolidated)
//!
//! Handles tool calls by dispatching to Daedalos CLI tools based on action parameter.

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

// Helper functions
fn get_str(args: &Map<String, Value>, key: &str) -> Option<String> {
    args.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

fn get_int(args: &Map<String, Value>, key: &str) -> Option<i64> {
    args.get(key).and_then(|v| v.as_i64())
}

fn get_bool(args: &Map<String, Value>, key: &str) -> Option<bool> {
    args.get(key).and_then(|v| v.as_bool())
}

fn get_float(args: &Map<String, Value>, key: &str) -> Option<f64> {
    args.get(key).and_then(|v| v.as_f64())
}

fn require_str(args: &Map<String, Value>, key: &str) -> Result<String, ToolResult> {
    get_str(args, key).ok_or_else(|| ToolResult::error(format!("Missing required argument: {}", key)))
}

/// Handle a tool call
pub async fn handle_tool(name: &str, arguments: Map<String, Value>) -> ToolResult {
    let cwd = get_str(&arguments, "path");
    let cwd_ref = cwd.as_deref();

    match name {
        // =========================================================================
        // AGENT - consolidated agent management
        // =========================================================================
        "agent" => {
            let action = match get_str(&arguments, "action") {
                Some(a) => a,
                None => return ToolResult::error("Missing required argument: action"),
            };

            match action.as_str() {
                "spawn" => {
                    let name = match require_str(&arguments, "name") {
                        Ok(n) => n,
                        Err(e) => return e,
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
                "list" => run_tool("agent", vec!["list".to_string()], None).await,
                "focus" => {
                    let name = match require_str(&arguments, "name") {
                        Ok(n) => n,
                        Err(e) => return e,
                    };
                    run_tool("agent", vec!["focus".to_string(), name], None).await
                }
                "kill" => {
                    let name = match require_str(&arguments, "name") {
                        Ok(n) => n,
                        Err(e) => return e,
                    };
                    let mut args = vec!["kill".to_string(), name];
                    if get_bool(&arguments, "force").unwrap_or(false) {
                        args.push("--force".to_string());
                    }
                    run_tool("agent", args, None).await
                }
                "search" => {
                    let query = match require_str(&arguments, "query") {
                        Ok(q) => q,
                        Err(e) => return e,
                    };
                    let mut args = vec!["search".to_string(), query];
                    if let Some(agent) = get_str(&arguments, "agent") {
                        args.extend(["-a".to_string(), agent]);
                    }
                    run_tool("agent", args, None).await
                }
                "register" => {
                    let name = match require_str(&arguments, "name") {
                        Ok(n) => n,
                        Err(e) => return e,
                    };
                    let mut args = vec!["register".to_string(), name];
                    if let Some(project) = get_str(&arguments, "project") {
                        args.extend(["-p".to_string(), project]);
                    }
                    run_tool("agent", args, cwd_ref).await
                }
                "unregister" => {
                    let name = match require_str(&arguments, "name") {
                        Ok(n) => n,
                        Err(e) => return e,
                    };
                    run_tool("agent", vec!["unregister".to_string(), name], None).await
                }
                "whoami" => run_tool("agent", vec!["whoami".to_string()], None).await,
                "snapshot" => {
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
                "snapshots" => run_tool("agent", vec!["snapshot".to_string(), "list".to_string()], None).await,
                "restore" => {
                    let snapshot = match require_str(&arguments, "snapshot_name") {
                        Ok(s) => s,
                        Err(_) => match require_str(&arguments, "name") {
                            Ok(s) => s,
                            Err(e) => return e,
                        },
                    };
                    let mut args = vec!["restore".to_string(), snapshot];
                    if let Some(agent) = get_str(&arguments, "agent") {
                        args.extend(["--agent".to_string(), agent]);
                    }
                    run_tool("agent", args, None).await
                }
                "send" => {
                    let to = match require_str(&arguments, "to") {
                        Ok(t) => t,
                        Err(e) => return e,
                    };
                    let message = match require_str(&arguments, "message") {
                        Ok(m) => m,
                        Err(e) => return e,
                    };
                    run_tool("agent", vec!["send".to_string(), to, message], None).await
                }
                "inbox" => {
                    let mut args = vec!["inbox".to_string()];
                    if let Some(agent) = get_str(&arguments, "agent") {
                        args.push(agent);
                    }
                    if get_bool(&arguments, "all").unwrap_or(false) {
                        args.push("--all".to_string());
                    }
                    run_tool("agent", args, None).await
                }
                "broadcast" => {
                    let message = match require_str(&arguments, "message") {
                        Ok(m) => m,
                        Err(e) => return e,
                    };
                    run_tool("agent", vec!["broadcast".to_string(), message], None).await
                }
                "request_help" => {
                    let task = match require_str(&arguments, "task") {
                        Ok(t) => t,
                        Err(e) => return e,
                    };
                    let mut args = vec!["request-help".to_string(), task];
                    if let Some(template) = get_str(&arguments, "template") {
                        args.extend(["--template".to_string(), template]);
                    }
                    run_tool("agent", args, None).await
                }
                "signal_complete" => {
                    let mut args = vec!["signal".to_string(), "complete".to_string()];
                    if let Some(status) = get_str(&arguments, "status") {
                        args.extend(["--status".to_string(), status]);
                    }
                    if let Some(data_file) = get_str(&arguments, "data_file") {
                        args.extend(["--data".to_string(), data_file]);
                    }
                    run_tool("agent", args, None).await
                }
                "signal_wait" => {
                    let agent = match require_str(&arguments, "agent") {
                        Ok(a) => a,
                        Err(e) => return e,
                    };
                    let mut args = vec!["signal".to_string(), "wait".to_string(), agent];
                    if let Some(timeout) = get_int(&arguments, "timeout") {
                        args.push(timeout.to_string());
                    }
                    run_tool("agent", args, None).await
                }
                "signal_check" => {
                    let agent = match require_str(&arguments, "agent") {
                        Ok(a) => a,
                        Err(e) => return e,
                    };
                    run_tool("agent", vec!["signal".to_string(), "check".to_string(), agent], None).await
                }
                "lock_acquire" => {
                    let name = match require_str(&arguments, "name") {
                        Ok(n) => n,
                        Err(e) => return e,
                    };
                    let mut args = vec!["lock".to_string(), "acquire".to_string(), name];
                    if let Some(timeout) = get_int(&arguments, "timeout") {
                        args.push(timeout.to_string());
                    }
                    run_tool("agent", args, None).await
                }
                "lock_release" => {
                    let name = match require_str(&arguments, "name") {
                        Ok(n) => n,
                        Err(e) => return e,
                    };
                    run_tool("agent", vec!["lock".to_string(), "release".to_string(), name], None).await
                }
                "lock_list" => run_tool("agent", vec!["lock".to_string(), "list".to_string()], None).await,
                "claim_create" => {
                    let task_id = match require_str(&arguments, "task_id") {
                        Ok(t) => t,
                        Err(e) => return e,
                    };
                    let mut args = vec!["claim".to_string(), "create".to_string(), task_id];
                    if let Some(desc) = get_str(&arguments, "description") {
                        args.push(desc);
                    }
                    run_tool("agent", args, None).await
                }
                "claim_release" => {
                    let task_id = match require_str(&arguments, "task_id") {
                        Ok(t) => t,
                        Err(e) => return e,
                    };
                    let mut args = vec!["claim".to_string(), "release".to_string(), task_id];
                    if let Some(status) = get_str(&arguments, "status") {
                        args.push(status);
                    }
                    run_tool("agent", args, None).await
                }
                "claim_list" => run_tool("agent", vec!["claim".to_string(), "list".to_string()], None).await,
                _ => ToolResult::error(format!("Unknown agent action: {}", action)),
            }
        }

        // =========================================================================
        // WORKFLOW - consolidated workflow management
        // =========================================================================
        "workflow" => {
            let action = match get_str(&arguments, "action") {
                Some(a) => a,
                None => return ToolResult::error("Missing required argument: action"),
            };

            match action.as_str() {
                "list" => {
                    let mut args = vec!["workflow".to_string(), "list".to_string()];
                    if get_bool(&arguments, "json").unwrap_or(false) {
                        args.push("--json".to_string());
                    }
                    run_tool("agent", args, None).await
                }
                "start" => {
                    let workflow = match require_str(&arguments, "workflow") {
                        Ok(w) => w,
                        Err(e) => return e,
                    };
                    let task = match require_str(&arguments, "task") {
                        Ok(t) => t,
                        Err(e) => return e,
                    };
                    let mut args = vec!["workflow".to_string(), "start".to_string(), workflow, task];
                    if let Some(project) = get_str(&arguments, "project") {
                        args.extend(["--project".to_string(), project]);
                    }
                    run_tool("agent", args, None).await
                }
                "status" => {
                    let mut args = vec!["workflow".to_string(), "status".to_string()];
                    if let Some(instance_id) = get_str(&arguments, "instance_id") {
                        args.push(instance_id);
                    }
                    run_tool("agent", args, None).await
                }
                "stop" => {
                    let instance_id = match require_str(&arguments, "instance_id") {
                        Ok(i) => i,
                        Err(e) => return e,
                    };
                    let mut args = vec!["workflow".to_string(), "stop".to_string(), instance_id];
                    if get_bool(&arguments, "force").unwrap_or(false) {
                        args.push("--force".to_string());
                    }
                    run_tool("agent", args, None).await
                }
                _ => ToolResult::error(format!("Unknown workflow action: {}", action)),
            }
        }

        // =========================================================================
        // SANDBOX - consolidated sandbox management
        // =========================================================================
        "sandbox" => {
            let action = match get_str(&arguments, "action") {
                Some(a) => a,
                None => return ToolResult::error("Missing required argument: action"),
            };

            match action.as_str() {
                "create" => {
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
                "list" => {
                    let mut args = vec!["list".to_string()];
                    if get_bool(&arguments, "json").unwrap_or(false) {
                        args.push("--json".to_string());
                    }
                    run_tool("sandbox", args, None).await
                }
                "enter" => {
                    let name = match require_str(&arguments, "name") {
                        Ok(n) => n,
                        Err(e) => return e,
                    };
                    let mut args = vec!["enter".to_string(), name];
                    if let Some(command) = get_str(&arguments, "command") {
                        args.extend(["--command".to_string(), command]);
                    }
                    run_tool("sandbox", args, None).await
                }
                "diff" => {
                    let name = match require_str(&arguments, "name") {
                        Ok(n) => n,
                        Err(e) => return e,
                    };
                    let mut args = vec!["diff".to_string(), name];
                    if get_bool(&arguments, "files_only").unwrap_or(false) {
                        args.push("--files".to_string());
                    }
                    run_tool("sandbox", args, None).await
                }
                "promote" => {
                    let name = match require_str(&arguments, "name") {
                        Ok(n) => n,
                        Err(e) => return e,
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
                "discard" => {
                    let name = match require_str(&arguments, "name") {
                        Ok(n) => n,
                        Err(e) => return e,
                    };
                    run_tool("sandbox", vec!["discard".to_string(), name, "--force".to_string()], None).await
                }
                "info" => {
                    let name = match require_str(&arguments, "name") {
                        Ok(n) => n,
                        Err(e) => return e,
                    };
                    let mut args = vec!["info".to_string(), name];
                    if get_bool(&arguments, "json").unwrap_or(false) {
                        args.push("--json".to_string());
                    }
                    run_tool("sandbox", args, None).await
                }
                "run" => {
                    let name = match require_str(&arguments, "name") {
                        Ok(n) => n,
                        Err(e) => return e,
                    };
                    let command = match require_str(&arguments, "command") {
                        Ok(c) => c,
                        Err(e) => return e,
                    };
                    run_tool("sandbox", vec!["run".to_string(), name, command], None).await
                }
                _ => ToolResult::error(format!("Unknown sandbox action: {}", action)),
            }
        }

        // =========================================================================
        // LSP_POOL - consolidated language server management
        // =========================================================================
        "lsp_pool" => {
            let action = match get_str(&arguments, "action") {
                Some(a) => a,
                None => return ToolResult::error("Missing required argument: action"),
            };

            match action.as_str() {
                "status" => {
                    let mut args = vec!["status".to_string()];
                    if get_bool(&arguments, "json").unwrap_or(false) {
                        args.push("--json".to_string());
                    }
                    run_tool("lsp-pool", args, None).await
                }
                "warm" => {
                    let language = match require_str(&arguments, "language") {
                        Ok(l) => l,
                        Err(e) => return e,
                    };
                    let mut args = vec!["warm".to_string(), language];
                    if let Some(path) = get_str(&arguments, "path") {
                        args.push(path);
                    }
                    run_tool("lsp-pool", args, None).await
                }
                "cool" => {
                    let language = match require_str(&arguments, "language") {
                        Ok(l) => l,
                        Err(e) => return e,
                    };
                    run_tool("lsp-pool", vec!["cool".to_string(), language], None).await
                }
                "list" => {
                    let mut args = vec!["list".to_string()];
                    if get_bool(&arguments, "json").unwrap_or(false) {
                        args.push("--json".to_string());
                    }
                    run_tool("lsp-pool", args, None).await
                }
                "query" => {
                    let command = match require_str(&arguments, "command") {
                        Ok(c) => c,
                        Err(e) => return e,
                    };
                    let file = match require_str(&arguments, "file") {
                        Ok(f) => f,
                        Err(e) => return e,
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
                "languages" => run_tool("lsp-pool", vec!["languages".to_string()], None).await,
                "logs" => {
                    let key = match require_str(&arguments, "key") {
                        Ok(k) => k,
                        Err(e) => return e,
                    };
                    run_tool("lsp-pool", vec!["logs".to_string(), key], None).await
                }
                "restart" => {
                    let key = match require_str(&arguments, "key") {
                        Ok(k) => k,
                        Err(e) => return e,
                    };
                    run_tool("lsp-pool", vec!["restart".to_string(), key], None).await
                }
                _ => ToolResult::error(format!("Unknown lsp_pool action: {}", action)),
            }
        }

        // =========================================================================
        // MCP_HUB - consolidated MCP server management
        // =========================================================================
        "mcp_hub" => {
            let action = match get_str(&arguments, "action") {
                Some(a) => a,
                None => return ToolResult::error("Missing required argument: action"),
            };

            match action.as_str() {
                "status" => {
                    let mut args = vec!["status".to_string()];
                    if get_bool(&arguments, "json").unwrap_or(false) {
                        args.push("--json".to_string());
                    }
                    run_tool("mcp-hub", args, None).await
                }
                "warm" => {
                    let servers = arguments.get("servers")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<Vec<_>>())
                        .unwrap_or_default();
                    let mut args = vec!["warm".to_string()];
                    args.extend(servers);
                    run_tool("mcp-hub", args, None).await
                }
                "list" => {
                    let mut args = vec!["list".to_string()];
                    if let Some(cat) = get_str(&arguments, "category") {
                        args.extend(["--category".to_string(), cat]);
                    }
                    run_tool("mcp-hub", args, None).await
                }
                "restart" => {
                    let server = match require_str(&arguments, "server") {
                        Ok(s) => s,
                        Err(e) => return e,
                    };
                    run_tool("mcp-hub", vec!["restart".to_string(), server], None).await
                }
                "logs" => {
                    let server = match require_str(&arguments, "server") {
                        Ok(s) => s,
                        Err(e) => return e,
                    };
                    let mut args = vec!["logs".to_string(), server];
                    if let Some(lines) = get_int(&arguments, "lines") {
                        args.extend(["-n".to_string(), lines.to_string()]);
                    }
                    run_tool("mcp-hub", args, None).await
                }
                "call" => {
                    let tool_name = match require_str(&arguments, "tool") {
                        Ok(t) => t,
                        Err(e) => return e,
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
                _ => ToolResult::error(format!("Unknown mcp_hub action: {}", action)),
            }
        }

        // =========================================================================
        // GATES - consolidated supervision gates
        // =========================================================================
        "gates" => {
            let action = match get_str(&arguments, "action") {
                Some(a) => a,
                None => return ToolResult::error("Missing required argument: action"),
            };

            match action.as_str() {
                "check" => {
                    let gate = match require_str(&arguments, "gate") {
                        Ok(g) => g,
                        Err(e) => return e,
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
                "level" => {
                    let mut args = vec!["level".to_string()];
                    if let Some(level) = get_str(&arguments, "level") {
                        args.push(level);
                    }
                    run_tool("gates", args, None).await
                }
                "set" => {
                    let gate = match require_str(&arguments, "gate") {
                        Ok(g) => g,
                        Err(e) => return e,
                    };
                    let gate_action = match require_str(&arguments, "gate_action") {
                        Ok(a) => a,
                        Err(e) => return e,
                    };
                    run_tool("gates", vec!["set".to_string(), gate, gate_action], None).await
                }
                "config" => {
                    let mut args = vec!["config".to_string()];
                    if get_bool(&arguments, "json").unwrap_or(true) {
                        args.push("--json".to_string());
                    }
                    run_tool("gates", args, None).await
                }
                "history" => {
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
                _ => ToolResult::error(format!("Unknown gates action: {}", action)),
            }
        }

        // =========================================================================
        // SPEC - consolidated spec management
        // =========================================================================
        "spec" => {
            let action = match get_str(&arguments, "action") {
                Some(a) => a,
                None => return ToolResult::error("Missing required argument: action"),
            };

            match action.as_str() {
                "show" => {
                    let component = match require_str(&arguments, "component") {
                        Ok(c) => c,
                        Err(e) => return e,
                    };
                    let mut args = vec!["show".to_string(), component];
                    if let Some(section) = get_str(&arguments, "section") {
                        args.extend(["--section".to_string(), section]);
                    }
                    run_tool("spec", args, cwd_ref).await
                }
                "query" => {
                    let query = match require_str(&arguments, "query") {
                        Ok(q) => q,
                        Err(e) => return e,
                    };
                    run_tool("spec", vec!["query".to_string(), query], cwd_ref).await
                }
                "list" => {
                    let mut args = vec!["list".to_string()];
                    if get_bool(&arguments, "missing").unwrap_or(false) {
                        args.push("--missing".to_string());
                    }
                    if get_bool(&arguments, "stale").unwrap_or(false) {
                        args.push("--stale".to_string());
                    }
                    run_tool("spec", args, cwd_ref).await
                }
                "context" => {
                    let task = match require_str(&arguments, "task") {
                        Ok(t) => t,
                        Err(e) => return e,
                    };
                    run_tool("spec", vec!["context".to_string(), task], cwd_ref).await
                }
                "validate" => {
                    let mut args = vec!["validate".to_string()];
                    if let Some(path) = get_str(&arguments, "path") {
                        args.push(path);
                    }
                    run_tool("spec", args, cwd_ref).await
                }
                "new" => {
                    let name = match require_str(&arguments, "name") {
                        Ok(n) => n,
                        Err(e) => return e,
                    };
                    let mut args = vec!["new".to_string(), name];
                    if let Some(spec_type) = get_str(&arguments, "type") {
                        args.extend(["--type".to_string(), spec_type]);
                    }
                    run_tool("spec", args, cwd_ref).await
                }
                _ => ToolResult::error(format!("Unknown spec action: {}", action)),
            }
        }

        // =========================================================================
        // UNDO - consolidated undo management
        // =========================================================================
        "undo" => {
            let action = match get_str(&arguments, "action") {
                Some(a) => a,
                None => return ToolResult::error("Missing required argument: action"),
            };

            match action.as_str() {
                "checkpoint" => {
                    let name = match require_str(&arguments, "name") {
                        Ok(n) => n,
                        Err(e) => return e,
                    };
                    run_tool("undo", vec!["checkpoint".to_string(), name], cwd_ref).await
                }
                "last" => run_tool("undo", vec!["last".to_string()], cwd_ref).await,
                "timeline" => {
                    let mut args = vec!["timeline".to_string()];
                    if let Some(n) = get_int(&arguments, "limit") {
                        args.extend(["-n".to_string(), n.to_string()]);
                    }
                    run_tool("undo", args, cwd_ref).await
                }
                "restore" => {
                    let id = match require_str(&arguments, "id") {
                        Ok(i) => i,
                        Err(e) => return e,
                    };
                    run_tool("undo", vec!["to".to_string(), id], cwd_ref).await
                }
                _ => ToolResult::error(format!("Unknown undo action: {}", action)),
            }
        }

        // =========================================================================
        // JOURNAL - consolidated journal management
        // =========================================================================
        "journal" => {
            let action = match get_str(&arguments, "action") {
                Some(a) => a,
                None => return ToolResult::error("Missing required argument: action"),
            };

            match action.as_str() {
                "what" => {
                    let mut args = vec!["what".to_string()];
                    if let Some(hours) = get_float(&arguments, "hours") {
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
                "events" => {
                    let mut args = vec!["events".to_string(), "--json".to_string()];
                    if let Some(hours) = get_float(&arguments, "hours") {
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
                "summary" => {
                    let mut args = vec!["summary".to_string(), "--json".to_string()];
                    if let Some(hours) = get_float(&arguments, "hours") {
                        args.extend(["-h".to_string(), hours.to_string()]);
                    }
                    run_tool("journal", args, None).await
                }
                "log" => {
                    let summary = match require_str(&arguments, "summary_text") {
                        Ok(s) => s,
                        Err(e) => return e,
                    };
                    let source = get_str(&arguments, "source").unwrap_or_else(|| "mcp".to_string());
                    let event_type = get_str(&arguments, "event_type").unwrap_or_else(|| "custom".to_string());
                    run_tool("journal", vec!["log".to_string(), summary, source, event_type], None).await
                }
                _ => ToolResult::error(format!("Unknown journal action: {}", action)),
            }
        }

        // =========================================================================
        // EVOLVE - consolidated code evolution
        // =========================================================================
        "evolve" => {
            let action = match get_str(&arguments, "action") {
                Some(a) => a,
                None => return ToolResult::error("Missing required argument: action"),
            };
            let path = match require_str(&arguments, "target_path") {
                Ok(p) => p,
                Err(e) => return e,
            };

            match action.as_str() {
                "analyze" => {
                    let mut args = vec![];
                    if get_bool(&arguments, "json").unwrap_or(true) {
                        args.push("--json".to_string());
                    }
                    args.push(path);
                    run_tool("evolve", args, cwd_ref).await
                }
                "intent" => run_tool("evolve", vec!["intent".to_string(), path], cwd_ref).await,
                "gaps" => run_tool("evolve", vec!["gaps".to_string(), path], cwd_ref).await,
                "path" => run_tool("evolve", vec!["path".to_string(), path], cwd_ref).await,
                _ => ToolResult::error(format!("Unknown evolve action: {}", action)),
            }
        }

        // =========================================================================
        // RESOLVE - consolidated uncertainty resolution
        // =========================================================================
        "resolve" => {
            let action = match get_str(&arguments, "action") {
                Some(a) => a,
                None => return ToolResult::error("Missing required argument: action"),
            };

            match action.as_str() {
                "resolve" => {
                    let question = match require_str(&arguments, "question") {
                        Ok(q) => q,
                        Err(e) => return e,
                    };
                    let mut args = vec![];
                    if get_bool(&arguments, "json").unwrap_or(true) {
                        args.push("--json".to_string());
                    }
                    args.push(question);
                    run_tool("resolve", args, cwd_ref).await
                }
                "intent" => {
                    let question = match require_str(&arguments, "question") {
                        Ok(q) => q,
                        Err(e) => return e,
                    };
                    run_tool("resolve", vec!["--intent".to_string(), question], cwd_ref).await
                }
                "gather" => {
                    let question = match require_str(&arguments, "question") {
                        Ok(q) => q,
                        Err(e) => return e,
                    };
                    run_tool("resolve", vec!["--gather".to_string(), question], cwd_ref).await
                }
                "log" => {
                    let decision = match require_str(&arguments, "decision") {
                        Ok(d) => d,
                        Err(e) => return e,
                    };
                    let mut args = vec!["--log".to_string(), decision];
                    if let Some(reasoning) = get_str(&arguments, "reasoning") {
                        args.push(reasoning);
                    }
                    run_tool("resolve", args, cwd_ref).await
                }
                _ => ToolResult::error(format!("Unknown resolve action: {}", action)),
            }
        }

        // =========================================================================
        // LOOP - consolidated iteration
        // =========================================================================
        "loop" => {
            let action = match get_str(&arguments, "action") {
                Some(a) => a,
                None => return ToolResult::error("Missing required argument: action"),
            };

            match action.as_str() {
                "start" => {
                    let task = match require_str(&arguments, "task") {
                        Ok(t) => t,
                        Err(e) => return e,
                    };
                    let promise = match require_str(&arguments, "promise") {
                        Ok(p) => p,
                        Err(e) => return e,
                    };
                    let mut args = vec!["start".to_string(), task, "--promise".to_string(), promise];
                    if let Some(n) = get_int(&arguments, "max_iterations") {
                        args.extend(["-n".to_string(), n.to_string()]);
                    }
                    run_tool("loop", args, cwd_ref).await
                }
                "status" => run_tool("loop", vec!["status".to_string()], cwd_ref).await,
                "stop" => run_tool("loop", vec!["stop".to_string()], cwd_ref).await,
                _ => ToolResult::error(format!("Unknown loop action: {}", action)),
            }
        }

        // =========================================================================
        // PROJECT - consolidated project analysis
        // =========================================================================
        "project" => {
            let action = match get_str(&arguments, "action") {
                Some(a) => a,
                None => return ToolResult::error("Missing required argument: action"),
            };

            match action.as_str() {
                "info" => run_tool("project", vec!["summary".to_string()], cwd_ref).await,
                "symbols" => {
                    let mut args = vec!["search".to_string()];
                    if let Some(t) = get_str(&arguments, "symbol_type") {
                        args.extend(["--type".to_string(), t]);
                    }
                    args.push("*".to_string());
                    run_tool("project", args, cwd_ref).await
                }
                "tree" => {
                    let mut args = vec!["tree".to_string()];
                    if let Some(d) = get_int(&arguments, "depth") {
                        args.extend(["--depth".to_string(), d.to_string()]);
                    }
                    run_tool("project", args, cwd_ref).await
                }
                _ => ToolResult::error(format!("Unknown project action: {}", action)),
            }
        }

        // =========================================================================
        // SCRATCH - consolidated scratch environments
        // =========================================================================
        "scratch" => {
            let action = match get_str(&arguments, "action") {
                Some(a) => a,
                None => return ToolResult::error("Missing required argument: action"),
            };

            match action.as_str() {
                "new" => {
                    let name = match require_str(&arguments, "name") {
                        Ok(n) => n,
                        Err(e) => return e,
                    };
                    run_tool("scratch", vec!["new".to_string(), name], cwd_ref).await
                }
                "list" => run_tool("scratch", vec!["list".to_string()], None).await,
                "destroy" => {
                    let name = match require_str(&arguments, "name") {
                        Ok(n) => n,
                        Err(e) => return e,
                    };
                    run_tool("scratch", vec!["destroy".to_string(), name], None).await
                }
                _ => ToolResult::error(format!("Unknown scratch action: {}", action)),
            }
        }

        // =========================================================================
        // CODEX - consolidated semantic search
        // =========================================================================
        "codex" => {
            let action = match get_str(&arguments, "action") {
                Some(a) => a,
                None => return ToolResult::error("Missing required argument: action"),
            };

            match action.as_str() {
                "search" => {
                    let query = match require_str(&arguments, "query") {
                        Ok(q) => q,
                        Err(e) => return e,
                    };
                    let mut args = vec!["search".to_string(), query];
                    if let Some(limit) = get_int(&arguments, "limit") {
                        args.extend(["--limit".to_string(), limit.to_string()]);
                    }
                    run_tool("codex", args, cwd_ref).await
                }
                "index" => run_tool("codex", vec!["index".to_string()], cwd_ref).await,
                _ => ToolResult::error(format!("Unknown codex action: {}", action)),
            }
        }

        // =========================================================================
        // CONTEXT - consolidated context analysis
        // =========================================================================
        "context" => {
            let action = match get_str(&arguments, "action") {
                Some(a) => a,
                None => return ToolResult::error("Missing required argument: action"),
            };

            match action.as_str() {
                "estimate" => run_tool("context", vec!["estimate".to_string()], None).await,
                "breakdown" => run_tool("context", vec!["breakdown".to_string()], None).await,
                _ => ToolResult::error(format!("Unknown context action: {}", action)),
            }
        }

        // =========================================================================
        // ERROR_DB - consolidated error database
        // =========================================================================
        "error_db" => {
            let action = match get_str(&arguments, "action") {
                Some(a) => a,
                None => return ToolResult::error("Missing required argument: action"),
            };

            match action.as_str() {
                "match" => {
                    let error = match require_str(&arguments, "error") {
                        Ok(e) => e,
                        Err(e) => return e,
                    };
                    run_tool("error-db", vec!["search".to_string(), error], None).await
                }
                "add" => {
                    let pattern = match require_str(&arguments, "pattern") {
                        Ok(p) => p,
                        Err(e) => return e,
                    };
                    let solution = match require_str(&arguments, "solution") {
                        Ok(s) => s,
                        Err(e) => return e,
                    };
                    let mut args = vec!["add".to_string(), pattern, "--solution".to_string(), solution];
                    if let Some(cat) = get_str(&arguments, "category") {
                        args.extend(["--category".to_string(), cat]);
                    }
                    run_tool("error-db", args, None).await
                }
                _ => ToolResult::error(format!("Unknown error_db action: {}", action)),
            }
        }

        // =========================================================================
        // ANALYZE - consolidated test analysis
        // =========================================================================
        "analyze" => {
            let action = match get_str(&arguments, "action") {
                Some(a) => a,
                None => return ToolResult::error("Missing required argument: action"),
            };

            match action.as_str() {
                "tests" => {
                    let mut args = vec!["tests".to_string()];
                    if let Some(path) = get_str(&arguments, "path") {
                        args.push(path);
                    }
                    if get_bool(&arguments, "dry_run").unwrap_or(false) {
                        args.push("--dry-run".to_string());
                    }
                    if let Some(n) = get_int(&arguments, "max_iterations") {
                        args.extend(["-n".to_string(), n.to_string()]);
                    }
                    if let Some(promise) = get_str(&arguments, "promise") {
                        args.extend(["--promise".to_string(), promise]);
                    }
                    args.push("--json".to_string());
                    run_tool("analyze", args, cwd_ref).await
                }
                "gaps" => {
                    let mut args = vec!["gaps".to_string()];
                    if let Some(path) = get_str(&arguments, "path") {
                        args.push(path);
                    }
                    args.push("--json".to_string());
                    run_tool("analyze", args, cwd_ref).await
                }
                _ => ToolResult::error(format!("Unknown analyze action: {}", action)),
            }
        }

        // =========================================================================
        // VERIFY - unchanged, single tool
        // =========================================================================
        "verify" => {
            let mut args = vec![];
            if get_bool(&arguments, "quick").unwrap_or(false) {
                args.push("--quick".to_string());
            }
            run_tool("verify", args, cwd_ref).await
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
