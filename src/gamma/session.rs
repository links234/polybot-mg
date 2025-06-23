//! Session-based storage management for gamma markets data
//! 
//! This module provides persistent session management for gamma markets fetching,
//! allowing resumption from last offset without re-fetching thousands of entries.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use anyhow::{Context, Result};
use tracing::{info, debug};
use crate::gamma::cache::Cursor;
use crate::gamma::types::MarketQuery;

/// Session metadata containing state for resumption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// Unique session identifier (auto-incrementing)
    pub session_id: u32,
    /// When the session was started
    pub start_date: DateTime<Utc>,
    /// Last update timestamp
    pub last_updated: DateTime<Utc>,
    /// Total markets fetched in this session
    pub total_markets_fetched: usize,
    /// Current offset/position for next fetch
    pub last_offset: usize,
    /// Cursor state for resumption
    pub cursor_state: Cursor,
    /// Whether this session has completed fetching all available data
    pub is_complete: bool,
    /// Last market ID fetched (for validation)
    pub last_market_id: Option<String>,
    /// Batch size used for fetching
    pub batch_size: usize,
    /// Query parameters used for this session
    pub query_params: MarketQuery,
    /// Total number of raw response files created
    pub total_files: usize,
    /// Last time we checked for new data (for incremental updates)
    pub last_check_time: Option<DateTime<Utc>>,
}

#[allow(dead_code)] // Session metadata API kept for future use
impl SessionMetadata {
    /// Create new session metadata
    pub fn new(session_id: u32, query: MarketQuery) -> Self {
        Self::new_with_offset(session_id, query, 0)
    }

    /// Create new session metadata starting from a specific offset
    pub fn new_with_offset(session_id: u32, query: MarketQuery, start_offset: usize) -> Self {
        let mut cursor = Cursor::new(500); // Default batch size
        cursor.count = start_offset;
        
        Self {
            session_id,
            start_date: Utc::now(),
            last_updated: Utc::now(),
            total_markets_fetched: 0,
            last_offset: start_offset,
            cursor_state: cursor,
            is_complete: false,
            last_market_id: None,
            batch_size: 500,
            query_params: query,
            total_files: 0,
            last_check_time: None,
        }
    }

    /// Update metadata after successful fetch
    pub fn update_after_fetch(
        &mut self, 
        markets_fetched: usize, 
        cursor: &Cursor,
        last_market_id: Option<String>
    ) {
        self.last_updated = Utc::now();
        self.last_check_time = Some(Utc::now());
        self.total_markets_fetched += markets_fetched;
        self.last_offset = cursor.count;
        self.cursor_state = cursor.clone();
        self.is_complete = cursor.is_exhausted;
        if let Some(id) = last_market_id {
            self.last_market_id = Some(id);
        }
        // Increment file count when we create a new raw response file
        if markets_fetched > 0 {
            self.total_files += 1;
        }
    }

    /// Get the filename for the raw response file at current offset
    pub fn get_current_raw_filename(&self) -> String {
        format!("raw-offset-{}.json", self.last_offset)
    }

    /// Get the filename for the next raw response file
    pub fn get_next_raw_filename(&self) -> String {
        format!("raw-offset-{}.json", self.last_offset)
    }
    
    /// Mark that we've checked for new data (even if none found)
    pub fn mark_checked_for_updates(&mut self) {
        self.last_check_time = Some(Utc::now());
        self.last_updated = Utc::now();
    }
}

/// Global session registry managing all sessions
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionRegistry {
    /// Map of session_id -> session directory name
    pub sessions: HashMap<u32, String>,
    /// Next session ID to assign
    pub next_session_id: u32,
    /// Currently active session (if any)
    pub active_session_id: Option<u32>,
    /// Last update timestamp
    pub last_updated: DateTime<Utc>,
}

#[allow(dead_code)] // Session registry API kept for future use
impl SessionRegistry {
    /// Load registry from disk or create new one
    pub fn load_or_create(base_path: &Path) -> Result<Self> {
        let registry_path = base_path.join("sessions.json");
        
        if registry_path.exists() {
            let content = fs::read_to_string(&registry_path)
                .context("Failed to read sessions registry")?;
            let registry: SessionRegistry = serde_json::from_str(&content)
                .context("Failed to parse sessions registry")?;
            Ok(registry)
        } else {
            Ok(SessionRegistry {
                next_session_id: 1,
                last_updated: Utc::now(),
                ..Default::default()
            })
        }
    }

    /// Save registry to disk
    pub fn save(&self, base_path: &Path) -> Result<()> {
        let registry_path = base_path.join("sessions.json");
        let content = serde_json::to_string_pretty(self)
            .context("Failed to serialize sessions registry")?;
        
        fs::write(&registry_path, content)
            .context("Failed to write sessions registry")?;
        
        Ok(())
    }

    /// Create a new session and return its ID and directory name
    pub fn create_session(&mut self) -> (u32, String) {
        let session_id = self.next_session_id;
        let session_dir = format!("session-{:03}", session_id);
        
        self.sessions.insert(session_id, session_dir.clone());
        self.active_session_id = Some(session_id);
        self.next_session_id += 1;
        self.last_updated = Utc::now();
        
        (session_id, session_dir)
    }

    /// Get the most recent incomplete session (for resumption)
    pub fn get_resumable_session(&self, base_path: &Path) -> Option<(u32, SessionMetadata)> {
        // Check sessions in reverse order (most recent first)
        for session_id in (1..self.next_session_id).rev() {
            if let Some(session_dir) = self.sessions.get(&session_id) {
                let session_path = base_path.join(session_dir);
                if let Ok(metadata) = SessionMetadata::load(&session_path) {
                    if !metadata.is_complete {
                        return Some((session_id, metadata));
                    }
                }
            }
        }
        None
    }

    /// Get active session metadata
    pub fn get_active_session(&self, base_path: &Path) -> Option<(u32, SessionMetadata)> {
        if let Some(session_id) = self.active_session_id {
            if let Some(session_dir) = self.sessions.get(&session_id) {
                let session_path = base_path.join(session_dir);
                if let Ok(metadata) = SessionMetadata::load(&session_path) {
                    return Some((session_id, metadata));
                }
            }
        }
        None
    }

    /// Get the last completed session to continue from its offset
    pub fn get_last_completed_session(&self, base_path: &Path) -> Option<(u32, SessionMetadata)> {
        debug!("Looking for last completed session. next_session_id: {}, sessions: {:?}", 
               self.next_session_id, self.sessions);
        
        // Check sessions in reverse order (most recent first)
        for session_id in (1..self.next_session_id).rev() {
            debug!("Checking session {}", session_id);
            
            if let Some(session_dir) = self.sessions.get(&session_id) {
                debug!("Found session directory: {}", session_dir);
                let session_path = base_path.join(session_dir);
                
                match SessionMetadata::load(&session_path) {
                    Ok(metadata) => {
                        debug!("Loaded metadata for session {}: is_complete={}, last_offset={}", 
                               session_id, metadata.is_complete, metadata.last_offset);
                        
                        if metadata.is_complete {
                            info!("Found last completed session {}", session_id);
                            return Some((session_id, metadata));
                        }
                    },
                    Err(e) => {
                        debug!("Failed to load metadata for session {}: {}", session_id, e);
                    }
                }
            } else {
                debug!("No directory found for session {}", session_id);
            }
        }
        
        debug!("No completed sessions found");
        None
    }
}

/// Session manager for handling session-based gamma markets fetching
pub struct SessionManager {
    /// Base directory for all sessions (data/gamma/raw/)
    pub base_path: PathBuf,
    /// Session registry
    pub registry: SessionRegistry,
}

impl SessionManager {
    /// Create new session manager
    pub fn new(base_path: PathBuf) -> Result<Self> {
        // Ensure base directory exists
        if !base_path.exists() {
            fs::create_dir_all(&base_path)
                .context("Failed to create session base directory")?;
        }

        let registry = SessionRegistry::load_or_create(&base_path)?;

        Ok(Self {
            base_path,
            registry,
        })
    }

    /// Start a new session or resume existing incomplete session
    pub fn start_or_resume_session(&mut self, query: MarketQuery, force_new: bool) -> Result<(u32, PathBuf, SessionMetadata)> {
        // Try to resume existing session if not forcing new
        if !force_new {
            if let Some((session_id, metadata)) = self.registry.get_resumable_session(&self.base_path) {
                let session_dir = self.registry.sessions.get(&session_id).unwrap();
                let session_path = self.base_path.join(session_dir);
                
                info!("Resuming session {} from offset {}", session_id, metadata.last_offset);
                return Ok((session_id, session_path, metadata));
            }
        }

        // Create new session, continuing from last completed session's offset
        let (session_id, session_dir) = self.registry.create_session();
        let session_path = self.base_path.join(&session_dir);
        
        // Create session directory
        fs::create_dir_all(&session_path)
            .context("Failed to create session directory")?;

        // Check if there's a completed session to continue from
        let start_offset = if let Some((last_session_id, last_metadata)) = self.registry.get_last_completed_session(&self.base_path) {
            // Continue from where the last session ended
            // Use the total count from cursor state as the next starting point
            let next_offset = last_metadata.cursor_state.count;
            info!("Continuing from completed session {} - starting at offset {} (total markets: {})", 
                  last_session_id, next_offset, last_metadata.total_markets_fetched);
            next_offset
        } else {
            info!("No previous completed sessions found - starting from offset 0");
            0
        };

        // Create initial metadata with appropriate starting offset
        let metadata = SessionMetadata::new_with_offset(session_id, query, start_offset);
        metadata.save(&session_path)?;

        // Save registry
        self.registry.save(&self.base_path)?;

        info!("Created new session {} in {}", session_id, session_dir);
        Ok((session_id, session_path, metadata))
    }

    /// Save metadata for a session
    pub fn save_session_metadata(&self, session_path: &Path, metadata: &SessionMetadata) -> Result<()> {
        metadata.save(session_path)?;
        self.registry.save(&self.base_path)?;
        Ok(())
    }

    /// Store raw markets response in session directory
    pub fn store_raw_response(
        &self, 
        session_path: &Path, 
        offset: usize, 
        raw_json: &str
    ) -> Result<()> {
        let filename = format!("raw-offset-{}.json", offset);
        let file_path = session_path.join(filename);
        
        fs::write(&file_path, raw_json)
            .context("Failed to write raw response file")?;
        
        debug!("Stored raw JSON response in file at offset {}", offset);
        Ok(())
    }

    /// List all sessions with their status
    pub fn list_sessions(&self) -> Vec<(u32, String, bool)> {
        let mut sessions = Vec::new();
        
        for (&session_id, session_dir) in &self.registry.sessions {
            let session_path = self.base_path.join(session_dir);
            let is_complete = if let Ok(metadata) = SessionMetadata::load(&session_path) {
                metadata.is_complete
            } else {
                false
            };
            
            sessions.push((session_id, session_dir.clone(), is_complete));
        }
        
        sessions.sort_by_key(|(id, _, _)| *id);
        sessions
    }
}

impl SessionMetadata {
    /// Load metadata from session directory
    pub fn load(session_path: &Path) -> Result<Self> {
        let metadata_path = session_path.join("metadata.json");
        let content = fs::read_to_string(&metadata_path)
            .context("Failed to read metadata")?;
        
        let metadata: SessionMetadata = serde_json::from_str(&content)
            .context("Failed to parse metadata")?;
        
        Ok(metadata)
    }

    /// Save metadata to session directory
    pub fn save(&self, session_path: &Path) -> Result<()> {
        let metadata_path = session_path.join("metadata.json");
        let content = serde_json::to_string_pretty(self)
            .context("Failed to serialize metadata")?;
        
        fs::write(&metadata_path, content)
            .context("Failed to write metadata")?;
        
        Ok(())
    }
}