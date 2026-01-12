//! Standard paths used by Daedalos tools

use std::path::PathBuf;

/// Standard Daedalos paths
pub struct Paths {
    /// Data directory (~/.local/share/daedalos)
    pub data: PathBuf,
    /// Config directory (~/.config/daedalos)
    pub config: PathBuf,
    /// Tools directory (~/.local/bin)
    pub tools: PathBuf,
    /// Runtime directory (/run/daedalos or ~/.local/share/daedalos)
    pub runtime: PathBuf,
}

impl Default for Paths {
    fn default() -> Self {
        Self::new()
    }
}

impl Paths {
    pub fn new() -> Self {
        let data = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("~/.local/share"))
            .join("daedalos");

        let config = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("daedalos");

        let tools = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(".local/bin");

        // Try /run/daedalos first (NixOS), fall back to data dir
        let runtime = if PathBuf::from("/run/daedalos").exists() {
            PathBuf::from("/run/daedalos")
        } else {
            data.clone()
        };

        Self {
            data,
            config,
            tools,
            runtime,
        }
    }

    /// Get socket path for a daemon
    pub fn socket(&self, daemon: &str) -> PathBuf {
        self.runtime.join(daemon).join(format!("{}.sock", daemon))
    }

    /// Get state file path for a tool
    pub fn state(&self, tool: &str) -> PathBuf {
        self.data.join(tool)
    }
}
