//! lsp-pool - Pre-warmed Language Server Pool
//!
//! "Intelligence should be instant" - provide instant code intelligence by
//! maintaining a pool of pre-warmed language servers.
//!
//! Language servers are notoriously slow to start. TypeScript: 10-30 seconds.
//! Rust: 30-60 seconds. This pool pre-warms what you'll likely need, keeps
//! active servers hot, and provides sub-second query latency.

mod config;
mod protocol;
mod server;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::config::Config;
use crate::protocol::*;
use crate::server::ServerManager;

#[derive(Parser)]
#[command(name = "lsp-pool")]
#[command(about = "Pre-warmed language server pool for instant code intelligence")]
#[command(version)]
#[command(after_help = "\
LSP-Pool manages language servers as a resource pool. Pre-warm servers for
instant code intelligence queries.

SUPPORTED LANGUAGES:
    typescript, python, rust, go, c, cpp, java, kotlin, swift, lua, ruby,
    elixir, zig, ocaml, haskell, bash, yaml, json, html, css, nix

QUERY COMMANDS:
    hover        Get type/documentation at position
    definition   Go to definition
    references   Find all references
    completion   Get completions at position
    diagnostics  Get file diagnostics

EXAMPLES:
    # Warm a server for a TypeScript project
    lsp-pool warm typescript ~/myproject

    # Query hover info
    lsp-pool query hover src/main.ts --line 42 --col 15

    # List running servers
    lsp-pool list

    # Cool (stop) all Python servers
    lsp-pool cool python")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Show pool status
    Status {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Warm a server for a language/project
    Warm {
        /// Language (typescript, python, rust, etc.)
        language: String,

        /// Project path (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Wait for server to be ready
        #[arg(long)]
        wait: bool,
    },

    /// Stop servers for a language
    Cool {
        /// Language to stop
        language: String,

        /// Specific project path (optional)
        #[arg(short, long)]
        path: Option<PathBuf>,
    },

    /// List running servers
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Send LSP query
    Query {
        /// Query command (hover, definition, references, completion, diagnostics)
        command: String,

        /// File to query
        file: PathBuf,

        /// Line number (1-indexed)
        #[arg(short, long, default_value = "1")]
        line: u32,

        /// Column number (1-indexed)
        #[arg(short, long, default_value = "1")]
        col: u32,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// List supported languages
    Languages,

    /// Get server logs
    Logs {
        /// Server key (language:project)
        key: String,

        /// Number of lines to show
        #[arg(short = 'n', long, default_value = "50")]
        lines: usize,
    },

    /// Restart a server
    Restart {
        /// Server key (language:project)
        key: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Status { json }) => cmd_status(json),
        Some(Commands::Warm {
            language,
            path,
            wait: _,
        }) => cmd_warm(&language, &path),
        Some(Commands::Cool { language, path }) => cmd_cool(&language, path.as_ref()),
        Some(Commands::List { json }) => cmd_list(json),
        Some(Commands::Query {
            command,
            file,
            line,
            col,
            json,
        }) => cmd_query(&command, &file, line, col, json),
        Some(Commands::Languages) => cmd_languages(),
        Some(Commands::Logs { key, lines }) => cmd_logs(&key, lines),
        Some(Commands::Restart { key }) => cmd_restart(&key),
        None => cmd_help(),
    }
}

fn cmd_help() -> Result<()> {
    println!("lsp-pool - Pre-warmed language server pool");
    println!();
    println!("Run 'lsp-pool --help' for usage information");
    Ok(())
}

fn cmd_status(json: bool) -> Result<()> {
    let config = Config::load()?;
    let mut manager = ServerManager::new(config.clone());
    let servers = manager.list_servers();

    if json {
        let status = serde_json::json!({
            "running": true,
            "servers": servers,
            "config": {
                "max_servers": config.max_servers,
                "memory_limit_mb": config.memory_limit_mb,
            }
        });
        println!("{}", serde_json::to_string_pretty(&status)?);
        return Ok(());
    }

    println!("LSP Pool Status");
    println!("{}", "=".repeat(50));
    println!("Max servers:   {}", config.max_servers);
    println!("Memory limit:  {} MB", config.memory_limit_mb);
    println!();

    if servers.is_empty() {
        println!("No servers running.");
    } else {
        println!("Running servers:");
        for s in &servers {
            let status_color = match s.status.as_str() {
                "warm" => "\x1b[32m",
                "initializing" => "\x1b[33m",
                "error" | "unhealthy" => "\x1b[31m",
                _ => "\x1b[0m",
            };
            println!(
                "  {:12} {}{:12}\x1b[0m {:>6.1} MB  {}",
                s.language, status_color, s.status, s.memory_mb, s.project
            );
        }
    }

    Ok(())
}

fn cmd_warm(language: &str, path: &PathBuf) -> Result<()> {
    let config = Config::load()?;

    // Validate language
    if config.get_server(language).is_none() {
        anyhow::bail!(
            "Unknown language: {}. Run 'lsp-pool languages' for supported languages.",
            language
        );
    }

    let project = std::fs::canonicalize(path).unwrap_or_else(|_| path.clone());

    let mut manager = ServerManager::new(config);
    let success = manager.warm(language, &project)?;

    if success {
        println!("Server warm: {}:{}", language, project.display());
    } else {
        println!("Failed to warm: {}", language);
        std::process::exit(1);
    }

    Ok(())
}

fn cmd_cool(language: &str, path: Option<&PathBuf>) -> Result<()> {
    let config = Config::load()?;
    let mut manager = ServerManager::new(config);

    let project = path.map(|p| std::fs::canonicalize(p).unwrap_or_else(|_| p.clone()));

    manager.cool(language, project.as_ref())?;
    println!("Servers cooled: {}", language);

    Ok(())
}

fn cmd_list(json: bool) -> Result<()> {
    let config = Config::load()?;
    let mut manager = ServerManager::new(config);
    let servers = manager.list_servers();

    if json {
        println!("{}", serde_json::to_string_pretty(&servers)?);
        return Ok(());
    }

    if servers.is_empty() {
        println!("No servers running.");
    } else {
        println!(
            "{:12} {:12} {:>8} {:>12} {:>10}  {}",
            "LANGUAGE", "STATUS", "PID", "MEMORY", "IDLE", "PROJECT"
        );
        for s in &servers {
            println!(
                "{:12} {:12} {:>8} {:>9.1} MB {:>8}s  {}",
                s.language, s.status, s.pid, s.memory_mb, s.idle_seconds, s.project
            );
        }
    }

    Ok(())
}

fn cmd_query(command: &str, file: &PathBuf, line: u32, col: u32, json_output: bool) -> Result<()> {
    let config = Config::load()?;
    let mut manager = ServerManager::new(config);

    let file = std::fs::canonicalize(file).context("File not found")?;

    // Detect language
    let language = ServerManager::detect_language(&file)
        .context("Could not detect language from file extension")?;

    // Find project root
    let project = ServerManager::find_project_root(&file);

    // Build query params
    let params = serde_json::json!({
        "textDocument": TextDocumentIdentifier::new(&file),
        "position": Position::from_1indexed(line, col),
    });

    let method = match command {
        "hover" => "textDocument/hover",
        "definition" => "textDocument/definition",
        "references" => "textDocument/references",
        "completion" => "textDocument/completion",
        "diagnostics" => {
            // Diagnostics are handled differently (need to open document first)
            return cmd_diagnostics(&mut manager, &language, &project, &file, json_output);
        }
        _ => anyhow::bail!("Unknown command: {}. Use: hover, definition, references, completion, diagnostics", command),
    };

    // Add context for references
    let params = if command == "references" {
        serde_json::json!({
            "textDocument": TextDocumentIdentifier::new(&file),
            "position": Position::from_1indexed(line, col),
            "context": { "includeDeclaration": true }
        })
    } else {
        params
    };

    let result = manager.query(&language, &project, method, params)?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    // Format output based on command
    match command {
        "hover" => format_hover(&result),
        "definition" => format_definition(&result),
        "references" => format_references(&result),
        "completion" => format_completion(&result),
        _ => Ok(()),
    }
}

fn cmd_diagnostics(
    manager: &mut ServerManager,
    language: &str,
    project: &PathBuf,
    file: &PathBuf,
    json_output: bool,
) -> Result<()> {
    // For diagnostics, we need to open the document first
    let content = std::fs::read_to_string(file).unwrap_or_default();

    // Send didOpen notification
    let _open_params = serde_json::json!({
        "textDocument": {
            "uri": format!("file://{}", file.display()),
            "languageId": language,
            "version": 1,
            "text": content
        }
    });

    // Get server (this may warm it)
    let _ = manager.get_server(language, project)?;

    // Note: Full diagnostic support would require handling publishDiagnostics notifications
    // For now, return empty diagnostics
    if json_output {
        println!("{}", serde_json::json!({ "diagnostics": [] }));
    } else {
        println!("No diagnostics available (async diagnostics not yet supported)");
    }

    Ok(())
}

fn format_hover(result: &serde_json::Value) -> Result<()> {
    if result.is_null() {
        println!("No hover information available.");
        return Ok(());
    }

    if let Some(contents) = result.get("contents") {
        // Could be string, object, or array
        let text = if let Some(s) = contents.as_str() {
            s.to_string()
        } else if let Some(obj) = contents.as_object() {
            obj.get("value")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        } else if let Some(arr) = contents.as_array() {
            arr.iter()
                .filter_map(|v| {
                    if let Some(s) = v.as_str() {
                        Some(s.to_string())
                    } else if let Some(obj) = v.as_object() {
                        obj.get("value").and_then(|v| v.as_str()).map(String::from)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            contents.to_string()
        };
        println!("{}", text);
    } else {
        println!("{}", result);
    }

    Ok(())
}

fn format_definition(result: &serde_json::Value) -> Result<()> {
    if result.is_null() {
        println!("Definition not found.");
        return Ok(());
    }

    if let Some(arr) = result.as_array() {
        for loc in arr {
            if let Some(formatted) = format_location(loc) {
                println!("{}", formatted);
            }
        }
    } else if let Some(formatted) = format_location(result) {
        println!("{}", formatted);
    }

    Ok(())
}

fn format_references(result: &serde_json::Value) -> Result<()> {
    if result.is_null() || (result.is_array() && result.as_array().unwrap().is_empty()) {
        println!("No references found.");
        return Ok(());
    }

    if let Some(arr) = result.as_array() {
        for loc in arr {
            if let Some(formatted) = format_location(loc) {
                println!("{}", formatted);
            }
        }
    }

    Ok(())
}

fn format_completion(result: &serde_json::Value) -> Result<()> {
    let items = if let Some(arr) = result.as_array() {
        arr.clone()
    } else if let Some(obj) = result.as_object() {
        obj.get("items")
            .and_then(|i| i.as_array())
            .cloned()
            .unwrap_or_default()
    } else {
        vec![]
    };

    if items.is_empty() {
        println!("No completions.");
        return Ok(());
    }

    for item in items.iter().take(20) {
        let label = item.get("label").and_then(|l| l.as_str()).unwrap_or("");
        let kind = item.get("kind").and_then(|k| k.as_u64()).unwrap_or(0);
        let detail = item.get("detail").and_then(|d| d.as_str()).unwrap_or("");

        let kind_name = match kind {
            1 => "Text",
            2 => "Method",
            3 => "Function",
            4 => "Constructor",
            5 => "Field",
            6 => "Variable",
            7 => "Class",
            8 => "Interface",
            9 => "Module",
            10 => "Property",
            13 => "Enum",
            14 => "Keyword",
            15 => "Snippet",
            21 => "Constant",
            22 => "Struct",
            _ => "",
        };

        if !kind_name.is_empty() {
            print!("  {} ({})", label, kind_name);
        } else {
            print!("  {}", label);
        }

        if !detail.is_empty() {
            print!(" - {}", detail);
        }

        println!();
    }

    if items.len() > 20 {
        println!("  ... and {} more", items.len() - 20);
    }

    Ok(())
}

fn format_location(loc: &serde_json::Value) -> Option<String> {
    let uri = loc.get("uri")?.as_str()?;
    let path = uri.strip_prefix("file://").unwrap_or(uri);

    let range = loc.get("range")?;
    let start = range.get("start")?;
    let line = start.get("line")?.as_u64()? + 1;
    let col = start.get("character")?.as_u64()? + 1;

    Some(format!("{}:{}:{}", path, line, col))
}

fn cmd_languages() -> Result<()> {
    let config = Config::default();

    println!("Supported languages:");
    println!();

    let mut languages: Vec<_> = config.servers.keys().collect();
    languages.sort();

    for lang in languages {
        if let Some(server_config) = config.get_server(lang) {
            let cmd = server_config.command.join(" ");
            let exts = server_config.extensions.join(", ");
            println!(
                "  {:12} {}  ({})",
                lang,
                cmd.chars().take(40).collect::<String>(),
                exts
            );
        }
    }

    Ok(())
}

fn cmd_logs(key: &str, lines: usize) -> Result<()> {
    let config = Config::load()?;
    let manager = ServerManager::new(config);

    let logs = manager.get_logs(key);

    if logs.is_empty() {
        println!("No logs available for: {}", key);
        println!("(Server may not be running or key format is: language:project_path)");
    } else {
        for line in logs.iter().rev().take(lines).rev() {
            println!("{}", line);
        }
    }

    Ok(())
}

fn cmd_restart(key: &str) -> Result<()> {
    let config = Config::load()?;
    let mut manager = ServerManager::new(config);

    let success = manager.restart_server(key)?;

    if success {
        println!("Server restarted: {}", key);
    } else {
        println!("Failed to restart: {}", key);
        std::process::exit(1);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_cli_parse() {
        Cli::command().debug_assert();
    }
}
