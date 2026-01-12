//! secrets - Local secrets vault for Daedalos
//!
//! Secure storage for API keys, tokens, and credentials.
//! Uses age encryption (https://age-encryption.org)
//!
//! Commands:
//! - init: Initialize vault, generate identity key
//! - set <KEY> [VALUE]: Store a secret (prompts if no value)
//! - get <KEY>: Retrieve a secret
//! - list: List all secret keys
//! - delete <KEY>: Delete a secret
//! - env [PREFIX]: Output as environment variables
//! - inject <CMD>: Run command with secrets in environment
//! - export [FILE]: Export secrets (encrypted)
//! - import <FILE>: Import secrets
//! - key: Show public key

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use daedalos_core::Paths;
use secrets::Vault;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser)]
#[command(name = "secrets")]
#[command(about = "Local secrets vault for Daedalos - encrypted storage for API keys, tokens, and credentials")]
#[command(version)]
#[command(after_help = r#"SECRET NAMING:
    Use namespaced keys for organization:
    - api/openai         OpenAI API key
    - api/anthropic      Anthropic API key
    - db/postgres        Database credentials
    - aws/access_key     AWS credentials

SECURITY:
    - Secrets are encrypted with age (X25519 + ChaCha20-Poly1305)
    - Identity key stored in ~/.local/share/daedalos/secrets/keys/
    - Vault stored in ~/.local/share/daedalos/secrets/vault/
    - Never logged or sent anywhere"#)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize vault and generate X25519 identity key
    Init,

    /// Store a secret (prompts securely if value not provided)
    Set {
        /// Secret key name (e.g., api/openai, db/postgres)
        key: String,
        /// Secret value (omit for secure hidden prompt)
        value: Option<String>,
    },

    /// Retrieve and print a secret value
    Get {
        /// Don't print trailing newline (useful for piping)
        #[arg(short = 'n')]
        no_newline: bool,
        /// Secret key name
        key: String,
    },

    /// List stored secret keys (values hidden)
    List {
        /// Filter by prefix (e.g., "api" to list api/*)
        prefix: Option<String>,
        /// Output as JSON for scripting
        #[arg(long)]
        json: bool,
    },

    /// Delete a secret permanently
    Delete {
        /// Secret key name to delete
        key: String,
    },

    /// Output secrets as shell export statements (eval $(secrets env))
    Env {
        /// Filter by prefix (e.g., "api" for API keys only)
        prefix: Option<String>,
    },

    /// Run a command with secrets injected as environment variables
    Inject {
        /// Command to run with secrets in environment
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
    },

    /// Export all secrets to encrypted .age file for backup/sharing
    Export {
        /// Output file path
        #[arg(default_value = "secrets_export.age")]
        file: PathBuf,
    },

    /// Import secrets from encrypted .age file
    Import {
        /// Encrypted secrets file to import
        file: PathBuf,
    },

    /// Show your public key (share with others to receive encrypted secrets)
    Key,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let paths = Paths::new();
    let secrets_dir = paths.data.join("secrets");
    let vault = Vault::new(&secrets_dir)?;

    match cli.command {
        Some(Commands::Init) => cmd_init(&vault),
        Some(Commands::Set { key, value }) => cmd_set(&vault, &key, value),
        Some(Commands::Get { no_newline, key }) => cmd_get(&vault, &key, no_newline),
        Some(Commands::List { prefix, json }) => cmd_list(&vault, prefix.as_deref(), json),
        Some(Commands::Delete { key }) => cmd_delete(&vault, &key),
        Some(Commands::Env { prefix }) => cmd_env(&vault, prefix.as_deref()),
        Some(Commands::Inject { command }) => cmd_inject(&vault, &command),
        Some(Commands::Export { file }) => cmd_export(&vault, &file),
        Some(Commands::Import { file }) => cmd_import(&vault, &file),
        Some(Commands::Key) => cmd_key(&vault),
        None => {
            // Default to listing secrets
            cmd_list(&vault, None, false)
        }
    }
}

/// Initialize the vault
fn cmd_init(vault: &Vault) -> Result<()> {
    if vault.is_initialized() {
        println!("warning: Vault already initialized");
        let pubkey = vault.get_public_key()?;
        println!("Public key: {}", pubkey);
        return Ok(());
    }

    println!("info: Generating new identity key...");
    let pubkey = vault.init()?;

    println!("success: Vault initialized");
    println!();
    println!("Your public key (for sharing encrypted secrets):");
    println!("  {}", pubkey);
    println!();
    println!("Store secrets with: secrets set <key> <value>");

    Ok(())
}

/// Store a secret
fn cmd_set(vault: &Vault, key: &str, value: Option<String>) -> Result<()> {
    // Get value - prompt if not provided
    let secret_value = match value {
        Some(v) => v,
        None => {
            // Prompt for hidden input
            let password = rpassword::prompt_password("Enter secret value: ")
                .context("Failed to read secret value")?;

            if password.is_empty() {
                bail!("Empty value not allowed");
            }

            password
        }
    };

    vault.set(key, &secret_value)?;

    println!("success: Secret stored: {}", key);

    Ok(())
}

/// Retrieve a secret
fn cmd_get(vault: &Vault, key: &str, no_newline: bool) -> Result<()> {
    let value = vault.get(key)?;

    if no_newline {
        print!("{}", value);
    } else {
        println!("{}", value);
    }

    Ok(())
}

/// List all secrets
fn cmd_list(vault: &Vault, prefix: Option<&str>, json: bool) -> Result<()> {
    let secrets = vault.list(prefix)?;

    if json {
        let keys: Vec<&str> = secrets.iter().map(|s| s.key.as_str()).collect();
        println!("{}", serde_json::to_string_pretty(&keys)?);
        return Ok(());
    }

    if secrets.is_empty() {
        if prefix.is_some() {
            println!("No secrets found with prefix: {}", prefix.unwrap());
        } else {
            println!("No secrets stored. Add one with: secrets set <key>");
        }
        return Ok(());
    }

    println!("Stored Secrets");
    println!();

    for secret in &secrets {
        println!("  {}", secret.key);
    }

    Ok(())
}

/// Delete a secret
fn cmd_delete(vault: &Vault, key: &str) -> Result<()> {
    vault.delete(key)?;
    println!("success: Secret deleted: {}", key);
    Ok(())
}

/// Output secrets as environment variable exports
fn cmd_env(vault: &Vault, prefix: Option<&str>) -> Result<()> {
    let env_vars = vault.env(prefix)?;

    for (name, value) in env_vars {
        // Escape single quotes in the value
        let escaped = value.replace('\'', "'\\''");
        println!("export {}='{}'", name, escaped);
    }

    Ok(())
}

/// Run a command with secrets injected
fn cmd_inject(vault: &Vault, command: &[String]) -> Result<()> {
    if command.is_empty() {
        bail!("Command required. Usage: secrets inject <command>");
    }

    let env_vars = vault.env(None)?;

    // Build command
    let program = &command[0];
    let args = &command[1..];

    // Create command with injected environment
    let mut cmd = Command::new(program);
    cmd.args(args);

    // Add secrets to environment
    for (name, value) in env_vars {
        cmd.env(name, value);
    }

    // Replace current process with the command
    let err = cmd.exec();

    // exec() only returns if there was an error
    bail!("Failed to execute command: {}", err);
}

/// Export secrets to encrypted file
fn cmd_export(vault: &Vault, file: &PathBuf) -> Result<()> {
    vault.export(file)?;

    println!("success: Secrets exported to: {}", file.display());
    println!("Share this file securely. Recipient needs your identity key to decrypt.");

    Ok(())
}

/// Import secrets from encrypted file
fn cmd_import(vault: &Vault, file: &PathBuf) -> Result<()> {
    if !file.exists() {
        bail!("Import file not found: {}", file.display());
    }

    vault.import(file)?;

    println!("success: Secrets imported");

    Ok(())
}

/// Show public key
fn cmd_key(vault: &Vault) -> Result<()> {
    let pubkey = vault.get_public_key()?;
    println!("{}", pubkey);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parse() {
        // Test that CLI parses correctly
        let cli = Cli::try_parse_from(["secrets", "init"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Init)));

        let cli = Cli::try_parse_from(["secrets", "set", "api/key", "value"]).unwrap();
        if let Some(Commands::Set { key, value }) = cli.command {
            assert_eq!(key, "api/key");
            assert_eq!(value, Some("value".to_string()));
        } else {
            panic!("Expected Set command");
        }

        let cli = Cli::try_parse_from(["secrets", "get", "api/key"]).unwrap();
        if let Some(Commands::Get { key, no_newline }) = cli.command {
            assert_eq!(key, "api/key");
            assert!(!no_newline);
        } else {
            panic!("Expected Get command");
        }

        let cli = Cli::try_parse_from(["secrets", "get", "-n", "api/key"]).unwrap();
        if let Some(Commands::Get { key, no_newline }) = cli.command {
            assert_eq!(key, "api/key");
            assert!(no_newline);
        } else {
            panic!("Expected Get command");
        }
    }

    #[test]
    fn test_cli_inject() {
        let cli = Cli::try_parse_from(["secrets", "inject", "npm", "run", "dev"]).unwrap();
        if let Some(Commands::Inject { command }) = cli.command {
            assert_eq!(command, vec!["npm", "run", "dev"]);
        } else {
            panic!("Expected Inject command");
        }
    }
}
