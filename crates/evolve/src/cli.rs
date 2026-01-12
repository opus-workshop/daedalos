//! CLI parsing and command handlers for evolve

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::analyze::analyze_state;
use crate::gaps::identify_gaps;
use crate::intent::extract_intent;
use crate::output::{print_full_evolution, print_json_evolution};
use crate::path::suggest_path;

#[derive(Parser)]
#[command(name = "evolve")]
#[command(about = "Understand code intent and suggest evolution paths")]
#[command(version)]
#[command(after_help = "\
TRIGGER:
    Use evolve when you need to understand what code is trying to become,
    before making changes to unfamiliar code, or when planning major refactors.

PHASES:
    intent       Discover what this code is trying to become
    analyze      Assess how well code serves the intent (included in full)
    gaps         Find what's missing to fully realize intent
    path         Chart evolution path from current state to full intent

EXAMPLES:
    evolve src/auth/              Full evolution analysis
    evolve intent src/cache.ts    Just intent analysis
    evolve gaps .                 Find missing pieces
    evolve path --json src/       Evolution path as JSON

The tool gathers intent from:
  - Specs (.spec.yaml files)
  - README and documentation
  - Git commit history
  - Tests and their expectations
  - Code comments and docstrings
  - Naming conventions")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Path to analyze (default: current directory)
    pub path: Option<String>,

    /// Output as JSON
    #[arg(long, global = true)]
    pub json: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Discover what this code is trying to become
    Intent {
        /// Path to analyze
        path: Option<String>,
    },

    /// Find what's missing to fully realize the intent
    Gaps {
        /// Path to analyze
        path: Option<String>,
    },

    /// Suggest evolution path with priorities
    Path {
        /// Path to analyze
        path: Option<String>,
    },
}

/// Run full evolution analysis
pub fn cmd_evolve(path: &str, json: bool) -> Result<()> {
    let abs_path = resolve_path(path)?;

    let intent = extract_intent(&abs_path)?;
    let analysis = analyze_state(&abs_path)?;
    let gaps = identify_gaps(&abs_path, &intent)?;
    let evolution_path = suggest_path(&abs_path, &intent, &analysis, &gaps)?;

    if json {
        print_json_evolution(&abs_path, &intent, &analysis, &gaps, &evolution_path)?;
    } else {
        print_full_evolution(&abs_path, &intent, &analysis, &gaps, &evolution_path)?;
    }

    Ok(())
}

/// Run intent analysis only
pub fn cmd_intent(path: &str, json: bool) -> Result<()> {
    let abs_path = resolve_path(path)?;
    let intent = extract_intent(&abs_path)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&intent)?);
    } else {
        crate::output::print_intent_phase(&intent)?;
    }

    Ok(())
}

/// Run gaps analysis only
pub fn cmd_gaps(path: &str, json: bool) -> Result<()> {
    let abs_path = resolve_path(path)?;
    let intent = extract_intent(&abs_path)?;
    let gaps = identify_gaps(&abs_path, &intent)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&gaps)?);
    } else {
        crate::output::print_gaps_phase(&gaps)?;
    }

    Ok(())
}

/// Run path suggestion only
pub fn cmd_path(path: &str, json: bool) -> Result<()> {
    let abs_path = resolve_path(path)?;
    let intent = extract_intent(&abs_path)?;
    let analysis = analyze_state(&abs_path)?;
    let gaps = identify_gaps(&abs_path, &intent)?;
    let evolution_path = suggest_path(&abs_path, &intent, &analysis, &gaps)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&evolution_path)?);
    } else {
        crate::output::print_path_phase(&evolution_path)?;
    }

    Ok(())
}

/// Resolve a path to absolute, checking it exists
fn resolve_path(path: &str) -> Result<std::path::PathBuf> {
    let path = std::path::Path::new(path);
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };

    if !abs_path.exists() {
        anyhow::bail!("Path not found: {}", abs_path.display());
    }

    Ok(abs_path.canonicalize()?)
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
