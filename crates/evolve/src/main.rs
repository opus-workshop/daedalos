//! evolve - Understand code intent and suggest evolution paths
//!
//! "Code wants to become something. Help it get there."
//!
//! Evolve analyzes code to understand its intent from specs, commits, tests,
//! and structure. Then it identifies gaps and suggests an evolution path
//! to fully realize that intent.

mod cli;
mod intent;
mod analyze;
mod gaps;
mod path;
mod output;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Commands};

fn main() -> Result<()> {
    let cli = Cli::parse();
    run_command(cli)
}

fn run_command(cli: Cli) -> Result<()> {
    let json = cli.json;

    match cli.command {
        Some(Commands::Intent { path }) => {
            let path = path.unwrap_or_else(|| ".".to_string());
            cli::cmd_intent(&path, json)
        }
        Some(Commands::Gaps { path }) => {
            let path = path.unwrap_or_else(|| ".".to_string());
            cli::cmd_gaps(&path, json)
        }
        Some(Commands::Path { path }) => {
            let path = path.unwrap_or_else(|| ".".to_string());
            cli::cmd_path(&path, json)
        }
        None => {
            // Default command: full evolution analysis on provided path
            let path = cli.path.unwrap_or_else(|| ".".to_string());
            cli::cmd_evolve(&path, json)
        }
    }
}
