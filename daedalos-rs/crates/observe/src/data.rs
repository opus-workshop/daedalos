//! Data fetching from Daedalos tools

use crate::app::{AgentInfo, LoopInfo};
use daedalos_core::Paths;
use serde::Deserialize;
use std::process::Command;

#[derive(Deserialize)]
struct LoopJson {
    id: Option<String>,
    task: Option<String>,
    status: Option<String>,
    iteration: Option<u32>,
    duration: Option<f64>,
}

#[derive(Deserialize)]
struct AgentJson {
    slot: Option<u32>,
    name: Option<String>,
    template: Option<String>,
    status: Option<String>,
    uptime: Option<f64>,
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
                        task: l.task.unwrap_or_default(),
                        status: l.status.unwrap_or_default(),
                        iteration: l.iteration.unwrap_or(0),
                        duration: l.duration.unwrap_or(0.0),
                    })
                    .collect(),
                Err(_) => Vec::new(),
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
                        uptime: a.uptime.unwrap_or(0.0),
                    })
                    .collect(),
                Err(_) => Vec::new(),
            }
        }
        _ => Vec::new(),
    }
}
