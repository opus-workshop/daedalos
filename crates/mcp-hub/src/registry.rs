//! Server registry - catalog of known MCP servers

#![allow(dead_code)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::config::get_data_dir;

/// Built-in MCP server definitions
pub static BUILTIN_SERVERS: &[ServerInfo] = &[
    ServerInfo {
        name: "filesystem",
        description: "File system operations (read, write, list, search)",
        command: &["npx", "-y", "@modelcontextprotocol/server-filesystem"],
        args: &["/"],
        category: "core",
        tools: &[
            "read_file",
            "write_file",
            "list_directory",
            "create_directory",
            "move_file",
            "search_files",
            "get_file_info",
            "read_multiple_files",
        ],
        resources: &[],
        requires_auth: false,
        auth_env_vars: &[],
        source: "npm",
        enabled: true,
    },
    ServerInfo {
        name: "github",
        description: "GitHub operations (issues, PRs, code search)",
        command: &["npx", "-y", "@modelcontextprotocol/server-github"],
        args: &[],
        category: "integrations",
        tools: &[
            "create_issue",
            "create_pull_request",
            "search_code",
            "list_commits",
            "get_file_contents",
            "push_files",
        ],
        resources: &["repo", "issue", "pull_request"],
        requires_auth: true,
        auth_env_vars: &["GITHUB_TOKEN"],
        source: "npm",
        enabled: true,
    },
    ServerInfo {
        name: "memory",
        description: "Persistent memory for conversations",
        command: &["npx", "-y", "@modelcontextprotocol/server-memory"],
        args: &[],
        category: "core",
        tools: &["store", "retrieve", "list", "delete"],
        resources: &[],
        requires_auth: false,
        auth_env_vars: &[],
        source: "npm",
        enabled: true,
    },
    ServerInfo {
        name: "sqlite",
        description: "SQLite database operations",
        command: &["npx", "-y", "@modelcontextprotocol/server-sqlite"],
        args: &[],
        category: "data",
        tools: &[
            "read_query",
            "write_query",
            "create_table",
            "list_tables",
            "describe_table",
        ],
        resources: &[],
        requires_auth: false,
        auth_env_vars: &[],
        source: "npm",
        enabled: true,
    },
    ServerInfo {
        name: "fetch",
        description: "HTTP fetch operations",
        command: &["npx", "-y", "@modelcontextprotocol/server-fetch"],
        args: &[],
        category: "network",
        tools: &["fetch"],
        resources: &[],
        requires_auth: false,
        auth_env_vars: &[],
        source: "npm",
        enabled: true,
    },
    ServerInfo {
        name: "brave-search",
        description: "Brave Search API",
        command: &["npx", "-y", "@modelcontextprotocol/server-brave-search"],
        args: &[],
        category: "search",
        tools: &["brave_web_search", "brave_local_search"],
        resources: &[],
        requires_auth: true,
        auth_env_vars: &["BRAVE_API_KEY"],
        source: "npm",
        enabled: true,
    },
    ServerInfo {
        name: "postgres",
        description: "PostgreSQL database operations",
        command: &["npx", "-y", "@modelcontextprotocol/server-postgres"],
        args: &[],
        category: "data",
        tools: &["query", "list_tables", "describe_table", "list_schemas"],
        resources: &[],
        requires_auth: true,
        auth_env_vars: &["POSTGRES_CONNECTION_STRING"],
        source: "npm",
        enabled: true,
    },
    ServerInfo {
        name: "puppeteer",
        description: "Browser automation and web scraping",
        command: &["npx", "-y", "@modelcontextprotocol/server-puppeteer"],
        args: &[],
        category: "browser",
        tools: &[
            "puppeteer_navigate",
            "puppeteer_screenshot",
            "puppeteer_click",
            "puppeteer_fill",
            "puppeteer_select",
            "puppeteer_hover",
            "puppeteer_evaluate",
        ],
        resources: &[],
        requires_auth: false,
        auth_env_vars: &[],
        source: "npm",
        enabled: true,
    },
    ServerInfo {
        name: "slack",
        description: "Slack workspace operations",
        command: &["npx", "-y", "@modelcontextprotocol/server-slack"],
        args: &[],
        category: "communication",
        tools: &[
            "list_channels",
            "post_message",
            "reply_to_thread",
            "add_reaction",
            "get_channel_history",
            "get_thread_replies",
            "search_messages",
            "get_users",
        ],
        resources: &[],
        requires_auth: true,
        auth_env_vars: &["SLACK_BOT_TOKEN"],
        source: "npm",
        enabled: true,
    },
    ServerInfo {
        name: "google-drive",
        description: "Google Drive file operations",
        command: &["npx", "-y", "@anthropics/google-drive-mcp"],
        args: &[],
        category: "storage",
        tools: &["search_files", "read_file", "list_files"],
        resources: &[],
        requires_auth: true,
        auth_env_vars: &["GOOGLE_CREDENTIALS"],
        source: "npm",
        enabled: true,
    },
    ServerInfo {
        name: "google-maps",
        description: "Google Maps location services",
        command: &["npx", "-y", "@anthropics/google-maps-mcp"],
        args: &[],
        category: "location",
        tools: &[
            "geocode",
            "reverse_geocode",
            "search_places",
            "get_directions",
            "distance_matrix",
        ],
        resources: &[],
        requires_auth: true,
        auth_env_vars: &["GOOGLE_MAPS_API_KEY"],
        source: "npm",
        enabled: true,
    },
    ServerInfo {
        name: "time",
        description: "Time and timezone operations",
        command: &["npx", "-y", "@modelcontextprotocol/server-time"],
        args: &[],
        category: "utility",
        tools: &["get_current_time", "convert_timezone"],
        resources: &[],
        requires_auth: false,
        auth_env_vars: &[],
        source: "npm",
        enabled: true,
    },
    ServerInfo {
        name: "sequential-thinking",
        description: "Step-by-step reasoning tool",
        command: &["npx", "-y", "@modelcontextprotocol/server-sequential-thinking"],
        args: &[],
        category: "reasoning",
        tools: &["think_step_by_step"],
        resources: &[],
        requires_auth: false,
        auth_env_vars: &[],
        source: "npm",
        enabled: true,
    },
    ServerInfo {
        name: "git",
        description: "Git repository operations",
        command: &["npx", "-y", "@modelcontextprotocol/server-git"],
        args: &[],
        category: "development",
        tools: &[
            "git_status",
            "git_diff",
            "git_log",
            "git_commit",
            "git_add",
            "git_reset",
            "git_branch",
            "git_checkout",
        ],
        resources: &[],
        requires_auth: false,
        auth_env_vars: &[],
        source: "npm",
        enabled: true,
    },
];

/// Information about an MCP server
#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub command: &'static [&'static str],
    pub args: &'static [&'static str],
    pub category: &'static str,
    pub tools: &'static [&'static str],
    pub resources: &'static [&'static str],
    pub requires_auth: bool,
    pub auth_env_vars: &'static [&'static str],
    pub source: &'static str,
    pub enabled: bool,
}

impl ServerInfo {
    /// Convert to JSON value
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.name,
            "description": self.description,
            "command": self.command,
            "args": self.args,
            "category": self.category,
            "tools": self.tools,
            "resources": self.resources,
            "requires_auth": self.requires_auth,
            "auth_env_vars": self.auth_env_vars,
            "source": self.source,
            "enabled": self.enabled,
        })
    }
}

/// Dynamic server info for installed servers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledServer {
    pub name: String,
    pub description: String,
    pub command: Vec<String>,
    pub args: Vec<String>,
    pub category: String,
    pub tools: Vec<String>,
    pub resources: Vec<String>,
    pub requires_auth: bool,
    pub auth_env_vars: Vec<String>,
    pub source: String,
    pub enabled: bool,
}

/// Server registry
pub struct ServerRegistry {
    registry_path: PathBuf,
    installed: HashMap<String, InstalledServer>,
    enabled_state: HashMap<String, bool>,
}

impl ServerRegistry {
    /// Create a new server registry
    pub fn new() -> Result<Self> {
        let registry_path = get_data_dir().join("registry");
        fs::create_dir_all(&registry_path)?;

        let mut registry = Self {
            registry_path,
            installed: HashMap::new(),
            enabled_state: HashMap::new(),
        };

        registry.load_installed()?;
        registry.load_state()?;

        Ok(registry)
    }

    /// Load installed servers from registry
    fn load_installed(&mut self) -> Result<()> {
        let installed_file = self.registry_path.join("installed.json");
        if installed_file.exists() {
            let content = fs::read_to_string(&installed_file)?;
            if let Ok(servers) = serde_json::from_str(&content) {
                self.installed = servers;
            }
        }
        Ok(())
    }

    /// Load enabled/disabled state
    fn load_state(&mut self) -> Result<()> {
        let state_file = self.registry_path.join("state.json");
        if state_file.exists() {
            let content = fs::read_to_string(&state_file)?;
            if let Ok(state) = serde_json::from_str(&content) {
                self.enabled_state = state;
            }
        }
        Ok(())
    }

    /// List servers, optionally filtered by category
    pub fn list(&self, category: Option<&str>) -> Vec<ServerInfo> {
        let mut servers: Vec<ServerInfo> = BUILTIN_SERVERS
            .iter()
            .filter(|s| category.map_or(true, |c| s.category == c))
            .cloned()
            .collect();

        // Apply enabled state overrides
        for server in &mut servers {
            if let Some(&enabled) = self.enabled_state.get(server.name) {
                // We can't modify static ServerInfo, so we return a modified copy
                // This is a bit awkward with static data, but works for display
                let _ = enabled; // State is tracked separately
            }
        }

        servers
    }

    /// Search for servers by name, tool, or description
    pub fn search(&self, query: &str) -> Vec<ServerInfo> {
        let query = query.to_lowercase();

        BUILTIN_SERVERS
            .iter()
            .filter(|s| {
                s.name.to_lowercase().contains(&query)
                    || s.description.to_lowercase().contains(&query)
                    || s.tools.iter().any(|t| t.to_lowercase().contains(&query))
            })
            .cloned()
            .collect()
    }

    /// Get a server by name
    pub fn get(&self, name: &str) -> Option<ServerInfo> {
        BUILTIN_SERVERS.iter().find(|s| s.name == name).cloned()
    }

    /// Check if a server is enabled
    pub fn is_enabled(&self, name: &str) -> bool {
        self.enabled_state.get(name).copied().unwrap_or(true)
    }

    /// Enable a server
    pub fn enable(&mut self, name: &str) -> Result<bool> {
        if BUILTIN_SERVERS.iter().any(|s| s.name == name) || self.installed.contains_key(name) {
            self.enabled_state.insert(name.to_string(), true);
            self.save_state()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Disable a server
    pub fn disable(&mut self, name: &str) -> Result<bool> {
        if BUILTIN_SERVERS.iter().any(|s| s.name == name) || self.installed.contains_key(name) {
            self.enabled_state.insert(name.to_string(), false);
            self.save_state()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Save enabled/disabled state
    fn save_state(&self) -> Result<()> {
        let state_file = self.registry_path.join("state.json");
        let content = serde_json::to_string_pretty(&self.enabled_state)?;
        fs::write(&state_file, content)?;
        Ok(())
    }

    /// Get all tools from all enabled servers
    pub fn get_tools(&self) -> Vec<ToolInfo> {
        let mut tools = Vec::new();

        for server in BUILTIN_SERVERS {
            if self.is_enabled(server.name) {
                for tool in server.tools {
                    tools.push(ToolInfo {
                        server: server.name.to_string(),
                        name: tool.to_string(),
                        description: format!("{} from {}", tool, server.name),
                    });
                }
            }
        }

        tools
    }
}

/// Tool information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub server: String,
    pub name: String,
    pub description: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_servers() {
        assert!(!BUILTIN_SERVERS.is_empty());

        // Check filesystem server exists
        let fs_server = BUILTIN_SERVERS.iter().find(|s| s.name == "filesystem");
        assert!(fs_server.is_some());
        let fs = fs_server.unwrap();
        assert!(fs.tools.contains(&"read_file"));
        assert!(!fs.requires_auth);
    }

    #[test]
    fn test_server_search() {
        let registry = ServerRegistry {
            registry_path: PathBuf::new(),
            installed: HashMap::new(),
            enabled_state: HashMap::new(),
        };

        let results = registry.search("file");
        assert!(!results.is_empty());
    }
}
