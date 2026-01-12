//! Loop state management
//!
//! Handles persistence and execution of loops.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use daedalos_core::Paths;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Instant;

use crate::checkpoint::CheckpointBackend;
use crate::promise::verify_promise_detailed;

/// Status of a loop
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoopStatus {
    Pending,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl LoopStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            LoopStatus::Pending => "pending",
            LoopStatus::Running => "running",
            LoopStatus::Paused => "paused",
            LoopStatus::Completed => "completed",
            LoopStatus::Failed => "failed",
            LoopStatus::Cancelled => "cancelled",
        }
    }
}

/// Record of a single loop iteration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopIteration {
    pub number: u32,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub checkpoint_id: String,
    pub promise_result: bool,
    pub promise_output: String,
    pub promise_exit_code: i32,
    pub changes_summary: String,
    pub duration_ms: u64,
}

/// Persistent state for a loop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopState {
    pub id: String,
    pub prompt: String,
    pub promise_cmd: String,
    pub status: LoopStatus,
    pub working_dir: PathBuf,
    pub max_iterations: u32,
    pub current_iteration: u32,
    pub iterations: Vec<LoopIteration>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub initial_checkpoint: Option<String>,
    pub injected_context: Vec<String>,
    pub checkpoint_strategy: String,
    pub timeout: u64,
    pub error_message: Option<String>,
}

impl LoopState {
    /// Create a new loop state
    pub fn new(config: &LoopConfig) -> Self {
        let id = generate_loop_id();
        let now = Utc::now();

        Self {
            id,
            prompt: config.prompt.clone(),
            promise_cmd: config.promise_cmd.clone(),
            status: LoopStatus::Pending,
            working_dir: config.working_dir.clone(),
            max_iterations: config.max_iterations,
            current_iteration: 0,
            iterations: Vec::new(),
            created_at: now,
            updated_at: now,
            initial_checkpoint: None,
            injected_context: Vec::new(),
            checkpoint_strategy: config.checkpoint_strategy.clone(),
            timeout: config.timeout,
            error_message: None,
        }
    }

    /// Get the state directory
    fn state_dir() -> PathBuf {
        let paths = Paths::new();
        paths.state("loop").join("states")
    }

    /// Save state to disk
    pub fn save(&self) -> Result<()> {
        let state_dir = Self::state_dir();
        std::fs::create_dir_all(&state_dir).context("Failed to create state directory")?;

        let path = state_dir.join(format!("{}.json", self.id));
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content).context("Failed to write state file")?;

        Ok(())
    }

    /// Load state from disk
    pub fn load(loop_id: &str) -> Result<Self> {
        let state_dir = Self::state_dir();
        let path = state_dir.join(format!("{}.json", loop_id));

        if !path.exists() {
            anyhow::bail!("Loop not found: {}", loop_id);
        }

        let content = std::fs::read_to_string(&path).context("Failed to read state file")?;
        let state: Self = serde_json::from_str(&content).context("Failed to parse state file")?;

        Ok(state)
    }

    /// List all loop states
    pub fn list_all() -> Result<Vec<Self>> {
        let state_dir = Self::state_dir();
        let mut states = Vec::new();

        if !state_dir.exists() {
            return Ok(states);
        }

        for entry in std::fs::read_dir(&state_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "json").unwrap_or(false) {
                match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        if let Ok(state) = serde_json::from_str::<Self>(&content) {
                            states.push(state);
                        }
                    }
                    Err(_) => continue,
                }
            }
        }

        // Sort by updated_at descending
        states.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        Ok(states)
    }
}

/// Configuration for starting a loop
pub struct LoopConfig {
    pub prompt: String,
    pub promise_cmd: String,
    pub working_dir: PathBuf,
    pub max_iterations: u32,
    pub timeout: u64,
    pub checkpoint_strategy: String,
}

/// Main loop execution engine
pub struct Loop {
    state: LoopState,
    checkpoint: Box<dyn CheckpointBackend>,
}

impl Loop {
    /// Create a new loop
    pub fn new(config: LoopConfig, checkpoint: Box<dyn CheckpointBackend>) -> Result<Self> {
        let state = LoopState::new(&config);

        Ok(Self { state, checkpoint })
    }

    /// Get the current state
    pub fn state(&self) -> &LoopState {
        &self.state
    }

    /// Get checkpoint backend name
    pub fn checkpoint_name(&self) -> &str {
        self.checkpoint.name()
    }

    /// Run the loop until promise is met or max iterations reached
    pub async fn run(&mut self) -> Result<bool> {
        // Create initial checkpoint
        match self.checkpoint.create(
            &format!("{}_initial", self.state.id),
            &self.state.working_dir,
        ) {
            Ok(checkpoint_id) => {
                self.state.initial_checkpoint = Some(checkpoint_id);
            }
            Err(e) => {
                self.state.error_message = Some(format!("Failed to create initial checkpoint: {}", e));
                self.state.status = LoopStatus::Failed;
                self.state.save()?;
                return Ok(false);
            }
        }

        self.state.status = LoopStatus::Running;
        self.state.save()?;

        while self.state.current_iteration < self.state.max_iterations {
            // Check for pause/cancel
            if self.state.status == LoopStatus::Cancelled {
                return Ok(false);
            }

            // Run iteration
            let success = self.run_iteration().await?;
            if success {
                self.state.status = LoopStatus::Completed;
                self.state.save()?;
                return Ok(true);
            }
        }

        // Max iterations reached
        self.state.error_message = Some(format!(
            "Max iterations ({}) reached without meeting promise",
            self.state.max_iterations
        ));
        self.state.status = LoopStatus::Failed;
        self.state.save()?;

        Ok(false)
    }

    /// Run a single iteration
    async fn run_iteration(&mut self) -> Result<bool> {
        self.state.current_iteration += 1;
        let iteration_num = self.state.current_iteration;
        let start_time = Instant::now();

        // Create checkpoint for this iteration
        let checkpoint_id = match self.checkpoint.create(
            &format!("{}_iter{}", self.state.id, iteration_num),
            &self.state.working_dir,
        ) {
            Ok(id) => id,
            Err(_) => format!("failed_{}", iteration_num),
        };

        // Print iteration info
        println!("{}", "=".repeat(60));
        println!("ITERATION {}/{}", iteration_num, self.state.max_iterations);
        println!("{}", "=".repeat(60));
        println!();

        // Send prompt to oracle (the LLM primitive)
        let prompt = self.build_prompt(iteration_num);
        println!("Sending prompt to oracle...");
        println!();

        if let Err(e) = self.invoke_oracle(&prompt).await {
            eprintln!("Warning: oracle invocation failed: {}", e);
            // Continue anyway - user might be running an external agent
        }

        println!();
        println!("Running verification: {}", self.state.promise_cmd);

        // Verify the promise
        let promise_result =
            verify_promise_detailed(&self.state.promise_cmd, &self.state.working_dir, self.state.timeout)
                .await?;

        let duration = start_time.elapsed();

        // Get changes summary
        let changes_summary = get_changes_summary(&self.state.working_dir).await;

        // Create iteration record
        let iteration = LoopIteration {
            number: iteration_num,
            started_at: Utc::now() - chrono::Duration::milliseconds(duration.as_millis() as i64),
            completed_at: Some(Utc::now()),
            checkpoint_id,
            promise_result: promise_result.success,
            promise_output: format!("{}\n{}", promise_result.stdout, promise_result.stderr),
            promise_exit_code: promise_result.exit_code,
            changes_summary,
            duration_ms: duration.as_millis() as u64,
        };

        // Print result
        let status_str = if promise_result.success { "PASS" } else { "FAIL" };
        println!();
        println!(
            "[Iteration {}] {} ({}ms)",
            iteration_num,
            status_str,
            iteration.duration_ms
        );

        if !promise_result.success && !promise_result.stdout.is_empty() {
            println!();
            // Show first few lines of output
            for line in promise_result.stdout.lines().take(10) {
                println!("  {}", line);
            }
        }

        self.state.iterations.push(iteration);
        self.state.updated_at = Utc::now();
        self.state.save()?;

        Ok(promise_result.success)
    }

    /// Build the full prompt for an iteration
    fn build_prompt(&self, iteration: u32) -> String {
        let mut parts = Vec::new();

        parts.push("=".repeat(60));
        parts.push(format!(
            "LOOP ITERATION {}/{}",
            iteration, self.state.max_iterations
        ));
        parts.push("=".repeat(60));

        parts.push(format!("\nTASK:\n{}", self.state.prompt));

        parts.push("\nSUCCESS CONDITION:".to_string());
        parts.push("The following command must exit with code 0:".to_string());
        parts.push(format!("  {}", self.state.promise_cmd));

        // Injected context
        if !self.state.injected_context.is_empty() {
            parts.push("\nADDITIONAL CONTEXT:".to_string());
            for ctx in &self.state.injected_context {
                parts.push(format!("- {}", ctx));
            }
        }

        // Previous iteration feedback
        if iteration > 1 {
            if let Some(last) = self.state.iterations.last() {
                parts.push(format!("\nPREVIOUS ITERATION ({}) RESULT:", iteration - 1));

                if last.promise_result {
                    parts.push("  Status: PASSED".to_string());
                } else {
                    parts.push("  Status: FAILED".to_string());
                    if !last.promise_output.is_empty() {
                        parts.push("  Output:".to_string());
                        for line in last.promise_output.lines().take(20) {
                            parts.push(format!("    {}", line));
                        }
                    }
                }

                parts.push("\nAnalyze what went wrong and try a different approach.".to_string());
            }
        }

        parts.push("\n".to_string() + &"=".repeat(60));
        parts.push("INSTRUCTIONS:".to_string());
        parts.push("Make changes to the codebase to satisfy the success condition.".to_string());
        parts.push("Focus on the specific task. Make minimal, targeted changes.".to_string());
        parts.push("=".repeat(60));

        parts.join("\n")
    }

    /// Inject additional context for the next iteration
    pub fn inject_context(&mut self, context: String) -> Result<()> {
        self.state.injected_context.push(context);
        self.state.save()?;
        Ok(())
    }

    /// Cancel the loop
    pub fn cancel(&mut self) -> Result<()> {
        self.state.status = LoopStatus::Cancelled;
        self.state.save()?;
        Ok(())
    }
}

/// Generate a unique loop ID
fn generate_loop_id() -> String {
    let bytes: [u8; 6] = rand_bytes();
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, bytes)
}

/// Generate random bytes (simple implementation)
fn rand_bytes() -> [u8; 6] {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    let mut bytes = [0u8; 6];
    bytes[0..4].copy_from_slice(&nanos.to_le_bytes());
    bytes[4] = (nanos >> 8) as u8;
    bytes[5] = (nanos >> 16) as u8;
    bytes
}

/// Get a summary of file changes
async fn get_changes_summary(working_dir: &PathBuf) -> String {
    let output = tokio::process::Command::new("git")
        .args(["diff", "--stat", "HEAD"])
        .current_dir(working_dir)
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            if !stdout.trim().is_empty() {
                return stdout;
            }

            // Try unstaged changes
            let unstaged = tokio::process::Command::new("git")
                .args(["diff", "--stat"])
                .current_dir(working_dir)
                .output()
                .await;

            match unstaged {
                Ok(out) => String::from_utf8_lossy(&out.stdout).to_string(),
                Err(_) => "Unable to detect changes".to_string(),
            }
        }
        _ => "Unable to detect changes".to_string(),
    }
}

/// List all loops
pub fn list_loops() -> Result<Vec<LoopState>> {
    LoopState::list_all()
}
