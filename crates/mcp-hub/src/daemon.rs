//! Daemon client for communicating with the MCP Hub daemon

#![allow(dead_code)]

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

/// Client for communicating with the MCP Hub daemon
pub struct DaemonClient {
    socket_path: PathBuf,
}

impl DaemonClient {
    /// Create a new daemon client
    pub fn new(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }

    /// Send a request to the daemon and get a response
    async fn send_request(&self, request: serde_json::Value) -> Result<serde_json::Value> {
        let mut stream = UnixStream::connect(&self.socket_path)
            .await
            .context("Failed to connect to daemon")?;

        // Send request
        let request_bytes = serde_json::to_vec(&request)?;
        stream.write_all(&request_bytes).await?;
        stream.shutdown().await?;

        // Read response
        let mut response_bytes = Vec::new();
        stream.read_to_end(&mut response_bytes).await?;

        let response: serde_json::Value =
            serde_json::from_slice(&response_bytes).context("Failed to parse daemon response")?;

        Ok(response)
    }

    /// Get hub status
    pub async fn status(&self) -> Result<serde_json::Value> {
        let request = serde_json::json!({
            "type": "status"
        });
        self.send_request(request).await
    }

    /// Warm up servers
    pub async fn warm(&self, servers: &[String]) -> Result<HashMap<String, bool>> {
        let request = serde_json::json!({
            "type": "warm",
            "servers": servers
        });

        let response = self.send_request(request).await?;

        // Parse results
        let mut results = HashMap::new();
        if let Some(res) = response.get("results").and_then(|r| r.as_object()) {
            for (name, success) in res {
                results.insert(name.clone(), success.as_bool().unwrap_or(false));
            }
        }

        Ok(results)
    }

    /// Restart a server
    pub async fn restart(&self, server: &str) -> Result<bool> {
        let request = serde_json::json!({
            "type": "restart_server",
            "server": server
        });

        let response = self.send_request(request).await?;
        Ok(response.get("success").and_then(|s| s.as_bool()).unwrap_or(false))
    }

    /// Get server logs
    pub async fn logs(&self, server: &str, lines: usize) -> Result<Vec<String>> {
        let request = serde_json::json!({
            "type": "logs",
            "server": server,
            "lines": lines
        });

        let response = self.send_request(request).await?;

        let logs = response
            .get("logs")
            .and_then(|l| l.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        Ok(logs)
    }

    /// Call an MCP tool
    pub async fn call_tool(
        &self,
        tool: &str,
        arguments: HashMap<String, serde_json::Value>,
        server: Option<&str>,
    ) -> Result<serde_json::Value> {
        let mut request = serde_json::json!({
            "type": "call_tool",
            "tool": tool,
            "arguments": arguments
        });

        if let Some(server_name) = server {
            request["server"] = serde_json::Value::String(server_name.to_string());
        }

        self.send_request(request).await
    }

    /// Start a server
    pub async fn start_server(&self, server: &str) -> Result<bool> {
        let request = serde_json::json!({
            "type": "start_server",
            "server": server
        });

        let response = self.send_request(request).await?;
        Ok(response.get("success").and_then(|s| s.as_bool()).unwrap_or(false))
    }

    /// Stop a server
    pub async fn stop_server(&self, server: &str) -> Result<bool> {
        let request = serde_json::json!({
            "type": "stop_server",
            "server": server
        });

        let response = self.send_request(request).await?;
        Ok(response.get("success").and_then(|s| s.as_bool()).unwrap_or(false))
    }

    /// List available tools
    pub async fn list_tools(&self) -> Result<Vec<serde_json::Value>> {
        let request = serde_json::json!({
            "type": "list_tools"
        });

        let response = self.send_request(request).await?;

        let tools = response
            .get("tools")
            .and_then(|t| t.as_array())
            .cloned()
            .unwrap_or_default();

        Ok(tools)
    }

    /// Reload configuration
    pub async fn reload(&self) -> Result<bool> {
        let request = serde_json::json!({
            "type": "reload"
        });

        let response = self.send_request(request).await?;
        Ok(response.get("success").and_then(|s| s.as_bool()).unwrap_or(false))
    }

    /// Stop the daemon
    pub async fn stop(&self) -> Result<()> {
        let request = serde_json::json!({
            "type": "stop"
        });

        // Send but don't wait for response (daemon is stopping)
        if let Ok(mut stream) = UnixStream::connect(&self.socket_path).await {
            let request_bytes = serde_json::to_vec(&request)?;
            let _ = stream.write_all(&request_bytes).await;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = DaemonClient::new(PathBuf::from("/tmp/test.sock"));
        assert_eq!(client.socket_path, PathBuf::from("/tmp/test.sock"));
    }
}
