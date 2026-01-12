//! Focus statistics calculation
//!
//! Aggregates completed session data into useful statistics:
//! - Total sessions and focus time
//! - Completion rate
//! - Average session duration

use crate::session::CompletedSession;

/// Aggregated focus statistics
#[derive(Debug, Clone, Default)]
pub struct FocusStats {
    /// Total number of sessions
    pub total_sessions: u32,
    /// Number of completed sessions (not stopped early)
    pub completed_sessions: u32,
    /// Total focus time in minutes
    pub total_minutes: u32,
    /// Completion rate as percentage (0-100)
    pub completion_rate: u32,
    /// Average session duration in minutes
    pub average_duration: u32,
}

impl FocusStats {
    /// Calculate statistics from a list of completed sessions
    pub fn from_sessions(sessions: &[CompletedSession]) -> Self {
        if sessions.is_empty() {
            return Self::default();
        }

        let total_sessions = sessions.len() as u32;
        let completed_sessions = sessions.iter().filter(|s| s.completed).count() as u32;
        let total_minutes: u32 = sessions.iter().map(|s| s.duration).sum();

        let completion_rate = if total_sessions > 0 {
            (completed_sessions * 100) / total_sessions
        } else {
            0
        };

        let average_duration = if total_sessions > 0 {
            total_minutes / total_sessions
        } else {
            0
        };

        Self {
            total_sessions,
            completed_sessions,
            total_minutes,
            completion_rate,
            average_duration,
        }
    }

    /// Get total hours and minutes as a tuple
    pub fn total_time(&self) -> (u32, u32) {
        let hours = self.total_minutes / 60;
        let mins = self.total_minutes % 60;
        (hours, mins)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_session(duration: u32, completed: bool) -> CompletedSession {
        let now = Utc::now().timestamp();
        CompletedSession {
            start: now - (duration as i64 * 60),
            end: now,
            duration,
            task: None,
            completed,
        }
    }

    #[test]
    fn test_empty_stats() {
        let stats = FocusStats::from_sessions(&[]);
        assert_eq!(stats.total_sessions, 0);
        assert_eq!(stats.total_minutes, 0);
        assert_eq!(stats.completion_rate, 0);
    }

    #[test]
    fn test_stats_calculation() {
        let sessions = vec![
            make_session(25, true),
            make_session(25, true),
            make_session(15, false),
            make_session(45, true),
        ];

        let stats = FocusStats::from_sessions(&sessions);
        assert_eq!(stats.total_sessions, 4);
        assert_eq!(stats.completed_sessions, 3);
        assert_eq!(stats.total_minutes, 110);
        assert_eq!(stats.completion_rate, 75); // 3/4 = 75%
        assert_eq!(stats.average_duration, 27); // 110/4 = 27
    }

    #[test]
    fn test_total_time() {
        let sessions = vec![
            make_session(90, true),
            make_session(45, true),
        ];

        let stats = FocusStats::from_sessions(&sessions);
        let (hours, mins) = stats.total_time();
        assert_eq!(hours, 2);
        assert_eq!(mins, 15);
    }
}
