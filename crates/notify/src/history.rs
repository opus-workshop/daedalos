//! Notification history tracking

use anyhow::Result;
use chrono::{DateTime, Local, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use crate::backend::Notification;

/// A record of a sent notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationRecord {
    /// Timestamp when the notification was sent
    pub timestamp: i64,
    /// Notification title
    pub title: String,
    /// Notification message
    pub message: String,
    /// Urgency level
    pub urgency: String,
}

impl NotificationRecord {
    /// Create a record from a notification
    pub fn from_notification(notification: &Notification) -> Self {
        Self {
            timestamp: Utc::now().timestamp(),
            title: notification.title.clone(),
            message: notification.message.clone(),
            urgency: notification.urgency.as_str().to_string(),
        }
    }

    /// Get the timestamp as a DateTime
    pub fn datetime(&self) -> DateTime<Local> {
        Local.timestamp_opt(self.timestamp, 0).unwrap()
    }

    /// Format as a display string
    pub fn format(&self) -> String {
        let dt = self.datetime();
        format!(
            "{} [{}] {}: {}",
            dt.format("%Y-%m-%d %H:%M:%S"),
            self.urgency,
            self.title,
            self.message
        )
    }
}

/// Notification history manager
pub struct NotificationHistory {
    history_file: PathBuf,
}

impl NotificationHistory {
    /// Create a new history manager
    pub fn new() -> Result<Self> {
        let data_dir = crate::data_dir();
        fs::create_dir_all(&data_dir)?;

        Ok(Self {
            history_file: data_dir.join("history"),
        })
    }

    /// Log a notification to history
    pub fn log(&self, notification: &Notification) -> Result<()> {
        let record = NotificationRecord::from_notification(notification);
        let line = format!(
            "{}|{}|{}|{}\n",
            record.timestamp, record.title, record.message, record.urgency
        );

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.history_file)?;

        file.write_all(line.as_bytes())?;
        Ok(())
    }

    /// Get recent notifications
    pub fn recent(&self, limit: usize) -> Result<Vec<NotificationRecord>> {
        if !self.history_file.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.history_file)?;
        let reader = BufReader::new(file);

        let mut records: Vec<NotificationRecord> = reader
            .lines()
            .filter_map(|line| line.ok())
            .filter_map(|line| Self::parse_line(&line))
            .collect();

        // Get the last N records
        if records.len() > limit {
            records = records.split_off(records.len() - limit);
        }

        Ok(records)
    }

    /// Parse a history line
    fn parse_line(line: &str) -> Option<NotificationRecord> {
        let parts: Vec<&str> = line.splitn(4, '|').collect();
        if parts.len() != 4 {
            return None;
        }

        let timestamp = parts[0].parse().ok()?;
        Some(NotificationRecord {
            timestamp,
            title: parts[1].to_string(),
            message: parts[2].to_string(),
            urgency: parts[3].to_string(),
        })
    }

    /// Clear all history
    pub fn clear(&self) -> Result<()> {
        if self.history_file.exists() {
            fs::remove_file(&self.history_file)?;
        }
        Ok(())
    }

    /// Check if there's any history
    pub fn is_empty(&self) -> bool {
        !self.history_file.exists()
            || self.history_file.metadata().map(|m| m.len() == 0).unwrap_or(true)
    }
}
