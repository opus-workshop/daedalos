//! spec - Rich specification management for Daedalos
//!
//! AI agents work dramatically better with rich specifications.
//! Specs capture not just WHAT but WHY, alternatives rejected,
//! and anti-patterns to avoid.

mod cli;
mod find;
mod output;
mod spec;
mod template;
mod validate;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Commands};

fn main() -> Result<()> {
    let cli = Cli::parse();
    run_command(cli.command)
}

fn run_command(command: Commands) -> Result<()> {
    match command {
        Commands::Show {
            component,
            section,
            json,
        } => cli::cmd_show(&component, section.as_deref(), json),
        Commands::Query { query } => cli::cmd_query(&query),
        Commands::List { missing, stale } => cli::cmd_list(missing, stale),
        Commands::Context { task } => cli::cmd_context(&task),
        Commands::Validate { path } => cli::cmd_validate(path.as_deref()),
        Commands::New { name, type_ } => cli::cmd_new(&name, &type_),
    }
}
