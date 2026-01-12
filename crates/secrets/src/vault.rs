//! Vault - Encrypted secrets storage
//!
//! Each secret is stored as a separate age-encrypted file in the vault directory.
//! Secrets are namespaced using path-like keys (e.g., api/openai, db/prod).

use age::secrecy::ExposeSecret;
use anyhow::{bail, Context, Result};
use std::fs::{self, File, Permissions};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Vault-specific errors
#[derive(Error, Debug)]
pub enum VaultError {
    #[error("Secret not found: {0}")]
    NotFound(String),

    #[error("Vault not initialized - run 'secrets init' first")]
    NotInitialized,

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Decryption error: {0}")]
    Decryption(String),

    #[error("Invalid key name: {0}")]
    InvalidKey(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// A stored secret (key only, value is decrypted on demand)
#[derive(Debug, Clone)]
pub struct Secret {
    /// The key name (e.g., "api/openai")
    pub key: String,
    /// Path to the encrypted file
    pub path: PathBuf,
}

impl Secret {
    /// Convert the key to an environment variable name
    /// e.g., "api/openai" -> "API_OPENAI"
    pub fn to_env_var(&self) -> String {
        self.key.replace('/', "_").to_uppercase()
    }
}

/// The secrets vault
pub struct Vault {
    /// Root directory for the vault
    root: PathBuf,
    /// Directory for encrypted secrets
    vault_dir: PathBuf,
    /// Directory for identity keys
    keys_dir: PathBuf,
    /// Path to the identity key file
    identity_path: PathBuf,
}

impl Vault {
    /// Create a new vault instance
    pub fn new(root: &Path) -> Result<Self> {
        let vault_dir = root.join("vault");
        let keys_dir = root.join("keys");
        let identity_path = keys_dir.join("identity.key");

        Ok(Self {
            root: root.to_path_buf(),
            vault_dir,
            keys_dir,
            identity_path,
        })
    }

    /// Initialize the vault - create directories and generate identity key
    pub fn init(&self) -> Result<String> {
        // Create directories with secure permissions
        fs::create_dir_all(&self.vault_dir)?;
        fs::create_dir_all(&self.keys_dir)?;

        // Set directory permissions to 700
        fs::set_permissions(&self.root, Permissions::from_mode(0o700))?;
        fs::set_permissions(&self.vault_dir, Permissions::from_mode(0o700))?;
        fs::set_permissions(&self.keys_dir, Permissions::from_mode(0o700))?;

        // Check if identity already exists
        if self.identity_path.exists() {
            // Return existing public key
            return self.get_public_key();
        }

        // Generate new identity
        let identity = age::x25519::Identity::generate();
        let identity_str = identity.to_string();

        // Write identity to file with secure permissions
        let mut file = File::create(&self.identity_path)?;
        file.write_all(identity_str.expose_secret().as_bytes())?;
        fs::set_permissions(&self.identity_path, Permissions::from_mode(0o600))?;

        // Return public key
        Ok(identity.to_public().to_string())
    }

    /// Check if the vault is initialized
    pub fn is_initialized(&self) -> bool {
        self.identity_path.exists()
    }

    /// Ensure the vault is initialized
    fn ensure_initialized(&self) -> Result<()> {
        if !self.is_initialized() {
            bail!(VaultError::NotInitialized);
        }
        Ok(())
    }

    /// Get the public key (recipient)
    pub fn get_public_key(&self) -> Result<String> {
        self.ensure_initialized()?;

        let identity = self.load_identity()?;
        Ok(identity.to_public().to_string())
    }

    /// Load the identity from file
    fn load_identity(&self) -> Result<age::x25519::Identity> {
        let content = fs::read_to_string(&self.identity_path)
            .context("Failed to read identity key")?;

        content
            .parse::<age::x25519::Identity>()
            .map_err(|e| anyhow::anyhow!("Failed to parse identity: {}", e))
    }

    /// Validate a key name
    fn validate_key(&self, key: &str) -> Result<()> {
        if key.is_empty() {
            bail!(VaultError::InvalidKey("Key cannot be empty".to_string()));
        }

        // Check for invalid characters
        if key.contains("..") || key.starts_with('/') || key.ends_with('/') {
            bail!(VaultError::InvalidKey(format!(
                "Invalid key format: {}",
                key
            )));
        }

        // Check for disallowed characters
        for c in key.chars() {
            if !c.is_alphanumeric() && c != '/' && c != '_' && c != '-' && c != '.' {
                bail!(VaultError::InvalidKey(format!(
                    "Invalid character '{}' in key",
                    c
                )));
            }
        }

        Ok(())
    }

    /// Get the path for a secret file
    fn secret_path(&self, key: &str) -> PathBuf {
        self.vault_dir.join(format!("{}.age", key))
    }

    /// Store a secret
    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        self.ensure_initialized()?;
        self.validate_key(key)?;

        if value.is_empty() {
            bail!("Empty value not allowed");
        }

        let identity = self.load_identity()?;
        let recipient = identity.to_public();

        // Create parent directories for namespaced keys
        let secret_path = self.secret_path(key);
        if let Some(parent) = secret_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Encrypt the value
        let encryptor = age::Encryptor::with_recipients(vec![Box::new(recipient)])
            .expect("Failed to create encryptor");

        let mut encrypted = vec![];
        let mut writer = encryptor
            .wrap_output(&mut encrypted)
            .map_err(|e| VaultError::Encryption(e.to_string()))?;

        writer
            .write_all(value.as_bytes())
            .map_err(|e| VaultError::Encryption(e.to_string()))?;

        writer
            .finish()
            .map_err(|e| VaultError::Encryption(e.to_string()))?;

        // Write encrypted data with secure permissions
        let mut file = File::create(&secret_path)?;
        file.write_all(&encrypted)?;
        fs::set_permissions(&secret_path, Permissions::from_mode(0o600))?;

        Ok(())
    }

    /// Retrieve a secret
    pub fn get(&self, key: &str) -> Result<String> {
        self.ensure_initialized()?;
        self.validate_key(key)?;

        let secret_path = self.secret_path(key);
        if !secret_path.exists() {
            bail!(VaultError::NotFound(key.to_string()));
        }

        let identity = self.load_identity()?;

        // Read encrypted data
        let encrypted = fs::read(&secret_path)?;

        // Decrypt
        let decryptor = match age::Decryptor::new(&encrypted[..])
            .map_err(|e| VaultError::Decryption(e.to_string()))?
        {
            age::Decryptor::Recipients(d) => d,
            _ => bail!(VaultError::Decryption(
                "Unexpected passphrase encryption".to_string()
            )),
        };

        let mut decrypted = vec![];
        let mut reader = decryptor
            .decrypt(std::iter::once(&identity as &dyn age::Identity))
            .map_err(|e| VaultError::Decryption(e.to_string()))?;

        reader
            .read_to_end(&mut decrypted)
            .map_err(|e| VaultError::Decryption(e.to_string()))?;

        String::from_utf8(decrypted).context("Secret is not valid UTF-8")
    }

    /// Delete a secret
    pub fn delete(&self, key: &str) -> Result<()> {
        self.ensure_initialized()?;
        self.validate_key(key)?;

        let secret_path = self.secret_path(key);
        if !secret_path.exists() {
            bail!(VaultError::NotFound(key.to_string()));
        }

        // Remove the file
        fs::remove_file(&secret_path)?;

        // Clean up empty parent directories
        let mut parent = secret_path.parent();
        while let Some(dir) = parent {
            if dir == self.vault_dir {
                break;
            }
            if dir.read_dir()?.next().is_none() {
                fs::remove_dir(dir)?;
                parent = dir.parent();
            } else {
                break;
            }
        }

        Ok(())
    }

    /// List all secrets (optionally filtered by prefix)
    pub fn list(&self, prefix: Option<&str>) -> Result<Vec<Secret>> {
        if !self.vault_dir.exists() {
            return Ok(vec![]);
        }

        let mut secrets = vec![];
        self.list_recursive(&self.vault_dir, "", prefix, &mut secrets)?;

        // Sort by key
        secrets.sort_by(|a, b| a.key.cmp(&b.key));

        Ok(secrets)
    }

    /// Recursively list secrets
    fn list_recursive(
        &self,
        dir: &Path,
        current_prefix: &str,
        filter_prefix: Option<&str>,
        secrets: &mut Vec<Secret>,
    ) -> Result<()> {
        if !dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if path.is_dir() {
                // Recurse into subdirectory
                let new_prefix = if current_prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{}/{}", current_prefix, name)
                };
                self.list_recursive(&path, &new_prefix, filter_prefix, secrets)?;
            } else if name.ends_with(".age") {
                // Extract key from filename
                let key_name = name.trim_end_matches(".age");
                let full_key = if current_prefix.is_empty() {
                    key_name.to_string()
                } else {
                    format!("{}/{}", current_prefix, key_name)
                };

                // Apply prefix filter if specified
                if let Some(prefix) = filter_prefix {
                    if !full_key.starts_with(prefix) {
                        continue;
                    }
                }

                secrets.push(Secret {
                    key: full_key,
                    path,
                });
            }
        }

        Ok(())
    }

    /// Check if a secret exists
    pub fn exists(&self, key: &str) -> bool {
        self.secret_path(key).exists()
    }

    /// Get all secrets as environment variable assignments
    pub fn env(&self, prefix: Option<&str>) -> Result<Vec<(String, String)>> {
        self.ensure_initialized()?;

        let secrets = self.list(prefix)?;
        let mut env_vars = vec![];

        for secret in secrets {
            match self.get(&secret.key) {
                Ok(value) => {
                    env_vars.push((secret.to_env_var(), value));
                }
                Err(e) => {
                    eprintln!("Warning: Failed to decrypt {}: {}", secret.key, e);
                }
            }
        }

        Ok(env_vars)
    }

    /// Export all secrets to an encrypted tarball
    pub fn export(&self, output: &Path) -> Result<()> {
        self.ensure_initialized()?;

        use flate2::write::GzEncoder;
        use flate2::Compression;
        use tar::Builder;

        // Create a tarball of the vault directory
        let mut tar_data = vec![];
        {
            let gz = GzEncoder::new(&mut tar_data, Compression::default());
            let mut tar_builder = Builder::new(gz);
            tar_builder.append_dir_all("vault", &self.vault_dir)?;
            tar_builder.finish()?;
        }

        // Encrypt the tarball
        let identity = self.load_identity()?;
        let recipient = identity.to_public();

        let encryptor = age::Encryptor::with_recipients(vec![Box::new(recipient)])
            .expect("Failed to create encryptor");

        let mut encrypted = vec![];
        let mut writer = encryptor
            .wrap_output(&mut encrypted)
            .map_err(|e| VaultError::Encryption(e.to_string()))?;

        writer.write_all(&tar_data)?;
        writer
            .finish()
            .map_err(|e| VaultError::Encryption(e.to_string()))?;

        // Write to output file
        fs::write(output, encrypted)?;

        Ok(())
    }

    /// Import secrets from an encrypted tarball
    pub fn import(&self, input: &Path) -> Result<()> {
        self.ensure_initialized()?;

        use flate2::read::GzDecoder;
        use tar::Archive;

        let identity = self.load_identity()?;

        // Read encrypted data
        let encrypted = fs::read(input)?;

        // Decrypt
        let decryptor = match age::Decryptor::new(&encrypted[..])
            .map_err(|e| VaultError::Decryption(e.to_string()))?
        {
            age::Decryptor::Recipients(d) => d,
            _ => bail!(VaultError::Decryption(
                "Unexpected passphrase encryption".to_string()
            )),
        };

        let mut tar_data = vec![];
        let mut reader = decryptor
            .decrypt(std::iter::once(&identity as &dyn age::Identity))
            .map_err(|e| VaultError::Decryption(e.to_string()))?;

        reader.read_to_end(&mut tar_data)?;

        // Extract tarball
        let gz = GzDecoder::new(&tar_data[..]);
        let mut archive = Archive::new(gz);
        archive.unpack(&self.root)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_vault() -> (Vault, PathBuf) {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = env::temp_dir().join(format!(
            "secrets_test_{}_{}",
            std::process::id(),
            id
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        let vault = Vault::new(&temp_dir).unwrap();
        (vault, temp_dir)
    }

    fn cleanup(path: &Path) {
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn test_init() {
        let (vault, temp_dir) = temp_vault();

        // Should not be initialized initially
        assert!(!vault.is_initialized());

        // Initialize
        let pubkey = vault.init().unwrap();
        assert!(!pubkey.is_empty());
        assert!(pubkey.starts_with("age1"));

        // Should now be initialized
        assert!(vault.is_initialized());

        // Re-init should return same key
        let pubkey2 = vault.init().unwrap();
        assert_eq!(pubkey, pubkey2);

        cleanup(&temp_dir);
    }

    #[test]
    fn test_set_get() {
        let (vault, temp_dir) = temp_vault();
        vault.init().unwrap();

        // Set a secret
        vault.set("api/openai", "sk-test123").unwrap();

        // Get it back
        let value = vault.get("api/openai").unwrap();
        assert_eq!(value, "sk-test123");

        cleanup(&temp_dir);
    }

    #[test]
    fn test_namespaced_keys() {
        let (vault, temp_dir) = temp_vault();
        vault.init().unwrap();

        vault.set("api/openai", "key1").unwrap();
        vault.set("api/anthropic", "key2").unwrap();
        vault.set("db/prod", "key3").unwrap();

        assert_eq!(vault.get("api/openai").unwrap(), "key1");
        assert_eq!(vault.get("api/anthropic").unwrap(), "key2");
        assert_eq!(vault.get("db/prod").unwrap(), "key3");

        cleanup(&temp_dir);
    }

    #[test]
    fn test_list() {
        let (vault, temp_dir) = temp_vault();
        vault.init().unwrap();

        vault.set("api/openai", "key1").unwrap();
        vault.set("api/anthropic", "key2").unwrap();
        vault.set("db/prod", "key3").unwrap();

        let all = vault.list(None).unwrap();
        assert_eq!(all.len(), 3);

        let api_only = vault.list(Some("api")).unwrap();
        assert_eq!(api_only.len(), 2);

        cleanup(&temp_dir);
    }

    #[test]
    fn test_delete() {
        let (vault, temp_dir) = temp_vault();
        vault.init().unwrap();

        vault.set("test/secret", "value").unwrap();
        assert!(vault.exists("test/secret"));

        vault.delete("test/secret").unwrap();
        assert!(!vault.exists("test/secret"));

        cleanup(&temp_dir);
    }

    #[test]
    fn test_env_var_conversion() {
        let secret = Secret {
            key: "api/openai".to_string(),
            path: PathBuf::new(),
        };
        assert_eq!(secret.to_env_var(), "API_OPENAI");
    }

    #[test]
    fn test_invalid_keys() {
        let (vault, temp_dir) = temp_vault();
        vault.init().unwrap();

        // Empty key
        assert!(vault.set("", "value").is_err());

        // Key with ..
        assert!(vault.set("../escape", "value").is_err());

        // Key starting with /
        assert!(vault.set("/absolute", "value").is_err());

        cleanup(&temp_dir);
    }

    #[test]
    fn test_not_found() {
        let (vault, temp_dir) = temp_vault();
        vault.init().unwrap();

        let result = vault.get("nonexistent");
        assert!(result.is_err());

        cleanup(&temp_dir);
    }
}
