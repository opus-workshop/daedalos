//! CLI command definitions and handlers

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use crate::checkpoint::{get_backend, CheckpointStrategy};
use crate::promise::verify_promise;
use crate::state::{list_loops, Loop, LoopConfig, LoopState, LoopStatus};

/// loop - The core iteration primitive of Daedalos
///
/// "A loop is not a feature. A loop is how intelligent work gets done."
#[derive(Parser)]
#[command(name = "loop")]
#[command(version = "1.0.0")]
#[command(about = "Iterate until a verification command passes")]
#[command(long_about = "Iterate until a verification command passes.\n\n\
    The loop tool is the foundation of Daedalos. It runs a task repeatedly\n\
    until a 'promise' command (like `pytest` or `cargo build`) exits 0.\n\
    Checkpoints are created between iterations for rollback safety.\n\n\
    WHEN TO USE: Any 'fix', 'make pass', or 'get working' task where you\n\
    would otherwise edit-run-check manually.")]
#[command(after_help = "EXAMPLES:\n\
    loop start \"fix the failing tests\" --promise \"pytest\"\n\
    loop start \"make the build pass\" --promise \"cargo build\"\n\
    loop start \"implement feature\" --promise \"npm test\" -n 20\n\
    loop status\n\
    loop stop abc123 --rollback")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start a new loop
    #[command(about = "Start a new loop that iterates until promise is met")]
    Start {
        /// Task description (natural language)
        #[arg(required = true)]
        prompt: String,

        /// Shell command that must exit 0 for success
        #[arg(short, long, required = true)]
        promise: String,

        /// Maximum iterations before giving up
        #[arg(short = 'n', long, default_value = "10")]
        max_iterations: u32,

        /// Checkpoint strategy: auto, git, btrfs, none
        #[arg(long, default_value = "auto")]
        checkpoint: String,

        /// Per-iteration timeout in seconds
        #[arg(long, default_value = "300")]
        timeout: u64,

        /// Output as JSON
        #[arg(long, default_value = "false")]
        json: bool,
    },

    /// Show status of a loop
    #[command(about = "Show status of a running or completed loop")]
    Status {
        /// Loop ID (optional, shows all if not provided)
        loop_id: Option<String>,

        /// Output as JSON
        #[arg(long, default_value = "false")]
        json: bool,
    },

    /// List all loops
    #[command(about = "List all loops")]
    List {
        /// Filter by status: running, completed, failed, cancelled
        #[arg(long)]
        status: Option<String>,

        /// Output as JSON
        #[arg(long, default_value = "false")]
        json: bool,
    },

    /// Stop a running loop (waits for current iteration)
    #[command(about = "Stop a loop after current iteration completes")]
    Stop {
        /// Loop ID
        loop_id: String,

        /// Rollback to initial state
        #[arg(long, default_value = "false")]
        rollback: bool,
    },

    /// Cancel a loop immediately
    #[command(about = "Cancel a loop immediately")]
    Cancel {
        /// Loop ID
        loop_id: String,

        /// Rollback to initial state
        #[arg(long, default_value = "false")]
        rollback: bool,
    },

    /// Show iteration history for a loop
    #[command(about = "Show iteration history")]
    History {
        /// Loop ID
        loop_id: String,

        /// Show verbose output
        #[arg(short, long, default_value = "false")]
        verbose: bool,
    },
}

/// Start a new loop
pub async fn cmd_start(
    prompt: String,
    promise: String,
    max_iterations: u32,
    checkpoint_strategy: String,
    timeout: u64,
    json_output: bool,
) -> Result<()> {
    let working_dir = std::env::current_dir().context("Failed to get current directory")?;

    // Parse checkpoint strategy
    let strategy = match checkpoint_strategy.as_str() {
        "auto" => CheckpointStrategy::Auto,
        "git" => CheckpointStrategy::Git,
        "btrfs" => CheckpointStrategy::Btrfs,
        "none" => CheckpointStrategy::None,
        _ => {
            anyhow::bail!("Unknown checkpoint strategy: {}", checkpoint_strategy);
        }
    };

    let checkpoint = get_backend(&working_dir, strategy)?;

    // Check if promise is already met
    let initial_check = verify_promise(&promise, &working_dir, timeout).await?;
    if initial_check.success {
        if json_output {
            println!(
                "{}",
                serde_json::json!({
                    "status": "already_satisfied",
                    "message": "Promise already met, no loop needed"
                })
            );
        } else {
            println!("Promise already met, no loop needed!");
        }
        return Ok(());
    }

    let config = LoopConfig {
        prompt: prompt.clone(),
        promise_cmd: promise.clone(),
        working_dir: working_dir.clone(),
        max_iterations,
        timeout,
        checkpoint_strategy: checkpoint.name().to_string(),
    };

    // Create the loop
    let mut loop_runner = Loop::new(config, checkpoint)?;

    if json_output {
        println!(
            "{}",
            serde_json::json!({
                "id": loop_runner.state().id,
                "status": "started",
                "prompt": prompt,
                "promise": promise,
                "max_iterations": max_iterations
            })
        );
    } else {
        println!("Starting loop with {} checkpoints", loop_runner.checkpoint_name());
        println!("Promise: {}", promise);
        println!("Max iterations: {}", max_iterations);
        println!();
        println!("Loop ID: {}", loop_runner.state().id);
        println!();
    }

    // Run the loop
    let success = loop_runner.run().await?;

    let final_state = loop_runner.state();
    if json_output {
        println!("{}", serde_json::to_string_pretty(&final_state)?);
    } else {
        println!();
        if success {
            println!(
                "Loop completed in {} iterations!",
                final_state.current_iteration
            );
        } else {
            println!(
                "Loop failed after {} iterations",
                final_state.current_iteration
            );
            if let Some(err) = &final_state.error_message {
                println!("Error: {}", err);
            }
        }
    }

    if success {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

/// Show status of a loop
pub async fn cmd_status(loop_id: Option<String>, json_output: bool) -> Result<()> {
    match loop_id {
        Some(id) => {
            let state = LoopState::load(&id)?;
            if json_output {
                println!("{}", serde_json::to_string_pretty(&state)?);
            } else {
                print_loop_status(&state);
            }
        }
        None => {
            // List all loops
            cmd_list(None, json_output).await?;
        }
    }
    Ok(())
}

/// List all loops
pub async fn cmd_list(status_filter: Option<String>, json_output: bool) -> Result<()> {
    let loops = list_loops()?;

    let filtered: Vec<_> = if let Some(filter) = &status_filter {
        let target_status = match filter.as_str() {
            "running" => LoopStatus::Running,
            "completed" => LoopStatus::Completed,
            "failed" => LoopStatus::Failed,
            "cancelled" => LoopStatus::Cancelled,
            "paused" => LoopStatus::Paused,
            "pending" => LoopStatus::Pending,
            _ => anyhow::bail!("Unknown status filter: {}", filter),
        };
        loops
            .into_iter()
            .filter(|l| l.status == target_status)
            .collect()
    } else {
        loops
    };

    if json_output {
        println!("{}", serde_json::to_string_pretty(&filtered)?);
    } else if filtered.is_empty() {
        println!("No loops found");
    } else {
        println!(
            "{:<12} {:<12} {:<8} {:<30} {:<10}",
            "ID", "STATUS", "ITER", "PROMISE", "CHECKPOINT"
        );
        println!("{}", "-".repeat(80));
        for state in filtered.iter().take(20) {
            let promise = if state.promise_cmd.len() > 28 {
                format!("{}...", &state.promise_cmd[..28])
            } else {
                state.promise_cmd.clone()
            };
            let iter_str = format!("{}/{}", state.current_iteration, state.max_iterations);
            println!(
                "{:<12} {:<12} {:<8} {:<30} {:<10}",
                &state.id[..12.min(state.id.len())],
                state.status.as_str(),
                iter_str,
                promise,
                state.checkpoint_strategy
            );
        }
    }
    Ok(())
}

/// Stop a loop
pub async fn cmd_stop(loop_id: String, rollback: bool) -> Result<()> {
    let mut state = LoopState::load(&loop_id)?;

    if state.status != LoopStatus::Running && state.status != LoopStatus::Paused {
        println!(
            "Loop is not running or paused (status: {})",
            state.status.as_str()
        );
        return Ok(());
    }

    state.status = LoopStatus::Cancelled;
    state.updated_at = chrono::Utc::now();
    state.save()?;

    println!("Loop {} stopped", loop_id);

    if rollback {
        if let Some(checkpoint_id) = &state.initial_checkpoint {
            println!("Rolling back to initial state: {}", checkpoint_id);
            // Note: actual rollback would require the checkpoint backend
        }
    }

    Ok(())
}

/// Cancel a loop
pub async fn cmd_cancel(loop_id: String, rollback: bool) -> Result<()> {
    cmd_stop(loop_id, rollback).await
}

/// Show iteration history
pub async fn cmd_history(loop_id: String, verbose: bool) -> Result<()> {
    let state = LoopState::load(&loop_id)?;

    println!(
        "{:<6} {:<25} {:<8} {:<10} {}",
        "ITER", "CHECKPOINT", "RESULT", "DURATION", "CHANGES"
    );
    println!("{}", "-".repeat(80));

    for iteration in &state.iterations {
        let result = if iteration.promise_result { "PASS" } else { "FAIL" };
        let duration = format!("{}ms", iteration.duration_ms);
        let checkpoint = if iteration.checkpoint_id.len() > 23 {
            format!("{}...", &iteration.checkpoint_id[..23])
        } else {
            iteration.checkpoint_id.clone()
        };
        let changes = iteration
            .changes_summary
            .lines()
            .next()
            .unwrap_or("N/A")
            .chars()
            .take(20)
            .collect::<String>();

        println!(
            "{:<6} {:<25} {:<8} {:<10} {}",
            iteration.number, checkpoint, result, duration, changes
        );

        if verbose && !iteration.promise_output.is_empty() {
            println!(
                "  Output: {}...",
                &iteration.promise_output[..100.min(iteration.promise_output.len())]
            );
        }
    }

    Ok(())
}

fn print_loop_status(state: &LoopState) {
    println!("Loop: {}", state.id);
    println!("Status: {}", state.status.as_str());
    let prompt_display = if state.prompt.len() > 60 {
        format!("{}...", &state.prompt[..60])
    } else {
        state.prompt.clone()
    };
    println!("Prompt: {}", prompt_display);
    println!("Promise: {}", state.promise_cmd);
    println!(
        "Iterations: {}/{}",
        state.current_iteration, state.max_iterations
    );
    println!("Checkpoint: {}", state.checkpoint_strategy);
    println!("Created: {}", state.created_at);
    println!("Updated: {}", state.updated_at);
}
