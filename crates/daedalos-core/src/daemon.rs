//! Daemon status checking

use crate::paths::Paths;
use std::path::PathBuf;

/// Status of a daemon
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonStatus {
    Running,
    Stopped,
    Error,
    Unknown,
}

impl DaemonStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Stopped => "stopped",
            Self::Error => "error",
            Self::Unknown => "unknown",
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Running => "●",
            Self::Stopped => "○",
            Self::Error => "●",
            Self::Unknown => "?",
        }
    }
}

/// Information about a daemon
#[derive(Debug, Clone)]
pub struct DaemonInfo {
    pub name: String,
    pub display_name: String,
    pub status: DaemonStatus,
    pub socket_path: Option<PathBuf>,
    pub pid: Option<u32>,
}

/// Check the status of all Daedalos daemons
pub fn check_all_daemons(paths: &Paths) -> Vec<DaemonInfo> {
    vec![
        check_daemon(paths, "loopd", "Loop Daemon", &["loopd"]),
        check_daemon(paths, "mcp-hub", "MCP Hub", &["daedalos_mcp", "mcp-hub"]),
        check_daemon(paths, "lsp-pool", "LSP Pool", &["lsp-pool", "lsp_pool"]),
        check_daemon(paths, "undod", "Undo Daemon", &["undod"]),
    ]
}

fn check_daemon(paths: &Paths, name: &str, display_name: &str, process_names: &[&str]) -> DaemonInfo {
    let socket_path = paths.socket(name);
    let socket_exists = socket_path.exists();

    // Check if any of the process names are running
    let process_running = process_names.iter().any(|pname| crate::process::is_running(pname));

    let status = match (socket_exists, process_running) {
        (true, true) => DaemonStatus::Running,
        (true, false) => DaemonStatus::Error, // Socket exists but process dead
        (false, true) => DaemonStatus::Running, // Process running, socket might be elsewhere
        (false, false) => DaemonStatus::Stopped,
    };

    DaemonInfo {
        name: name.to_string(),
        display_name: display_name.to_string(),
        status,
        socket_path: if socket_exists { Some(socket_path) } else { None },
        pid: None,
    }
}
