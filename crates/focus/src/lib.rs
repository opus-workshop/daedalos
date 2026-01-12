//! focus - Pomodoro timer and distraction blocking for deep work
//!
//! "Protect the sacred space of deep work."
//!
//! Focus mode helps humans stay concentrated during development by providing:
//! - Configurable focus session timers (pomodoro, deep work, quick sprints)
//! - Break reminders to prevent burnout
//! - Session statistics and tracking
//! - Optional distraction blocking hooks
//!
//! Commands:
//! - start [MINS]: Start a focus session (default: 25 minutes)
//! - stop: End current focus session
//! - status: Show current session progress
//! - break [MINS]: Take a break (default: 5 minutes)
//! - stats [DAYS]: Show focus statistics

pub mod session;
pub mod stats;
pub mod store;

pub use session::{FocusSession, SessionType};
pub use stats::FocusStats;
pub use store::FocusStore;
