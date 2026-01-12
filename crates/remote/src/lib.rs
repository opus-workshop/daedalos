//! Remote - SSH and remote development management for Daedalos
//!
//! Manage SSH connections, sync files, execute commands remotely,
//! and handle SSH configurations for streamlined remote development.

pub mod host;
pub mod ssh;
pub mod sync;
pub mod tunnel;

pub use host::{Host, HostStore};
pub use ssh::SshConnection;
pub use sync::SyncOptions;
pub use tunnel::TunnelConfig;
