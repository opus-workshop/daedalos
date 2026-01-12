//! Focus session types and management
//!
//! Handles the core focus session model: start times, durations, tasks,
//! and session types (pomodoro, deep work, quick sprint).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Type of focus session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionType {
    /// Pomodoro: 25 min focus, 5 min break - sustainable rhythm
    Pomodoro,
    /// Deep: 90 min focus, 20 min break - maximum concentration
    Deep,
    /// Quick: 15 min focus, 3 min break - fast iteration
    Quick,
    /// Custom duration
    Custom,
}

impl SessionType {
    /// Get the default duration in minutes for this session type
    pub fn default_duration(&self) -> u32 {
        match self {
            SessionType::Pomodoro => 25,
            SessionType::Deep => 90,
            SessionType::Quick => 15,
            SessionType::Custom => 25,
        }
    }

    /// Get the recommended break duration in minutes
    pub fn break_duration(&self) -> u32 {
        match self {
            SessionType::Pomodoro => 5,
            SessionType::Deep => 20,
            SessionType::Quick => 3,
            SessionType::Custom => 5,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            SessionType::Pomodoro => "pomodoro",
            SessionType::Deep => "deep",
            SessionType::Quick => "quick",
            SessionType::Custom => "custom",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "pomodoro" => Some(SessionType::Pomodoro),
            "deep" => Some(SessionType::Deep),
            "quick" => Some(SessionType::Quick),
            "custom" => Some(SessionType::Custom),
            _ => None,
        }
    }

    /// Get a description of this session type
    pub fn description(&self) -> &'static str {
        match self {
            SessionType::Pomodoro => "Sustainable rhythm, good for maintenance work",
            SessionType::Deep => "Maximum depth, for complex problems",
            SessionType::Quick => "Fast iteration, for small tasks",
            SessionType::Custom => "Custom duration session",
        }
    }
}

impl Default for SessionType {
    fn default() -> Self {
        SessionType::Pomodoro
    }
}

/// An active focus session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusSession {
    /// When the session started (Unix timestamp)
    pub start: i64,
    /// Duration in minutes
    pub duration: u32,
    /// Optional task description
    pub task: Option<String>,
    /// Type of session
    #[serde(rename = "type")]
    pub session_type: SessionType,
    /// Whether distraction blocking is enabled
    #[serde(default)]
    pub blocking: bool,
}

impl FocusSession {
    /// Create a new focus session
    pub fn new(duration: u32, task: Option<String>, session_type: SessionType, blocking: bool) -> Self {
        Self {
            start: Utc::now().timestamp(),
            duration,
            task,
            session_type,
            blocking,
        }
    }

    /// Get the start time as a DateTime
    pub fn start_time(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.start, 0).unwrap_or_else(Utc::now)
    }

    /// Get the end time as a DateTime
    pub fn end_time(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.start + (self.duration as i64 * 60), 0)
            .unwrap_or_else(Utc::now)
    }

    /// Get elapsed time in minutes
    pub fn elapsed_minutes(&self) -> u32 {
        let now = Utc::now().timestamp();
        let elapsed_secs = (now - self.start).max(0) as u32;
        elapsed_secs / 60
    }

    /// Get remaining time in minutes
    pub fn remaining_minutes(&self) -> u32 {
        let elapsed = self.elapsed_minutes();
        if elapsed >= self.duration {
            0
        } else {
            self.duration - elapsed
        }
    }

    /// Get progress as a percentage (0-100)
    pub fn progress_percent(&self) -> u32 {
        let elapsed = self.elapsed_minutes();
        if self.duration == 0 {
            return 100;
        }
        ((elapsed * 100) / self.duration).min(100)
    }

    /// Check if the session is complete (time has elapsed)
    pub fn is_complete(&self) -> bool {
        self.remaining_minutes() == 0
    }
}

/// A completed session record (stored in session logs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedSession {
    /// When the session started (Unix timestamp)
    pub start: i64,
    /// When the session ended (Unix timestamp)
    pub end: i64,
    /// Duration in minutes (actual time focused)
    pub duration: u32,
    /// Optional task description
    pub task: Option<String>,
    /// Whether the session was completed (reached the timer vs stopped early)
    pub completed: bool,
}

impl CompletedSession {
    /// Create a completed session record from an active session
    pub fn from_session(session: &FocusSession, completed: bool) -> Self {
        let now = Utc::now().timestamp();
        let actual_duration = ((now - session.start).max(0) as u32) / 60;

        Self {
            start: session.start,
            end: now,
            duration: actual_duration.min(session.duration),
            task: session.task.clone(),
            completed,
        }
    }

    /// Get the start time as a DateTime
    pub fn start_time(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.start, 0).unwrap_or_else(Utc::now)
    }

    /// Get the end time as a DateTime
    pub fn end_time(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.end, 0).unwrap_or_else(Utc::now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_type_defaults() {
        assert_eq!(SessionType::Pomodoro.default_duration(), 25);
        assert_eq!(SessionType::Pomodoro.break_duration(), 5);
        assert_eq!(SessionType::Deep.default_duration(), 90);
        assert_eq!(SessionType::Deep.break_duration(), 20);
        assert_eq!(SessionType::Quick.default_duration(), 15);
        assert_eq!(SessionType::Quick.break_duration(), 3);
    }

    #[test]
    fn test_session_type_roundtrip() {
        for st in [
            SessionType::Pomodoro,
            SessionType::Deep,
            SessionType::Quick,
            SessionType::Custom,
        ] {
            let s = st.as_str();
            let parsed = SessionType::from_str(s).unwrap();
            assert_eq!(st, parsed);
        }
    }

    #[test]
    fn test_session_progress() {
        let session = FocusSession::new(25, None, SessionType::Pomodoro, false);
        // Session just started, so progress should be 0%
        assert!(session.progress_percent() <= 4); // Allow for test timing
        assert!(!session.is_complete());
    }

    #[test]
    fn test_completed_session() {
        let session = FocusSession::new(25, Some("Test task".to_string()), SessionType::Pomodoro, false);
        let completed = CompletedSession::from_session(&session, true);

        assert!(completed.completed);
        assert_eq!(completed.task, Some("Test task".to_string()));
    }
}
