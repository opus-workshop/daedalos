//! handoff - Context summaries for shift changes
//!
//! Context is the most expensive thing in development.
//!
//! When you stop working and someone else starts, there's a brutal context
//! switch. The new person has to reconstruct: What were you doing? Why?
//! What's the current state? What did you try that didn't work?
//!
//! This tool captures context at transition points:
//! - End of day handoffs
//! - Human to AI agent transitions
//! - AI agent to human transitions
//! - Team member shift changes
//!
//! Commands:
//! - create: Create a handoff summary
//! - receive: View a handoff summary
//! - list: List available handoffs
//! - status: Quick status for handoff

mod generator;
mod storage;

pub use generator::{HandoffGenerator, GitInfo, EnvInfo};
pub use storage::{HandoffStorage, Handoff, HandoffMetadata};

use anyhow::Result;
use daedalos_core::Paths;

/// Default hours of history to include in handoff
pub const DEFAULT_HOURS: u64 = 8;

/// Get the handoff storage directory
pub fn handoff_dir() -> std::path::PathBuf {
    Paths::new().data.join("handoff")
}

/// Create a handoff with the given parameters
pub fn create_handoff(
    name: Option<&str>,
    to: Option<&str>,
    hours: u64,
) -> Result<Handoff> {
    let storage = HandoffStorage::new()?;
    let generator = HandoffGenerator::new();

    let handoff = generator.generate(name, to, hours)?;
    storage.save(&handoff)?;

    Ok(handoff)
}

/// Receive (view) a handoff by name
pub fn receive_handoff(name: Option<&str>) -> Result<Handoff> {
    let storage = HandoffStorage::new()?;
    storage.get(name)
}

/// List all available handoffs
pub fn list_handoffs() -> Result<Vec<HandoffMetadata>> {
    let storage = HandoffStorage::new()?;
    storage.list()
}

/// Get quick status (without creating a handoff)
pub fn get_status(hours: u64) -> Result<String> {
    let generator = HandoffGenerator::new();
    generator.quick_status(hours)
}
