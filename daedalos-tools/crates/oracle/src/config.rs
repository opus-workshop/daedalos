//! Configuration loading for oracle

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::backend::Backend;

/// Oracle configuration
#[derive(Debug, Deserialize)]
pub struct Config {
    /// Default backend name
    #[serde(default = "default_backend")]
    pub default: String,

    /// Backend configurations
    #[serde(default = "default_backends")]
    pub backends: HashMap<String, BackendConfig>,
}

fn default_backend() -> String {
    "claude".to_string()
}

fn default_backends() -> HashMap<String, BackendConfig> {
    let mut backends = HashMap::new();

    backends.insert(
        "claude".to_string(),
        BackendConfig {
            command: "claude".to_string(),
            args: vec!["-p".to_string(), "{prompt}".to_string()],
            continue_flag: Some("-c".to_string()),
            session_flag: Some("--resume".to_string()),
            json_flag: Some("--output-format".to_string()),
            json_value: Some("json".to_string()),
        },
    );

    backends.insert(
        "opencode".to_string(),
        BackendConfig {
            command: "opencode".to_string(),
            args: vec!["-p".to_string(), "{prompt}".to_string()],
            continue_flag: None,
            session_flag: None,
            json_flag: Some("-f".to_string()),
            json_value: Some("json".to_string()),
        },
    );

    backends.insert(
        "ollama".to_string(),
        BackendConfig {
            command: "ollama".to_string(),
            args: vec![
                "run".to_string(),
                "llama3".to_string(),
                "{prompt}".to_string(),
            ],
            continue_flag: None,
            session_flag: None,
            json_flag: None,
            json_value: None,
        },
    );

    backends
}

/// Configuration for a single backend
#[derive(Debug, Clone, Deserialize)]
pub struct BackendConfig {
    /// The command to run
    pub command: String,

    /// Arguments (use {prompt} as placeholder)
    #[serde(default)]
    pub args: Vec<String>,

    /// Flag to continue last conversation
    pub continue_flag: Option<String>,

    /// Flag to resume a specific session
    pub session_flag: Option<String>,

    /// Flag for JSON output
    pub json_flag: Option<String>,

    /// Value for JSON output flag
    pub json_value: Option<String>,
}

impl Config {
    /// Load configuration from file or use defaults
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path();

        if config_path.exists() {
            let content = fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read config from {:?}", config_path))?;

            let mut config: Config = toml::from_str(&content)
                .with_context(|| format!("Failed to parse config from {:?}", config_path))?;

            // Merge with defaults (user config overrides defaults)
            let defaults = default_backends();
            for (name, backend) in defaults {
                config.backends.entry(name).or_insert(backend);
            }

            Ok(config)
        } else {
            // Use defaults
            Ok(Config {
                default: default_backend(),
                backends: default_backends(),
            })
        }
    }

    /// Get the config file path
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("oracle")
            .join("config.toml")
    }

    /// Get a backend by name
    pub fn get_backend(&self, name: &str) -> Result<Backend> {
        let config = self
            .backends
            .get(name)
            .with_context(|| format!("Unknown backend: {}", name))?;

        Ok(Backend {
            name: name.to_string(),
            command: config.command.clone(),
            args: config.args.clone(),
            continue_flag: config.continue_flag.clone(),
            session_flag: config.session_flag.clone(),
            json_flag: config.json_flag.clone(),
            json_value: config.json_value.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::load().unwrap();
        assert_eq!(config.default, "claude");
        assert!(config.backends.contains_key("claude"));
        assert!(config.backends.contains_key("opencode"));
        assert!(config.backends.contains_key("ollama"));
    }

    #[test]
    fn test_get_backend() {
        let config = Config::load().unwrap();
        let backend = config.get_backend("claude").unwrap();
        assert_eq!(backend.command, "claude");
    }
}
