//! Cache management for project index

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::database::ProjectDatabase;

/// Cache metadata
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CacheMeta {
    last_indexed: f64,
    project_path: String,
    project_type: Option<String>,
}

/// Cache manager for a project
pub struct CacheManager {
    project_path: PathBuf,
    cache_dir: PathBuf,
    meta_file: PathBuf,
    db_file: PathBuf,
    db: Option<ProjectDatabase>,
}

impl CacheManager {
    /// Create a new cache manager for a project
    pub fn new(project_path: &Path) -> Result<Self> {
        let project_path = project_path.to_path_buf();
        let cache_dir = get_cache_path(&project_path)?;

        std::fs::create_dir_all(&cache_dir).context("Failed to create cache directory")?;

        let meta_file = cache_dir.join("meta.json");
        let db_file = cache_dir.join("index.db");

        Ok(Self {
            project_path,
            cache_dir,
            meta_file,
            db_file,
            db: None,
        })
    }

    /// Get or create the database
    pub fn get_database(&mut self) -> Result<&ProjectDatabase> {
        if self.db.is_none() {
            self.db = Some(ProjectDatabase::new(&self.db_file)?);
        }
        Ok(self.db.as_ref().unwrap())
    }

    /// Open a new database connection (for use when you need separate ownership)
    pub fn open_database(&self) -> Result<ProjectDatabase> {
        ProjectDatabase::new(&self.db_file)
    }

    /// Get the database file path
    pub fn db_path(&self) -> &Path {
        &self.db_file
    }

    /// Check if the cache is stale (older than max_age_seconds)
    pub fn is_stale(&self) -> bool {
        self.is_stale_with_age(3600) // 1 hour default
    }

    /// Check if the cache is stale with custom max age
    pub fn is_stale_with_age(&self, max_age_seconds: u64) -> bool {
        if !self.meta_file.exists() {
            return true;
        }

        match self.load_meta() {
            Ok(meta) => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs_f64())
                    .unwrap_or(0.0);
                now - meta.last_indexed > max_age_seconds as f64
            }
            Err(_) => true,
        }
    }

    /// Mark the cache as freshly indexed
    pub fn mark_fresh(&mut self) -> Result<()> {
        let mut meta = self.load_meta().unwrap_or_default();
        meta.last_indexed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        meta.project_path = self.project_path.to_string_lossy().to_string();
        self.save_meta(&meta)
    }

    /// Get the stored project type
    pub fn get_project_type(&self) -> Option<String> {
        self.load_meta().ok().and_then(|m| m.project_type)
    }

    /// Set the project type
    pub fn set_project_type(&mut self, project_type: &str) -> Result<()> {
        let mut meta = self.load_meta().unwrap_or_default();
        meta.project_type = Some(project_type.to_string());
        self.save_meta(&meta)
    }

    /// Clear all cached data
    pub fn clear(&mut self) -> Result<()> {
        self.db = None;

        if self.db_file.exists() {
            std::fs::remove_file(&self.db_file)?;
        }
        if self.meta_file.exists() {
            std::fs::remove_file(&self.meta_file)?;
        }
        Ok(())
    }

    fn load_meta(&self) -> Result<CacheMeta> {
        let content = std::fs::read_to_string(&self.meta_file)?;
        let meta: CacheMeta = serde_json::from_str(&content)?;
        Ok(meta)
    }

    fn save_meta(&self, meta: &CacheMeta) -> Result<()> {
        let content = serde_json::to_string_pretty(meta)?;
        std::fs::write(&self.meta_file, content)?;
        Ok(())
    }
}

/// Get the cache directory for Daedalos
fn get_cache_dir() -> Result<PathBuf> {
    let cache_dir = if let Ok(xdg_cache) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(xdg_cache)
    } else if let Some(home) = dirs::home_dir() {
        home.join(".cache")
    } else {
        return Err(anyhow::anyhow!("Could not determine cache directory"));
    };

    Ok(cache_dir.join("daedalos").join("project"))
}

/// Get the cache path for a specific project
fn get_cache_path(project_path: &Path) -> Result<PathBuf> {
    let base = get_cache_dir()?;
    let path_str = project_path.to_string_lossy();

    let mut hasher = Sha256::new();
    hasher.update(path_str.as_bytes());
    let hash = hex::encode(&hasher.finalize()[..8]);

    Ok(base.join(hash))
}
