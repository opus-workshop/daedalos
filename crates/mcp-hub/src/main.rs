//! mcp-hub - MCP server hub for managing, routing, and pooling MCP servers
//!
//! "Infrastructure should be invisible. MCP-Hub converts configuration burden
//! into instant capability."
//!
//! MCP-Hub provides a single point of management for all MCP servers. It handles
//! server lifecycle, routes requests to appropriate servers, and maintains a warm
//! pool for fast response times.

mod config;
mod daemon;
mod registry;
mod router;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::collections::HashMap;

use crate::config::get_socket_path;
use crate::daemon::DaemonClient;
use crate::registry::ServerRegistry;

#[derive(Parser)]
#[command(name = "mcp-hub")]
#[command(about = "MCP server hub - manages, routes, and pools MCP servers")]
#[command(version)]
#[command(after_help = "\
MCP-Hub collapses MCP server complexity into a single point of management.
One hub that knows all your servers, handles authentication once, and routes
requests transparently.

COMMANDS:
    status      Show hub daemon status and running servers
    list        List all registered MCP servers
    warm        Pre-start servers for fast response
    restart     Restart a running server
    logs        View server logs
    call        Call an MCP tool directly

EXAMPLES:
    mcp-hub status                   # Check hub status
    mcp-hub warm github postgres     # Pre-start servers
    mcp-hub call read_file --path /etc/hosts
    mcp-hub logs github -n 50        # View server logs")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Show hub daemon status and running servers
    Status {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// List available MCP servers in the registry
    List {
        /// Filter by category
        #[arg(short, long)]
        category: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Pre-start servers for fast response
    Warm {
        /// Server names to warm up
        servers: Vec<String>,
    },

    /// Restart a running MCP server
    Restart {
        /// Server name to restart
        server: String,
    },

    /// View server logs
    Logs {
        /// Server name
        server: String,

        /// Number of lines to show
        #[arg(short = 'n', long, default_value = "50")]
        lines: usize,
    },

    /// Call an MCP tool directly
    Call {
        /// Tool name to call
        tool: String,

        /// Specific server to use (optional)
        #[arg(short, long)]
        server: Option<String>,

        /// Arguments as JSON
        #[arg(long)]
        json_args: Option<String>,

        /// Tool arguments in --key value format
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Status { json }) => cmd_status(json),
        Some(Commands::List { category, json }) => cmd_list(category.as_deref(), json),
        Some(Commands::Warm { servers }) => cmd_warm(&servers),
        Some(Commands::Restart { server }) => cmd_restart(&server),
        Some(Commands::Logs { server, lines }) => cmd_logs(&server, lines),
        Some(Commands::Call { tool, server, json_args, args }) => {
            cmd_call(&tool, server.as_deref(), json_args.as_deref(), &args)
        }
        None => cmd_help(),
    }
}

/// Show help message
fn cmd_help() -> Result<()> {
    println!("mcp-hub - MCP server hub for managing, routing, and pooling MCP servers");
    println!();
    println!("Run 'mcp-hub --help' for usage information");
    Ok(())
}

/// Show hub daemon status
fn cmd_status(json: bool) -> Result<()> {
    let socket_path = get_socket_path();

    if !socket_path.exists() {
        if json {
            println!(r#"{{"running": false}}"#);
        } else {
            println!("MCP Hub Status");
            println!("==============");
            println!();
            println!("Daemon: not running");
            println!();
            println!("Start the daemon with: mcp-hub start");
        }
        return Ok(());
    }

    // Try to get status from daemon
    let rt = tokio::runtime::Runtime::new()?;
    let result = rt.block_on(async {
        let client = DaemonClient::new(socket_path);
        client.status().await
    });

    match result {
        Ok(status) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&status)?);
            } else {
                println!("MCP Hub Status");
                println!("==============");
                println!();
                println!("Daemon: running");
                println!();

                let servers = status.get("servers").and_then(|s| s.as_array());
                if let Some(servers) = servers {
                    if servers.is_empty() {
                        println!("No servers running.");
                    } else {
                        println!("Running servers:");
                        for server in servers {
                            let name = server.get("name").and_then(|n| n.as_str()).unwrap_or("?");
                            let status = server.get("status").and_then(|s| s.as_str()).unwrap_or("?");
                            let tools = server.get("tools").and_then(|t| t.as_u64()).unwrap_or(0);
                            let health_failures = server.get("health_failures").and_then(|h| h.as_u64()).unwrap_or(0);

                            let health_info = if health_failures > 0 {
                                format!(" (health: {} failures)", health_failures)
                            } else {
                                String::new()
                            };

                            println!("  {:15} {:10} tools:{}{}", name, status, tools, health_info);
                        }
                    }
                }

                let tools = status.get("tools").and_then(|t| t.as_array());
                if let Some(tools) = tools {
                    if !tools.is_empty() {
                        println!();
                        println!("Available tools: {}", tools.len());
                    }
                }
            }
        }
        Err(e) => {
            if json {
                println!(r#"{{"running": false, "error": "{}"}}"#, e);
            } else {
                println!("Could not connect to daemon: {}", e);
            }
        }
    }

    Ok(())
}

/// List available MCP servers
fn cmd_list(category: Option<&str>, json: bool) -> Result<()> {
    let registry = ServerRegistry::new()?;
    let servers = registry.list(category);

    if json {
        let server_data: Vec<serde_json::Value> = servers
            .iter()
            .map(|s| s.to_json())
            .collect();
        println!("{}", serde_json::to_string_pretty(&server_data)?);
        return Ok(());
    }

    if servers.is_empty() {
        if let Some(cat) = category {
            println!("No servers found in category: {}", cat);
        } else {
            println!("No servers registered.");
        }
        return Ok(());
    }

    // Group by category for nicer display
    let mut by_category: HashMap<String, Vec<&registry::ServerInfo>> = HashMap::new();
    for server in &servers {
        by_category
            .entry(server.category.to_string())
            .or_default()
            .push(server);
    }

    let mut categories: Vec<_> = by_category.keys().collect();
    categories.sort();

    for cat in categories {
        if let Some(servers) = by_category.get(cat.as_str()) {
            println!("\n{}", cat.to_uppercase());
            println!("{}", "-".repeat(cat.len()));
            for server in servers {
                let status = if server.enabled { "[ON]" } else { "[--]" };
                let auth = if server.requires_auth { "*" } else { " " };
                println!("{} {:15}{} {}", status, server.name, auth, server.description);
            }
        }
    }

    println!();
    println!("* = requires authentication");

    Ok(())
}

/// Pre-start servers for fast response
fn cmd_warm(servers: &[String]) -> Result<()> {
    if servers.is_empty() {
        println!("Usage: mcp-hub warm <server1> [server2] ...");
        println!();
        println!("Pre-starts specified servers for fast response times.");
        println!("Use 'mcp-hub list' to see available servers.");
        return Ok(());
    }

    let socket_path = get_socket_path();

    if !socket_path.exists() {
        println!("Daemon is not running. Start it first with: mcp-hub start");
        std::process::exit(6);
    }

    let rt = tokio::runtime::Runtime::new()?;
    let result = rt.block_on(async {
        let client = DaemonClient::new(socket_path);
        client.warm(servers).await
    });

    match result {
        Ok(results) => {
            for (name, success) in results {
                let status = if success { "started" } else { "failed" };
                println!("  {}: {}", name, status);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

/// Restart a running server
fn cmd_restart(server: &str) -> Result<()> {
    let socket_path = get_socket_path();

    if !socket_path.exists() {
        println!("Daemon is not running.");
        std::process::exit(6);
    }

    let rt = tokio::runtime::Runtime::new()?;
    let result = rt.block_on(async {
        let client = DaemonClient::new(socket_path);
        client.restart(server).await
    });

    match result {
        Ok(success) => {
            if success {
                println!("Restarted: {}", server);
            } else {
                eprintln!("Failed to restart: {}", server);
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

/// View server logs
fn cmd_logs(server: &str, lines: usize) -> Result<()> {
    let socket_path = get_socket_path();

    if !socket_path.exists() {
        println!("Daemon is not running.");
        std::process::exit(6);
    }

    let rt = tokio::runtime::Runtime::new()?;
    let result = rt.block_on(async {
        let client = DaemonClient::new(socket_path);
        client.logs(server, lines).await
    });

    match result {
        Ok(logs) => {
            if logs.is_empty() {
                println!("No logs for {}", server);
            } else {
                for line in logs {
                    println!("{}", line);
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

/// Call an MCP tool
fn cmd_call(
    tool: &str,
    server: Option<&str>,
    json_args: Option<&str>,
    args: &[String],
) -> Result<()> {
    let socket_path = get_socket_path();

    if !socket_path.exists() {
        println!("Daemon is not running. Start it first with: mcp-hub start");
        std::process::exit(6);
    }

    // Parse arguments
    let arguments: HashMap<String, serde_json::Value> = if let Some(json_str) = json_args {
        serde_json::from_str(json_str).context("Failed to parse --json-args")?
    } else {
        parse_arguments(args)?
    };

    let rt = tokio::runtime::Runtime::new()?;
    let result = rt.block_on(async {
        let client = DaemonClient::new(socket_path);
        client.call_tool(tool, arguments, server).await
    });

    match result {
        Ok(response) => {
            if let Some(error) = response.get("error") {
                eprintln!("Error: {}", error);
                std::process::exit(1);
            }

            if let Some(result) = response.get("result") {
                // Handle MCP content array format
                if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
                    for item in content {
                        if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                            println!("{}", text);
                        } else {
                            println!("{}", item);
                        }
                    }
                } else {
                    println!("{}", serde_json::to_string_pretty(result)?);
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

/// Parse CLI arguments into a HashMap
fn parse_arguments(args: &[String]) -> Result<HashMap<String, serde_json::Value>> {
    let mut result = HashMap::new();
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];
        if arg.starts_with("--") {
            let key = arg.trim_start_matches("--");

            // Handle --key=value format
            if let Some((k, v)) = key.split_once('=') {
                let value = parse_value(v);
                result.insert(k.to_string(), value);
            } else if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                // Handle --key value format
                i += 1;
                let value = parse_value(&args[i]);
                result.insert(key.to_string(), value);
            } else {
                // Boolean flag
                result.insert(key.to_string(), serde_json::Value::Bool(true));
            }
        }
        i += 1;
    }

    Ok(result)
}

/// Parse a string value, trying JSON first
fn parse_value(s: &str) -> serde_json::Value {
    // Try parsing as JSON
    if let Ok(v) = serde_json::from_str(s) {
        return v;
    }

    // Otherwise return as string
    serde_json::Value::String(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parse() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }

    #[test]
    fn test_parse_arguments() {
        let args = vec![
            "--path".to_string(),
            "/etc/hosts".to_string(),
            "--recursive".to_string(),
        ];
        let result = parse_arguments(&args).unwrap();

        assert_eq!(
            result.get("path"),
            Some(&serde_json::Value::String("/etc/hosts".to_string()))
        );
        assert_eq!(
            result.get("recursive"),
            Some(&serde_json::Value::Bool(true))
        );
    }

    #[test]
    fn test_parse_value() {
        assert_eq!(parse_value("42"), serde_json::json!(42));
        assert_eq!(parse_value("true"), serde_json::json!(true));
        assert_eq!(parse_value("hello"), serde_json::json!("hello"));
        assert_eq!(
            parse_value(r#"{"key": "value"}"#),
            serde_json::json!({"key": "value"})
        );
    }
}
