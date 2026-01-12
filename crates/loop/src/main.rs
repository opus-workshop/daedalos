//! loop - The core iteration primitive of Daedalos
//!
//! "A loop is not a feature. A loop is how intelligent work gets done."
//!
//! This tool implements the Ralph Wiggum technique: iterate until done.
//! You define a task and a promise (verification command), and the loop
//! runs until the promise is met or max iterations are reached.

mod checkpoint;
mod cli;
mod promise;
mod state;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

use cli::{Cli, Commands};

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    // Run the async runtime for commands that need it
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async { run_command(cli.command).await })
}

async fn run_command(command: Commands) -> Result<()> {
    match command {
        Commands::Start {
            prompt,
            promise,
            max_iterations,
            checkpoint,
            timeout,
            json,
        } => {
            cli::cmd_start(prompt, promise, max_iterations, checkpoint, timeout, json).await
        }
        Commands::Status { loop_id, json } => cli::cmd_status(loop_id, json).await,
        Commands::List { status, json } => cli::cmd_list(status, json).await,
        Commands::Stop { loop_id, rollback } => cli::cmd_stop(loop_id, rollback).await,
        Commands::Cancel { loop_id, rollback } => cli::cmd_cancel(loop_id, rollback).await,
        Commands::History { loop_id, verbose } => cli::cmd_history(loop_id, verbose).await,
    }
}
