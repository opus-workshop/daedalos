//! Data fetching from Daedalos tools

use crate::app::{AgentInfo, ClaudeTaskInfo, EventInfo, LoopInfo};
use chrono::{Local, TimeZone};
use daedalos_core::Paths;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Deserialize)]
struct LoopJson {
    id: Option<String>,
    prompt: Option<String>,
    status: Option<String>,
    current_iteration: Option<u32>,
    max_iterations: Option<u32>,
}

#[derive(Deserialize)]
struct AgentJson {
    slot: Option<u32>,
    name: Option<String>,
    template: Option<String>,
    status: Option<String>,
    uptime: Option<String>,  // String like "1d 1h", not float
}

/// Fetch active loops from the loop tool
pub fn fetch_loops(paths: &Paths) -> Vec<LoopInfo> {
    let loop_tool = paths.tools.join("loop");
    if !loop_tool.exists() {
        return Vec::new();
    }

    let output = Command::new(&loop_tool)
        .args(["list", "--json"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.trim().is_empty() {
                return Vec::new();
            }

            match serde_json::from_str::<Vec<LoopJson>>(&stdout) {
                Ok(loops) => loops
                    .into_iter()
                    .map(|l| LoopInfo {
                        id: l.id.unwrap_or_default(),
                        task: l.prompt.unwrap_or_default(),
                        status: l.status.unwrap_or_default(),
                        iteration: l.current_iteration.unwrap_or(0),
                        duration: 0.0, // Duration not available in loop list
                    })
                    .collect(),
                Err(e) => {
                    eprintln!("Failed to parse loop JSON: {}", e);
                    Vec::new()
                }
            }
        }
        _ => Vec::new(),
    }
}

/// Fetch active agents from the agent tool
pub fn fetch_agents(paths: &Paths) -> Vec<AgentInfo> {
    let agent_tool = paths.tools.join("agent");
    if !agent_tool.exists() {
        return Vec::new();
    }

    let output = Command::new(&agent_tool)
        .args(["list", "--json"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.trim().is_empty() {
                return Vec::new();
            }

            match serde_json::from_str::<Vec<AgentJson>>(&stdout) {
                Ok(agents) => agents
                    .into_iter()
                    .map(|a| AgentInfo {
                        slot: a.slot.unwrap_or(0),
                        name: a.name.unwrap_or_default(),
                        template: a.template.unwrap_or_default(),
                        status: a.status.unwrap_or_default(),
                        uptime: a.uptime.unwrap_or_default(),
                    })
                    .collect(),
                Err(_) => Vec::new(),
            }
        }
        _ => Vec::new(),
    }
}

#[derive(Deserialize)]
struct JournalEventJson {
    timestamp: Option<f64>,
    source: Option<String>,
    event_type: Option<String>,
    summary: Option<String>,
}

/// Fetch Claude Code subagent tasks
pub fn fetch_claude_tasks() -> Vec<ClaudeTaskInfo> {
    let mut tasks = Vec::new();

    // Claude Code stores tasks in /tmp/claude/-{path-encoded}/tasks/
    let tmp_claude = PathBuf::from("/tmp/claude");
    if !tmp_claude.exists() {
        return tasks;
    }

    // Find all task directories
    if let Ok(entries) = fs::read_dir(&tmp_claude) {
        for entry in entries.flatten() {
            let tasks_dir = entry.path().join("tasks");
            if tasks_dir.is_dir() {
                if let Ok(task_files) = fs::read_dir(&tasks_dir) {
                    for task_file in task_files.flatten() {
                        let path = task_file.path();
                        if path.extension().map_or(false, |e| e == "output") {
                            // Check if task is still running by looking for active tail process
                            let task_id = path.file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("unknown")
                                .to_string();

                            // Read first line for description
                            let description = fs::read_to_string(&path)
                                .ok()
                                .and_then(|s| s.lines().next().map(|l| l.to_string()))
                                .unwrap_or_else(|| "Running...".to_string());

                            // Check modified time to determine if active
                            let is_active = task_file.metadata()
                                .and_then(|m| m.modified())
                                .map(|t| t.elapsed().map(|e| e.as_secs() < 30).unwrap_or(false))
                                .unwrap_or(false);

                            if is_active {
                                tasks.push(ClaudeTaskInfo {
                                    task_id,
                                    description, // Don't truncate - UI handles it responsively
                                    status: "running".to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    tasks
}

/// Fetch recent events from journal
pub fn fetch_events(paths: &Paths, limit: usize) -> Vec<EventInfo> {
    let journal_tool = paths.tools.join("journal");
    if !journal_tool.exists() {
        return Vec::new();
    }

    let output = Command::new(&journal_tool)
        .args(["events", "--json", "--hours", "1", "--limit", &limit.to_string()])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.trim().is_empty() {
                return Vec::new();
            }

            match serde_json::from_str::<Vec<JournalEventJson>>(&stdout) {
                Ok(events) => events
                    .into_iter()
                    .filter_map(|e| {
                        let ts = e.timestamp?;
                        let secs = ts as i64;
                        let nsecs = ((ts - secs as f64) * 1_000_000_000.0) as u32;
                        let dt = Local.timestamp_opt(secs, nsecs).single()?;

                        Some(EventInfo {
                            timestamp: dt,
                            source: e.source.unwrap_or_else(|| "unknown".to_string()),
                            message: e.summary.unwrap_or_default(),
                        })
                    })
                    .collect(),
                Err(_) => Vec::new(),
            }
        }
        _ => Vec::new(),
    }
}
