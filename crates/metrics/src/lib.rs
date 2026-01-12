//! metrics - Productivity statistics for Daedalos
//!
//! What gets measured gets managed. What gets visualized gets understood.
//!
//! The metrics tool aggregates signals from multiple sources - git commits, focus sessions,
//! journal events, loop iterations - to build a picture of actual work. Not hours at desk,
//! but tangible output.
//!
//! Commands:
//! - today: Today's activity summary with productivity score
//! - week: This week's summary with daily commit chart
//! - month: This month's summary with weekly breakdown
//! - commits: Detailed git commit statistics
//! - focus: Focus session statistics
//! - activity: Raw activity timeline from journal
//! - trends: Compare current period to previous period
//! - export: Raw data export for external tools

pub mod db;
pub mod git;
pub mod focus;
pub mod display;

pub use db::{MetricsDatabase, DailyMetrics};
pub use git::GitStats;
pub use focus::FocusStats;
pub use display::{draw_bar, format_duration, ProductivityScore};
