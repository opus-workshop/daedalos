//! verify - Universal project verification
//!
//! "One command. All checks. No excuses."
//!
//! This tool provides a universal interface for project verification.
//! It auto-detects project type and runs appropriate checks (lint, types, build, test).
//! Verification is the default promise for loops.

mod cli;
mod detect;
mod pipeline;
mod runner;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

use cli::Cli;

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    // Run the async runtime for commands that need it
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async { cli::run(cli).await })
}
