//! container - Docker/Podman management for Daedalos
//!
//! Unified interface for container operations.
//!
//! Commands:
//! - status: Show container runtime status
//! - ps: List running containers
//! - images: List images
//! - run: Run a container interactively
//! - exec: Execute command in container
//! - logs: Show container logs
//! - stop: Stop a container
//! - rm: Remove a container
//! - build: Build image from Dockerfile
//! - dev: Start development container
//! - clean: Remove stopped containers and unused images
//! - compose: Docker/Podman compose wrapper

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use container::{
    BuildOptions, ContainerManager, RunOptions, Runtime,
    detect_project_image,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "container")]
#[command(about = "Docker/Podman management for Daedalos - unified interface for container operations")]
#[command(version = VERSION)]
#[command(after_help = r#"WHEN TO USE:
    Managing Docker/Podman containers without remembering which runtime.
    Auto-detects Docker or Podman and uses consistent commands.

RUNTIME DETECTION:
    Prefers Docker if available, falls back to Podman.
    Use --runtime=podman or --runtime=docker to force one.

EXAMPLES:
    container status            # Check runtime and daemon
    container ps                # List running containers
    container run ubuntu        # Run interactive ubuntu
    container dev               # Start dev container (auto-detects image)
    container build -t myapp    # Build image from Dockerfile
    container exec myapp bash   # Shell into running container
    container logs -f myapp     # Follow container logs
    container up                # docker-compose up -d
    container down              # docker-compose down
    container clean             # Remove stopped containers + dangling images

DEV CONTAINERS:
    'container dev' mounts current directory and auto-detects project type:
    - Python projects use python:3.11
    - Node projects use node:20
    - Rust projects use rust:latest

ALIASES:
    container ps, container ls  # list containers
    container r                 # run
    container e                 # exec
    container l                 # logs
    container b                 # build
    container d                 # dev
    container c                 # compose
"#)]
struct Cli {
    /// Force a specific runtime (docker or podman)
    #[arg(long, global = true)]
    runtime: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Show container runtime status
    Status,

    /// List containers
    #[command(alias = "list", alias = "ls")]
    Ps {
        /// Show all containers (including stopped)
        #[arg(short, long)]
        all: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// List images
    #[command(alias = "img")]
    Images {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Run a container
    #[command(alias = "r")]
    Run {
        /// Image to run
        image: String,

        /// Container name
        #[arg(long)]
        name: Option<String>,

        /// Mount volume (format: host:container)
        #[arg(short = 'v', long = "mount")]
        mounts: Vec<String>,

        /// Port mapping (format: host:container)
        #[arg(short, long = "port")]
        ports: Vec<String>,

        /// Environment variable (format: KEY=VALUE)
        #[arg(short, long = "env")]
        envs: Vec<String>,

        /// Run in detached mode
        #[arg(short, long)]
        detach: bool,

        /// Command to run in the container
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },

    /// Execute command in a running container
    #[command(alias = "e")]
    Exec {
        /// Container name or ID
        container: String,

        /// Command to execute (default: /bin/bash)
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },

    /// Show container logs
    #[command(alias = "l")]
    Logs {
        /// Container name or ID
        container: String,

        /// Follow log output
        #[arg(short, long)]
        follow: bool,

        /// Number of lines to show from end
        #[arg(long)]
        tail: Option<u32>,
    },

    /// Stop a container
    Stop {
        /// Container name or ID
        container: String,

        /// Timeout in seconds
        #[arg(short, long)]
        timeout: Option<u32>,
    },

    /// Remove a container
    #[command(alias = "remove")]
    Rm {
        /// Container name or ID
        container: String,

        /// Force removal
        #[arg(short, long)]
        force: bool,
    },

    /// Build an image from Dockerfile
    #[command(alias = "b")]
    Build {
        /// Path to build context (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Image tag
        #[arg(short, long)]
        tag: Option<String>,

        /// Dockerfile path
        #[arg(short, long)]
        file: Option<String>,

        /// Build without cache
        #[arg(long)]
        no_cache: bool,
    },

    /// Start a development container
    #[command(alias = "d")]
    Dev {
        /// Image to use (auto-detected if not specified)
        image: Option<String>,

        /// Container name
        #[arg(long)]
        name: Option<String>,

        /// Mount volume (format: host:container)
        #[arg(short = 'v', long = "mount")]
        mounts: Vec<String>,

        /// Port mapping (format: host:container)
        #[arg(short, long = "port")]
        ports: Vec<String>,

        /// Shell to use
        #[arg(long, default_value = "/bin/bash")]
        shell: String,
    },

    /// Remove stopped containers and unused images
    #[command(alias = "prune")]
    Clean {
        /// Show what would be removed without removing
        #[arg(long)]
        dry_run: bool,
    },

    /// Run compose command
    #[command(alias = "c")]
    Compose {
        /// Compose arguments
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },

    /// Compose up -d shortcut
    Up {
        /// Additional arguments
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },

    /// Compose down shortcut
    Down {
        /// Additional arguments
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },

    /// Compose restart shortcut
    Restart {
        /// Additional arguments
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
}

// Colors for terminal output
#[allow(dead_code)]
struct Colors {
    red: &'static str,
    green: &'static str,
    yellow: &'static str,
    blue: &'static str,
    cyan: &'static str,
    bold: &'static str,
    dim: &'static str,
    reset: &'static str,
}

impl Colors {
    fn new() -> Self {
        if atty::is(atty::Stream::Stdout) {
            Self {
                red: "\x1b[0;31m",
                green: "\x1b[0;32m",
                yellow: "\x1b[0;33m",
                blue: "\x1b[0;34m",
                cyan: "\x1b[0;36m",
                bold: "\x1b[1m",
                dim: "\x1b[2m",
                reset: "\x1b[0m",
            }
        } else {
            Self {
                red: "",
                green: "",
                yellow: "",
                blue: "",
                cyan: "",
                bold: "",
                dim: "",
                reset: "",
            }
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let colors = Colors::new();

    // Determine runtime
    let manager = if let Some(rt) = &cli.runtime {
        let runtime = match rt.to_lowercase().as_str() {
            "docker" => Runtime::Docker,
            "podman" => Runtime::Podman,
            _ => bail!("Unknown runtime: {}. Use 'docker' or 'podman'.", rt),
        };
        ContainerManager::with_runtime(runtime)?
    } else {
        match ContainerManager::new() {
            Ok(m) => m,
            Err(e) => {
                eprintln!("{}error:{} {}", colors.red, colors.reset, e);
                eprintln!();
                eprintln!("Install Docker or Podman:");
                eprintln!("  Docker: https://docs.docker.com/get-docker/");
                eprintln!("  Podman: brew install podman / apt install podman");
                std::process::exit(1);
            }
        }
    };

    match cli.command {
        Some(Commands::Status) => cmd_status(&manager, &colors),
        Some(Commands::Ps { all, json }) => cmd_ps(&manager, all, json, &colors),
        Some(Commands::Images { json }) => cmd_images(&manager, json, &colors),
        Some(Commands::Run { image, name, mounts, ports, envs, detach, command }) => {
            cmd_run(&manager, &image, name, mounts, ports, envs, detach, command, &colors)
        }
        Some(Commands::Exec { container, command }) => cmd_exec(&manager, &container, command, &colors),
        Some(Commands::Logs { container, follow, tail }) => cmd_logs(&manager, &container, follow, tail),
        Some(Commands::Stop { container, timeout }) => cmd_stop(&manager, &container, timeout, &colors),
        Some(Commands::Rm { container, force }) => cmd_rm(&manager, &container, force, &colors),
        Some(Commands::Build { path, tag, file, no_cache }) => cmd_build(&manager, &path, tag, file, no_cache, &colors),
        Some(Commands::Dev { image, name, mounts, ports, shell }) => {
            cmd_dev(&manager, image, name, mounts, ports, shell, &colors)
        }
        Some(Commands::Clean { dry_run }) => cmd_clean(&manager, dry_run, &colors),
        Some(Commands::Compose { args }) => cmd_compose(&manager, &args),
        Some(Commands::Up { args }) => {
            let mut full_args = vec!["up".to_string(), "-d".to_string()];
            full_args.extend(args);
            cmd_compose(&manager, &full_args)
        }
        Some(Commands::Down { args }) => {
            let mut full_args = vec!["down".to_string()];
            full_args.extend(args);
            cmd_compose(&manager, &full_args)
        }
        Some(Commands::Restart { args }) => {
            let mut full_args = vec!["restart".to_string()];
            full_args.extend(args);
            cmd_compose(&manager, &full_args)
        }
        None => cmd_status(&manager, &colors),
    }
}

fn cmd_status(manager: &ContainerManager, colors: &Colors) -> Result<()> {
    println!("{}Container Runtime Status{}", colors.bold, colors.reset);
    println!();

    let status = manager.status()?;

    println!("{}Runtime:{} {}", colors.cyan, colors.reset, status.runtime);
    println!("{}Version:{} {}", colors.cyan, colors.reset, status.version);
    println!();

    if status.running {
        println!("{}ok:{} Daemon is running", colors.green, colors.reset);
        println!();
        println!("{}Running containers:{} {}", colors.cyan, colors.reset, status.containers);
        println!("{}Images:{} {}", colors.cyan, colors.reset, status.images);
    } else {
        println!("{}error:{} Daemon is not running", colors.red, colors.reset);
        println!();
        println!("Start with:");
        if manager.runtime() == Runtime::Podman {
            println!("  podman machine start");
        } else {
            println!("  sudo systemctl start docker");
        }
    }

    Ok(())
}

fn cmd_ps(manager: &ContainerManager, all: bool, json: bool, colors: &Colors) -> Result<()> {
    let containers = manager.list_containers(all)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&containers)?);
        return Ok(());
    }

    println!("{}Containers{}", colors.bold, colors.reset);
    println!();

    if containers.is_empty() {
        println!("No containers running. Start one with: container run <IMAGE>");
        return Ok(());
    }

    // Table header
    println!(
        "{:<20} {:<25} {:<20} {:<20}",
        "NAME", "IMAGE", "STATUS", "PORTS"
    );
    println!("{}", "-".repeat(85));

    for c in &containers {
        // Truncate fields if too long
        let name = truncate(&c.name, 18);
        let image = truncate(&c.image, 23);
        let status = truncate(&c.status, 18);
        let ports = truncate(&c.ports, 18);

        println!("{:<20} {:<25} {:<20} {:<20}", name, image, status, ports);
    }

    Ok(())
}

fn cmd_images(manager: &ContainerManager, json: bool, colors: &Colors) -> Result<()> {
    let images = manager.list_images()?;

    if json {
        println!("{}", serde_json::to_string_pretty(&images)?);
        return Ok(());
    }

    println!("{}Images{}", colors.bold, colors.reset);
    println!();

    if images.is_empty() {
        println!("No images found.");
        return Ok(());
    }

    // Table header
    println!(
        "{:<30} {:<15} {:<12} {:<15}",
        "REPOSITORY", "TAG", "SIZE", "CREATED"
    );
    println!("{}", "-".repeat(75));

    for img in &images {
        let repo = truncate(&img.repository, 28);
        let tag = truncate(&img.tag, 13);
        let size = truncate(&img.size, 10);
        let created = truncate(&img.created, 13);

        println!("{:<30} {:<15} {:<12} {:<15}", repo, tag, size, created);
    }

    Ok(())
}

fn cmd_run(
    manager: &ContainerManager,
    image: &str,
    name: Option<String>,
    mounts: Vec<String>,
    ports: Vec<String>,
    envs: Vec<String>,
    detach: bool,
    command: Vec<String>,
    colors: &Colors,
) -> Result<()> {
    let options = RunOptions {
        name,
        mounts: parse_mount_args(&mounts),
        ports: parse_port_args(&ports),
        env: parse_env_args(&envs),
        detach,
        remove: !detach,
        interactive: !detach,
        workdir: None,
    };

    let cmd: Vec<&str> = command.iter().map(|s| s.as_str()).collect();
    let cmd_opt = if cmd.is_empty() { None } else { Some(cmd.as_slice()) };

    println!("{}info:{} Running container from image: {}", colors.blue, colors.reset, image);

    let output = manager.run(image, &options, cmd_opt)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to run container: {}", stderr);
    }

    if detach {
        let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        println!("{}ok:{} Started container: {}", colors.green, colors.reset, container_id);
    }

    Ok(())
}

fn cmd_exec(manager: &ContainerManager, container: &str, command: Vec<String>, _colors: &Colors) -> Result<()> {
    let cmd: Vec<&str> = if command.is_empty() {
        vec!["/bin/bash"]
    } else {
        command.iter().map(|s| s.as_str()).collect()
    };

    let output = manager.exec(container, &cmd, true)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to execute command: {}", stderr);
    }

    Ok(())
}

fn cmd_logs(manager: &ContainerManager, container: &str, follow: bool, tail: Option<u32>) -> Result<()> {
    let output = manager.logs(container, follow, tail)?;

    print!("{}", String::from_utf8_lossy(&output.stdout));
    eprint!("{}", String::from_utf8_lossy(&output.stderr));

    Ok(())
}

fn cmd_stop(manager: &ContainerManager, container: &str, timeout: Option<u32>, colors: &Colors) -> Result<()> {
    manager.stop(container, timeout)?;
    println!("{}ok:{} Stopped: {}", colors.green, colors.reset, container);
    Ok(())
}

fn cmd_rm(manager: &ContainerManager, container: &str, force: bool, colors: &Colors) -> Result<()> {
    manager.remove(container, force)?;
    println!("{}ok:{} Removed: {}", colors.green, colors.reset, container);
    Ok(())
}

fn cmd_build(
    manager: &ContainerManager,
    path: &PathBuf,
    tag: Option<String>,
    dockerfile: Option<String>,
    no_cache: bool,
    colors: &Colors,
) -> Result<()> {
    let final_tag = tag.unwrap_or_else(|| {
        let dir_name = std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_else(|| "image".to_string());
        format!("{}:latest", dir_name)
    });

    let options = BuildOptions {
        tag: Some(final_tag.clone()),
        dockerfile,
        no_cache,
    };

    println!("{}info:{} Building image: {}", colors.blue, colors.reset, final_tag);

    let output = manager.build(path, &options)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to build image: {}", stderr);
    }

    println!("{}ok:{} Built: {}", colors.green, colors.reset, final_tag);

    Ok(())
}

fn cmd_dev(
    manager: &ContainerManager,
    image: Option<String>,
    name: Option<String>,
    mounts: Vec<String>,
    ports: Vec<String>,
    shell: String,
    colors: &Colors,
) -> Result<()> {
    let cwd = std::env::current_dir().context("Could not determine current directory")?;

    // Auto-detect image if not specified
    let final_image = image.unwrap_or_else(|| {
        let detected = detect_project_image(&cwd);
        println!("{}info:{} Auto-detected image: {}", colors.blue, colors.reset, detected);
        detected
    });

    // Generate container name from directory
    let dir_name = cwd
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".to_string());
    let container_name = name.unwrap_or_else(|| format!("dev-{}", dir_name));

    // Check if container already exists
    let existing = manager.list_containers(true)?;
    if existing.iter().any(|c| c.name == container_name) {
        println!("{}info:{} Attaching to existing container: {}", colors.blue, colors.reset, container_name);

        // Start if not running
        let _ = std::process::Command::new(manager.runtime().command())
            .args(["start", &container_name])
            .output();

        // Exec into it
        let cmd = vec![shell.as_str()];
        manager.exec(&container_name, &cmd, true)?;
        return Ok(());
    }

    // Build mount options - default mount current directory
    let mut mount_opts = parse_mount_args(&mounts);
    if mount_opts.is_empty() {
        mount_opts.push((cwd.to_string_lossy().to_string(), "/workspace".to_string()));
    }

    let options = RunOptions {
        name: Some(container_name.clone()),
        mounts: mount_opts,
        ports: parse_port_args(&ports),
        env: vec![],
        detach: false,
        remove: false,
        interactive: true,
        workdir: Some("/workspace".to_string()),
    };

    println!("{}info:{} Starting dev container: {}", colors.blue, colors.reset, container_name);

    let cmd = vec![shell.as_str()];
    manager.run(&final_image, &options, Some(&cmd))?;

    Ok(())
}

fn cmd_clean(manager: &ContainerManager, dry_run: bool, colors: &Colors) -> Result<()> {
    println!("{}Cleaning up containers and images{}", colors.bold, colors.reset);
    println!();

    let result = manager.clean(dry_run)?;

    if dry_run {
        if result.containers_removed > 0 {
            println!("Would remove {} stopped containers", result.containers_removed);
        } else {
            println!("No stopped containers to remove");
        }

        if result.images_removed > 0 {
            println!("Would remove {} dangling images", result.images_removed);
        } else {
            println!("No dangling images to remove");
        }
    } else {
        if result.containers_removed > 0 {
            println!("{}ok:{} Removed {} stopped containers", colors.green, colors.reset, result.containers_removed);
        } else {
            println!("No stopped containers");
        }

        if result.images_removed > 0 {
            println!("{}ok:{} Removed {} dangling images", colors.green, colors.reset, result.images_removed);
        } else {
            println!("No dangling images");
        }
    }

    Ok(())
}

fn cmd_compose(manager: &ContainerManager, args: &[String]) -> Result<()> {
    let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let output = manager.compose(&args_refs)?;

    print!("{}", String::from_utf8_lossy(&output.stdout));
    eprint!("{}", String::from_utf8_lossy(&output.stderr));

    if !output.status.success() {
        std::process::exit(output.status.code().unwrap_or(1));
    }

    Ok(())
}

// Helper functions

fn parse_mount_args(mounts: &[String]) -> Vec<(String, String)> {
    mounts
        .iter()
        .filter_map(|m| {
            let parts: Vec<&str> = m.splitn(2, ':').collect();
            if parts.len() == 2 {
                Some((parts[0].to_string(), parts[1].to_string()))
            } else {
                None
            }
        })
        .collect()
}

fn parse_port_args(ports: &[String]) -> Vec<(String, String)> {
    ports
        .iter()
        .filter_map(|p| {
            let parts: Vec<&str> = p.splitn(2, ':').collect();
            if parts.len() == 2 {
                Some((parts[0].to_string(), parts[1].to_string()))
            } else {
                None
            }
        })
        .collect()
}

fn parse_env_args(envs: &[String]) -> Vec<(String, String)> {
    envs
        .iter()
        .filter_map(|e| {
            let parts: Vec<&str> = e.splitn(2, '=').collect();
            if parts.len() == 2 {
                Some((parts[0].to_string(), parts[1].to_string()))
            } else {
                None
            }
        })
        .collect()
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
