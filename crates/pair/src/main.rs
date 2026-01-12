//! pair - Pair programming via shared tmux sessions
//!
//! "Share your terminal session with another human or AI."
//!
//! Pair enables collaborative development by creating shared tmux sessions
//! that multiple users can connect to simultaneously.

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use daedalos_core::Paths;
use std::path::PathBuf;

use pair::{
    check_tmate, check_tmux, generate_session_name, get_hostname, get_username,
    session::{
        current_session_name, end_session, join_session, leave_session, start_session,
        start_tmate_session, PairMode, PairSession,
    },
    PairStore,
};

#[derive(Parser)]
#[command(name = "pair")]
#[command(about = "Pair programming via shared tmux sessions - real-time collaboration")]
#[command(version)]
#[command(
    long_about = "Share your terminal session with another human or AI.\n\n\
    Pair creates shared tmux sessions that multiple users can connect to\n\
    simultaneously, enabling real-time collaborative development.\n\n\
    SHARING OPTIONS:\n\
    \n\
    Local (same machine):\n\
        Partner runs: pair join <session-name>\n\
    \n\
    Remote (SSH):\n\
        Partner runs: ssh user@host -t 'pair join <session-name>'\n\
    \n\
    Tmate (public):\n\
        pair start --tmate  # Uses tmate for public sharing"
)]
#[command(after_help = r#"WHEN TO USE:
    - Working with another developer on the same code
    - AI agent needs to collaborate with human in real-time
    - Debugging sessions that benefit from multiple eyes

EXAMPLES:
    pair start                          # Start session, auto-named
    pair start my-project               # Named session
    pair start --tmate                  # Public session via tmate
    pair join my-project                # Join existing session
    pair invite                         # Get shareable join command
    pair list                           # Show active sessions
    pair end                            # End current session

MODES:
    equal     - Both can type (default)
    driver    - Only host can type, guest watches
    navigator - Guest guides, host types
"#)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start a new shared tmux session for pair programming
    #[command(visible_aliases = &["new", "create"])]
    Start {
        /// Session name (auto-generated if not provided)
        name: Option<String>,

        /// Project directory to start in
        #[arg(short, long)]
        project: Option<PathBuf>,

        /// Pairing mode: equal (both type), driver (host only), navigator (guest guides)
        #[arg(short, long, default_value = "equal")]
        mode: String,

        /// Use tmate for public internet sharing (no SSH needed)
        #[arg(long)]
        tmate: bool,
    },

    /// Join an existing pair session (local or via SSH)
    #[command(visible_aliases = &["attach", "connect"])]
    Join {
        /// Session name to join
        name: String,

        /// Join in read-only observer mode
        #[arg(long)]
        readonly: bool,
    },

    /// Leave current pair session (detach without ending)
    #[command(visible_alias = "detach")]
    Leave,

    /// List all active pair sessions on this machine
    #[command(visible_alias = "ls")]
    List {
        /// Output as JSON for scripting
        #[arg(long)]
        json: bool,
    },

    /// Generate shareable invite command for your partner
    #[command(visible_alias = "share")]
    Invite {
        /// Session name (uses current session if not specified)
        name: Option<String>,
    },

    /// End a pair session and disconnect all participants
    #[command(visible_aliases = &["kill", "stop"])]
    End {
        /// Session name (uses current session if not specified)
        name: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let paths = Paths::new();
    let data_dir = paths.data.join("pair");
    let store = PairStore::new(&data_dir)?;

    match cli.command {
        Some(Commands::Start { name, project, mode, tmate }) => {
            cmd_start(&store, name, project, mode, tmate)
        }
        Some(Commands::Join { name, readonly }) => cmd_join(&store, &name, readonly),
        Some(Commands::Leave) => cmd_leave(),
        Some(Commands::List { json }) => cmd_list(&store, json),
        Some(Commands::Invite { name }) => cmd_invite(&store, name),
        Some(Commands::End { name }) => cmd_end(&store, name),
        None => cmd_list(&store, false),
    }
}

/// Start a new pair session
fn cmd_start(
    store: &PairStore,
    name: Option<String>,
    project: Option<PathBuf>,
    mode: String,
    use_tmate: bool,
) -> Result<()> {
    // Check for tmux/tmate
    if use_tmate {
        if !check_tmate() {
            bail!("tmate not found\nInstall with: brew install tmate / apt install tmate");
        }
    } else if !check_tmux() {
        bail!("tmux is required for pair programming\nInstall with: brew install tmux / apt install tmux");
    }

    // Parse mode
    let pair_mode = PairMode::from_str(&mode)
        .ok_or_else(|| anyhow::anyhow!("Invalid mode '{}'. Valid modes: equal, driver, navigator", mode))?;

    // Generate or use provided name
    let session_name = name.unwrap_or_else(generate_session_name);

    // Check if session already exists
    if store.exists(&session_name) {
        bail!(
            "Session already exists: {}\nJoin with: pair join {}",
            session_name,
            session_name
        );
    }

    // Resolve project directory
    let project_dir = if let Some(ref p) = project {
        p.canonicalize()
            .with_context(|| format!("Project directory not found: {}", p.display()))?
    } else {
        std::env::current_dir().context("Could not determine current directory")?
    };

    let socket_path = store.socket_path(&session_name);
    let username = get_username();
    let hostname = get_hostname();

    if use_tmate {
        println!("info: Starting tmate session: {}", session_name);

        let ssh_string = start_tmate_session(&session_name, &project_dir, &socket_path)?;

        // Save session metadata
        let session = PairSession::new(
            session_name.clone(),
            project_dir.clone(),
            socket_path.clone(),
            pair_mode,
            username.clone(),
            true,
        );
        store.save_session(&session)?;

        println!();
        println!("\x1b[1mTmate Session Started\x1b[0m");
        println!();
        println!("{}", ssh_string);
        println!();
        println!("Share the SSH command above with your pair partner");
        println!();
        println!("Attach with: tmate -S {} attach", socket_path.display());
    } else {
        println!("info: Starting pair session: {}", session_name);

        start_session(&session_name, &project_dir, &socket_path, pair_mode)?;

        // Save session metadata
        let session = PairSession::new(
            session_name.clone(),
            project_dir.clone(),
            socket_path.clone(),
            pair_mode,
            username.clone(),
            false,
        );
        store.save_session(&session)?;

        println!("\x1b[32m+\x1b[0m Pair session started: {}", session_name);
        println!();
        println!("\x1b[1mSession Info\x1b[0m");
        println!("  \x1b[36mName:\x1b[0m    {}", session_name);
        println!("  \x1b[36mProject:\x1b[0m {}", project_dir.display());
        println!("  \x1b[36mMode:\x1b[0m    {}", pair_mode.as_str());
        println!();
        println!("\x1b[1mTo join locally:\x1b[0m");
        println!("  pair join {}", session_name);
        println!();
        println!("\x1b[1mTo join remotely:\x1b[0m");
        println!("  ssh {}@{} -t 'pair join {}'", username, hostname, session_name);
        println!();
        println!("Attaching to session...");

        // Attach to the session
        join_session(&socket_path, &session_name, false, false)?;
    }

    Ok(())
}

/// Join an existing pair session
fn cmd_join(store: &PairStore, name: &str, readonly: bool) -> Result<()> {
    if !check_tmux() {
        bail!("tmux is required for pair programming\nInstall with: brew install tmux / apt install tmux");
    }

    let socket_path = store.socket_path(name);

    if !socket_path.exists() {
        // List available sessions
        cmd_list(store, false)?;
        println!();
        bail!("Session not found: {}\nUse 'pair list' to see active sessions", name);
    }

    // Load session info to check if it's tmate
    let session = store.load_session(name)?;
    let is_tmate = session.as_ref().map(|s| s.tmate).unwrap_or(false);

    println!("info: Joining pair session: {}", name);

    // TODO: Integrate with notify/journal when available
    // notify "Partner joined: $USER" --title "Pair Session"
    // journal log "Joined pair session: $name" "pair" "pair_join"

    join_session(&socket_path, name, readonly, is_tmate)?;

    Ok(())
}

/// Leave current pair session
fn cmd_leave() -> Result<()> {
    leave_session()?;
    println!("info: Left pair session");
    Ok(())
}

/// List active pair sessions
fn cmd_list(store: &PairStore, json: bool) -> Result<()> {
    let sessions = store.list_sessions()?;

    if json {
        let json_output: Vec<_> = sessions
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "project": s.project.to_string_lossy(),
                    "mode": s.mode.as_str(),
                    "host": s.host,
                    "started": s.started.to_rfc3339(),
                    "clients": s.client_count(),
                    "tmate": s.tmate,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_output)?);
        return Ok(());
    }

    if sessions.is_empty() {
        println!("No active pair sessions");
        return Ok(());
    }

    println!("\x1b[1mActive Pair Sessions\x1b[0m");
    println!();

    for session in &sessions {
        println!("  \x1b[36m{}\x1b[0m", session.name);

        // Truncate project path if too long
        let project_str = session.project.to_string_lossy();
        let project_display = if project_str.len() > 40 {
            format!("...{}", &project_str[project_str.len() - 37..])
        } else {
            project_str.to_string()
        };

        println!("    \x1b[2mProject:\x1b[0m {}", project_display);
        println!("    \x1b[2mHost:\x1b[0m    {}", session.host);
        println!("    \x1b[2mClients:\x1b[0m {} connected", session.client_count());
        if session.tmate {
            println!("    \x1b[2mType:\x1b[0m    tmate (public)");
        }
        println!();
    }

    Ok(())
}

/// Generate invite command for partner
fn cmd_invite(store: &PairStore, name: Option<String>) -> Result<()> {
    // Determine session name
    let session_name = if let Some(n) = name {
        n
    } else if let Some(current) = current_session_name() {
        current
    } else if let Some(session) = store.most_recent_session()? {
        session.name
    } else {
        bail!("No active session\nStart a session first: pair start");
    };

    let socket_path = store.socket_path(&session_name);
    let username = get_username();
    let hostname = get_hostname();

    if !socket_path.exists() {
        bail!("Session not found: {}", session_name);
    }

    // Check if it's a tmate session
    let session = store.load_session(&session_name)?;
    let is_tmate = session.as_ref().map(|s| s.tmate).unwrap_or(false);

    println!("\x1b[1mInvite to Pair Session\x1b[0m");
    println!();
    println!("\x1b[36mSession:\x1b[0m {}", session_name);
    println!();

    if is_tmate {
        // For tmate, show the SSH command
        let output = std::process::Command::new("tmate")
            .args(["-S", &socket_path.to_string_lossy(), "display", "-p", "#{tmate_ssh}"])
            .output()
            .context("Failed to get tmate SSH string")?;

        let ssh_string = String::from_utf8_lossy(&output.stdout).trim().to_string();
        println!("\x1b[1mTmate SSH (works anywhere):\x1b[0m");
        println!("  {}", ssh_string);
    } else {
        println!("\x1b[1mLocal (same machine):\x1b[0m");
        println!("  pair join {}", session_name);
        println!();
        println!("\x1b[1mRemote (SSH):\x1b[0m");
        println!("  ssh {}@{} -t 'pair join {}'", username, hostname, session_name);
        println!();
        println!("\x1b[1mDirect tmux:\x1b[0m");
        println!("  tmux -S {} attach -t {}", socket_path.display(), session_name);
    }

    Ok(())
}

/// End a pair session
fn cmd_end(store: &PairStore, name: Option<String>) -> Result<()> {
    // Determine session name
    let session_name = if let Some(n) = name {
        n
    } else if let Some(current) = current_session_name() {
        current
    } else {
        bail!("Session name required");
    };

    let socket_path = store.socket_path(&session_name);

    if !socket_path.exists() {
        bail!("Session not found: {}", session_name);
    }

    // Check if it's a tmate session
    let session = store.load_session(&session_name)?;
    let is_tmate = session.as_ref().map(|s| s.tmate).unwrap_or(false);

    // End the session
    end_session(&socket_path, &session_name, is_tmate)?;

    // Remove metadata
    store.remove_session(&session_name)?;

    println!("\x1b[32m+\x1b[0m Pair session ended: {}", session_name);

    // TODO: Integrate with journal when available
    // journal log "Ended pair session: $name" "pair" "pair_end"

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        // Just verify the CLI structure parses correctly
        let cli = Cli::try_parse_from(["pair", "list"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::List { json: false })));

        let cli = Cli::try_parse_from(["pair", "start", "my-session"]).unwrap();
        if let Some(Commands::Start { name, .. }) = cli.command {
            assert_eq!(name, Some("my-session".to_string()));
        } else {
            panic!("Expected Start command");
        }
    }
}
