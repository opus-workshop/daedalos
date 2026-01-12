//! handoff - Context summaries for shift changes
//!
//! Context is the most expensive thing in development.
//!
//! When you stop working and someone else starts, there's a brutal context
//! switch. The new person has to reconstruct: What were you doing? Why?
//! What's the current state? What did you try that didn't work?
//!
//! Commands:
//! - create: Create a handoff summary
//! - receive: View a handoff summary
//! - list: List available handoffs
//! - status: Quick status for handoff

mod generator;
mod storage;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::io::{self, Write};
use std::process::Command;

use crate::generator::HandoffGenerator;
use crate::storage::HandoffStorage;

#[derive(Parser)]
#[command(name = "handoff")]
#[command(about = "Context summaries for shift changes between humans and AI agents")]
#[command(version)]
#[command(after_help = r#"WHAT'S CAPTURED:
    - Current task/goal
    - Recent changes (from journal/git)
    - Blockers and issues
    - Next steps
    - Open questions
    - Important context

USE CASES:
    - End of work day -> start of next day
    - Human -> AI agent handoff
    - AI agent -> human review
    - Between team members

INTEGRATION:
    Aggregates from:
    - journal: Recent events
    - git: Recent commits and changes
    - loop: Active iteration loops
    - agent: Agent activity

EXAMPLES:
    handoff create                  # Create summary
    handoff create "end-of-day"     # Named handoff
    handoff receive "end-of-day"    # View handoff
    handoff status                  # Quick current status
"#)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a handoff summary from recent activity (git, journal, loops)
    #[command(visible_aliases = ["new", "make"])]
    Create {
        /// Name for the handoff (timestamp if not provided)
        name: Option<String>,

        /// Who you're handing off to (human name or "ai")
        #[arg(long)]
        to: Option<String>,

        /// Hours of activity to summarize
        #[arg(long, default_value = "8")]
        hours: u64,

        /// Include more details in the summary
        #[arg(short, long)]
        verbose: bool,

        /// Don't open editor after creation (non-interactive)
        #[arg(long)]
        no_edit: bool,
    },

    /// View a handoff summary (renders markdown with glow/bat if available)
    #[command(visible_aliases = ["view", "show", "get"])]
    Receive {
        /// Name of the handoff to view (most recent if not provided)
        name: Option<String>,

        /// Output as JSON for scripting
        #[arg(long)]
        json: bool,
    },

    /// List all saved handoff summaries
    #[command(visible_alias = "ls")]
    List {
        /// Output as JSON for scripting
        #[arg(long)]
        json: bool,
    },

    /// Quick status of recent activity (preview before creating handoff)
    #[command(visible_alias = "s")]
    Status {
        /// Hours of activity to check
        #[arg(long, default_value = "4")]
        hours: u64,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Create {
            name,
            to,
            hours,
            verbose: _,
            no_edit,
        }) => cmd_create(name.as_deref(), to.as_deref(), hours, no_edit),

        Some(Commands::Receive { name, json }) => cmd_receive(name.as_deref(), json),

        Some(Commands::List { json }) => cmd_list(json),

        Some(Commands::Status { hours }) => cmd_status(hours),

        None => {
            // Default: show help
            Cli::parse_from(["handoff", "--help"]);
            Ok(())
        }
    }
}

/// Create a handoff
fn cmd_create(name: Option<&str>, to: Option<&str>, hours: u64, no_edit: bool) -> Result<()> {
    let storage = HandoffStorage::new()?;
    let generator = HandoffGenerator::new();

    let handoff = generator.generate(name, to, hours)?;
    let path = storage.save(&handoff)?;

    println!("Handoff created: {}", handoff.name);
    println!();
    println!("Edit to add your notes:");
    println!("  {} {}", std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string()), path.display());
    println!();
    println!("Or view raw summary:");
    println!("  handoff receive {}", handoff.name);

    // Offer to open editor if not --no-edit and we're in a terminal
    if !no_edit && atty::is(atty::Stream::Stdout) {
        print!("\nOpen for editing? [Y/n] ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let input = input.trim().to_lowercase();
        if input.is_empty() || input == "y" || input == "yes" {
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
            Command::new(&editor)
                .arg(&path)
                .status()
                .ok();
        }
    }

    Ok(())
}

/// Receive (view) a handoff
fn cmd_receive(name: Option<&str>, json: bool) -> Result<()> {
    let storage = HandoffStorage::new()?;
    let handoff = storage.get(name)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&handoff)?);
    } else {
        // Try to use a pager or formatter if available
        let content = &handoff.content;

        // Check for glow (markdown renderer)
        if Command::new("glow").arg("--version").output().is_ok() {
            let mut child = Command::new("glow")
                .arg("-")
                .stdin(std::process::Stdio::piped())
                .spawn()?;

            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(content.as_bytes())?;
            }
            child.wait()?;
        }
        // Check for bat (syntax highlighter)
        else if Command::new("bat").arg("--version").output().is_ok() {
            let mut child = Command::new("bat")
                .args(["--style=plain", "--language=markdown"])
                .stdin(std::process::Stdio::piped())
                .spawn()?;

            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(content.as_bytes())?;
            }
            child.wait()?;
        } else {
            // Plain output
            println!("{}", content);
        }
    }

    Ok(())
}

/// List available handoffs
fn cmd_list(json: bool) -> Result<()> {
    let storage = HandoffStorage::new()?;
    let handoffs = storage.list()?;

    if handoffs.is_empty() {
        println!("No handoffs available");
        return Ok(());
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&handoffs)?);
    } else {
        println!("Available Handoffs\n");

        for h in handoffs {
            println!("  {}", h.name);
            println!("    Date: {}", h.created.format("%Y-%m-%d %H:%M"));
            println!("    From: {}", h.from);
            if let Some(to) = h.to {
                println!("    To: {}", to);
            }
            println!();
        }
    }

    Ok(())
}

/// Quick status
fn cmd_status(hours: u64) -> Result<()> {
    let generator = HandoffGenerator::new();
    let status = generator.quick_status(hours)?;
    print!("{}", status);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        // Just verify the CLI parses without panic
        let cli = Cli::try_parse_from(["handoff", "--help"]);
        // --help returns an error (it exits), so we just check it doesn't panic
        assert!(cli.is_err());
    }

    #[test]
    fn test_create_command_parsing() {
        let cli = Cli::try_parse_from(["handoff", "create", "test", "--hours", "4"]).unwrap();
        match cli.command {
            Some(Commands::Create { name, hours, .. }) => {
                assert_eq!(name, Some("test".to_string()));
                assert_eq!(hours, 4);
            }
            _ => panic!("Expected Create command"),
        }
    }

    #[test]
    fn test_receive_command_parsing() {
        let cli = Cli::try_parse_from(["handoff", "receive", "test-handoff"]).unwrap();
        match cli.command {
            Some(Commands::Receive { name, .. }) => {
                assert_eq!(name, Some("test-handoff".to_string()));
            }
            _ => panic!("Expected Receive command"),
        }
    }

    #[test]
    fn test_list_command_parsing() {
        let cli = Cli::try_parse_from(["handoff", "list", "--json"]).unwrap();
        match cli.command {
            Some(Commands::List { json }) => {
                assert!(json);
            }
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn test_status_command_parsing() {
        let cli = Cli::try_parse_from(["handoff", "status", "--hours", "2"]).unwrap();
        match cli.command {
            Some(Commands::Status { hours }) => {
                assert_eq!(hours, 2);
            }
            _ => panic!("Expected Status command"),
        }
    }

    #[test]
    fn test_command_aliases() {
        // Test "new" alias for "create"
        let cli = Cli::try_parse_from(["handoff", "new", "test"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Create { .. })));

        // Test "view" alias for "receive"
        let cli = Cli::try_parse_from(["handoff", "view", "test"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Receive { .. })));

        // Test "ls" alias for "list"
        let cli = Cli::try_parse_from(["handoff", "ls"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::List { .. })));

        // Test "s" alias for "status"
        let cli = Cli::try_parse_from(["handoff", "s"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Status { .. })));
    }
}
