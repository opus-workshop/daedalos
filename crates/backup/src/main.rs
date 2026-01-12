//! backup - Project backup CLI for Daedalos
//!
//! Create and manage project backups with optional encryption.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use backup::{BackupManager, BackupType, format_size};

#[derive(Parser)]
#[command(name = "backup")]
#[command(about = "Project backup with optional encryption for Daedalos")]
#[command(version)]
#[command(after_help = r#"WHEN TO USE:
    Before major refactors, experiments, or risky operations.
    More comprehensive than 'undo checkpoint' - captures entire project state.

BACKUP TYPES:
    full          Complete project snapshot (default)
    incremental   Only changed files since last backup
    git           Git bundle (includes history)

EXAMPLES:
    backup create                   # Backup current directory
    backup create ~/myproject       # Backup specific path
    backup create --encrypt         # Backup with age encryption
    backup create -t git            # Create git bundle
    backup list                     # Show all backups
    backup restore myproject-2024   # Restore from backup
    backup prune --keep 5           # Keep only 5 most recent

ENCRYPTION:
    Uses age encryption. Key stored in ~/.config/daedalos/backup/key.age.
    Encrypted backups can be safely stored on untrusted storage.

STORAGE:
    Backups stored in ~/.local/share/daedalos/backup/.
    Export to move backups between machines.

COMPARISON WITH UNDO:
    backup: Complete project snapshots, portable, encrypted
    undo: Fast file-level changes, local only, automatic
    Use backup for major milestones, undo for frequent saves.
"#)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a backup of a project
    Create {
        /// Path to backup (default: current directory)
        path: Option<PathBuf>,

        /// Custom backup name
        #[arg(short, long)]
        name: Option<String>,

        /// Backup type: full, incremental, git
        #[arg(short = 't', long, default_value = "full")]
        backup_type: String,

        /// Compress the backup (gzip)
        #[arg(long, default_value = "true")]
        compress: bool,

        /// Don't compress the backup
        #[arg(long)]
        no_compress: bool,

        /// Encrypt with age
        #[arg(long)]
        encrypt: bool,

        /// Exclude files matching pattern (can be repeated)
        #[arg(long, action = clap::ArgAction::Append)]
        exclude: Vec<String>,
    },

    /// Restore from a backup
    Restore {
        /// Backup name to restore
        backup: String,

        /// Target directory (default: project name)
        target: Option<PathBuf>,

        /// Overwrite existing files
        #[arg(long)]
        force: bool,
    },

    /// List available backups
    List {
        /// Filter by project name
        #[arg(long)]
        project: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show backup details
    Show {
        /// Backup name
        name: String,
    },

    /// Delete a backup
    Delete {
        /// Backup name
        name: String,

        /// Don't ask for confirmation
        #[arg(long)]
        force: bool,
    },

    /// Remove old backups, keeping the most recent
    Prune {
        /// Number of backups to keep per project
        #[arg(long, default_value = "10")]
        keep: usize,

        /// Only affect a specific project
        #[arg(long)]
        project: Option<String>,

        /// Show what would be deleted without deleting
        #[arg(long)]
        dry_run: bool,
    },

    /// Export a backup to a file
    Export {
        /// Backup name
        name: String,

        /// Output file path
        output: Option<PathBuf>,
    },

    /// Import a backup from a file
    Import {
        /// Backup file to import
        file: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let manager = BackupManager::new()
        .context("Failed to initialize backup manager")?;

    match cli.command {
        Some(Commands::Create {
            path,
            name,
            backup_type,
            compress,
            no_compress,
            encrypt,
            exclude,
        }) => {
            cmd_create(
                &manager,
                path,
                name,
                &backup_type,
                compress && !no_compress,
                encrypt,
                exclude,
            )
        }
        Some(Commands::Restore { backup, target, force }) => {
            cmd_restore(&manager, &backup, target, force)
        }
        Some(Commands::List { project, json }) => {
            cmd_list(&manager, project, json)
        }
        Some(Commands::Show { name }) => {
            cmd_show(&manager, &name)
        }
        Some(Commands::Delete { name, force }) => {
            cmd_delete(&manager, &name, force)
        }
        Some(Commands::Prune { keep, project, dry_run }) => {
            cmd_prune(&manager, keep, project, dry_run)
        }
        Some(Commands::Export { name, output }) => {
            cmd_export(&manager, &name, output)
        }
        Some(Commands::Import { file }) => {
            cmd_import(&manager, &file)
        }
        None => {
            // Default: list backups
            cmd_list(&manager, None, false)
        }
    }
}

fn cmd_create(
    manager: &BackupManager,
    path: Option<PathBuf>,
    name: Option<String>,
    backup_type: &str,
    compress: bool,
    encrypt: bool,
    excludes: Vec<String>,
) -> Result<()> {
    let source = path.unwrap_or_else(|| std::env::current_dir().unwrap());
    let source = BackupManager::get_project_root(&source);

    let project = source
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".to_string());

    let bt = BackupType::from_str(backup_type)
        .ok_or_else(|| anyhow::anyhow!("Invalid backup type: {}", backup_type))?;

    println!("Creating Backup");
    println!("  Project: {}", project);
    println!("  Path: {}", source.display());
    println!("  Type: {}", bt.as_str());
    println!();

    let meta = manager.create(
        &source,
        name.as_deref(),
        bt,
        compress,
        encrypt,
        &excludes,
    )?;

    println!("Backup created: {}", meta.name);
    println!("  File: {}", manager.backup_dir().join(&meta.name).display());
    println!("  Size: {}", format_size(meta.size));

    Ok(())
}

fn cmd_restore(
    manager: &BackupManager,
    name: &str,
    target: Option<PathBuf>,
    force: bool,
) -> Result<()> {
    let meta = manager.get(name)?
        .ok_or_else(|| anyhow::anyhow!("Backup not found: {}", name))?;

    println!("Restoring Backup");
    println!("  Backup: {}", name);
    println!("  Target: {}", target.as_ref().map(|p| p.display().to_string()).unwrap_or(meta.project.clone()));
    println!();

    let restored_path = manager.restore(name, target.as_deref(), force)?;

    println!("Backup restored to: {}", restored_path.display());

    Ok(())
}

fn cmd_list(manager: &BackupManager, project: Option<String>, json: bool) -> Result<()> {
    let backups = manager.list(project.as_deref())?;

    if json {
        println!("{}", serde_json::to_string_pretty(&backups)?);
        return Ok(());
    }

    if backups.is_empty() {
        println!("No backups found");
        return Ok(());
    }

    println!("Available Backups");
    println!();

    for backup in &backups {
        println!("  {}", backup.name);
        println!("    Project: {}", backup.project);
        println!("    Type: {} | Size: {}", backup.backup_type.as_str(), format_size(backup.size));
        println!("    Created: {}", backup.created_at);
        println!();
    }

    Ok(())
}

fn cmd_show(manager: &BackupManager, name: &str) -> Result<()> {
    let meta = manager.get(name)?
        .ok_or_else(|| anyhow::anyhow!("Backup not found: {}", name))?;

    println!("Backup Details: {}", name);
    println!();
    println!("{}", serde_json::to_string_pretty(&meta)?);

    Ok(())
}

fn cmd_delete(manager: &BackupManager, name: &str, force: bool) -> Result<()> {
    // Check if backup exists
    let meta = manager.get(name)?
        .ok_or_else(|| anyhow::anyhow!("Backup not found: {}", name))?;

    if !force {
        println!("Delete backup: {} ({})?", meta.name, format_size(meta.size));
        print!("[y/N] ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled");
            return Ok(());
        }
    }

    manager.delete(name)?;
    println!("Backup deleted: {}", name);

    Ok(())
}

fn cmd_prune(
    manager: &BackupManager,
    keep: usize,
    project: Option<String>,
    dry_run: bool,
) -> Result<()> {
    println!("Pruning Backups");
    println!("  Keep: {} most recent", keep);
    if let Some(ref p) = project {
        println!("  Project: {}", p);
    }
    println!();

    let pruned = manager.prune(keep, project.as_deref(), dry_run)?;

    if pruned.is_empty() {
        println!("Nothing to prune");
    } else if dry_run {
        println!("Would delete {} backups:", pruned.len());
        for name in &pruned {
            println!("  {}", name);
        }
    } else {
        println!("Deleted {} backups:", pruned.len());
        for name in &pruned {
            println!("  {}", name);
        }
    }

    Ok(())
}

fn cmd_export(manager: &BackupManager, name: &str, output: Option<PathBuf>) -> Result<()> {
    let backup_file = manager.find_backup_file(name)
        .ok_or_else(|| anyhow::anyhow!("Backup not found: {}", name))?;

    let output = output.unwrap_or_else(|| {
        backup_file.file_name().unwrap().into()
    });

    manager.export(name, &output)?;
    println!("Exported to: {}", output.display());

    Ok(())
}

fn cmd_import(manager: &BackupManager, file: &PathBuf) -> Result<()> {
    if !file.exists() {
        anyhow::bail!("File not found: {}", file.display());
    }

    let imported = manager.import(file)?;
    println!("Imported: {}", imported);

    Ok(())
}
