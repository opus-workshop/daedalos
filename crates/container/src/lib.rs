//! Container management library for Daedalos
//!
//! Provides a unified interface for Docker and Podman container operations.
//! Supports listing containers, managing images, running containers,
//! and compose operations.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::{Command, Output};
use thiserror::Error;
use which::which;

/// Errors specific to container operations
#[derive(Error, Debug)]
pub enum ContainerError {
    #[error("No container runtime found. Install Docker or Podman.")]
    NoRuntime,

    #[error("Container runtime daemon is not running")]
    DaemonNotRunning,

    #[error("Container not found: {0}")]
    ContainerNotFound(String),

    #[error("Image not found: {0}")]
    ImageNotFound(String),

    #[error("Container operation failed: {0}")]
    OperationFailed(String),

    #[error("Compose not available for runtime: {0}")]
    ComposeNotAvailable(String),
}

/// Container runtime type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Runtime {
    Docker,
    Podman,
}

impl Runtime {
    /// Get the command name for this runtime
    pub fn command(&self) -> &'static str {
        match self {
            Runtime::Docker => "docker",
            Runtime::Podman => "podman",
        }
    }

    /// Get the compose command for this runtime
    pub fn compose_command(&self) -> Result<String> {
        match self {
            Runtime::Docker => {
                // Check for docker compose (v2) first
                if Command::new("docker")
                    .args(["compose", "version"])
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false)
                {
                    return Ok("docker compose".to_string());
                }
                // Fall back to docker-compose (v1)
                if which("docker-compose").is_ok() {
                    return Ok("docker-compose".to_string());
                }
                bail!(ContainerError::ComposeNotAvailable("docker".to_string()));
            }
            Runtime::Podman => {
                if which("podman-compose").is_ok() {
                    return Ok("podman-compose".to_string());
                }
                bail!(ContainerError::ComposeNotAvailable("podman".to_string()));
            }
        }
    }

    /// Display name
    pub fn name(&self) -> &'static str {
        match self {
            Runtime::Docker => "Docker",
            Runtime::Podman => "Podman",
        }
    }
}

/// Container information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub ports: String,
    pub created: String,
}

/// Image information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    pub id: String,
    pub repository: String,
    pub tag: String,
    pub size: String,
    pub created: String,
}

/// Runtime status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeStatus {
    pub runtime: String,
    pub version: String,
    pub running: bool,
    pub containers: usize,
    pub images: usize,
}

/// Options for running a container
#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    pub name: Option<String>,
    pub mounts: Vec<(String, String)>,
    pub ports: Vec<(String, String)>,
    pub env: Vec<(String, String)>,
    pub detach: bool,
    pub remove: bool,
    pub workdir: Option<String>,
    pub interactive: bool,
}

/// Options for building an image
#[derive(Debug, Clone, Default)]
pub struct BuildOptions {
    pub tag: Option<String>,
    pub dockerfile: Option<String>,
    pub no_cache: bool,
}

/// Container manager - main interface for container operations
pub struct ContainerManager {
    runtime: Runtime,
}

impl ContainerManager {
    /// Create a new container manager with auto-detected runtime
    pub fn new() -> Result<Self> {
        let runtime = detect_runtime()?;
        Ok(Self { runtime })
    }

    /// Create a container manager with a specific runtime
    pub fn with_runtime(runtime: Runtime) -> Result<Self> {
        // Verify runtime is available
        if which(runtime.command()).is_err() {
            bail!(ContainerError::NoRuntime);
        }
        Ok(Self { runtime })
    }

    /// Get the current runtime
    pub fn runtime(&self) -> Runtime {
        self.runtime
    }

    /// Get runtime status
    pub fn status(&self) -> Result<RuntimeStatus> {
        let cmd = self.runtime.command();

        // Get version
        let version_output = Command::new(cmd)
            .args(["--version"])
            .output()
            .context("Failed to get runtime version")?;
        let version = String::from_utf8_lossy(&version_output.stdout)
            .trim()
            .to_string();

        // Check if daemon is running
        let info_result = Command::new(cmd).args(["info"]).output();
        let running = info_result
            .map(|o| o.status.success())
            .unwrap_or(false);

        // Count containers and images
        let containers = if running {
            self.list_containers(false)
                .map(|c| c.len())
                .unwrap_or(0)
        } else {
            0
        };

        let images = if running {
            self.list_images().map(|i| i.len()).unwrap_or(0)
        } else {
            0
        };

        Ok(RuntimeStatus {
            runtime: self.runtime.name().to_string(),
            version,
            running,
            containers,
            images,
        })
    }

    /// List containers
    pub fn list_containers(&self, all: bool) -> Result<Vec<ContainerInfo>> {
        let cmd = self.runtime.command();
        let mut args = vec!["ps", "--format", "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}\t{{.CreatedAt}}"];
        if all {
            args.insert(1, "-a");
        }

        let output = Command::new(cmd)
            .args(&args)
            .output()
            .context("Failed to list containers")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(ContainerError::OperationFailed(stderr.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let containers = stdout
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| {
                let parts: Vec<&str> = line.split('\t').collect();
                ContainerInfo {
                    id: parts.first().unwrap_or(&"").to_string(),
                    name: parts.get(1).unwrap_or(&"").to_string(),
                    image: parts.get(2).unwrap_or(&"").to_string(),
                    status: parts.get(3).unwrap_or(&"").to_string(),
                    ports: parts.get(4).unwrap_or(&"").to_string(),
                    created: parts.get(5).unwrap_or(&"").to_string(),
                }
            })
            .collect();

        Ok(containers)
    }

    /// List images
    pub fn list_images(&self) -> Result<Vec<ImageInfo>> {
        let cmd = self.runtime.command();
        let output = Command::new(cmd)
            .args(["images", "--format", "{{.ID}}\t{{.Repository}}\t{{.Tag}}\t{{.Size}}\t{{.CreatedSince}}"])
            .output()
            .context("Failed to list images")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(ContainerError::OperationFailed(stderr.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let images = stdout
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| {
                let parts: Vec<&str> = line.split('\t').collect();
                ImageInfo {
                    id: parts.first().unwrap_or(&"").to_string(),
                    repository: parts.get(1).unwrap_or(&"").to_string(),
                    tag: parts.get(2).unwrap_or(&"").to_string(),
                    size: parts.get(3).unwrap_or(&"").to_string(),
                    created: parts.get(4).unwrap_or(&"").to_string(),
                }
            })
            .collect();

        Ok(images)
    }

    /// Run a container
    pub fn run(&self, image: &str, options: &RunOptions, command: Option<&[&str]>) -> Result<Output> {
        let cmd = self.runtime.command();
        let mut args = vec!["run".to_string()];

        // Add options
        if let Some(ref name) = options.name {
            args.push("--name".to_string());
            args.push(name.clone());
        }

        for (src, dst) in &options.mounts {
            args.push("-v".to_string());
            args.push(format!("{}:{}", src, dst));
        }

        for (host, container) in &options.ports {
            args.push("-p".to_string());
            args.push(format!("{}:{}", host, container));
        }

        for (key, value) in &options.env {
            args.push("-e".to_string());
            args.push(format!("{}={}", key, value));
        }

        if let Some(ref workdir) = options.workdir {
            args.push("-w".to_string());
            args.push(workdir.clone());
        }

        if options.detach {
            args.push("-d".to_string());
        }

        if options.interactive && !options.detach {
            args.push("-it".to_string());
        }

        if options.remove {
            args.push("--rm".to_string());
        }

        args.push(image.to_string());

        // Add command if provided
        if let Some(cmd_args) = command {
            for arg in cmd_args {
                args.push(arg.to_string());
            }
        }

        let output = Command::new(cmd)
            .args(&args)
            .output()
            .context("Failed to run container")?;

        Ok(output)
    }

    /// Execute a command in a running container
    pub fn exec(&self, container: &str, command: &[&str], interactive: bool) -> Result<Output> {
        let cmd = self.runtime.command();
        let mut args = vec!["exec"];

        if interactive {
            args.push("-it");
        }

        args.push(container);
        args.extend(command);

        let output = Command::new(cmd)
            .args(&args)
            .output()
            .context("Failed to execute command in container")?;

        Ok(output)
    }

    /// Get container logs
    pub fn logs(&self, container: &str, follow: bool, tail: Option<u32>) -> Result<Output> {
        let cmd = self.runtime.command();
        let mut args = vec!["logs".to_string()];

        if follow {
            args.push("-f".to_string());
        }

        if let Some(n) = tail {
            args.push("--tail".to_string());
            args.push(n.to_string());
        }

        args.push(container.to_string());

        let output = Command::new(cmd)
            .args(&args)
            .output()
            .context("Failed to get container logs")?;

        Ok(output)
    }

    /// Stop a container
    pub fn stop(&self, container: &str, timeout: Option<u32>) -> Result<()> {
        let cmd = self.runtime.command();
        let mut args = vec!["stop".to_string()];

        if let Some(t) = timeout {
            args.push("-t".to_string());
            args.push(t.to_string());
        }

        args.push(container.to_string());

        let output = Command::new(cmd)
            .args(&args)
            .output()
            .context("Failed to stop container")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(ContainerError::OperationFailed(stderr.to_string()));
        }

        Ok(())
    }

    /// Remove a container
    pub fn remove(&self, container: &str, force: bool) -> Result<()> {
        let cmd = self.runtime.command();
        let mut args = vec!["rm"];

        if force {
            args.push("-f");
        }

        args.push(container);

        let output = Command::new(cmd)
            .args(&args)
            .output()
            .context("Failed to remove container")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(ContainerError::OperationFailed(stderr.to_string()));
        }

        Ok(())
    }

    /// Build an image from a Dockerfile
    pub fn build(&self, path: &Path, options: &BuildOptions) -> Result<Output> {
        let cmd = self.runtime.command();
        let mut args = vec!["build".to_string()];

        if let Some(ref tag) = options.tag {
            args.push("-t".to_string());
            args.push(tag.clone());
        }

        if let Some(ref dockerfile) = options.dockerfile {
            args.push("-f".to_string());
            args.push(dockerfile.clone());
        }

        if options.no_cache {
            args.push("--no-cache".to_string());
        }

        args.push(path.to_string_lossy().to_string());

        let output = Command::new(cmd)
            .args(&args)
            .output()
            .context("Failed to build image")?;

        Ok(output)
    }

    /// Clean up stopped containers and dangling images
    pub fn clean(&self, dry_run: bool) -> Result<CleanResult> {
        let cmd = self.runtime.command();

        // Count stopped containers
        let stopped_output = Command::new(cmd)
            .args(["ps", "-a", "-q", "-f", "status=exited"])
            .output()
            .context("Failed to list stopped containers")?;
        let stopped_count = String::from_utf8_lossy(&stopped_output.stdout)
            .lines()
            .filter(|l| !l.is_empty())
            .count();

        // Count dangling images
        let dangling_output = Command::new(cmd)
            .args(["images", "-q", "-f", "dangling=true"])
            .output()
            .context("Failed to list dangling images")?;
        let dangling_count = String::from_utf8_lossy(&dangling_output.stdout)
            .lines()
            .filter(|l| !l.is_empty())
            .count();

        if dry_run {
            return Ok(CleanResult {
                containers_removed: stopped_count,
                images_removed: dangling_count,
                dry_run: true,
            });
        }

        // Prune containers
        if stopped_count > 0 {
            let _ = Command::new(cmd)
                .args(["container", "prune", "-f"])
                .output();
        }

        // Prune images
        if dangling_count > 0 {
            let _ = Command::new(cmd)
                .args(["image", "prune", "-f"])
                .output();
        }

        Ok(CleanResult {
            containers_removed: stopped_count,
            images_removed: dangling_count,
            dry_run: false,
        })
    }

    /// Run a compose command
    pub fn compose(&self, args: &[&str]) -> Result<Output> {
        let compose_cmd = self.runtime.compose_command()?;

        let output = if compose_cmd.contains(' ') {
            // Handle "docker compose" (two words)
            let parts: Vec<&str> = compose_cmd.split_whitespace().collect();
            Command::new(parts[0])
                .args(&parts[1..])
                .args(args)
                .output()
        } else {
            Command::new(&compose_cmd).args(args).output()
        }
        .context("Failed to run compose command")?;

        Ok(output)
    }
}

/// Result of a clean operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanResult {
    pub containers_removed: usize,
    pub images_removed: usize,
    pub dry_run: bool,
}

/// Detect the available container runtime
pub fn detect_runtime() -> Result<Runtime> {
    // Prefer Podman over Docker (more Linux-native)
    if which("podman").is_ok() {
        return Ok(Runtime::Podman);
    }

    if which("docker").is_ok() {
        return Ok(Runtime::Docker);
    }

    bail!(ContainerError::NoRuntime);
}

/// Auto-detect the best image for a project based on files present
pub fn detect_project_image(path: &Path) -> String {
    if path.join("package.json").exists() {
        "node:20".to_string()
    } else if path.join("requirements.txt").exists() || path.join("pyproject.toml").exists() {
        "python:3.11".to_string()
    } else if path.join("Cargo.toml").exists() {
        "rust:latest".to_string()
    } else if path.join("go.mod").exists() {
        "golang:latest".to_string()
    } else if path.join("Gemfile").exists() {
        "ruby:latest".to_string()
    } else if path.join("pom.xml").exists() || path.join("build.gradle").exists() {
        "openjdk:21".to_string()
    } else {
        "ubuntu:latest".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_command() {
        assert_eq!(Runtime::Docker.command(), "docker");
        assert_eq!(Runtime::Podman.command(), "podman");
    }

    #[test]
    fn test_runtime_name() {
        assert_eq!(Runtime::Docker.name(), "Docker");
        assert_eq!(Runtime::Podman.name(), "Podman");
    }

    #[test]
    fn test_detect_project_image() {
        use std::env::temp_dir;
        use std::fs::{self, File};

        let temp = temp_dir().join("container_test_detect");
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).unwrap();

        // Default to ubuntu
        assert_eq!(detect_project_image(&temp), "ubuntu:latest");

        // Node project
        File::create(temp.join("package.json")).unwrap();
        assert_eq!(detect_project_image(&temp), "node:20");

        // Cleanup
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_run_options_default() {
        let opts = RunOptions::default();
        assert!(opts.name.is_none());
        assert!(opts.mounts.is_empty());
        assert!(opts.ports.is_empty());
        assert!(opts.env.is_empty());
        assert!(!opts.detach);
        assert!(!opts.remove);
        assert!(!opts.interactive);
    }

    #[test]
    fn test_build_options_default() {
        let opts = BuildOptions::default();
        assert!(opts.tag.is_none());
        assert!(opts.dockerfile.is_none());
        assert!(!opts.no_cache);
    }
}
