//! Promise verification
//!
//! A promise is a verifiable condition that must be true for the loop to complete.
//! Promises are shell commands that return exit code 0 on success.
//!
//! Examples:
//!   - "make test"           -> All tests pass
//!   - "cargo clippy"        -> No linter warnings
//!   - "npm run build"       -> Build succeeds
//!   - "./verify.sh"         -> Custom verification script

use anyhow::Result;
use std::path::PathBuf;
use std::time::Duration;

/// Result of a promise verification
#[derive(Debug, Clone)]
pub struct PromiseResult {
    pub success: bool,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub duration_ms: u64,
}

/// Run the promise command and return True if it succeeds (exit code 0)
pub async fn verify_promise(
    command: &str,
    working_dir: &PathBuf,
    timeout_secs: u64,
) -> Result<PromiseResult> {
    verify_promise_detailed(command, working_dir, timeout_secs).await
}

/// Run promise command and return detailed result
pub async fn verify_promise_detailed(
    command: &str,
    working_dir: &PathBuf,
    timeout_secs: u64,
) -> Result<PromiseResult> {
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(timeout_secs);

    // Use sh -c to run the command as a shell command
    let child = tokio::process::Command::new("sh")
        .args(["-c", command])
        .current_dir(working_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();

    let child = match child {
        Ok(c) => c,
        Err(e) => {
            let duration = start.elapsed();
            return Ok(PromiseResult {
                success: false,
                exit_code: -1,
                stdout: String::new(),
                stderr: format!("Failed to spawn command: {}", e),
                timed_out: false,
                duration_ms: duration.as_millis() as u64,
            });
        }
    };

    // Wait with timeout
    let result = tokio::time::timeout(timeout, child.wait_with_output()).await;

    let duration = start.elapsed();

    match result {
        Ok(Ok(output)) => {
            let exit_code = output.status.code().unwrap_or(-1);
            Ok(PromiseResult {
                success: exit_code == 0,
                exit_code,
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                timed_out: false,
                duration_ms: duration.as_millis() as u64,
            })
        }
        Ok(Err(e)) => Ok(PromiseResult {
            success: false,
            exit_code: -1,
            stdout: String::new(),
            stderr: format!("Command failed: {}", e),
            timed_out: false,
            duration_ms: duration.as_millis() as u64,
        }),
        Err(_) => {
            // Timeout - process is killed when the future is dropped
            Ok(PromiseResult {
                success: false,
                exit_code: -1,
                stdout: String::new(),
                stderr: format!("Command timed out after {} seconds", timeout_secs),
                timed_out: true,
                duration_ms: duration.as_millis() as u64,
            })
        }
    }
}

/// Parse a promise command and return metadata about it
pub fn parse_promise_command(promise: &str) -> PromiseInfo {
    let promise_lower = promise.to_lowercase();

    // Test commands
    let test_patterns = ["test", "pytest", "jest", "mocha", "cargo test", "go test"];
    for pattern in test_patterns {
        if promise_lower.contains(pattern) {
            return PromiseInfo {
                promise_type: PromiseType::Test,
                description: "Tests must pass".to_string(),
                command: promise.to_string(),
            };
        }
    }

    // Build commands
    let build_patterns = ["build", "compile", "make"];
    for pattern in build_patterns {
        if promise_lower.contains(pattern) {
            return PromiseInfo {
                promise_type: PromiseType::Build,
                description: "Build must succeed".to_string(),
                command: promise.to_string(),
            };
        }
    }

    // Lint commands
    let lint_patterns = ["lint", "clippy", "eslint", "ruff", "pylint", "flake8"];
    for pattern in lint_patterns {
        if promise_lower.contains(pattern) {
            return PromiseInfo {
                promise_type: PromiseType::Lint,
                description: "Linting must pass".to_string(),
                command: promise.to_string(),
            };
        }
    }

    // Type check commands
    let typecheck_patterns = ["tsc", "mypy", "pyright", "typecheck"];
    for pattern in typecheck_patterns {
        if promise_lower.contains(pattern) {
            return PromiseInfo {
                promise_type: PromiseType::TypeCheck,
                description: "Type checking must pass".to_string(),
                command: promise.to_string(),
            };
        }
    }

    // Default
    PromiseInfo {
        promise_type: PromiseType::Custom,
        description: "Command must exit with code 0".to_string(),
        command: promise.to_string(),
    }
}

/// Type of promise
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromiseType {
    Test,
    Build,
    Lint,
    TypeCheck,
    Custom,
}

/// Information about a promise
#[derive(Debug, Clone)]
pub struct PromiseInfo {
    pub promise_type: PromiseType,
    pub description: String,
    pub command: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_promise_command() {
        let info = parse_promise_command("npm test");
        assert_eq!(info.promise_type, PromiseType::Test);

        let info = parse_promise_command("cargo build");
        assert_eq!(info.promise_type, PromiseType::Build);

        let info = parse_promise_command("cargo clippy");
        assert_eq!(info.promise_type, PromiseType::Lint);

        let info = parse_promise_command("./verify.sh");
        assert_eq!(info.promise_type, PromiseType::Custom);
    }
}
