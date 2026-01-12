//! Inter-agent messaging
//!
//! Provides message passing between agents using JSON Lines files.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use crate::state::AgentState;

/// Message type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    Message,
    HelpRequest,
    HelpResponse,
    SharedArtifact,
    Broadcast,
}

impl std::fmt::Display for MessageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageType::Message => write!(f, "message"),
            MessageType::HelpRequest => write!(f, "help_request"),
            MessageType::HelpResponse => write!(f, "help_response"),
            MessageType::SharedArtifact => write!(f, "shared_artifact"),
            MessageType::Broadcast => write!(f, "broadcast"),
        }
    }
}

/// Message status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageStatus {
    Pending,
    Read,
}

/// A message between agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub from: String,
    pub to: String,
    #[serde(rename = "type")]
    pub msg_type: MessageType,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub status: MessageStatus,
}

impl Message {
    /// Create a new message
    pub fn new(from: String, to: String, msg_type: MessageType, content: String) -> Self {
        let now = Utc::now();
        let id = format!(
            "msg-{:x}",
            md5_hash(&format!("{}{}{}", from, to, now.timestamp_nanos_opt().unwrap_or(0)))
        );

        Self {
            id,
            from,
            to,
            msg_type,
            content,
            timestamp: now,
            status: MessageStatus::Pending,
        }
    }
}

/// Simple hash function for message IDs
fn md5_hash(input: &str) -> u64 {
    let mut hash: u64 = 0;
    for byte in input.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
    }
    hash
}

/// Messaging operations
pub struct Messaging<'a> {
    state: &'a AgentState,
}

impl<'a> Messaging<'a> {
    pub fn new(state: &'a AgentState) -> Self {
        Self { state }
    }

    /// Send a message to an agent
    pub fn send(
        &self,
        to: &str,
        from: &str,
        msg_type: MessageType,
        content: &str,
    ) -> Result<String> {
        // Verify target agent exists
        if !self.state.agent_exists(to)? {
            anyhow::bail!("Target agent not found: {}", to);
        }

        let message = Message::new(
            from.to_string(),
            to.to_string(),
            msg_type,
            content.to_string(),
        );

        let queue_path = self.state.message_queue(to);
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&queue_path)
            .context("Failed to open message queue")?;

        let json = serde_json::to_string(&message)?;
        writeln!(file, "{}", json)?;

        Ok(message.id)
    }

    /// Get messages for an agent
    pub fn inbox(&self, agent: &str, include_read: bool) -> Result<Vec<Message>> {
        let queue_path = self.state.message_queue(agent);

        if !queue_path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&queue_path).context("Failed to open message queue")?;
        let reader = BufReader::new(file);
        let mut messages = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            if let Ok(msg) = serde_json::from_str::<Message>(&line) {
                if include_read || msg.status == MessageStatus::Pending {
                    messages.push(msg);
                }
            }
        }

        Ok(messages)
    }

    /// Mark messages as read
    pub fn mark_read(&self, agent: &str, message_id: Option<&str>) -> Result<()> {
        let queue_path = self.state.message_queue(agent);

        if !queue_path.exists() {
            return Ok(());
        }

        let file = File::open(&queue_path)?;
        let reader = BufReader::new(file);
        let mut messages: Vec<Message> = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            if let Ok(mut msg) = serde_json::from_str::<Message>(&line) {
                if message_id.map_or(true, |id| msg.id == id) {
                    msg.status = MessageStatus::Read;
                }
                messages.push(msg);
            }
        }

        // Write back
        let tmp_path = queue_path.with_extension("tmp");
        let mut file = File::create(&tmp_path)?;
        for msg in messages {
            let json = serde_json::to_string(&msg)?;
            writeln!(file, "{}", json)?;
        }
        fs::rename(&tmp_path, &queue_path)?;

        Ok(())
    }

    /// Broadcast a message to all agents
    pub fn broadcast(&self, from: &str, content: &str) -> Result<usize> {
        let agents = self.state.list_agents()?;
        let mut count = 0;

        for agent in agents {
            if agent.name == from {
                continue;
            }

            self.send(&agent.name, from, MessageType::Broadcast, content)?;
            count += 1;
        }

        Ok(count)
    }

    /// Get pending message count for an agent
    pub fn pending_count(&self, agent: &str) -> Result<usize> {
        let messages = self.inbox(agent, false)?;
        Ok(messages.len())
    }
}

/// Format messages for display
pub fn format_messages(messages: &[Message], as_json: bool) -> String {
    if as_json {
        serde_json::to_string_pretty(messages).unwrap_or_else(|_| "[]".to_string())
    } else {
        if messages.is_empty() {
            return "No messages.".to_string();
        }

        let mut output = String::new();
        for msg in messages {
            output.push_str(&format!(
                "[{}] from {} ({})\n  {}\n\n",
                msg.msg_type, msg.from, msg.timestamp, msg.content
            ));
        }
        output
    }
}
