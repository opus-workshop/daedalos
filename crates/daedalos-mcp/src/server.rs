//! MCP Server
//!
//! Handles the MCP protocol over stdio, processing JSON-RPC 2.0 messages.

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use serde_json::{json, Value};
use tracing::{debug, error, info, warn};

use crate::protocol::{
    JsonRpcRequest, JsonRpcResponse,
    InitializeResult, ServerCapabilities, ServerInfo, ToolsCapability, ResourcesCapability,
    ListToolsResult, CallToolParams,
    ListResourcesResult, Resource, ReadResourceParams, ReadResourceResult, TextResourceContents,
    METHOD_NOT_FOUND, INVALID_PARAMS, INTERNAL_ERROR,
};
use crate::tools::all_tools;
use crate::handler::handle_tool;

/// MCP Server that communicates over stdio
pub struct McpServer {
    initialized: bool,
}

impl McpServer {
    pub fn new() -> Self {
        Self { initialized: false }
    }

    /// Run the server, reading from stdin and writing to stdout
    pub async fn run(&mut self) -> anyhow::Result<()> {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin);
        let mut stdout = stdout;

        let mut line = String::new();

        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line).await?;

            if bytes_read == 0 {
                // EOF - client disconnected
                info!("Client disconnected");
                break;
            }

            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            debug!("Received: {}", line);

            let response = self.handle_message(line).await;

            if let Some(resp) = response {
                let resp_str = serde_json::to_string(&resp)?;
                debug!("Sending: {}", resp_str);
                stdout.write_all(resp_str.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
            }
        }

        Ok(())
    }

    /// Handle a single JSON-RPC message
    async fn handle_message(&mut self, message: &str) -> Option<JsonRpcResponse> {
        let request: JsonRpcRequest = match serde_json::from_str(message) {
            Ok(req) => req,
            Err(e) => {
                error!("Failed to parse request: {}", e);
                return Some(JsonRpcResponse::error(
                    None,
                    -32700, // Parse error
                    format!("Parse error: {}", e),
                ));
            }
        };

        let id = request.id.clone();

        // Handle notifications (no id means no response expected)
        if id.is_none() {
            self.handle_notification(&request.method, request.params).await;
            return None;
        }

        // Handle request
        let result = self.handle_request(&request.method, request.params).await;

        match result {
            Ok(value) => Some(JsonRpcResponse::success(id, value)),
            Err((code, message)) => Some(JsonRpcResponse::error(id, code, message)),
        }
    }

    /// Handle a notification (no response expected)
    async fn handle_notification(&mut self, method: &str, _params: Option<Value>) {
        match method {
            "notifications/initialized" => {
                info!("Client initialized");
                self.initialized = true;
            }
            "notifications/cancelled" => {
                debug!("Request cancelled");
            }
            _ => {
                debug!("Unknown notification: {}", method);
            }
        }
    }

    /// Handle a request and return the result
    async fn handle_request(&mut self, method: &str, params: Option<Value>) -> Result<Value, (i32, String)> {
        match method {
            "initialize" => self.handle_initialize(params),
            "tools/list" => self.handle_list_tools(),
            "tools/call" => self.handle_call_tool(params).await,
            "resources/list" => self.handle_list_resources(),
            "resources/read" => self.handle_read_resource(params).await,
            "ping" => Ok(json!({})),
            _ => {
                warn!("Unknown method: {}", method);
                Err((METHOD_NOT_FOUND, format!("Method not found: {}", method)))
            }
        }
    }

    /// Handle the initialize request
    fn handle_initialize(&mut self, _params: Option<Value>) -> Result<Value, (i32, String)> {
        info!("Initializing MCP server");

        let result = InitializeResult {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {}),
                resources: Some(ResourcesCapability { subscribe: false }),
            },
            server_info: ServerInfo {
                name: "daedalos".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };

        serde_json::to_value(result)
            .map_err(|e| (INTERNAL_ERROR, format!("Serialization error: {}", e)))
    }

    /// Handle the tools/list request
    fn handle_list_tools(&self) -> Result<Value, (i32, String)> {
        let tools = all_tools();
        let result = ListToolsResult { tools };

        serde_json::to_value(result)
            .map_err(|e| (INTERNAL_ERROR, format!("Serialization error: {}", e)))
    }

    /// Handle the tools/call request
    async fn handle_call_tool(&self, params: Option<Value>) -> Result<Value, (i32, String)> {
        let params: CallToolParams = match params {
            Some(p) => serde_json::from_value(p)
                .map_err(|e| (INVALID_PARAMS, format!("Invalid params: {}", e)))?,
            None => return Err((INVALID_PARAMS, "Missing params".to_string())),
        };

        info!("Calling tool: {}", params.name);
        let result = handle_tool(&params.name, params.arguments).await;

        serde_json::to_value(result)
            .map_err(|e| (INTERNAL_ERROR, format!("Serialization error: {}", e)))
    }

    /// Handle the resources/list request
    fn handle_list_resources(&self) -> Result<Value, (i32, String)> {
        let resources = vec![
            Resource {
                uri: "daedalos://inbox".to_string(),
                name: "Agent Inbox".to_string(),
                description: Some("Unread messages for this agent. Check this resource to see messages from other agents.".to_string()),
                mime_type: Some("text/plain".to_string()),
            },
            Resource {
                uri: "daedalos://agents".to_string(),
                name: "Active Agents".to_string(),
                description: Some("List of all active agents that can send/receive messages.".to_string()),
                mime_type: Some("application/json".to_string()),
            },
        ];

        let result = ListResourcesResult { resources };

        serde_json::to_value(result)
            .map_err(|e| (INTERNAL_ERROR, format!("Serialization error: {}", e)))
    }

    /// Handle the resources/read request
    async fn handle_read_resource(&self, params: Option<Value>) -> Result<Value, (i32, String)> {
        let params: ReadResourceParams = match params {
            Some(p) => serde_json::from_value(p)
                .map_err(|e| (INVALID_PARAMS, format!("Invalid params: {}", e)))?,
            None => return Err((INVALID_PARAMS, "Missing params".to_string())),
        };

        let contents = match params.uri.as_str() {
            "daedalos://inbox" => {
                let output = tokio::process::Command::new("agent")
                    .args(["inbox", "--all"])
                    .output()
                    .await
                    .map_err(|e| (INTERNAL_ERROR, format!("Failed to run agent: {}", e)))?;

                let text = if output.status.success() {
                    String::from_utf8_lossy(&output.stdout).to_string()
                } else {
                    format!("Error: {}", String::from_utf8_lossy(&output.stderr))
                };

                TextResourceContents {
                    uri: params.uri,
                    mime_type: "text/plain".to_string(),
                    text: if text.trim().is_empty() { "No messages.".to_string() } else { text },
                }
            }
            "daedalos://agents" => {
                let output = tokio::process::Command::new("agent")
                    .args(["list", "--json"])
                    .output()
                    .await
                    .map_err(|e| (INTERNAL_ERROR, format!("Failed to run agent: {}", e)))?;

                let text = if output.status.success() {
                    String::from_utf8_lossy(&output.stdout).to_string()
                } else {
                    format!("Error: {}", String::from_utf8_lossy(&output.stderr))
                };

                TextResourceContents {
                    uri: params.uri,
                    mime_type: "application/json".to_string(),
                    text,
                }
            }
            _ => return Err((INVALID_PARAMS, format!("Unknown resource: {}", params.uri))),
        };

        let result = ReadResourceResult {
            contents: vec![contents],
        };

        serde_json::to_value(result)
            .map_err(|e| (INTERNAL_ERROR, format!("Serialization error: {}", e)))
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}
