//! Daedalos MCP Server
//!
//! Exposes all Daedalos tools to Claude and other AI assistants via the
//! Model Context Protocol (MCP). Implements MCP over stdio using JSON-RPC 2.0.

pub mod protocol;
pub mod tools;
pub mod handler;
pub mod server;

pub use server::McpServer;
