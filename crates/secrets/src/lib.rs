//! secrets - Local secrets vault for Daedalos
//!
//! "Secrets should be invisible until you need them."
//!
//! API keys, tokens, passwords - developers handle dozens of these.
//! The secrets tool makes the right thing easy. Encrypted storage by
//! default. Namespaced organization. Inject into environment only when
//! needed, never persisted to disk in plaintext.
//!
//! Uses age encryption (X25519 + ChaCha20-Poly1305).

pub mod vault;

pub use vault::{Secret, Vault, VaultError};
