//! Configuration management for Daedalos tools

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Global Daedalos configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// Refresh interval for observe (seconds)
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval: f64,

    /// Default supervision level
    #[serde(default = "default_supervision_level")]
    pub supervision_level: String,
}

fn default_refresh_interval() -> f64 {
    2.0
}

fn default_supervision_level() -> String {
    "supervised".to_string()
}

impl Config {
    /// Load config from file
    pub fn load(path: &Path) -> Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            Ok(serde_json::from_str(&content)?)
        } else {
            Ok(Self::default())
        }
    }

    /// Save config to file
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}
