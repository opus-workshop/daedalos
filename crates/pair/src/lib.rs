//! pair - Pair programming via shared tmux sessions
//!
//! "Share your terminal session with another human or AI."
//!
//! Pair enables collaborative development by creating shared tmux sessions
//! that multiple users can connect to simultaneously. Supports both local
//! (same machine) and remote (SSH/tmate) pairing scenarios.
//!
//! Commands:
//! - start [NAME]: Start a pair session (creates shared tmux socket)
//! - join <NAME>: Join an existing pair session
//! - leave: Detach from current pair session
//! - list: List all active pair sessions
//! - invite: Generate invite command for partner
//! - end [NAME]: End a pair session (kills the tmux session)

pub mod session;
mod store;

pub use session::{PairMode, PairSession};
pub use store::PairStore;

use rand::Rng;

/// Word lists for generating session names
const ADJECTIVES: &[&str] = &[
    "swift", "bright", "calm", "eager", "fair", "glad", "keen", "neat", "wise", "bold",
];

const NOUNS: &[&str] = &[
    "fox", "owl", "bee", "elk", "jay", "ant", "bat", "cod", "emu", "gnu",
];

/// Generate a random session name like "swift-fox-42"
pub fn generate_session_name() -> String {
    let mut rng = rand::thread_rng();
    let adj = ADJECTIVES[rng.gen_range(0..ADJECTIVES.len())];
    let noun = NOUNS[rng.gen_range(0..NOUNS.len())];
    let num: u32 = rng.gen_range(0..100);
    format!("{}-{}-{}", adj, noun, num)
}

/// Check if tmux is available on the system
pub fn check_tmux() -> bool {
    std::process::Command::new("tmux")
        .arg("-V")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if tmate is available on the system
pub fn check_tmate() -> bool {
    std::process::Command::new("tmate")
        .arg("-V")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get the current hostname
pub fn get_hostname() -> String {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "localhost".to_string())
}

/// Get the current username
pub fn get_username() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "user".to_string())
}
