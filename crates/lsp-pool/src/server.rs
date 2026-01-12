//! Language Server Management
//!
//! Manages the lifecycle of language server processes.

// Allow unused code - some methods kept for daemon implementation
#![allow(dead_code)]

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::io::BufReader;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{Duration, Instant};
use sysinfo::ProcessesToUpdate;

use crate::config::{extension_to_language, Config};
use crate::protocol::*;

/// Status of a language server
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerStatus {
    Initializing,
    Warm,
    Busy,
    Error,
    Unhealthy,
}

impl ServerStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Initializing => "initializing",
            Self::Warm => "warm",
            Self::Busy => "busy",
            Self::Error => "error",
            Self::Unhealthy => "unhealthy",
        }
    }
}

impl std::fmt::Display for ServerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// State of a running language server
pub struct ServerState {
    pub language: String,
    pub project: PathBuf,
    pub process: Child,
    pub status: ServerStatus,
    pub started_at: Instant,
    pub last_query: Instant,
    pub memory_mb: f64,
    pub request_id: AtomicI64,
    pub stderr_log: VecDeque<String>,
    pub health_failures: u32,
    pub restart_count: u32,
}

impl ServerState {
    /// Get server key (language:project)
    pub fn key(&self) -> String {
        format!("{}:{}", self.language, self.project.display())
    }

    /// Get next request ID
    pub fn next_request_id(&self) -> i64 {
        self.request_id.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Update memory usage from system info
    pub fn update_memory(&mut self) {
        use sysinfo::{Pid, System};
        let pid = self.process.id();
        let mut system = System::new();
        let pids = [Pid::from_u32(pid)];
        system.refresh_processes(
            ProcessesToUpdate::Some(&pids),
            true,
        );

        if let Some(process) = system.process(Pid::from_u32(pid)) {
            self.memory_mb = process.memory() as f64 / 1024.0 / 1024.0;
        }
    }

    /// Check if process is still running
    pub fn is_alive(&mut self) -> bool {
        match self.process.try_wait() {
            Ok(None) => true,
            _ => false,
        }
    }

    /// Add line to stderr log
    pub fn log_stderr(&mut self, line: String) {
        self.stderr_log.push_back(line);
        // Keep only last 500 lines
        while self.stderr_log.len() > 500 {
            self.stderr_log.pop_front();
        }
    }
}

/// Server info for display/serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub language: String,
    pub project: String,
    pub pid: u32,
    pub memory_mb: f64,
    pub status: String,
    pub uptime_seconds: u64,
    pub idle_seconds: u64,
}

/// Server manager - manages pool of language servers
pub struct ServerManager {
    pub servers: HashMap<String, ServerState>,
    config: Config,
}

impl ServerManager {
    pub fn new(config: Config) -> Self {
        Self {
            servers: HashMap::new(),
            config,
        }
    }

    /// Warm a server for a language/project
    pub fn warm(&mut self, language: &str, project: &PathBuf) -> Result<bool> {
        let key = format!("{}:{}", language, project.display());

        // Already warm?
        if let Some(server) = self.servers.get(&key) {
            if server.status == ServerStatus::Warm {
                return Ok(true);
            }
        }

        // Check limits
        if self.servers.len() >= self.config.max_servers {
            self.evict_lru()?;
        }

        // Check memory
        let estimated = self
            .config
            .get_server(language)
            .map(|c| c.memory_estimate_mb)
            .unwrap_or(300);
        let current_mem = self.current_memory();
        if current_mem + estimated > self.config.memory_limit_mb {
            self.evict_for_memory(estimated)?;
        }

        // Get server config
        let server_config = self
            .config
            .get_server(language)
            .context(format!("No configuration for language: {}", language))?;

        // Start the server process
        let mut cmd = Command::new(&server_config.command[0]);
        if server_config.command.len() > 1 {
            cmd.args(&server_config.command[1..]);
        }

        let process = cmd
            .current_dir(project)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context(format!("Failed to start {} server", language))?;

        let now = Instant::now();
        let mut server = ServerState {
            language: language.to_string(),
            project: project.clone(),
            process,
            status: ServerStatus::Initializing,
            started_at: now,
            last_query: now,
            memory_mb: 0.0,
            request_id: AtomicI64::new(0),
            stderr_log: VecDeque::new(),
            health_failures: 0,
            restart_count: 0,
        };

        // Initialize LSP connection
        if self.initialize_server(&mut server).is_ok() {
            server.status = ServerStatus::Warm;
            self.servers.insert(key, server);
            Ok(true)
        } else {
            // Kill failed process
            let _ = server.process.kill();
            Ok(false)
        }
    }

    /// Initialize LSP connection with server
    fn initialize_server(&mut self, server: &mut ServerState) -> Result<()> {
        // Get request ID first (immutable borrow)
        let request_id = server.next_request_id();
        let project_display = server.project.display().to_string();

        // Build initialize request
        let init_params = InitializeParams {
            process_id: Some(std::process::id() as i64),
            root_uri: Some(format!("file://{}", project_display)),
            root_path: Some(project_display),
            capabilities: ClientCapabilities {
                text_document: Some(TextDocumentClientCapabilities {
                    hover: Some(HoverCapability {
                        content_format: Some(vec!["markdown".to_string(), "plaintext".to_string()]),
                    }),
                    completion: Some(CompletionCapability {
                        completion_item: Some(CompletionItemCapability {
                            snippet_support: Some(true),
                        }),
                    }),
                    definition: Some(serde_json::json!({})),
                    references: Some(serde_json::json!({})),
                }),
            },
        };

        let request = Request::new(
            request_id,
            "initialize",
            Some(serde_json::to_value(init_params)?),
        );

        // Send request
        let msg = LspMessage::new(serde_json::to_value(&request)?);
        {
            let stdin = server
                .process
                .stdin
                .as_mut()
                .context("No stdin for server")?;
            msg.write_to(stdin)?;
        }

        // Read response with timeout
        {
            let stdout = server
                .process
                .stdout
                .as_mut()
                .context("No stdout for server")?;
            let mut reader = BufReader::new(stdout);
            let response = LspMessage::read_from(&mut reader)?;

            if response.is_none() {
                bail!("No response from server");
            }
        }

        // Send initialized notification
        let notification = Notification::new("initialized", Some(serde_json::json!({})));
        let msg = LspMessage::new(serde_json::to_value(&notification)?);

        let stdin = server
            .process
            .stdin
            .as_mut()
            .context("No stdin for server")?;
        msg.write_to(stdin)?;

        Ok(())
    }

    /// Cool (stop) servers for a language
    pub fn cool(&mut self, language: &str, project: Option<&PathBuf>) -> Result<()> {
        let keys: Vec<String> = if let Some(proj) = project {
            let key = format!("{}:{}", language, proj.display());
            if self.servers.contains_key(&key) {
                vec![key]
            } else {
                vec![]
            }
        } else {
            self.servers
                .keys()
                .filter(|k| k.starts_with(&format!("{}:", language)))
                .cloned()
                .collect()
        };

        for key in keys {
            self.stop_server(&key)?;
        }
        Ok(())
    }

    /// Stop a specific server
    pub fn stop_server(&mut self, key: &str) -> Result<()> {
        if let Some(mut server) = self.servers.remove(key) {
            // Try graceful shutdown first
            if let Ok(()) = self.send_shutdown(&mut server) {
                // Wait briefly for exit
                std::thread::sleep(Duration::from_millis(500));
            }
            // Force kill if still running
            let _ = server.process.kill();
            let _ = server.process.wait();
        }
        Ok(())
    }

    /// Send shutdown request to server
    fn send_shutdown(&mut self, server: &mut ServerState) -> Result<()> {
        let request = Request::new(server.next_request_id(), "shutdown", None);
        let msg = LspMessage::new(serde_json::to_value(&request)?);

        if let Some(stdin) = server.process.stdin.as_mut() {
            msg.write_to(stdin)?;

            // Send exit notification
            let notification = Notification::new("exit", None);
            let msg = LspMessage::new(serde_json::to_value(&notification)?);
            msg.write_to(stdin)?;
        }
        Ok(())
    }

    /// Get server for language/project, warming if needed
    pub fn get_server(&mut self, language: &str, project: &PathBuf) -> Result<&mut ServerState> {
        let key = format!("{}:{}", language, project.display());

        if !self.servers.contains_key(&key) {
            self.warm(language, project)?;
        }

        let server = self.servers.get_mut(&key).context("Server not available")?;
        server.last_query = Instant::now();
        Ok(server)
    }

    /// List all running servers
    pub fn list_servers(&mut self) -> Vec<ServerInfo> {
        let now = Instant::now();
        self.servers
            .values_mut()
            .map(|s| {
                s.update_memory();
                ServerInfo {
                    language: s.language.clone(),
                    project: s.project.display().to_string(),
                    pid: s.process.id(),
                    memory_mb: s.memory_mb,
                    status: s.status.as_str().to_string(),
                    uptime_seconds: now.duration_since(s.started_at).as_secs(),
                    idle_seconds: now.duration_since(s.last_query).as_secs(),
                }
            })
            .collect()
    }

    /// Get current total memory usage
    fn current_memory(&mut self) -> u64 {
        let mut total = 0.0;
        for server in self.servers.values_mut() {
            server.update_memory();
            total += server.memory_mb;
        }
        total as u64
    }

    /// Evict least recently used server
    fn evict_lru(&mut self) -> Result<()> {
        let lru_key = self
            .servers
            .iter()
            .min_by_key(|(_, s)| s.last_query)
            .map(|(k, _)| k.clone());

        if let Some(key) = lru_key {
            self.stop_server(&key)?;
        }
        Ok(())
    }

    /// Evict servers until enough memory is available
    fn evict_for_memory(&mut self, needed: u64) -> Result<()> {
        while self.current_memory() + needed > self.config.memory_limit_mb {
            if self.servers.is_empty() {
                break;
            }
            self.evict_lru()?;
        }
        Ok(())
    }

    /// Stop all servers
    pub fn stop_all(&mut self) -> Result<()> {
        let keys: Vec<String> = self.servers.keys().cloned().collect();
        for key in keys {
            self.stop_server(&key)?;
        }
        Ok(())
    }

    /// Restart a specific server
    pub fn restart_server(&mut self, key: &str) -> Result<bool> {
        let (language, project, restart_count) = {
            let server = self.servers.get(key).context("Server not found")?;
            (
                server.language.clone(),
                server.project.clone(),
                server.restart_count,
            )
        };

        self.stop_server(key)?;

        let result = self.warm(&language, &project)?;
        if result {
            let new_key = format!("{}:{}", language, project.display());
            if let Some(server) = self.servers.get_mut(&new_key) {
                server.restart_count = restart_count + 1;
            }
        }
        Ok(result)
    }

    /// Get stderr logs for a server
    pub fn get_logs(&self, key: &str) -> Vec<String> {
        self.servers
            .get(key)
            .map(|s| s.stderr_log.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Send an LSP query to a server
    pub fn query(
        &mut self,
        language: &str,
        project: &PathBuf,
        method: &str,
        params: Value,
    ) -> Result<Value> {
        let server = self.get_server(language, project)?;

        let request = Request::new(server.next_request_id(), method, Some(params));
        let msg = LspMessage::new(serde_json::to_value(&request)?);

        let stdin = server
            .process
            .stdin
            .as_mut()
            .context("No stdin for server")?;
        msg.write_to(stdin)?;

        let stdout = server
            .process
            .stdout
            .as_mut()
            .context("No stdout for server")?;
        let mut reader = BufReader::new(stdout);

        let response =
            LspMessage::read_from(&mut reader)?.context("No response from server")?;

        if let Some(result) = response.content.get("result") {
            Ok(result.clone())
        } else if let Some(error) = response.content.get("error") {
            bail!("LSP error: {}", error)
        } else {
            Ok(Value::Null)
        }
    }

    /// Find project root from a file path
    pub fn find_project_root(file: &PathBuf) -> PathBuf {
        let markers = [
            "package.json",
            "Cargo.toml",
            "pyproject.toml",
            "go.mod",
            "Package.swift",
            "build.gradle",
            "pom.xml",
            ".git",
        ];

        let mut current = if file.is_file() {
            file.parent().unwrap_or(file).to_path_buf()
        } else {
            file.clone()
        };

        while current.parent().is_some() {
            for marker in &markers {
                if current.join(marker).exists() {
                    return current;
                }
            }
            if let Some(parent) = current.parent() {
                current = parent.to_path_buf();
            } else {
                break;
            }
        }

        file.parent().unwrap_or(file).to_path_buf()
    }

    /// Detect language from file path
    pub fn detect_language(file: &PathBuf) -> Option<String> {
        file.extension()
            .and_then(|e| e.to_str())
            .and_then(|e| extension_to_language(e))
            .map(|s| s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_project_root() {
        let file = PathBuf::from("/tmp/test.rs");
        let root = ServerManager::find_project_root(&file);
        // Should return parent since no markers exist
        assert_eq!(root, PathBuf::from("/tmp"));
    }

    #[test]
    fn test_detect_language() {
        assert_eq!(
            ServerManager::detect_language(&PathBuf::from("test.rs")),
            Some("rust".to_string())
        );
        assert_eq!(
            ServerManager::detect_language(&PathBuf::from("test.py")),
            Some("python".to_string())
        );
        assert_eq!(
            ServerManager::detect_language(&PathBuf::from("test.unknown")),
            None
        );
    }
}
