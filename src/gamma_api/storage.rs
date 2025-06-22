//! Storage layer for Gamma API data

use anyhow::{Result, Context};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{info, debug, warn};
use chrono::{DateTime, Utc};

use super::types::*;
use crate::data::DataPaths;

/// User-specific data paths for gamma data
#[derive(Debug, Clone)]
pub struct UserDataPaths {
    /// Base directory for this user
    base_dir: PathBuf,
    
    /// User address
    address: String,
}

impl UserDataPaths {
    /// Create new user data paths
    pub fn new(data_paths: &DataPaths, address: &str) -> Self {
        let base_dir = data_paths.root()
            .join("gamma")
            .join("tracker")
            .join("raw")
            .join(address);
        
        Self {
            base_dir,
            address: address.to_string(),
        }
    }
    
    /// Get metadata file path
    pub fn metadata(&self) -> PathBuf {
        self.base_dir.join("metadata.json")
    }
    
    /// Get state file path
    pub fn state(&self) -> PathBuf {
        self.base_dir.join("state.json")
    }
    
    /// Get positions file path
    pub fn positions(&self) -> PathBuf {
        self.base_dir.join("positions.json")
    }
    
    /// Get activity file path
    pub fn activity(&self) -> PathBuf {
        self.base_dir.join("activity.json")
    }
    
    /// Get historical activity directory
    pub fn activity_history(&self) -> PathBuf {
        self.base_dir.join("activity_history")
    }
    
    /// Get backup directory
    pub fn backups(&self) -> PathBuf {
        self.base_dir.join("backups")
    }
    
    /// Get logs directory
    pub fn logs(&self) -> PathBuf {
        self.base_dir.join("logs")
    }
    
    /// Get user address
    pub fn address(&self) -> &str {
        &self.address
    }
    
    /// Get base directory
    pub fn base(&self) -> &Path {
        &self.base_dir
    }
}

/// Gamma data storage manager
pub struct GammaStorage {
    /// Base data paths
    data_paths: DataPaths,
}

impl GammaStorage {
    /// Create new gamma storage
    pub fn new(data_paths: DataPaths) -> Self {
        Self { data_paths }
    }
    
    /// Get user data paths
    pub fn user_paths(&self, address: &str) -> UserDataPaths {
        UserDataPaths::new(&self.data_paths, address)
    }
    
    /// Initialize storage for a user
    pub async fn init_user(&self, address: &str) -> Result<UserDataPaths> {
        let paths = self.user_paths(address);
        
        // Create all necessary directories
        fs::create_dir_all(&paths.base_dir).await
            .context("Failed to create user base directory")?;
        fs::create_dir_all(paths.activity_history()).await
            .context("Failed to create activity history directory")?;
        fs::create_dir_all(paths.backups()).await
            .context("Failed to create backups directory")?;
        fs::create_dir_all(paths.logs()).await
            .context("Failed to create logs directory")?;
        
        info!("Initialized gamma storage for user: {}", address);
        Ok(paths)
    }
    
    /// Save user metadata
    pub async fn save_metadata(&self, address: &str, metadata: &GammaMetadata) -> Result<()> {
        let paths = self.user_paths(address);
        self.ensure_user_dirs(&paths).await?;
        
        let json = serde_json::to_string_pretty(metadata)
            .context("Failed to serialize metadata")?;
        
        fs::write(paths.metadata(), json).await
            .context("Failed to write metadata file")?;
        
        debug!("Saved metadata for user: {}", address);
        Ok(())
    }
    
    /// Load user metadata
    pub async fn load_metadata(&self, address: &str) -> Result<Option<GammaMetadata>> {
        let paths = self.user_paths(address);
        
        if !paths.metadata().exists() {
            return Ok(None);
        }
        
        let content = fs::read_to_string(paths.metadata()).await
            .context("Failed to read metadata file")?;
        
        let metadata: GammaMetadata = serde_json::from_str(&content)
            .context("Failed to parse metadata")?;
        
        debug!("Loaded metadata for user: {}", address);
        Ok(Some(metadata))
    }
    
    /// Save user state
    pub async fn save_state(&self, address: &str, state: &UserState) -> Result<()> {
        let paths = self.user_paths(address);
        self.ensure_user_dirs(&paths).await?;
        
        let json = serde_json::to_string_pretty(state)
            .context("Failed to serialize state")?;
        
        fs::write(paths.state(), json).await
            .context("Failed to write state file")?;
        
        debug!("Saved state for user: {}", address);
        Ok(())
    }
    
    /// Load user state
    pub async fn load_state(&self, address: &str) -> Result<Option<UserState>> {
        let paths = self.user_paths(address);
        
        if !paths.state().exists() {
            return Ok(None);
        }
        
        let content = fs::read_to_string(paths.state()).await
            .context("Failed to read state file")?;
        
        let state: UserState = serde_json::from_str(&content)
            .context("Failed to parse state")?;
        
        debug!("Loaded state for user: {}", address);
        Ok(Some(state))
    }
    
    /// Save positions data
    pub async fn save_positions(&self, address: &str, positions: &[GammaPosition]) -> Result<()> {
        let paths = self.user_paths(address);
        self.ensure_user_dirs(&paths).await?;
        
        let json = serde_json::to_string_pretty(positions)
            .context("Failed to serialize positions")?;
        
        fs::write(paths.positions(), json).await
            .context("Failed to write positions file")?;
        
        debug!("Saved {} positions for user: {}", positions.len(), address);
        Ok(())
    }
    
    /// Load positions data
    pub async fn load_positions(&self, address: &str) -> Result<Vec<GammaPosition>> {
        let paths = self.user_paths(address);
        
        if !paths.positions().exists() {
            return Ok(Vec::new());
        }
        
        let content = fs::read_to_string(paths.positions()).await
            .context("Failed to read positions file")?;
        
        let positions: Vec<GammaPosition> = serde_json::from_str(&content)
            .context("Failed to parse positions")?;
        
        debug!("Loaded {} positions for user: {}", positions.len(), address);
        Ok(positions)
    }
    
    /// Save activity data
    pub async fn save_activity(&self, address: &str, activity: &[GammaActivity]) -> Result<()> {
        let paths = self.user_paths(address);
        self.ensure_user_dirs(&paths).await?;
        
        let json = serde_json::to_string_pretty(activity)
            .context("Failed to serialize activity")?;
        
        fs::write(paths.activity(), json).await
            .context("Failed to write activity file")?;
        
        debug!("Saved {} activities for user: {}", activity.len(), address);
        Ok(())
    }
    
    /// Load activity data
    pub async fn load_activity(&self, address: &str) -> Result<Vec<GammaActivity>> {
        let paths = self.user_paths(address);
        
        if !paths.activity().exists() {
            return Ok(Vec::new());
        }
        
        let content = fs::read_to_string(paths.activity()).await
            .context("Failed to read activity file")?;
        
        let activity: Vec<GammaActivity> = serde_json::from_str(&content)
            .context("Failed to parse activity")?;
        
        debug!("Loaded {} activities for user: {}", activity.len(), address);
        Ok(activity)
    }
    
    /// Archive activity data by date
    pub async fn archive_activity(&self, address: &str, activity: &[GammaActivity], date: DateTime<Utc>) -> Result<()> {
        let paths = self.user_paths(address);
        self.ensure_user_dirs(&paths).await?;
        
        let archive_file = paths.activity_history()
            .join(format!("activity_{}.json", date.format("%Y%m%d")));
        
        let json = serde_json::to_string_pretty(activity)
            .context("Failed to serialize activity for archiving")?;
        
        fs::write(archive_file, json).await
            .context("Failed to write activity archive")?;
        
        debug!("Archived {} activities for user: {} (date: {})", activity.len(), address, date.format("%Y-%m-%d"));
        Ok(())
    }
    
    /// Create backup of user data
    pub async fn create_backup(&self, address: &str) -> Result<()> {
        let paths = self.user_paths(address);
        self.ensure_user_dirs(&paths).await?;
        
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let backup_dir = paths.backups().join(format!("backup_{}", timestamp));
        
        fs::create_dir_all(&backup_dir).await
            .context("Failed to create backup directory")?;
        
        // Copy key files to backup
        for file in &["metadata.json", "state.json", "positions.json", "activity.json"] {
            let src = paths.base().join(file);
            let dst = backup_dir.join(file);
            
            if src.exists() {
                fs::copy(&src, &dst).await
                    .with_context(|| format!("Failed to backup {}", file))?;
            }
        }
        
        debug!("Created backup for user: {} at {:?}", address, backup_dir);
        Ok(())
    }
    
    /// List all tracked users
    pub async fn list_users(&self) -> Result<Vec<String>> {
        let gamma_dir = self.data_paths.root()
            .join("gamma")
            .join("tracker")
            .join("raw");
        
        if !gamma_dir.exists() {
            return Ok(Vec::new());
        }
        
        let mut users = Vec::new();
        let mut entries = fs::read_dir(&gamma_dir).await
            .context("Failed to read gamma directory")?;
        
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    // Validate it looks like an Ethereum address
                    if name.len() == 42 && name.starts_with("0x") {
                        users.push(name.to_string());
                    }
                }
            }
        }
        
        debug!("Found {} tracked users", users.len());
        Ok(users)
    }
    
    /// Get storage statistics for a user
    pub async fn get_user_stats(&self, address: &str) -> Result<UserStorageStats> {
        let paths = self.user_paths(address);
        
        let mut stats = UserStorageStats {
            address: address.to_string(),
            has_metadata: paths.metadata().exists(),
            has_state: paths.state().exists(),
            has_positions: paths.positions().exists(),
            has_activity: paths.activity().exists(),
            total_size_bytes: 0,
            last_modified: None,
        };
        
        // Calculate total size and last modified
        for path in &[paths.metadata(), paths.state(), paths.positions(), paths.activity()] {
            if let Ok(metadata) = fs::metadata(path).await {
                stats.total_size_bytes += metadata.len();
                
                if let Ok(modified) = metadata.modified() {
                    let modified_utc: DateTime<Utc> = modified.into();
                    if stats.last_modified.is_none() || stats.last_modified.unwrap() < modified_utc {
                        stats.last_modified = Some(modified_utc);
                    }
                }
            }
        }
        
        Ok(stats)
    }
    
    /// Ensure user directories exist
    async fn ensure_user_dirs(&self, paths: &UserDataPaths) -> Result<()> {
        fs::create_dir_all(&paths.base_dir).await
            .context("Failed to create user directories")?;
        Ok(())
    }
    
    /// Clean old backups (keep last N)
    pub async fn clean_old_backups(&self, address: &str, keep_count: usize) -> Result<()> {
        let paths = self.user_paths(address);
        let backups_dir = paths.backups();
        
        if !backups_dir.exists() {
            return Ok(());
        }
        
        let mut entries = fs::read_dir(&backups_dir).await?;
        let mut backups = Vec::new();
        
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                if let Ok(metadata) = entry.metadata().await {
                    if let Ok(modified) = metadata.modified() {
                        backups.push((entry.path(), modified));
                    }
                }
            }
        }
        
        // Sort by modification time, newest first
        backups.sort_by_key(|(_, time)| std::cmp::Reverse(*time));
        
        // Remove old backups
        for (path, _) in backups.into_iter().skip(keep_count) {
            if let Err(e) = fs::remove_dir_all(&path).await {
                warn!("Failed to remove old backup {:?}: {}", path, e);
            } else {
                debug!("Removed old backup: {:?}", path);
            }
        }
        
        Ok(())
    }
}

/// User storage statistics
#[derive(Debug, Clone)]
pub struct UserStorageStats {
    pub address: String,
    pub has_metadata: bool,
    pub has_state: bool,
    pub has_positions: bool,
    pub has_activity: bool,
    pub total_size_bytes: u64,
    pub last_modified: Option<DateTime<Utc>>,
}