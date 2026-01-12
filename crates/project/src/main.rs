//! project - Pre-computed codebase intelligence for AI agents
//!
//! "Agents waste massive context reading files to understand architecture."
//!
//! This tool provides instant codebase intelligence through pre-computed indexes.
//! When an agent enters a new project, `project info` gives architectural context
//! in seconds, not minutes of exploration.

mod cache;
mod cli;
mod database;
mod detectors;
mod parsers;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Commands};

fn main() -> Result<()> {
    let cli = Cli::parse();
    run_command(cli.command)
}

fn run_command(command: Commands) -> Result<()> {
    match command {
        Commands::Info { path, json, refresh } => cli::cmd_info(path, json, refresh),
        Commands::Tree { path, depth, json, refresh } => cli::cmd_tree(path, depth, json, refresh),
        Commands::Symbols { path, type_filter, json, limit, refresh } => {
            cli::cmd_symbols(path, type_filter, json, limit, refresh)
        }
        Commands::Deps { file_path, project, json, refresh } => {
            cli::cmd_deps(file_path, project, json, refresh)
        }
        Commands::Dependents { file_path, project, json, refresh } => {
            cli::cmd_dependents(file_path, project, json, refresh)
        }
        Commands::Index { path, full } => cli::cmd_index(path, full),
        Commands::Stats { path, json, refresh } => cli::cmd_stats(path, json, refresh),
    }
}
