//! Pipeline definitions for different project types

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::detect::ProjectType;

/// A verification pipeline containing steps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pipeline {
    /// Pipeline description
    pub description: String,
    /// Verification steps
    pub steps: Vec<PipelineStep>,
}

/// A single verification step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStep {
    /// Step name
    pub name: String,
    /// Command to run
    pub command: String,
    /// Command to run in fix mode (optional)
    #[serde(default)]
    pub fix_command: Option<String>,
    /// Timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    /// Whether this step runs in quick mode
    #[serde(default)]
    pub quick: bool,
}

fn default_timeout() -> u64 {
    60
}

/// Load pipeline for a project type
pub fn load_pipeline(project_type: &ProjectType) -> Result<Pipeline> {
    match project_type {
        ProjectType::Rust => Ok(rust_pipeline()),
        ProjectType::Python => Ok(python_pipeline()),
        ProjectType::TypeScript => Ok(typescript_pipeline()),
        ProjectType::JavaScript => Ok(javascript_pipeline()),
        ProjectType::Go => Ok(go_pipeline()),
        ProjectType::Swift => Ok(swift_pipeline()),
        ProjectType::Shell => Ok(shell_pipeline()),
        ProjectType::Unknown => anyhow::bail!("No pipeline for unknown project type"),
    }
}

/// Rust pipeline
fn rust_pipeline() -> Pipeline {
    Pipeline {
        description: "Rust/Cargo projects".to_string(),
        steps: vec![
            PipelineStep {
                name: "lint".to_string(),
                command: "cargo clippy -- -D warnings".to_string(),
                fix_command: Some("cargo clippy --fix --allow-dirty".to_string()),
                timeout: 120,
                quick: true,
            },
            PipelineStep {
                name: "format".to_string(),
                command: "cargo fmt --check".to_string(),
                fix_command: Some("cargo fmt".to_string()),
                timeout: 30,
                quick: true,
            },
            PipelineStep {
                name: "check".to_string(),
                command: "cargo check".to_string(),
                fix_command: None,
                timeout: 120,
                quick: true,
            },
            PipelineStep {
                name: "build".to_string(),
                command: "cargo build".to_string(),
                fix_command: None,
                timeout: 300,
                quick: false,
            },
            PipelineStep {
                name: "test".to_string(),
                command: "cargo test".to_string(),
                fix_command: None,
                timeout: 600,
                quick: false,
            },
        ],
    }
}

/// Python pipeline
fn python_pipeline() -> Pipeline {
    Pipeline {
        description: "Python projects".to_string(),
        steps: vec![
            PipelineStep {
                name: "lint".to_string(),
                command: "ruff check .".to_string(),
                fix_command: Some("ruff check . --fix".to_string()),
                timeout: 30,
                quick: true,
            },
            PipelineStep {
                name: "format".to_string(),
                command: "ruff format --check .".to_string(),
                fix_command: Some("ruff format .".to_string()),
                timeout: 30,
                quick: true,
            },
            PipelineStep {
                name: "types".to_string(),
                command: "mypy .".to_string(),
                fix_command: None,
                timeout: 120,
                quick: true,
            },
            PipelineStep {
                name: "test".to_string(),
                command: "pytest".to_string(),
                fix_command: None,
                timeout: 600,
                quick: false,
            },
        ],
    }
}

/// TypeScript pipeline
fn typescript_pipeline() -> Pipeline {
    Pipeline {
        description: "TypeScript/Node.js projects".to_string(),
        steps: vec![
            PipelineStep {
                name: "lint".to_string(),
                command: "npx eslint . --max-warnings 0".to_string(),
                fix_command: Some("npx eslint . --fix".to_string()),
                timeout: 60,
                quick: true,
            },
            PipelineStep {
                name: "format".to_string(),
                command: "npx prettier --check .".to_string(),
                fix_command: Some("npx prettier --write .".to_string()),
                timeout: 30,
                quick: true,
            },
            PipelineStep {
                name: "types".to_string(),
                command: "npx tsc --noEmit".to_string(),
                fix_command: None,
                timeout: 120,
                quick: true,
            },
            PipelineStep {
                name: "build".to_string(),
                command: "<pm> run build".to_string(),
                fix_command: None,
                timeout: 300,
                quick: false,
            },
            PipelineStep {
                name: "test".to_string(),
                command: "<pm> test".to_string(),
                fix_command: None,
                timeout: 600,
                quick: false,
            },
        ],
    }
}

/// JavaScript pipeline
fn javascript_pipeline() -> Pipeline {
    Pipeline {
        description: "JavaScript/Node.js projects".to_string(),
        steps: vec![
            PipelineStep {
                name: "lint".to_string(),
                command: "npx eslint . --max-warnings 0".to_string(),
                fix_command: Some("npx eslint . --fix".to_string()),
                timeout: 60,
                quick: true,
            },
            PipelineStep {
                name: "format".to_string(),
                command: "npx prettier --check .".to_string(),
                fix_command: Some("npx prettier --write .".to_string()),
                timeout: 30,
                quick: true,
            },
            PipelineStep {
                name: "build".to_string(),
                command: "<pm> run build".to_string(),
                fix_command: None,
                timeout: 300,
                quick: false,
            },
            PipelineStep {
                name: "test".to_string(),
                command: "<pm> test".to_string(),
                fix_command: None,
                timeout: 600,
                quick: false,
            },
        ],
    }
}

/// Go pipeline
fn go_pipeline() -> Pipeline {
    Pipeline {
        description: "Go projects".to_string(),
        steps: vec![
            PipelineStep {
                name: "lint".to_string(),
                command: "go vet ./...".to_string(),
                fix_command: None,
                timeout: 60,
                quick: true,
            },
            PipelineStep {
                name: "format".to_string(),
                command: "gofmt -l . | grep -q . && exit 1 || exit 0".to_string(),
                fix_command: Some("gofmt -w .".to_string()),
                timeout: 30,
                quick: true,
            },
            PipelineStep {
                name: "build".to_string(),
                command: "go build ./...".to_string(),
                fix_command: None,
                timeout: 120,
                quick: false,
            },
            PipelineStep {
                name: "test".to_string(),
                command: "go test ./...".to_string(),
                fix_command: None,
                timeout: 600,
                quick: false,
            },
        ],
    }
}

/// Swift pipeline
fn swift_pipeline() -> Pipeline {
    Pipeline {
        description: "Swift and Xcode projects".to_string(),
        steps: vec![
            PipelineStep {
                name: "lint".to_string(),
                command: "swiftlint lint --quiet".to_string(),
                fix_command: Some("swiftlint lint --fix --quiet".to_string()),
                timeout: 30,
                quick: true,
            },
            PipelineStep {
                name: "format".to_string(),
                command: "swift-format lint --recursive .".to_string(),
                fix_command: Some("swift-format format --recursive --in-place .".to_string()),
                timeout: 30,
                quick: true,
            },
            PipelineStep {
                name: "build".to_string(),
                command: "xcodebuild build -scheme <scheme> -destination 'platform=macOS' -quiet"
                    .to_string(),
                fix_command: None,
                timeout: 300,
                quick: false,
            },
            PipelineStep {
                name: "test".to_string(),
                command: "xcodebuild test -scheme <scheme> -destination 'platform=macOS' -quiet"
                    .to_string(),
                fix_command: None,
                timeout: 600,
                quick: false,
            },
        ],
    }
}

/// Shell pipeline
fn shell_pipeline() -> Pipeline {
    Pipeline {
        description: "Shell script projects".to_string(),
        steps: vec![
            PipelineStep {
                name: "lint".to_string(),
                command: "shellcheck *.sh **/*.sh 2>/dev/null || shellcheck *.sh".to_string(),
                fix_command: None,
                timeout: 30,
                quick: true,
            },
            PipelineStep {
                name: "syntax".to_string(),
                command: "bash -n *.sh".to_string(),
                fix_command: None,
                timeout: 10,
                quick: true,
            },
        ],
    }
}
