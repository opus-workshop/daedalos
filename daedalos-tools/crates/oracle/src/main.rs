//! oracle - Unified LLM interface for Daedalos
//!
//! Oracle provides a distraction-free interface to language models.
//! It wraps existing CLI tools (claude, opencode, ollama) to provide
//! a consistent experience across backends.
//!
//! # Invocation Names
//!
//! - `oracle` - REPL mode, verbose
//! - `ora` - REPL mode, standard
//! - `ask` - One-shot if prompt given, REPL otherwise
//!
//! # Examples
//!
//! ```bash
//! ask "what does this function do?"
//! git diff | ask "review this change"
//! ask -c "what about edge cases?"
//! ora
//! ```

mod backend;
mod cli;
mod config;
mod repl;
mod session;

use anyhow::Result;
use colored::Colorize;
use std::env;
use std::io::{self, Read};

use crate::backend::Backend;
use crate::cli::{Cli, InvocationStyle};
use crate::config::Config;
use crate::session::SessionManager;

fn main() -> Result<()> {
    let invocation = detect_invocation();
    let cli = Cli::parse_with_style(invocation);
    let config = Config::load()?;

    // Check for piped input
    let piped_input = read_piped_input()?;

    // Determine the prompt (CLI arg + piped input)
    let prompt = build_prompt(&cli.prompt, &piped_input);

    // Get the backend
    let env_backend = env::var("ORACLE_BACKEND").ok();
    let backend_name = cli
        .backend
        .as_ref()
        .or(env_backend.as_ref())
        .map(|s| s.as_str())
        .unwrap_or(&config.default);
    let backend = config.get_backend(backend_name)?;

    // Initialize session manager
    let session_mgr = SessionManager::new()?;

    if let Some(prompt) = prompt {
        // One-shot mode
        run_oneshot(&cli, &backend, &session_mgr, &prompt)
    } else {
        // REPL mode
        run_repl(&cli, &backend, &session_mgr)
    }
}

/// Detect how we were invoked (oracle, ora, or ask)
fn detect_invocation() -> InvocationStyle {
    let arg0 = env::args().next().unwrap_or_default();
    let name = std::path::Path::new(&arg0)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("oracle");

    match name {
        "ask" => InvocationStyle::Ask,
        "ora" => InvocationStyle::Ora,
        _ => InvocationStyle::Oracle,
    }
}

/// Read piped input if available
fn read_piped_input() -> Result<Option<String>> {
    use std::os::unix::io::AsRawFd;

    // Check if stdin is a TTY
    let stdin = io::stdin();
    let is_tty = unsafe { libc::isatty(stdin.as_raw_fd()) } != 0;

    if is_tty {
        return Ok(None);
    }

    let mut input = String::new();
    stdin.lock().read_to_string(&mut input)?;

    if input.is_empty() {
        Ok(None)
    } else {
        Ok(Some(input))
    }
}

/// Build the full prompt from CLI arg and piped input
fn build_prompt(cli_prompt: &Option<String>, piped: &Option<String>) -> Option<String> {
    match (cli_prompt, piped) {
        (Some(p), Some(input)) => Some(format!("{}\n\n{}", p, input)),
        (Some(p), None) => Some(p.clone()),
        (None, Some(input)) => Some(input.clone()),
        (None, None) => None,
    }
}

/// Run in one-shot mode
fn run_oneshot(
    cli: &Cli,
    backend: &Backend,
    session_mgr: &SessionManager,
    prompt: &str,
) -> Result<()> {
    // Get session ID if continuing
    let session_id = if cli.continue_session {
        session_mgr.get_last_session()?
    } else if let Some(name) = &cli.session {
        session_mgr.get_or_create_session(name)?
    } else {
        None
    };

    // Execute the backend
    let result = backend.execute(prompt, session_id.as_deref(), cli.json)?;

    // Update session if we got a new ID
    if let Some(new_id) = &result.session_id {
        if let Some(name) = &cli.session {
            session_mgr.save_session(name, new_id)?;
        } else {
            session_mgr.save_last_session(new_id)?;
        }
    }

    // Output
    if cli.json {
        let output = serde_json::json!({
            "response": result.response,
            "session_id": result.session_id,
            "backend": backend.name,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        print!("{}", result.response);
        // Ensure newline at end
        if !result.response.ends_with('\n') {
            println!();
        }
    }

    Ok(())
}

/// Run in REPL mode
fn run_repl(cli: &Cli, backend: &Backend, session_mgr: &SessionManager) -> Result<()> {
    if !cli.quiet {
        println!(
            "{}",
            format!("oracle v{} ({})", env!("CARGO_PKG_VERSION"), backend.name).dimmed()
        );
        println!("{}", "Type 'exit' or Ctrl-D to quit".dimmed());
        println!();
    }

    repl::run(cli, backend, session_mgr)
}
