//! Focus session storage
//!
//! Handles persisting the current session state and completed session logs.
//! - Current session: ~/.local/share/daedalos/focus/current_session
//! - Session logs: ~/.local/share/daedalos/focus/sessions-YYYY-MM-DD.jsonl
//! - Blocking state: ~/.local/share/daedalos/focus/blocking_active

use anyhow::{Context, Result};
use chrono::{NaiveDate, Utc};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use crate::session::{CompletedSession, FocusSession};

/// Focus session store
pub struct FocusStore {
    /// Base directory for focus data
    data_dir: PathBuf,
}

impl FocusStore {
    /// Create a new focus store with the given data directory
    pub fn new(data_dir: &Path) -> Result<Self> {
        fs::create_dir_all(data_dir)
            .with_context(|| format!("Failed to create focus data directory: {}", data_dir.display()))?;

        Ok(Self {
            data_dir: data_dir.to_path_buf(),
        })
    }

    /// Get the path to the current session file
    fn current_session_path(&self) -> PathBuf {
        self.data_dir.join("current_session")
    }

    /// Get the path to the blocking indicator file
    fn blocking_path(&self) -> PathBuf {
        self.data_dir.join("blocking_active")
    }

    /// Get the path to the session log for a specific date
    fn session_log_path(&self, date: &NaiveDate) -> PathBuf {
        self.data_dir.join(format!("sessions-{}.jsonl", date))
    }

    /// Get the current active session, if any
    pub fn get_current_session(&self) -> Result<Option<FocusSession>> {
        let path = self.current_session_path();
        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read current session: {}", path.display()))?;

        if content.trim().is_empty() {
            return Ok(None);
        }

        let session: FocusSession = serde_json::from_str(&content)
            .with_context(|| "Failed to parse current session JSON")?;

        Ok(Some(session))
    }

    /// Save the current session
    pub fn save_current_session(&self, session: &FocusSession) -> Result<()> {
        let path = self.current_session_path();
        let content = serde_json::to_string_pretty(session)
            .context("Failed to serialize session")?;

        fs::write(&path, content)
            .with_context(|| format!("Failed to write current session: {}", path.display()))
    }

    /// Clear the current session
    pub fn clear_current_session(&self) -> Result<()> {
        let path = self.current_session_path();
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("Failed to remove current session: {}", path.display()))?;
        }
        Ok(())
    }

    /// Record a completed session to the daily log
    pub fn record_session(&self, session: &CompletedSession) -> Result<()> {
        let date = session.start_time().date_naive();
        let path = self.session_log_path(&date);

        let line = serde_json::to_string(session)
            .context("Failed to serialize completed session")?;

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("Failed to open session log: {}", path.display()))?;

        writeln!(file, "{}", line)
            .with_context(|| format!("Failed to write to session log: {}", path.display()))
    }

    /// Enable blocking indicator
    pub fn enable_blocking(&self) -> Result<()> {
        let path = self.blocking_path();
        fs::write(&path, "")
            .with_context(|| format!("Failed to enable blocking: {}", path.display()))
    }

    /// Disable blocking indicator
    pub fn disable_blocking(&self) -> Result<()> {
        let path = self.blocking_path();
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("Failed to disable blocking: {}", path.display()))?;
        }
        Ok(())
    }

    /// Check if blocking is enabled
    pub fn is_blocking_enabled(&self) -> bool {
        self.blocking_path().exists()
    }

    /// Get all completed sessions for a specific date
    pub fn get_sessions_for_date(&self, date: &NaiveDate) -> Result<Vec<CompletedSession>> {
        let path = self.session_log_path(date);
        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = fs::File::open(&path)
            .with_context(|| format!("Failed to open session log: {}", path.display()))?;

        let reader = BufReader::new(file);
        let mut sessions = Vec::new();

        for (line_num, line) in reader.lines().enumerate() {
            let line = line.with_context(|| format!("Failed to read line {} of session log", line_num + 1))?;

            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<CompletedSession>(&line) {
                Ok(session) => sessions.push(session),
                Err(e) => {
                    // Log but don't fail on individual parse errors
                    eprintln!("Warning: Failed to parse session on line {}: {}", line_num + 1, e);
                }
            }
        }

        Ok(sessions)
    }

    /// Get all completed sessions for the last N days
    pub fn get_sessions_for_days(&self, days: u32) -> Result<Vec<CompletedSession>> {
        let today = Utc::now().date_naive();
        let mut all_sessions = Vec::new();

        for i in 0..days {
            let date = today - chrono::Duration::days(i as i64);
            let sessions = self.get_sessions_for_date(&date)?;
            all_sessions.extend(sessions);
        }

        // Sort by start time (oldest first)
        all_sessions.sort_by_key(|s| s.start);

        Ok(all_sessions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionType;
    use std::env;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_store(test_name: &str) -> (FocusStore, PathBuf) {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = env::temp_dir().join(format!(
            "focus_test_{}_{}_{}",
            std::process::id(),
            test_name,
            counter
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        let store = FocusStore::new(&temp_dir).unwrap();
        (store, temp_dir)
    }

    #[test]
    fn test_no_current_session() {
        let (store, temp_dir) = temp_store("no_current");
        assert!(store.get_current_session().unwrap().is_none());
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_save_and_get_session() {
        let (store, temp_dir) = temp_store("save_get");

        let session = FocusSession::new(25, Some("Test task".to_string()), SessionType::Pomodoro, false);
        store.save_current_session(&session).unwrap();

        let loaded = store.get_current_session().unwrap().unwrap();
        assert_eq!(loaded.duration, 25);
        assert_eq!(loaded.task, Some("Test task".to_string()));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_clear_session() {
        let (store, temp_dir) = temp_store("clear");

        let session = FocusSession::new(25, None, SessionType::Pomodoro, false);
        store.save_current_session(&session).unwrap();
        assert!(store.get_current_session().unwrap().is_some());

        store.clear_current_session().unwrap();
        assert!(store.get_current_session().unwrap().is_none());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_blocking_indicator() {
        let (store, temp_dir) = temp_store("blocking");

        assert!(!store.is_blocking_enabled());

        store.enable_blocking().unwrap();
        assert!(store.is_blocking_enabled());

        store.disable_blocking().unwrap();
        assert!(!store.is_blocking_enabled());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_record_and_retrieve_sessions() {
        let (store, temp_dir) = temp_store("record_retrieve");

        let session1 = FocusSession::new(25, Some("Task 1".to_string()), SessionType::Pomodoro, false);
        let completed1 = CompletedSession::from_session(&session1, true);
        store.record_session(&completed1).unwrap();

        let session2 = FocusSession::new(15, Some("Task 2".to_string()), SessionType::Quick, false);
        let completed2 = CompletedSession::from_session(&session2, false);
        store.record_session(&completed2).unwrap();

        let sessions = store.get_sessions_for_days(1).unwrap();
        assert_eq!(sessions.len(), 2);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
