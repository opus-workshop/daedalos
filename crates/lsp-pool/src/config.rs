//! Configuration for LSP Pool
//!
//! Manages language server configurations and pool settings.

// Allow unused code - some methods kept for daemon implementation
#![allow(dead_code)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Maximum number of concurrent servers
    #[serde(default = "default_max_servers")]
    pub max_servers: usize,

    /// Memory limit in MB for the entire pool
    #[serde(default = "default_memory_limit")]
    pub memory_limit_mb: u64,

    /// Idle timeout in minutes before evicting a server
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_minutes: u64,

    /// Health check interval in seconds
    #[serde(default = "default_health_check_interval")]
    pub health_check_interval: u64,

    /// Language server configurations
    #[serde(default = "default_servers")]
    pub servers: HashMap<String, ServerConfig>,
}

fn default_max_servers() -> usize {
    5
}
fn default_memory_limit() -> u64 {
    2048
}
fn default_idle_timeout() -> u64 {
    30
}
fn default_health_check_interval() -> u64 {
    60
}

/// Configuration for a specific language server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Command to start the server
    pub command: Vec<String>,

    /// File extensions this server handles
    pub extensions: Vec<String>,

    /// Estimated memory usage in MB
    #[serde(default = "default_memory_estimate")]
    pub memory_estimate_mb: u64,

    /// Alternative server commands (fallbacks)
    #[serde(default)]
    pub alternatives: Vec<String>,
}

fn default_memory_estimate() -> u64 {
    300
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_servers: default_max_servers(),
            memory_limit_mb: default_memory_limit(),
            idle_timeout_minutes: default_idle_timeout(),
            health_check_interval: default_health_check_interval(),
            servers: default_servers(),
        }
    }
}

fn default_servers() -> HashMap<String, ServerConfig> {
    let mut servers = HashMap::new();

    servers.insert(
        "typescript".to_string(),
        ServerConfig {
            command: vec![
                "typescript-language-server".to_string(),
                "--stdio".to_string(),
            ],
            extensions: vec![
                ".ts".to_string(),
                ".tsx".to_string(),
                ".js".to_string(),
                ".jsx".to_string(),
                ".mjs".to_string(),
                ".cjs".to_string(),
            ],
            memory_estimate_mb: 400,
            alternatives: vec![],
        },
    );

    servers.insert(
        "python".to_string(),
        ServerConfig {
            command: vec!["pyright-langserver".to_string(), "--stdio".to_string()],
            extensions: vec![".py".to_string(), ".pyi".to_string()],
            memory_estimate_mb: 300,
            alternatives: vec!["pylsp".to_string(), "jedi-language-server".to_string()],
        },
    );

    servers.insert(
        "rust".to_string(),
        ServerConfig {
            command: vec!["rust-analyzer".to_string()],
            extensions: vec![".rs".to_string()],
            memory_estimate_mb: 500,
            alternatives: vec![],
        },
    );

    servers.insert(
        "go".to_string(),
        ServerConfig {
            command: vec!["gopls".to_string(), "serve".to_string()],
            extensions: vec![".go".to_string()],
            memory_estimate_mb: 200,
            alternatives: vec![],
        },
    );

    servers.insert(
        "c".to_string(),
        ServerConfig {
            command: vec!["clangd".to_string()],
            extensions: vec![".c".to_string(), ".h".to_string()],
            memory_estimate_mb: 400,
            alternatives: vec![],
        },
    );

    servers.insert(
        "cpp".to_string(),
        ServerConfig {
            command: vec!["clangd".to_string()],
            extensions: vec![
                ".cpp".to_string(),
                ".hpp".to_string(),
                ".cc".to_string(),
                ".hh".to_string(),
                ".cxx".to_string(),
            ],
            memory_estimate_mb: 500,
            alternatives: vec![],
        },
    );

    servers.insert(
        "java".to_string(),
        ServerConfig {
            command: vec!["jdtls".to_string()],
            extensions: vec![".java".to_string()],
            memory_estimate_mb: 800,
            alternatives: vec![],
        },
    );

    servers.insert(
        "kotlin".to_string(),
        ServerConfig {
            command: vec!["kotlin-language-server".to_string()],
            extensions: vec![".kt".to_string(), ".kts".to_string()],
            memory_estimate_mb: 600,
            alternatives: vec![],
        },
    );

    servers.insert(
        "swift".to_string(),
        ServerConfig {
            command: vec!["sourcekit-lsp".to_string()],
            extensions: vec![".swift".to_string()],
            memory_estimate_mb: 400,
            alternatives: vec![],
        },
    );

    servers.insert(
        "lua".to_string(),
        ServerConfig {
            command: vec!["lua-language-server".to_string()],
            extensions: vec![".lua".to_string()],
            memory_estimate_mb: 150,
            alternatives: vec![],
        },
    );

    servers.insert(
        "ruby".to_string(),
        ServerConfig {
            command: vec!["solargraph".to_string(), "stdio".to_string()],
            extensions: vec![".rb".to_string(), ".rake".to_string()],
            memory_estimate_mb: 300,
            alternatives: vec![],
        },
    );

    servers.insert(
        "elixir".to_string(),
        ServerConfig {
            command: vec!["elixir-ls".to_string()],
            extensions: vec![".ex".to_string(), ".exs".to_string()],
            memory_estimate_mb: 400,
            alternatives: vec![],
        },
    );

    servers.insert(
        "zig".to_string(),
        ServerConfig {
            command: vec!["zls".to_string()],
            extensions: vec![".zig".to_string()],
            memory_estimate_mb: 200,
            alternatives: vec![],
        },
    );

    servers.insert(
        "ocaml".to_string(),
        ServerConfig {
            command: vec!["ocamllsp".to_string()],
            extensions: vec![".ml".to_string(), ".mli".to_string()],
            memory_estimate_mb: 300,
            alternatives: vec![],
        },
    );

    servers.insert(
        "haskell".to_string(),
        ServerConfig {
            command: vec![
                "haskell-language-server-wrapper".to_string(),
                "--lsp".to_string(),
            ],
            extensions: vec![".hs".to_string()],
            memory_estimate_mb: 600,
            alternatives: vec![],
        },
    );

    servers.insert(
        "bash".to_string(),
        ServerConfig {
            command: vec!["bash-language-server".to_string(), "start".to_string()],
            extensions: vec![".sh".to_string(), ".bash".to_string()],
            memory_estimate_mb: 100,
            alternatives: vec![],
        },
    );

    servers.insert(
        "yaml".to_string(),
        ServerConfig {
            command: vec!["yaml-language-server".to_string(), "--stdio".to_string()],
            extensions: vec![".yaml".to_string(), ".yml".to_string()],
            memory_estimate_mb: 100,
            alternatives: vec![],
        },
    );

    servers.insert(
        "json".to_string(),
        ServerConfig {
            command: vec![
                "vscode-json-language-server".to_string(),
                "--stdio".to_string(),
            ],
            extensions: vec![".json".to_string(), ".jsonc".to_string()],
            memory_estimate_mb: 100,
            alternatives: vec![],
        },
    );

    servers.insert(
        "html".to_string(),
        ServerConfig {
            command: vec![
                "vscode-html-language-server".to_string(),
                "--stdio".to_string(),
            ],
            extensions: vec![".html".to_string(), ".htm".to_string()],
            memory_estimate_mb: 100,
            alternatives: vec![],
        },
    );

    servers.insert(
        "css".to_string(),
        ServerConfig {
            command: vec![
                "vscode-css-language-server".to_string(),
                "--stdio".to_string(),
            ],
            extensions: vec![".css".to_string(), ".scss".to_string(), ".less".to_string()],
            memory_estimate_mb: 100,
            alternatives: vec![],
        },
    );

    servers.insert(
        "nix".to_string(),
        ServerConfig {
            command: vec!["nil".to_string()],
            extensions: vec![".nix".to_string()],
            memory_estimate_mb: 200,
            alternatives: vec![],
        },
    );

    servers
}

impl Config {
    /// Load configuration from the default path
    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        if path.exists() {
            let contents = std::fs::read_to_string(&path)?;
            let config: Config = serde_json::from_str(&contents)?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    /// Save configuration to the default path
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(path, contents)?;
        Ok(())
    }

    /// Get the configuration file path
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("daedalos")
            .join("lsp-pool")
            .join("config.json")
    }

    /// Get the data directory path
    pub fn data_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("~/.local/share"))
            .join("daedalos")
            .join("lsp-pool")
    }

    /// Get the socket path
    pub fn socket_path() -> PathBuf {
        // Try /run/daedalos first (NixOS), fall back to data dir
        let run_path = PathBuf::from("/run/daedalos/lsp-pool.sock");
        if run_path.parent().map(|p| p.exists()).unwrap_or(false) {
            run_path
        } else {
            Self::data_dir().join("lsp-pool.sock")
        }
    }

    /// Get server configuration for a language
    pub fn get_server(&self, language: &str) -> Option<&ServerConfig> {
        self.servers.get(language)
    }

    /// Detect language from file extension
    pub fn detect_language(&self, path: &std::path::Path) -> Option<String> {
        let ext = path.extension()?.to_str()?;
        let ext_with_dot = format!(".{}", ext);

        for (language, config) in &self.servers {
            if config.extensions.iter().any(|e| e == &ext_with_dot) {
                return Some(language.clone());
            }
        }
        None
    }
}

/// Extension to language mapping
pub fn extension_to_language(ext: &str) -> Option<&'static str> {
    match ext.to_lowercase().as_str() {
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" => Some("typescript"),
        "py" | "pyi" => Some("python"),
        "rs" => Some("rust"),
        "go" => Some("go"),
        "c" | "h" => Some("c"),
        "cpp" | "hpp" | "cc" | "hh" | "cxx" => Some("cpp"),
        "java" => Some("java"),
        "kt" | "kts" => Some("kotlin"),
        "swift" => Some("swift"),
        "lua" => Some("lua"),
        "rb" | "rake" => Some("ruby"),
        "ex" | "exs" => Some("elixir"),
        "zig" => Some("zig"),
        "ml" | "mli" => Some("ocaml"),
        "hs" => Some("haskell"),
        "sh" | "bash" => Some("bash"),
        "yaml" | "yml" => Some("yaml"),
        "json" | "jsonc" => Some("json"),
        "html" | "htm" => Some("html"),
        "css" | "scss" | "less" => Some("css"),
        "nix" => Some("nix"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.max_servers, 5);
        assert_eq!(config.memory_limit_mb, 2048);
        assert!(!config.servers.is_empty());
    }

    #[test]
    fn test_extension_to_language() {
        assert_eq!(extension_to_language("rs"), Some("rust"));
        assert_eq!(extension_to_language("py"), Some("python"));
        assert_eq!(extension_to_language("ts"), Some("typescript"));
        assert_eq!(extension_to_language("unknown"), None);
    }

    #[test]
    fn test_detect_language() {
        let config = Config::default();
        let path = std::path::Path::new("test.rs");
        assert_eq!(config.detect_language(path), Some("rust".to_string()));
    }
}
