//! Daedalos Trust - Context-aware permission system
//!
//! "Permissions without friction. Safety without interruption."
//!
//! Traditional permission systems are OPERATION-CENTRIC: "May I run `rm`?"
//! Daedalos trust is CONTEXT-CENTRIC: "May I run `rm *.pyc` in `~/Daedalos`
//! during a `loop` session that you started?"
//!
//! Context changes everything:
//! - Same operation, different project → different answer
//! - Same operation, 1st vs 50th time → different answer
//! - Same operation, human-initiated vs agent-initiated → different answer
//!
//! The goal: Make --dangerously-skip-permissions unnecessary by making
//! permissions intelligent.

pub mod audit;
pub mod config;
pub mod evaluator;
pub mod level;
pub mod pattern;
pub mod session;

pub use audit::AuditLog;
pub use config::TrustConfig;
pub use evaluator::{Decision, TrustEvaluator};
pub use level::TrustLevel;
pub use pattern::{Pattern, PatternStore};
pub use session::{Session, SessionManager};
