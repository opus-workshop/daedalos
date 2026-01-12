//! Backend abstraction for oracle
//!
//! Backends are external CLI tools that oracle delegates to.
//! Each backend is configured with a command and arguments.

use anyhow::{Context, Result};
use std::io::Write;
use std::process::{Command, Stdio};

/// A configured backend
#[derive(Debug, Clone)]
pub struct Backend {
    /// Backend name
    pub name: String,

    /// The command to run
    pub command: String,

    /// Argument template (use {prompt} as placeholder)
    pub args: Vec<String>,

    /// Flag to continue last conversation
    pub continue_flag: Option<String>,

    /// Flag to resume a specific session
    pub session_flag: Option<String>,

    /// Flag for JSON output
    pub json_flag: Option<String>,

    /// Value for JSON output flag
    pub json_value: Option<String>,
}

/// Result of executing a backend
#[derive(Debug)]
pub struct BackendResult {
    /// The response text
    pub response: String,

    /// Session ID for continuation (if available)
    pub session_id: Option<String>,
}

impl Backend {
    /// Execute the backend with a prompt
    pub fn execute(
        &self,
        prompt: &str,
        session_id: Option<&str>,
        json_output: bool,
    ) -> Result<BackendResult> {
        let mut cmd = Command::new(&self.command);

        // Build arguments, replacing {prompt} placeholder
        for arg in &self.args {
            if arg == "{prompt}" {
                cmd.arg(prompt);
            } else {
                cmd.arg(arg);
            }
        }

        // Add continue/session flag if resuming
        if let Some(sid) = session_id {
            if let Some(ref flag) = self.session_flag {
                cmd.arg(flag);
                cmd.arg(sid);
            } else if let Some(ref flag) = self.continue_flag {
                // Fallback to continue flag if no session flag
                cmd.arg(flag);
            }
        }

        // Add JSON output flag if requested
        if json_output {
            if let (Some(ref flag), Some(ref value)) = (&self.json_flag, &self.json_value) {
                cmd.arg(flag);
                cmd.arg(value);
            }
        }

        // Execute and capture output
        let output = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .output()
            .with_context(|| format!("Failed to execute backend: {}", self.command))?;

        if !output.status.success() {
            anyhow::bail!(
                "Backend {} exited with status: {}",
                self.command,
                output.status
            );
        }

        let response = String::from_utf8_lossy(&output.stdout).to_string();

        // Try to extract session ID from JSON response
        let session_id = if json_output {
            extract_session_id(&response)
        } else {
            None
        };

        Ok(BackendResult {
            response,
            session_id,
        })
    }

    /// Execute the backend with streaming output
    pub fn execute_streaming(
        &self,
        prompt: &str,
        session_id: Option<&str>,
    ) -> Result<Option<String>> {
        let mut cmd = Command::new(&self.command);

        // Build arguments, replacing {prompt} placeholder
        for arg in &self.args {
            if arg == "{prompt}" {
                cmd.arg(prompt);
            } else {
                cmd.arg(arg);
            }
        }

        // Add continue/session flag if resuming
        if let Some(sid) = session_id {
            if let Some(ref flag) = self.session_flag {
                cmd.arg(flag);
                cmd.arg(sid);
            } else if let Some(ref flag) = self.continue_flag {
                cmd.arg(flag);
            }
        }

        // Execute with inherited stdout for streaming
        let status = cmd
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .with_context(|| format!("Failed to execute backend: {}", self.command))?;

        if !status.success() {
            anyhow::bail!("Backend {} exited with status: {}", self.command, status);
        }

        // For streaming, we can't easily extract session ID
        // The REPL will need to handle this differently
        Ok(None)
    }

    /// Execute and return the session ID (for REPL mode)
    pub fn execute_for_session(
        &self,
        prompt: &str,
        session_id: Option<&str>,
    ) -> Result<Option<String>> {
        // First try with JSON to get session ID
        let result = self.execute(prompt, session_id, true)?;

        // Print the response (extract from JSON if needed)
        if let Some(text) = extract_response_text(&result.response) {
            print!("{}", text);
            if !text.ends_with('\n') {
                println!();
            }
        } else {
            // Fallback: print raw response
            print!("{}", result.response);
            if !result.response.ends_with('\n') {
                println!();
            }
        }

        std::io::stdout().flush()?;

        Ok(result.session_id)
    }
}

/// Try to extract session ID from JSON response
fn extract_session_id(json_str: &str) -> Option<String> {
    // Try to parse as JSON and extract session_id
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str) {
        if let Some(id) = value.get("session_id").and_then(|v| v.as_str()) {
            return Some(id.to_string());
        }
    }
    None
}

/// Try to extract response text from JSON
fn extract_response_text(json_str: &str) -> Option<String> {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str) {
        // Try common response field names
        for field in &["result", "response", "content", "text", "message"] {
            if let Some(text) = value.get(*field).and_then(|v| v.as_str()) {
                return Some(text.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_session_id() {
        let json = r#"{"session_id": "abc123", "result": "hello"}"#;
        assert_eq!(extract_session_id(json), Some("abc123".to_string()));

        let no_session = r#"{"result": "hello"}"#;
        assert_eq!(extract_session_id(no_session), None);
    }

    #[test]
    fn test_extract_response_text() {
        let json = r#"{"result": "Hello world"}"#;
        assert_eq!(
            extract_response_text(json),
            Some("Hello world".to_string())
        );

        let json2 = r#"{"response": "Hi there"}"#;
        assert_eq!(extract_response_text(json2), Some("Hi there".to_string()));
    }
}
