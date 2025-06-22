//! Database storage for immutable historical data
//! 
//! This module provides persistent storage for activity and position data
//! to enable incremental syncing and avoid re-fetching historical data.

use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use tracing::{info, warn};

/// Database for storing immutable historical data
pub struct HistoricalDatabase {
    root_path: PathBuf,
}

/// Sync state for an address
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SyncState {
    pub address: String,
    pub last_activity_timestamp: Option<DateTime<Utc>>,
    pub last_activity_id: Option<String>,
    pub total_activities_stored: usize,
    pub total_positions_stored: usize,
    pub last_sync_completed: Option<DateTime<Utc>>,
    pub last_sync_started: DateTime<Utc>,
    pub sync_in_progress: bool,
}

/// Summary of stored data
#[derive(Debug, Clone)]
pub struct StoredDataSummary {
    pub total_activities: usize,
    pub total_positions: usize,
    pub _earliest_activity: Option<DateTime<Utc>>,
    pub _latest_activity: Option<DateTime<Utc>>,
    pub last_sync: Option<DateTime<Utc>>,
}

impl HistoricalDatabase {
    /// Create new database instance
    pub fn new(data_paths: &crate::data::DataPaths) -> Self {
        let root_path = data_paths.root().join("historical_db");
        Self { root_path }
    }

    /// Get database path for an address
    fn get_address_path(&self, address: &str) -> PathBuf {
        self.root_path.join(address.to_lowercase())
    }

    /// Initialize database for an address
    pub async fn init_address(&self, address: &str) -> Result<()> {
        let addr_path = self.get_address_path(address);
        
        // Create directory structure
        tokio::fs::create_dir_all(&addr_path).await
            .with_context(|| format!("Failed to create DB directory for {}", address))?;
        
        // Create subdirectories for different data types
        tokio::fs::create_dir_all(addr_path.join("activities")).await?;
        tokio::fs::create_dir_all(addr_path.join("positions")).await?;
        tokio::fs::create_dir_all(addr_path.join("state")).await?;
        
        info!("Initialized historical database for {}", address);
        Ok(())
    }

    /// Get current sync state for an address
    pub async fn get_sync_state(&self, address: &str) -> Result<Option<SyncState>> {
        let state_file = self.get_address_path(address).join("state").join("sync_state.json");
        
        if !tokio::fs::try_exists(&state_file).await.unwrap_or(false) {
            return Ok(None);
        }
        
        let content = tokio::fs::read_to_string(&state_file).await
            .with_context(|| format!("Failed to read sync state for {}", address))?;
        
        let state: SyncState = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse sync state for {}", address))?;
        
        Ok(Some(state))
    }

    /// Save sync state
    pub async fn save_sync_state(&self, state: &SyncState) -> Result<()> {
        let state_file = self.get_address_path(&state.address).join("state").join("sync_state.json");
        
        // Ensure directory exists
        if let Some(parent) = state_file.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        let json = serde_json::to_string_pretty(state)?;
        
        // Atomic write
        let temp_file = state_file.with_extension("tmp");
        tokio::fs::write(&temp_file, &json).await?;
        tokio::fs::rename(&temp_file, &state_file).await?;
        
        Ok(())
    }

    /// Store activities in batches
    pub async fn store_activities(&self, address: &str, activities: &[Value], batch_id: usize) -> Result<()> {
        let activities_dir = self.get_address_path(address).join("activities");
        
        // Create batch file
        let batch_file = activities_dir.join(format!("batch_{:06}.json", batch_id));
        
        let batch_data = serde_json::json!({
            "batch_id": batch_id,
            "timestamp": Utc::now(),
            "count": activities.len(),
            "activities": activities
        });
        
        // Atomic write
        let temp_file = batch_file.with_extension("tmp");
        let json = serde_json::to_string_pretty(&batch_data)?;
        tokio::fs::write(&temp_file, &json).await?;
        tokio::fs::rename(&temp_file, &batch_file).await?;
        
        info!("Stored {} activities in batch {} for {}", activities.len(), batch_id, address);
        Ok(())
    }

    /// Store positions in batches
    pub async fn store_positions(&self, address: &str, positions: &[Value], batch_id: usize) -> Result<()> {
        let positions_dir = self.get_address_path(address).join("positions");
        
        // Create batch file
        let batch_file = positions_dir.join(format!("batch_{:06}.json", batch_id));
        
        let batch_data = serde_json::json!({
            "batch_id": batch_id,
            "timestamp": Utc::now(),
            "count": positions.len(),
            "positions": positions
        });
        
        // Atomic write
        let temp_file = batch_file.with_extension("tmp");
        let json = serde_json::to_string_pretty(&batch_data)?;
        tokio::fs::write(&temp_file, &json).await?;
        tokio::fs::rename(&temp_file, &batch_file).await?;
        
        info!("Stored {} positions in batch {} for {}", positions.len(), batch_id, address);
        Ok(())
    }

    /// Load all stored activities for an address
    pub async fn load_all_activities(&self, address: &str) -> Result<Vec<Value>> {
        let activities_dir = self.get_address_path(address).join("activities");
        
        if !tokio::fs::try_exists(&activities_dir).await.unwrap_or(false) {
            return Ok(Vec::new());
        }
        
        let mut all_activities = Vec::new();
        let mut entries = tokio::fs::read_dir(&activities_dir).await?;
        
        let mut batch_files = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                    if file_name.starts_with("batch_") {
                        batch_files.push(path);
                    }
                }
            }
        }
        
        // Sort batch files to maintain order
        batch_files.sort();
        
        // Load each batch
        for batch_file in batch_files {
            match tokio::fs::read_to_string(&batch_file).await {
                Ok(content) => {
                    match serde_json::from_str::<Value>(&content) {
                        Ok(batch_data) => {
                            if let Some(activities) = batch_data.get("activities").and_then(|a| a.as_array()) {
                                all_activities.extend(activities.iter().cloned());
                            }
                        }
                        Err(e) => {
                            warn!("Failed to parse batch file {:?}: {}", batch_file, e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to read batch file {:?}: {}", batch_file, e);
                }
            }
        }
        
        info!("Loaded {} stored activities for {}", all_activities.len(), address);
        Ok(all_activities)
    }

    /// Load all stored positions for an address
    pub async fn load_all_positions(&self, address: &str) -> Result<Vec<Value>> {
        let positions_dir = self.get_address_path(address).join("positions");
        
        if !tokio::fs::try_exists(&positions_dir).await.unwrap_or(false) {
            return Ok(Vec::new());
        }
        
        let mut all_positions = Vec::new();
        let mut entries = tokio::fs::read_dir(&positions_dir).await?;
        
        let mut batch_files = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                    if file_name.starts_with("batch_") {
                        batch_files.push(path);
                    }
                }
            }
        }
        
        // Sort batch files to maintain order
        batch_files.sort();
        
        // Load each batch
        for batch_file in batch_files {
            match tokio::fs::read_to_string(&batch_file).await {
                Ok(content) => {
                    match serde_json::from_str::<Value>(&content) {
                        Ok(batch_data) => {
                            if let Some(positions) = batch_data.get("positions").and_then(|p| p.as_array()) {
                                all_positions.extend(positions.iter().cloned());
                            }
                        }
                        Err(e) => {
                            warn!("Failed to parse batch file {:?}: {}", batch_file, e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to read batch file {:?}: {}", batch_file, e);
                }
            }
        }
        
        info!("Loaded {} stored positions for {}", all_positions.len(), address);
        Ok(all_positions)
    }

    /// Get summary of stored data
    pub async fn get_stored_summary(&self, address: &str) -> Result<StoredDataSummary> {
        let sync_state = self.get_sync_state(address).await?;
        
        let activities_dir = self.get_address_path(address).join("activities");
        let positions_dir = self.get_address_path(address).join("positions");
        
        // Count activity batches
        let mut _activity_count = 0;
        if tokio::fs::try_exists(&activities_dir).await.unwrap_or(false) {
            let mut entries = tokio::fs::read_dir(&activities_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                if entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
                    _activity_count += 1;
                }
            }
        }
        
        // Count position batches  
        let mut _position_count = 0;
        if tokio::fs::try_exists(&positions_dir).await.unwrap_or(false) {
            let mut entries = tokio::fs::read_dir(&positions_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                if entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
                    _position_count += 1;
                }
            }
        }
        
        Ok(StoredDataSummary {
            total_activities: sync_state.as_ref().map(|s| s.total_activities_stored).unwrap_or(0),
            total_positions: sync_state.as_ref().map(|s| s.total_positions_stored).unwrap_or(0),
            _earliest_activity: None, // Could be extracted from first batch
            _latest_activity: sync_state.as_ref().and_then(|s| s.last_activity_timestamp),
            last_sync: sync_state.as_ref().and_then(|s| s.last_sync_completed),
        })
    }

    /// Clear all stored data for an address (use with caution)
    pub async fn _clear_address_data(&self, address: &str) -> Result<()> {
        let addr_path = self.get_address_path(address);
        
        if tokio::fs::try_exists(&addr_path).await.unwrap_or(false) {
            tokio::fs::remove_dir_all(&addr_path).await
                .with_context(|| format!("Failed to clear data for {}", address))?;
            info!("Cleared all stored data for {}", address);
        }
        
        Ok(())
    }

    /// Get the latest activity timestamp from stored data
    pub async fn _get_latest_activity_timestamp(&self, address: &str) -> Result<Option<DateTime<Utc>>> {
        let sync_state = self.get_sync_state(address).await?;
        Ok(sync_state.and_then(|s| s.last_activity_timestamp))
    }
}