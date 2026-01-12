//! Codex CLI - Semantic code search
//!
//! Ask natural language questions about your codebase.
//!
//! # Examples
//!
//! ```bash
//! codex search "where is authentication handled?"
//! codex search "what functions call the database?"
//! codex search -f auth.py "login logic"
//! codex index
//! codex status
//! ```

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;

use codex::{
    find_project_root, format_results, format_results_json,
    CodeIndex, CodeSearcher,
};

#[derive(Parser)]
#[command(name = "codex")]
#[command(about = "Semantic code search - ask natural language questions about your codebase")]
#[command(version)]
#[command(after_help = "\
TRIGGER:
    Use codex when exploring unfamiliar code or searching for where something
    is implemented. Search by meaning, not just keywords.

EXAMPLES:
    codex search \"where is authentication handled?\"
    codex search \"what functions call the database?\"
    codex search -f auth.py \"login logic\"
    codex search -t function \"error handling\"
    codex index                   Build/update the search index
    codex status                  Show index statistics
    codex similar src/api.rs 42   Find code similar to line 42
    codex explain src/db.rs \"connection pooling\"

COMMANDS:
    search    Search codebase with natural language query
    index     Build or update the search index
    status    Show index status and statistics
    clear     Clear the search index
    similar   Find code similar to a specific location
    explain   Search within a specific file")]
struct Cli {
    /// Project path (default: auto-detect from current directory)
    #[arg(short, long, global = true)]
    project: Option<PathBuf>,

    /// Output as JSON
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Search the codebase with a natural language query
    #[command(name = "search")]
    Search {
        /// The search query
        query: String,

        /// Number of results to return
        #[arg(short = 'n', long, default_value = "5")]
        limit: usize,

        /// Filter by file path pattern
        #[arg(short, long)]
        file: Option<String>,

        /// Filter by chunk type (function, class, struct, etc.)
        #[arg(short = 't', long = "type")]
        type_filter: Option<String>,

        /// Show code content in results
        #[arg(short = 'c', long = "show-content")]
        show_content: bool,

        /// Force reindex before searching
        #[arg(long)]
        reindex: bool,
    },

    /// Build or update the search index
    #[command(name = "index")]
    Index {
        /// Force full reindex
        #[arg(long)]
        force: bool,
    },

    /// Show index status and statistics
    #[command(name = "status")]
    Status,

    /// Clear the search index
    #[command(name = "clear")]
    Clear {
        /// Skip confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Find code similar to a specific location
    #[command(name = "similar")]
    Similar {
        /// File path
        file_path: String,

        /// Line number
        line: usize,

        /// Number of results
        #[arg(short = 'n', long, default_value = "5")]
        limit: usize,
    },

    /// Search within a specific file
    #[command(name = "explain")]
    Explain {
        /// File path
        file_path: String,

        /// The search query
        query: String,

        /// Number of results
        #[arg(short = 'n', long, default_value = "5")]
        limit: usize,
    },
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {:#}", e);
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    // Determine project root
    let project_path = match cli.project {
        Some(p) => p.canonicalize().unwrap_or(p),
        None => find_project_root(&std::env::current_dir()?),
    };

    match cli.command {
        Commands::Search {
            query,
            limit,
            file,
            type_filter,
            show_content,
            reindex,
        } => {
            let mut index = CodeIndex::new(&project_path)
                .context("Failed to create index")?;

            // Check if index exists or reindex requested
            if reindex || !index.is_indexed() {
                eprintln!("Indexing {}...", project_path.display());
                let stats = index.index_project(reindex)?;
                eprintln!(
                    "Indexed {} files, created {} chunks\n",
                    stats.files_indexed, stats.chunks_created
                );
            }

            let mut searcher = CodeSearcher::new(&mut index);
            let results = searcher.search(
                &query,
                limit,
                file.as_deref(),
                type_filter.as_deref(),
            )?;

            if cli.json {
                println!("{}", format_results_json(&results)?);
            } else {
                println!("{}", format_results(&results, show_content));
            }

            if results.is_empty() {
                process::exit(1);
            }
        }

        Commands::Index { force } => {
            let mut index = CodeIndex::new(&project_path)
                .context("Failed to create index")?;

            eprintln!("Indexing {}...", project_path.display());
            let stats = index.index_project(force)?;

            if cli.json {
                println!(
                    "{}",
                    serde_json::json!({
                        "files_indexed": stats.files_indexed,
                        "chunks_created": stats.chunks_created,
                        "skipped": stats.skipped,
                    })
                );
            } else {
                eprintln!(
                    "Indexed {} files, created {} chunks ({} skipped)",
                    stats.files_indexed, stats.chunks_created, stats.skipped
                );
            }
        }

        Commands::Status => {
            let mut index = CodeIndex::new(&project_path)
                .context("Failed to create index")?;

            if !index.is_indexed() {
                if cli.json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "indexed": false,
                            "message": "Not indexed. Run 'codex index' first."
                        })
                    );
                } else {
                    println!("Not indexed. Run 'codex index' first.");
                }
                process::exit(2);
            }

            let stats = index.get_stats()?;

            if cli.json {
                println!(
                    "{}",
                    serde_json::json!({
                        "indexed": true,
                        "project_path": stats.project_path,
                        "db_path": stats.db_path,
                        "backend": stats.backend,
                        "files": stats.files,
                        "chunks": stats.chunks,
                    })
                );
            } else {
                println!("Project:  {}", stats.project_path);
                println!("Database: {}", stats.db_path);
                println!("Backend:  {}", stats.backend);
                println!("Files:    {}", stats.files);
                println!("Chunks:   {}", stats.chunks);
            }
        }

        Commands::Clear { yes } => {
            let mut index = CodeIndex::new(&project_path)
                .context("Failed to create index")?;

            if !yes {
                eprint!("Clear the index? [y/N] ");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Cancelled.");
                    return Ok(());
                }
            }

            index.clear()?;

            if cli.json {
                println!("{}", serde_json::json!({"cleared": true}));
            } else {
                println!("Index cleared.");
            }
        }

        Commands::Similar {
            file_path,
            line,
            limit,
        } => {
            let mut index = CodeIndex::new(&project_path)
                .context("Failed to create index")?;

            if !index.is_indexed() {
                eprintln!("Not indexed. Run 'codex index' first.");
                process::exit(2);
            }

            // Make path relative to project
            let rel_path = PathBuf::from(&file_path)
                .strip_prefix(&project_path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or(file_path);

            let mut searcher = CodeSearcher::new(&mut index);
            let results = searcher.find_similar(&rel_path, line, limit)?;

            if cli.json {
                println!("{}", format_results_json(&results)?);
            } else {
                if results.is_empty() {
                    println!("No similar code found.");
                } else {
                    println!("{}", format_results(&results, true));
                }
            }
        }

        Commands::Explain {
            file_path,
            query,
            limit,
        } => {
            let mut index = CodeIndex::new(&project_path)
                .context("Failed to create index")?;

            if !index.is_indexed() {
                eprintln!("Not indexed. Run 'codex index' first.");
                process::exit(2);
            }

            // Make path relative to project
            let rel_path = PathBuf::from(&file_path)
                .strip_prefix(&project_path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or(file_path);

            let mut searcher = CodeSearcher::new(&mut index);
            let results = searcher.search_file(&query, &rel_path, limit)?;

            if cli.json {
                println!("{}", format_results_json(&results)?);
            } else {
                println!("{}", format_results(&results, true));
            }
        }
    }

    Ok(())
}
