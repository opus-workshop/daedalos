//! gates - Configurable approval checkpoints for AI agents
//!
//! "Trust is earned, not given. Control how much autonomy AI gets."
//!
//! Gates provides configurable permission/supervision gates for AI actions.
//! Supervision levels range from "autonomous" (minimal gates) to "manual"
//! (everything requires approval).

mod checker;
mod config;

use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use clap::{Parser, Subcommand};
use std::collections::HashMap;

use crate::checker::{check_gate, get_gate_history};
use crate::config::{
    get_config_path, load_config, load_project_config, save_config, GateAction,
    SupervisionConfig, SupervisionLevel,
};

#[derive(Parser)]
#[command(name = "gates")]
#[command(about = "Permission gates controlling what AI agents can do autonomously")]
#[command(version)]
#[command(after_help = r#"WHEN TO USE:
    Use gates to control AI autonomy levels and require approval for
    sensitive operations. Check gates programmatically before actions.

TRIGGER:
    - Before deploying to production
    - When working with sensitive files (.env, credentials)
    - To prevent accidental force pushes or destructive operations
    - When you want visibility into what AI is doing

EXAMPLES:
    gates level collaborative      # Require approval for major actions
    gates check file_delete        # Check if deletion is allowed
    gates set git_force_push deny  # Never allow force push
    gates history --days 1         # See what was checked today
    gates config                   # Show current permission setup

SUPERVISION LEVELS:
    autonomous       AI runs freely, minimal gates
    supervised       AI runs, human gets notifications
    collaborative    AI proposes, human approves major actions
    assisted         Human drives, AI suggests
    manual           AI only responds to direct commands

GATE ACTIONS:
    allow            Proceed without asking
    notify           Notify but don't block
    approve          Require explicit approval
    deny             Always deny

BUILT-IN GATES:
    file_delete      Deleting files
    file_create      Creating new files
    file_modify      Modifying existing files
    git_commit       Making git commits
    git_push         Pushing to remote
    git_force_push   Force pushing (dangerous)
    loop_start       Starting iteration loops
    agent_spawn      Spawning new agents
    shell_command    Running shell commands
    sensitive_file   Modifying sensitive files"#)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Check if an action is allowed (exits 0=allowed, 1=denied)
    #[command(after_help = "Example: gates check file_delete '{\"path\": \"/etc/passwd\"}'")]
    Check {
        /// Gate name to check (e.g., file_delete, git_push, shell_command)
        gate: String,

        /// Context as JSON - provides details for the gate check
        #[arg(default_value = "{}")]
        context: String,

        /// Source of the request (for audit trail)
        #[arg(short, long, default_value = "cli")]
        source: String,
    },

    /// Get or set the global supervision level
    #[command(after_help = "Levels: autonomous, supervised, collaborative, assisted, manual")]
    Level {
        /// Level to set (omit to show current level)
        level: Option<String>,
    },

    /// Override a specific gate's action
    #[command(after_help = "Example: gates set git_force_push deny")]
    Set {
        /// Gate name to configure
        gate: String,

        /// Action: allow, notify, approve, or deny
        action: String,
    },

    /// Show current gates configuration
    Config {
        /// Output as JSON for scripting
        #[arg(long)]
        json: bool,
    },

    /// Show audit trail of gate checks
    #[command(after_help = "Example: gates history --gate file_delete --days 1")]
    History {
        /// Filter to specific gate
        #[arg(long)]
        gate: Option<String>,

        /// Number of days to look back
        #[arg(long, default_value = "7")]
        days: u32,

        /// Maximum entries to show
        #[arg(long, default_value = "20")]
        limit: usize,

        /// Output as JSON for scripting
        #[arg(long)]
        json: bool,
    },

    /// Create config file with default settings
    Init,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Check { gate, context, source }) => cmd_check(&gate, &context, &source),
        Some(Commands::Level { level }) => cmd_level(level.as_deref()),
        Some(Commands::Set { gate, action }) => cmd_set(&gate, &action),
        Some(Commands::Config { json }) => cmd_config(json),
        Some(Commands::History { gate, days, limit, json }) => {
            cmd_history(gate.as_deref(), days, limit, json)
        }
        Some(Commands::Init) => cmd_init(),
        None => cmd_help(),
    }
}

/// Show help message
fn cmd_help() -> Result<()> {
    println!("gates - Configurable approval checkpoints for AI agents");
    println!();
    println!("Run 'gates --help' for usage information");
    Ok(())
}

/// Check if an action is allowed through a gate
fn cmd_check(gate: &str, context_str: &str, source: &str) -> Result<()> {
    let context: HashMap<String, serde_json::Value> = if context_str.is_empty() || context_str == "{}" {
        HashMap::new()
    } else {
        serde_json::from_str(context_str).context("Failed to parse context JSON")?
    };

    let result = check_gate(gate, Some(context), source, None)?;

    if result.allowed {
        println!("allowed: {}", result.reason);
        Ok(())
    } else {
        println!("denied: {}", result.reason);
        std::process::exit(1);
    }
}

/// Get or set supervision level
fn cmd_level(new_level: Option<&str>) -> Result<()> {
    match new_level {
        None => {
            // Get current level
            let config = load_config()?;
            println!("{}", config.level);
        }
        Some(level_str) => {
            // Set new level
            let level = SupervisionLevel::from_str(level_str).ok_or_else(|| {
                let valid: Vec<_> = SupervisionLevel::all().iter().map(|l| l.as_str()).collect();
                anyhow::anyhow!(
                    "Invalid level '{}'. Valid levels: {}",
                    level_str,
                    valid.join(", ")
                )
            })?;

            // Create new config with default gates for this level
            let old_config = load_config()?;
            let mut config = SupervisionConfig::with_level(level);
            config.autonomy = old_config.autonomy;

            save_config(&config)?;
            println!("Supervision level set to: {}", level);
        }
    }
    Ok(())
}

/// Set a gate action
fn cmd_set(gate: &str, action_str: &str) -> Result<()> {
    let action = GateAction::from_str(action_str).ok_or_else(|| {
        let valid: Vec<_> = GateAction::all().iter().map(|a| a.as_str()).collect();
        anyhow::anyhow!(
            "Invalid action '{}'. Valid actions: {}",
            action_str,
            valid.join(", ")
        )
    })?;

    let mut config = load_config()?;
    config.gates.insert(gate.to_string(), action);
    save_config(&config)?;

    println!("Gate '{}' set to: {}", gate, action);
    Ok(())
}

/// Show current configuration
fn cmd_config(json: bool) -> Result<()> {
    let config = load_project_config(None)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&config.to_json_value())?);
        return Ok(());
    }

    // Pretty print configuration
    println!("Config file: {}", get_config_path().display());
    println!("Level: {}", config.level);
    println!();
    println!("Gates:");

    let mut gates: Vec<_> = config.gates.iter().collect();
    gates.sort_by_key(|(k, _)| k.as_str());

    for (gate, action) in gates {
        let color = match action {
            GateAction::Allow => "\x1b[32m",   // Green
            GateAction::Notify => "\x1b[33m",  // Yellow
            GateAction::Approve => "\x1b[34m", // Blue
            GateAction::Deny => "\x1b[31m",    // Red
        };
        println!("  {}: {}{}\x1b[0m", gate, color, action);
    }

    if !config.overrides.is_empty() {
        println!();
        println!("Project Overrides:");
        for (gate, action) in &config.overrides {
            println!("  {}: {}", gate, action);
        }
    }

    println!();
    println!("Autonomy Limits:");
    println!("  max_iterations: {}", config.autonomy.max_iterations);
    println!("  max_file_changes: {}", config.autonomy.max_file_changes);
    println!("  max_lines_changed: {}", config.autonomy.max_lines_changed);
    println!("  sensitive_paths:");
    for path in &config.autonomy.sensitive_paths {
        println!("    - {}", path);
    }

    Ok(())
}

/// Show gate check history
fn cmd_history(gate: Option<&str>, days: u32, limit: usize, json: bool) -> Result<()> {
    let history = get_gate_history(gate, days, limit)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&history)?);
        return Ok(());
    }

    if history.is_empty() {
        println!("No gate history found");
        return Ok(());
    }

    for event in &history {
        let timestamp = event.timestamp as i64;
        let dt = DateTime::from_timestamp(timestamp, 0)
            .map(|t| t.with_timezone(&Local))
            .unwrap_or_else(|| Local::now());
        let time_str = dt.format("%Y-%m-%d %H:%M:%S").to_string();

        let allowed_marker = if event.result.allowed { "\x1b[32m+\x1b[0m" } else { "\x1b[31m-\x1b[0m" };

        println!(
            "{}  {}  {:<20}  {:<8}  {}",
            time_str,
            allowed_marker,
            event.gate,
            event.result.action,
            event.source
        );
    }

    Ok(())
}

/// Initialize config file with defaults
fn cmd_init() -> Result<()> {
    let config_path = get_config_path();

    if config_path.exists() {
        println!("Config already exists: {}", config_path.display());
        println!("Use 'gates level <level>' to change supervision level");
        return Ok(());
    }

    let config = SupervisionConfig::default();
    save_config(&config)?;

    println!("Created config: {}", config_path.display());
    println!("Default level: {}", config.level);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parse() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
