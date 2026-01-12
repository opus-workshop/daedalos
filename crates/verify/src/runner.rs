//! Verification step runner

use anyhow::Result;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::time::timeout;

use crate::detect::{detect_package_manager, detect_xcode_scheme};
use crate::pipeline::{Pipeline, PipelineStep};

/// Result of running a verification step
#[derive(Debug, Clone)]
pub struct StepResult {
    /// Step name
    pub name: String,
    /// Whether the step succeeded
    pub success: bool,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Output from the command
    pub output: String,
    /// Whether the step was skipped
    #[allow(dead_code)]
    pub skipped: bool,
    /// Reason for skipping (if skipped)
    #[allow(dead_code)]
    pub skip_reason: Option<String>,
}

/// Result of running the full verification
#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// Whether all steps passed
    pub success: bool,
    /// Total duration in milliseconds
    pub duration_ms: u64,
    /// Results for each step
    pub steps: Vec<StepResult>,
}

/// Run verification
pub async fn run_verification(
    pipeline: &Pipeline,
    path: &Path,
    quick: bool,
    staged: bool,
    fix: bool,
    only_step: Option<&str>,
    skip_steps: &[String],
    json: bool,
    quiet: bool,
    verbose: bool,
) -> Result<VerificationResult> {
    let start = Instant::now();
    let mut results = Vec::new();
    let mut all_passed = true;

    for step in &pipeline.steps {
        // Check if we should skip this step
        if let Some(only) = only_step {
            if step.name != only {
                continue;
            }
        }

        if skip_steps.contains(&step.name) {
            let result = StepResult {
                name: step.name.clone(),
                success: true,
                duration_ms: 0,
                output: String::new(),
                skipped: true,
                skip_reason: Some("user skip".to_string()),
            };

            if !quiet && !json {
                print_step_skipped(&step.name, "user skip");
            }

            results.push(result);
            continue;
        }

        // Check quick mode
        if quick && !step.quick {
            let result = StepResult {
                name: step.name.clone(),
                success: true,
                duration_ms: 0,
                output: String::new(),
                skipped: true,
                skip_reason: Some("not quick".to_string()),
            };

            if !quiet && !json {
                print_step_skipped(&step.name, "not quick");
            }

            results.push(result);
            continue;
        }

        // Show step start
        if !quiet && !json {
            print_step_start(&step.name);
        }

        // Run the step
        let step_result = run_step(step, path, fix, staged, verbose).await;

        if !step_result.success {
            all_passed = false;
        }

        // Print result
        if !quiet && !json {
            print_step_result(&step_result);
            if !step_result.success && !step_result.output.is_empty() {
                print_step_errors(&step_result.output);
            }
        }

        results.push(step_result);
    }

    let duration = start.elapsed();

    Ok(VerificationResult {
        success: all_passed,
        duration_ms: duration.as_millis() as u64,
        steps: results,
    })
}

/// Run a single step
async fn run_step(
    step: &PipelineStep,
    path: &Path,
    fix: bool,
    staged: bool,
    verbose: bool,
) -> StepResult {
    let start = Instant::now();

    // Choose command
    let base_cmd = if fix && step.fix_command.is_some() {
        step.fix_command.as_ref().unwrap()
    } else {
        &step.command
    };

    // Substitute variables
    let cmd = substitute_vars(base_cmd, path);

    // Adapt for staged mode
    let cmd = if staged {
        adapt_for_staged(&cmd, &step.name, path)
    } else {
        cmd
    };

    if verbose {
        eprintln!("Running: {}", cmd);
    }

    // Run the command
    let result = run_command_with_timeout(&cmd, path, step.timeout).await;

    let duration = start.elapsed();

    match result {
        Ok((output, success)) => StepResult {
            name: step.name.clone(),
            success,
            duration_ms: duration.as_millis() as u64,
            output,
            skipped: false,
            skip_reason: None,
        },
        Err(e) => StepResult {
            name: step.name.clone(),
            success: false,
            duration_ms: duration.as_millis() as u64,
            output: e.to_string(),
            skipped: false,
            skip_reason: None,
        },
    }
}

/// Substitute variables in command
fn substitute_vars(cmd: &str, path: &Path) -> String {
    let mut result = cmd.to_string();

    // Substitute <pm> with detected package manager
    if result.contains("<pm>") {
        let pm = detect_package_manager(path);
        result = result.replace("<pm>", pm);
    }

    // Substitute <scheme> with detected Xcode scheme
    if result.contains("<scheme>") {
        let scheme = detect_xcode_scheme(path).unwrap_or_else(|| "Unknown".to_string());
        result = result.replace("<scheme>", &scheme);
    }

    // Substitute <project> with project name
    if result.contains("<project>") {
        let project_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("project");
        result = result.replace("<project>", project_name);
    }

    result
}

/// Adapt command for staged files only
fn adapt_for_staged(cmd: &str, _step_name: &str, path: &Path) -> String {
    // Get staged files
    let staged = get_staged_files(path);

    if staged.is_empty() {
        // No staged files - return a passing command
        return "true".to_string();
    }

    // Adapt command based on tool
    if cmd.contains("eslint") {
        let js_files: Vec<_> = staged
            .iter()
            .filter(|f| {
                f.ends_with(".js")
                    || f.ends_with(".jsx")
                    || f.ends_with(".ts")
                    || f.ends_with(".tsx")
            })
            .collect();

        if js_files.is_empty() {
            return "true".to_string();
        }

        let files_str: Vec<&str> = js_files.iter().map(|s| s.as_str()).collect();
        return format!("npx eslint {}", files_str.join(" "));
    }

    if cmd.contains("ruff") {
        let py_files: Vec<_> = staged.iter().filter(|f| f.ends_with(".py")).collect();

        if py_files.is_empty() {
            return "true".to_string();
        }

        let files_str: Vec<&str> = py_files.iter().map(|s| s.as_str()).collect();
        return format!("ruff check {}", files_str.join(" "));
    }

    if cmd.contains("swiftlint") {
        let swift_files: Vec<_> = staged.iter().filter(|f| f.ends_with(".swift")).collect();

        if swift_files.is_empty() {
            return "true".to_string();
        }

        let files_str: Vec<&str> = swift_files.iter().map(|s| s.as_str()).collect();
        return format!("swiftlint lint {}", files_str.join(" "));
    }

    // For other commands (like clippy), run on all
    cmd.to_string()
}

/// Get list of staged files
fn get_staged_files(path: &Path) -> Vec<String> {
    let output = std::process::Command::new("git")
        .arg("diff")
        .arg("--cached")
        .arg("--name-only")
        .current_dir(path)
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            stdout.lines().map(|s| s.to_string()).collect()
        }
        _ => Vec::new(),
    }
}

/// Run a command with timeout
async fn run_command_with_timeout(
    cmd: &str,
    path: &Path,
    timeout_secs: u64,
) -> Result<(String, bool)> {
    let child = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .current_dir(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let result = timeout(Duration::from_secs(timeout_secs), child.wait_with_output()).await;

    match result {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{}{}", stdout, stderr);
            Ok((combined, output.status.success()))
        }
        Ok(Err(e)) => Err(e.into()),
        Err(_) => Ok(("Timeout exceeded".to_string(), false)),
    }
}

// Output formatting functions

fn print_step_start(name: &str) {
    print!("\x1b[2m[     ]\x1b[0m {}...", name);
    use std::io::Write;
    let _ = std::io::stdout().flush();
}

fn print_step_result(result: &StepResult) {
    let duration_str = format_duration(result.duration_ms);

    // Move to start of line
    print!("\r");

    if result.success {
        println!(
            "[\x1b[2m{:>5}\x1b[0m] \x1b[32mok\x1b[0m {}",
            duration_str, result.name
        );
    } else {
        println!(
            "[\x1b[2m{:>5}\x1b[0m] \x1b[31mFAIL\x1b[0m {}",
            duration_str, result.name
        );
    }
}

fn print_step_skipped(name: &str, reason: &str) {
    println!(
        "[\x1b[2m  -  \x1b[0m] \x1b[33mskip\x1b[0m {} \x1b[2m({})\x1b[0m",
        name, reason
    );
}

fn print_step_errors(output: &str) {
    let max_lines = 10;
    let lines: Vec<_> = output.lines().take(max_lines).collect();
    let total_lines = output.lines().count();

    for line in &lines {
        println!("     \x1b[2m{}\x1b[0m", line);
    }

    if total_lines > max_lines {
        println!(
            "     \x1b[2m... and {} more lines\x1b[0m",
            total_lines - max_lines
        );
    }
}

fn format_duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{}ms", ms)
    } else if ms < 60000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        let mins = ms / 60000;
        let secs = (ms % 60000) / 1000;
        format!("{}m{}s", mins, secs)
    }
}
