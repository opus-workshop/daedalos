//! Interactive REPL for oracle

use anyhow::Result;
use colored::Colorize;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

use crate::backend::Backend;
use crate::cli::Cli;
use crate::session::SessionManager;

/// Run the interactive REPL
pub fn run(cli: &Cli, backend: &Backend, session_mgr: &SessionManager) -> Result<()> {
    let mut rl = DefaultEditor::new()?;

    // Load history
    let history_path = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("oracle")
        .join("history.txt");

    let _ = rl.load_history(&history_path);

    // Get initial session ID if continuing
    let mut session_id: Option<String> = if cli.continue_session {
        session_mgr.get_last_session()?
    } else if let Some(name) = &cli.session {
        session_mgr.get_or_create_session(name)?
    } else {
        None
    };

    loop {
        let prompt = if session_id.is_some() {
            format!("{} ", ">".cyan())
        } else {
            format!("{} ", ">".white())
        };

        match rl.readline(&prompt) {
            Ok(line) => {
                let line = line.trim();

                // Skip empty lines
                if line.is_empty() {
                    continue;
                }

                // Handle special commands
                if line == "exit" || line == "quit" || line == "q" {
                    break;
                }

                if line == "clear" {
                    session_id = None;
                    if !cli.quiet {
                        println!("{}", "Session cleared".dimmed());
                    }
                    continue;
                }

                if line == "help" || line == "?" {
                    print_help();
                    continue;
                }

                if line.starts_with("session ") {
                    let name = line.strip_prefix("session ").unwrap().trim();
                    session_id = session_mgr.get_or_create_session(name)?;
                    if !cli.quiet {
                        println!("{}", format!("Switched to session: {}", name).dimmed());
                    }
                    continue;
                }

                if line == "sessions" {
                    let sessions = session_mgr.list_sessions()?;
                    if sessions.is_empty() {
                        println!("{}", "No saved sessions".dimmed());
                    } else {
                        println!("{}", "Sessions:".bold());
                        for s in sessions {
                            println!("  {}", s);
                        }
                    }
                    continue;
                }

                // Add to history
                let _ = rl.add_history_entry(line);

                // Execute the prompt
                println!();
                match backend.execute_for_session(line, session_id.as_deref()) {
                    Ok(new_session_id) => {
                        if let Some(id) = new_session_id {
                            session_id = Some(id.clone());
                            // Save session
                            if let Some(name) = &cli.session {
                                let _ = session_mgr.save_session(name, &id);
                            } else {
                                let _ = session_mgr.save_last_session(&id);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("{} {}", "Error:".red(), e);
                    }
                }
                println!();
            }
            Err(ReadlineError::Interrupted) => {
                // Ctrl-C - cancel current input
                continue;
            }
            Err(ReadlineError::Eof) => {
                // Ctrl-D - exit
                break;
            }
            Err(err) => {
                eprintln!("{} {:?}", "Error:".red(), err);
                break;
            }
        }
    }

    // Save history
    let _ = std::fs::create_dir_all(history_path.parent().unwrap());
    let _ = rl.save_history(&history_path);

    Ok(())
}

/// Print REPL help
fn print_help() {
    println!();
    println!("{}", "Oracle REPL Commands".bold());
    println!("{}", "=".repeat(20).dimmed());
    println!();
    println!("  {}        - Exit the REPL", "exit, quit, q".cyan());
    println!("  {}            - Clear current session", "clear".cyan());
    println!("  {}         - Show this help", "help, ?".cyan());
    println!("  {} - Switch to named session", "session <name>".cyan());
    println!("  {}         - List saved sessions", "sessions".cyan());
    println!();
    println!("  {}          - Cancel current input", "Ctrl-C".dimmed());
    println!("  {}          - Exit", "Ctrl-D".dimmed());
    println!();
    println!("The {} prompt indicates an active session.", ">".cyan());
    println!();
}

#[cfg(test)]
mod tests {
    // REPL is interactive, hard to unit test
    // Integration tests would be more appropriate
}
