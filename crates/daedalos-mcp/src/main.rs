//! Daedalos MCP Server
//!
//! Exposes all Daedalos tools to Claude and other AI assistants via the
//! Model Context Protocol (MCP).
//!
//! Usage:
//!   daedalos-mcp
//!
//! The server communicates over stdio using JSON-RPC 2.0.

use anyhow::Result;
use tracing_subscriber::EnvFilter;
use daedalos_mcp::McpServer;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging to stderr (stdout is for MCP protocol)
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    tracing::info!("Starting Daedalos MCP server");

    let mut server = McpServer::new();
    server.run().await?;

    Ok(())
}
