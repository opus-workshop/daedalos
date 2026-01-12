//! remote - SSH and remote development for Daedalos
//!
//! Manage SSH connections and remote development environments.
//!
//! Commands:
//! - connect: Connect to a remote host
//! - add: Add a new remote host
//! - remove: Remove a remote host
//! - list: List configured hosts
//! - sync: Sync files to/from remote
//! - tunnel: Create SSH tunnel
//! - exec: Execute command on remote
//! - copy-id: Copy SSH key to remote
//! - status: Show status of all hosts
//! - dev: Start remote dev session

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use daedalos_core::Paths;
use remote::{
    host::{Host, HostStore},
    ssh::SshConnection,
    sync::{SyncDirection, SyncOptions, Syncer},
    tunnel::{Tunnel, TunnelConfig},
};
use std::io::{self, Write};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "remote")]
#[command(about = "SSH and remote development management for Daedalos")]
#[command(version)]
#[command(after_help = r#"WHEN TO USE:
    Managing SSH connections to dev servers, VMs, or cloud instances.
    Save host configs and sync files without remembering SSH options.

WORKFLOW:
    1. remote add prod --host 192.168.1.100 --user admin
    2. remote connect prod           # SSH in
    3. remote sync prod ./src        # Push local files
    4. remote sync prod --from       # Pull remote files

EXAMPLES:
    remote add myserver              # Interactive host setup
    remote connect myserver          # SSH to saved host
    remote connect user@host         # Direct SSH (not saved)
    remote sync myserver .           # Sync current dir to remote
    remote sync myserver --from      # Pull from remote
    remote tunnel myserver -L 8080 -R 3000  # Port forward
    remote exec myserver "ls -la"    # Run command remotely
    remote status                    # Check all hosts reachable

FILE SYNC:
    Uses rsync for efficient transfers. Respects .gitignore by default.
    --delete removes files on destination not in source.
    --dry-run shows what would change.

ALIASES:
    remote c, remote ssh    # connect
    remote s                # sync
    remote t                # tunnel
    remote e                # exec
    remote ls               # list
    remote st               # status
"#)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Connect to a remote host (alias: c, ssh)
    #[command(alias = "c", alias = "ssh")]
    Connect {
        /// Host name (from config) or direct SSH target
        host: String,
    },

    /// Add a new remote host (alias: a)
    #[command(alias = "a")]
    Add {
        /// Friendly name for the host
        name: String,

        /// Hostname or IP address
        #[arg(long)]
        host: Option<String>,

        /// SSH username
        #[arg(long, short)]
        user: Option<String>,

        /// SSH port
        #[arg(long, short, default_value = "22")]
        port: u16,

        /// Path to SSH key
        #[arg(long, short)]
        key: Option<String>,

        /// Default remote working directory
        #[arg(long)]
        path: Option<String>,
    },

    /// Remove a remote host (alias: rm, delete)
    #[command(alias = "rm", alias = "delete")]
    Remove {
        /// Host name to remove
        name: String,
    },

    /// List configured hosts (alias: ls)
    #[command(alias = "ls")]
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Edit host configuration
    Edit {
        /// Host name to edit (opens editor if not specified)
        name: Option<String>,
    },

    /// Sync files to/from remote (alias: s)
    #[command(alias = "s")]
    Sync {
        /// Host name
        host: String,

        /// Local path (default: current directory)
        #[arg(default_value = ".")]
        local_path: PathBuf,

        /// Remote path (uses host default if not specified)
        #[arg(long)]
        remote_path: Option<String>,

        /// Sync remote to local instead of local to remote
        #[arg(long, conflicts_with = "to")]
        from: bool,

        /// Sync local to remote (default)
        #[arg(long)]
        to: bool,

        /// Exclude patterns
        #[arg(long, short = 'x')]
        exclude: Vec<String>,

        /// Show what would be synced without doing it
        #[arg(long)]
        dry_run: bool,

        /// Delete files on destination not in source
        #[arg(long)]
        delete: bool,
    },

    /// Create SSH tunnel (alias: t)
    #[command(alias = "t")]
    Tunnel {
        /// Host name
        host: String,

        /// Local port to bind
        #[arg(long, short = 'L')]
        local: u16,

        /// Remote port to connect to
        #[arg(long, short = 'R')]
        remote: u16,

        /// Create reverse tunnel (remote -> local)
        #[arg(long)]
        reverse: bool,

        /// Run in background
        #[arg(long, short)]
        background: bool,
    },

    /// Execute command on remote (alias: e)
    #[command(alias = "e")]
    Exec {
        /// Host name
        host: String,

        /// Command to execute
        command: Vec<String>,
    },

    /// Copy SSH key to remote (alias: copy)
    #[command(alias = "copy")]
    CopyId {
        /// Host name
        host: String,
    },

    /// Show status of all hosts (alias: st)
    #[command(alias = "st")]
    Status,

    /// Start remote dev session (alias: d)
    #[command(alias = "d")]
    Dev {
        /// Host name
        host: String,
    },

    /// Interactive shell with tools (alias: sh)
    #[command(alias = "sh")]
    Shell {
        /// Host name
        host: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let paths = Paths::new();
    let config_dir = paths.config.join("remote");
    let store = HostStore::new(&config_dir)?;

    match cli.command {
        Some(Commands::Connect { host }) => cmd_connect(&store, &host),
        Some(Commands::Add { name, host, user, port, key, path }) => {
            cmd_add(&store, &name, host, user, port, key, path)
        }
        Some(Commands::Remove { name }) => cmd_remove(&store, &name),
        Some(Commands::List { json }) => cmd_list(&store, json),
        Some(Commands::Edit { name }) => cmd_edit(&store, name, &config_dir),
        Some(Commands::Sync { host, local_path, remote_path, from, to: _, exclude, dry_run, delete }) => {
            cmd_sync(&store, &host, &local_path, remote_path, from, exclude, dry_run, delete)
        }
        Some(Commands::Tunnel { host, local, remote, reverse, background }) => {
            cmd_tunnel(&store, &host, local, remote, reverse, background)
        }
        Some(Commands::Exec { host, command }) => cmd_exec(&store, &host, &command),
        Some(Commands::CopyId { host }) => cmd_copy_id(&store, &host),
        Some(Commands::Status) => cmd_status(&store),
        Some(Commands::Dev { host }) => cmd_dev(&store, &host),
        Some(Commands::Shell { host }) => cmd_connect(&store, &host),
        None => cmd_list(&store, false),
    }
}

/// Connect to a remote host
fn cmd_connect(store: &HostStore, name: &str) -> Result<()> {
    let host = match store.get(name)? {
        Some(h) => h,
        None => {
            // Try direct SSH if not in config
            println!("info: Host '{}' not in config, connecting directly...", name);
            let status = std::process::Command::new("ssh")
                .arg(name)
                .status()
                .context("Failed to SSH")?;

            if !status.success() {
                bail!("SSH connection failed");
            }
            return Ok(());
        }
    };

    println!("info: Connecting to {}...", name);

    let conn = SshConnection::new(host);
    let status = conn.connect()?;

    // Update last connected
    let _ = store.update_last_connected(name);

    if !status.success() {
        bail!("SSH connection failed");
    }

    Ok(())
}

/// Add a new remote host
fn cmd_add(
    store: &HostStore,
    name: &str,
    host: Option<String>,
    user: Option<String>,
    port: u16,
    key: Option<String>,
    path: Option<String>,
) -> Result<()> {
    // If host or user not provided, prompt interactively
    let hostname = match host {
        Some(h) => h,
        None => {
            print!("Hostname or IP: ");
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            input.trim().to_string()
        }
    };

    let username = match user {
        Some(u) => u,
        None => {
            print!("Username: ");
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            input.trim().to_string()
        }
    };

    if hostname.is_empty() || username.is_empty() {
        bail!("Hostname and username are required");
    }

    let mut new_host = Host::new(name, &hostname, &username)
        .with_port(port);

    if let Some(k) = key {
        new_host = new_host.with_key(&k);
    }

    if let Some(p) = path {
        new_host = new_host.with_path(&p);
    }

    store.add(new_host)?;

    println!("success: Host added: {}", name);
    println!();
    println!("Connect with: remote connect {}", name);

    Ok(())
}

/// Remove a remote host
fn cmd_remove(store: &HostStore, name: &str) -> Result<()> {
    if store.remove(name)? {
        println!("success: Host removed: {}", name);
    } else {
        println!("warning: Host not found: {}", name);
    }
    Ok(())
}

/// List configured hosts
fn cmd_list(store: &HostStore, json: bool) -> Result<()> {
    let hosts = store.load()?;

    if json {
        let output = serde_json::to_string_pretty(&hosts)?;
        println!("{}", output);
        return Ok(());
    }

    if hosts.is_empty() {
        println!("No hosts configured");
        println!();
        println!("Add one with: remote add <name>");
        return Ok(());
    }

    println!("Remote Hosts");
    println!();

    for host in &hosts {
        println!("  {}", host.name);
        println!("    Address: {}", host.display_address());
        if let Some(ref path) = host.path {
            println!("    Path: {}", path);
        }
        if let Some(ref key) = host.key {
            println!("    Key: {}", key);
        }
        println!();
    }

    Ok(())
}

/// Edit host configuration
fn cmd_edit(_store: &HostStore, name: Option<String>, config_dir: &std::path::Path) -> Result<()> {
    let hosts_file = config_dir.join("hosts.json");

    if !hosts_file.exists() {
        // Create empty hosts file
        std::fs::write(&hosts_file, "[]")?;
    }

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());

    if let Some(ref host_name) = name {
        println!("info: Editing host: {}", host_name);
    }

    std::process::Command::new(&editor)
        .arg(&hosts_file)
        .status()
        .with_context(|| format!("Failed to open editor: {}", editor))?;

    Ok(())
}

/// Sync files to/from remote
fn cmd_sync(
    store: &HostStore,
    name: &str,
    local_path: &std::path::Path,
    remote_path: Option<String>,
    from_remote: bool,
    excludes: Vec<String>,
    dry_run: bool,
    delete: bool,
) -> Result<()> {
    let host = store.get(name)?
        .ok_or_else(|| anyhow::anyhow!("Host not found: {}", name))?;

    // Determine remote path
    let remote = remote_path
        .or_else(|| host.path.clone())
        .unwrap_or_else(|| "~/".to_string());

    let direction = if from_remote {
        SyncDirection::FromRemote
    } else {
        SyncDirection::ToRemote
    };

    let options = SyncOptions {
        direction,
        excludes,
        dry_run,
        delete,
        progress: true,
    };

    let syncer = Syncer::new(host.clone());

    match direction {
        SyncDirection::ToRemote => {
            println!("info: Syncing {} -> {}:{}", local_path.display(), name, remote);
        }
        SyncDirection::FromRemote => {
            println!("info: Syncing {}:{} -> {}", name, remote, local_path.display());
        }
    }

    if dry_run {
        println!("info: Dry run - no changes will be made");
    }

    let status = syncer.sync(local_path, &remote, &options)?;

    if status.success() {
        println!("success: Sync complete");
    } else {
        bail!("Sync failed");
    }

    Ok(())
}

/// Create SSH tunnel
fn cmd_tunnel(
    store: &HostStore,
    name: &str,
    local_port: u16,
    remote_port: u16,
    reverse: bool,
    background: bool,
) -> Result<()> {
    let host = store.get(name)?
        .ok_or_else(|| anyhow::anyhow!("Host not found: {}", name))?;

    let config = if reverse {
        TunnelConfig::reverse(local_port, remote_port)
    } else {
        TunnelConfig::forward(local_port, remote_port)
    }.background(background);

    let tunnel = Tunnel::new(host, config);

    if reverse {
        println!("info: Creating reverse tunnel: remote:{} -> local:{}", remote_port, local_port);
    } else {
        println!("info: Creating tunnel: local:{} -> remote:{}", local_port, remote_port);
    }

    let status = tunnel.start()?;

    if !status.success() {
        bail!("Tunnel failed");
    }

    Ok(())
}

/// Execute command on remote
fn cmd_exec(store: &HostStore, name: &str, command: &[String]) -> Result<()> {
    if command.is_empty() {
        bail!("Command required");
    }

    let remote_cmd = command.join(" ");

    let host = match store.get(name)? {
        Some(h) => h,
        None => {
            // Try direct SSH
            let status = std::process::Command::new("ssh")
                .arg(name)
                .arg(&remote_cmd)
                .status()
                .context("Failed to execute command")?;

            if !status.success() {
                bail!("Command execution failed");
            }
            return Ok(());
        }
    };

    let conn = SshConnection::new(host);
    let status = conn.exec(&remote_cmd)?;

    if !status.success() {
        bail!("Command execution failed");
    }

    Ok(())
}

/// Copy SSH key to remote
fn cmd_copy_id(store: &HostStore, name: &str) -> Result<()> {
    let host = match store.get(name)? {
        Some(h) => h,
        None => {
            // Try direct ssh-copy-id
            let status = std::process::Command::new("ssh-copy-id")
                .arg(name)
                .status()
                .context("Failed to copy SSH key")?;

            if status.success() {
                println!("success: SSH key copied to {}", name);
            }
            return Ok(());
        }
    };

    let conn = SshConnection::new(host);
    let status = conn.copy_id()?;

    if status.success() {
        println!("success: SSH key copied to {}", name);
    } else {
        bail!("Failed to copy SSH key");
    }

    Ok(())
}

/// Show status of all hosts
fn cmd_status(store: &HostStore) -> Result<()> {
    let hosts = store.load()?;

    if hosts.is_empty() {
        println!("No hosts configured");
        return Ok(());
    }

    println!("Remote Host Status");
    println!();

    for host in &hosts {
        let conn = SshConnection::new(host.clone());
        let reachable = conn.check_reachable();

        let status_icon = if reachable { "+" } else { "-" };
        let status_text = if reachable { "" } else { " - unreachable" };

        println!("  {} {} ({}){}", status_icon, host.name, host.display_address(), status_text);
    }

    Ok(())
}

/// Start remote dev session
fn cmd_dev(store: &HostStore, name: &str) -> Result<()> {
    let host = store.get(name)?
        .ok_or_else(|| anyhow::anyhow!("Host not found: {}", name))?;

    println!("info: Starting dev session on {}...", name);

    let conn = SshConnection::new(host);
    let status = conn.dev_session()?;

    // Update last connected
    let _ = store.update_last_connected(name);

    if !status.success() {
        bail!("Dev session failed");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
    }
}
