//! Immutable data cache and cursor-based pagination system for Gamma API
//! 
//! This module provides efficient caching and pagination to avoid refetching data
//! every time and to support incremental loading of large datasets.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use tokio::fs as tokio_fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tracing::{debug, info, warn};

use super::types::*;

/// Cursor for pagination through large datasets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cursor {
    /// Last fetched ID for markets
    pub last_market_id: Option<String>,
    /// Last fetched timestamp
    pub last_timestamp: Option<DateTime<Utc>>,
    /// Total count fetched so far
    pub count: usize,
    /// Page size for each fetch
    pub page_size: usize,
    /// Whether we've reached the end
    pub is_exhausted: bool,
    /// Whether the last page was incomplete and needs refresh
    pub last_page_incomplete: bool,
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            last_market_id: None,
            last_timestamp: None,
            count: 0,
            page_size: 500,
            is_exhausted: false,
            last_page_incomplete: false,
        }
    }
}

#[allow(dead_code)] // Cursor API kept for future use
impl Cursor {
    #[allow(dead_code)]
    pub fn new(page_size: usize) -> Self {
        Self {
            page_size,
            ..Default::default()
        }
    }

    #[allow(dead_code)]
    pub fn advance(&mut self, markets: &[GammaMarket]) {
        if let Some(last_market) = markets.last() {
            self.last_market_id = Some(last_market.id.0.to_string());
            self.last_timestamp = Some(last_market.created_at);
            self.count += markets.len();
            
            // Mark as exhausted if we got fewer than expected
            if markets.len() < self.page_size {
                self.is_exhausted = true;
                self.last_page_incomplete = false; // Complete smaller page
                info!("Cursor advanced to position {} - Exhausted (received {} < {})", 
                      self.count, markets.len(), self.page_size);
            } else {
                self.last_page_incomplete = false; // Full page received
                info!("Cursor advanced to position {} - More data available", self.count);
            }
        } else {
            self.is_exhausted = true;
            info!("Cursor marked as exhausted - Empty response");
        }
    }

    pub fn advance_by_count(&mut self, markets: &[GammaMarket], actual_added: usize) {
        if let Some(last_market) = markets.last() {
            self.last_market_id = Some(last_market.id.0.to_string());
            self.last_timestamp = Some(last_market.created_at);
            
            // CRITICAL FIX: Only advance by markets actually received from API, not stored count
            // This preserves the cursor position for pagination even when we have duplicates
            self.count += markets.len(); // Always advance by what API sent us
            
            // Mark as exhausted if we got fewer than expected FROM THE API
            if markets.len() < self.page_size {
                self.is_exhausted = true;
                self.last_page_incomplete = false; // Complete smaller page
                info!("Cursor advanced to position {} - Exhausted (API sent {} < {}, {} new stored)", 
                      self.count, markets.len(), self.page_size, actual_added);
            } else {
                self.last_page_incomplete = false; // Full page received
                info!("Cursor advanced to position {} - More data available ({} new stored)", 
                      self.count, actual_added);
            }
        } else {
            self.is_exhausted = true;
            info!("Cursor marked as exhausted - Empty response");
        }
    }

    pub fn reset(&mut self) {
        self.last_market_id = None;
        self.last_timestamp = None;
        self.count = 0;
        self.is_exhausted = false;
        self.last_page_incomplete = false;
    }
}

/// Serializable cache data for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableCache {
    pub markets: BTreeMap<String, GammaMarket>,
    pub events: HashMap<EventId, GammaEvent>,
    pub metadata: CacheMetadata,
    pub cursor: Cursor,
}

/// Lightweight cache metadata for quick saves
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheSnapshot {
    pub cursor: Cursor,
    pub metadata: CacheMetadata,
    pub market_count: usize,
    pub timestamp: DateTime<Utc>,
}

/// Cache metadata for tracking freshness and validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMetadata {
    pub last_updated: DateTime<Utc>,
    pub total_count: usize,
    pub last_refresh: DateTime<Utc>,
    pub is_complete: bool,
}

impl Default for CacheMetadata {
    fn default() -> Self {
        Self {
            last_updated: Utc::now(),
            total_count: 0,
            last_refresh: Utc::now(),
            is_complete: false,
        }
    }
}

/// Immutable cache for Gamma API data with cursor-based pagination
#[derive(Debug)]
pub struct GammaCache {
    /// Markets cache - immutable once stored
    markets: Arc<RwLock<BTreeMap<String, GammaMarket>>>,
    /// Events cache
    events: Arc<RwLock<HashMap<EventId, GammaEvent>>>,
    /// Cache metadata
    metadata: Arc<RwLock<CacheMetadata>>,
    /// Current cursor for pagination
    cursor: Arc<RwLock<Cursor>>,
    /// Cache expiry time in seconds
    cache_ttl: u64,
    /// Path for persistent storage
    cache_file_path: Option<PathBuf>,
}

impl Default for GammaCache {
    fn default() -> Self {
        Self::new(3600) // 1 hour TTL by default
    }
}

#[allow(dead_code)] // Cache API kept for future use
impl GammaCache {
    /// Create new cache with specified TTL
    pub fn new(cache_ttl: u64) -> Self {
        Self::new_with_path(cache_ttl, None)
    }

    /// Create new cache with specified TTL and persistent storage path
    /// For immutable data, use cache_ttl = 0 to disable expiration
    pub fn new_with_path(cache_ttl: u64, cache_file_path: Option<PathBuf>) -> Self {
        let mut cache = Self {
            markets: Arc::new(RwLock::new(BTreeMap::new())),
            events: Arc::new(RwLock::new(HashMap::new())),
            metadata: Arc::new(RwLock::new(CacheMetadata::default())),
            cursor: Arc::new(RwLock::new(Cursor::default())),
            cache_ttl, // 0 means no expiration
            cache_file_path: cache_file_path.clone(),
        };

        // Try to load from disk if path is provided
        if let Some(ref path) = cache_file_path {
            // Try loading full cache first
            match cache.load_from_disk(path) {
                Ok(()) => {
                    let stats = cache.stats();
                    info!("Successfully loaded full cache: {} markets, cursor at position {}", 
                          stats.markets_count, stats.cursor_position);
                }
                Err(e) => {
                    warn!("Failed to load full cache from disk: {}", e);
                    
                    // CRITICAL: NEVER reset/clear data! Try snapshot as fallback
                    if let Err(e) = cache.load_snapshot() {
                        warn!("Failed to load snapshot: {}", e);
                        info!("Starting with empty cache but preserving any existing data");
                        // DO NOT RESET - start from position 0 but keep any existing data
                    } else {
                        info!("Loaded snapshot successfully - continuing from cursor position");
                        // Mark that we need to verify the last batch since cache loading failed
                        let mut cursor = cache.cursor.write().unwrap();
                        cursor.last_page_incomplete = true; // Force re-query of last batch
                        info!("Marked last page as incomplete to re-verify from position {}", cursor.count);
                    }
                }
            }
        }

        cache
    }

    /// Check if cache needs refresh
    /// Returns false if cache_ttl is 0 (immutable data)
    pub fn needs_refresh(&self) -> bool {
        if self.cache_ttl == 0 {
            return false; // Immutable data never expires
        }
        let metadata = self.metadata.read().unwrap();
        let elapsed = Utc::now() - metadata.last_refresh;
        elapsed.num_seconds() > self.cache_ttl as i64
    }

    /// Get current cache statistics
    pub fn stats(&self) -> CacheStats {
        let markets = self.markets.read().unwrap();
        let events = self.events.read().unwrap();
        let metadata = self.metadata.read().unwrap();
        let cursor = self.cursor.read().unwrap();

        CacheStats {
            markets_count: markets.len(),
            events_count: events.len(),
            last_updated: metadata.last_updated,
            is_complete: metadata.is_complete,
            cursor_position: cursor.count,
            is_exhausted: cursor.is_exhausted,
        }
    }

    /// Store markets in cache (append-only for immutability)
    pub async fn store_markets(&self, new_markets: Vec<GammaMarket>) -> Result<usize> {
        let mut markets = self.markets.write().unwrap();
        let mut metadata = self.metadata.write().unwrap();
        let mut cursor = self.cursor.write().unwrap();

        let _initial_count = markets.len();
        let mut added_count = 0;

        for market in &new_markets {
            // Only add if not already present (immutability)
            let key = market.id.0.to_string();
            if !markets.contains_key(&key) {
                markets.insert(key, market.clone());
                added_count += 1;
            }
        }

        // Update cursor - only advance by NEW markets added, not total received
        cursor.advance_by_count(&new_markets, added_count);

        // Update metadata
        metadata.total_count = markets.len();
        metadata.last_updated = Utc::now();
        if cursor.is_exhausted {
            metadata.is_complete = true;
        }

        info!(
            "Cache updated: {} new markets added (total: {}), cursor at position {}",
            added_count, metadata.total_count, cursor.count
        );

        // Check if we need to save snapshot before dropping locks
        let should_save_snapshot = added_count > 0 && metadata.total_count % 1000 == 0;
        
        // Drop write locks before async operations
        drop(markets);
        drop(metadata);
        drop(cursor);

        // Save lightweight snapshot every 1000 markets for quick recovery
        if should_save_snapshot {
            if let Err(e) = self.save_snapshot().await {
                warn!("Failed to save snapshot: {}", e);
            }
        }

        Ok(added_count)
    }

    /// Get markets from cache with optional filtering
    pub fn get_markets(&self, filter: Option<&MarketFilter>) -> Vec<GammaMarket> {
        let markets = self.markets.read().unwrap();
        
        let mut results: Vec<GammaMarket> = if let Some(filter) = filter {
            markets.values()
                .filter(|market| self.market_matches_filter(market, filter))
                .cloned()
                .collect()
        } else {
            markets.values().cloned().collect()
        };

        // Sort by volume (descending) by default
        results.sort_by(|a, b| b.volume().cmp(&a.volume()));

        debug!("Retrieved {} markets from cache", results.len());
        results
    }

    /// Get specific market by ID
    #[allow(dead_code)]
    pub fn get_market(&self, id: &str) -> Option<GammaMarket> {
        let markets = self.markets.read().unwrap();
        markets.get(id).cloned()
    }

    /// Store events in cache
    #[allow(dead_code)]
    pub fn store_events(&self, new_events: Vec<GammaEvent>) -> Result<usize> {
        let mut events = self.events.write().unwrap();
        let mut added_count = 0;

        for event in new_events {
            if !events.contains_key(&event.id) {
                events.insert(event.id.clone(), event);
                added_count += 1;
            }
        }

        info!("Cache updated: {} new events added (total: {})", added_count, events.len());
        Ok(added_count)
    }

    /// Get events from cache
    #[allow(dead_code)]
    pub fn get_events(&self) -> Vec<GammaEvent> {
        let events = self.events.read().unwrap();
        events.values().cloned().collect()
    }

    /// Get current cursor state
    pub fn get_cursor(&self) -> Cursor {
        let cursor = self.cursor.read().unwrap();
        cursor.clone()
    }

    /// Check if we need more data based on cursor
    pub fn needs_more_data(&self, requested_count: usize) -> bool {
        let cursor = self.cursor.read().unwrap();
        let markets = self.markets.read().unwrap();
        
        !cursor.is_exhausted && markets.len() < requested_count
    }

    /// Check if we need to fetch ALL available markets
    pub fn needs_all_markets(&self) -> bool {
        let cursor = self.cursor.read().unwrap();
        !cursor.is_exhausted || cursor.last_page_incomplete
    }

    /// Mark last page as incomplete (needs refresh)
    #[allow(dead_code)]
    pub fn mark_last_page_incomplete(&self) {
        let mut cursor = self.cursor.write().unwrap();
        cursor.last_page_incomplete = true;
        cursor.is_exhausted = false; // Allow fetching to continue
        info!("Last page marked as incomplete, will refresh on next fetch");
    }
    
    /// Mark cache as not exhausted (more data available)
    pub fn mark_cache_not_exhausted(&self) {
        let mut cursor = self.cursor.write().unwrap();
        cursor.is_exhausted = false;
        cursor.last_page_incomplete = false;
        info!("Cache marked as not exhausted, will continue fetching");
    }

    /// Check if the last page needs refresh due to being incomplete
    pub fn last_page_needs_refresh(&self) -> bool {
        let cursor = self.cursor.read().unwrap();
        cursor.last_page_incomplete
    }

    /// Reset cache and cursor (force refresh)
    pub fn reset(&self) {
        let mut markets = self.markets.write().unwrap();
        let mut events = self.events.write().unwrap();
        let mut metadata = self.metadata.write().unwrap();
        let mut cursor = self.cursor.write().unwrap();

        markets.clear();
        events.clear();
        cursor.reset();
        *metadata = CacheMetadata::default();

        info!("Cache reset - all data cleared");
    }

    /// Save cache to disk
    pub fn save_to_disk(&self) -> Result<()> {
        if let Some(ref path) = self.cache_file_path {
            self.save_to_path(path)
        } else {
            Ok(()) // No-op if no path configured
        }
    }

    /// Save cache to specific path with atomic write
    pub fn save_to_path(&self, path: &Path) -> Result<()> {
        info!("Starting cache save to {}", path.display());
        
        let markets = self.markets.read().unwrap();
        let events = self.events.read().unwrap();
        let metadata = self.metadata.read().unwrap();
        let cursor = self.cursor.read().unwrap();

        let markets_count = markets.len();
        let events_count = events.len();

        let serializable = SerializableCache {
            markets: markets.clone(),
            events: events.clone(),
            metadata: metadata.clone(),
            cursor: cursor.clone(),
        };

        // Release locks before heavy serialization
        drop(markets);
        drop(events);
        drop(metadata);
        drop(cursor);

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        info!("Serializing {} markets to JSON...", markets_count);
        
        // Use compact JSON instead of pretty to reduce file size and save time
        let json = serde_json::to_string(&serializable)?;
        
        // Atomic write: write to temporary file first, then rename
        let temp_path = path.with_extension("tmp");
        
        info!("Writing cache file ({} bytes) atomically...", json.len());
        fs::write(&temp_path, &json)?;
        
        // Atomic rename
        fs::rename(&temp_path, path)?;

        info!("Cache saved to disk: {} markets, {} events at {}", 
              markets_count, events_count, path.display());
        Ok(())
    }

    /// Load cache from disk
    pub fn load_from_disk(&mut self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(()); // No cache file exists yet
        }

        let json = fs::read_to_string(path)?;
        let serializable: SerializableCache = serde_json::from_str(&json)?;

        // Replace current data with loaded data
        *self.markets.write().unwrap() = serializable.markets;
        *self.events.write().unwrap() = serializable.events;
        *self.metadata.write().unwrap() = serializable.metadata;
        *self.cursor.write().unwrap() = serializable.cursor;

        let markets_count = self.markets.read().unwrap().len();
        let events_count = self.events.read().unwrap().len();
        let cursor_position = self.cursor.read().unwrap().count;

        info!("Cache loaded from disk: {} markets, {} events, cursor at position {}", 
              markets_count, events_count, cursor_position);

        Ok(())
    }

    /// Save lightweight snapshot for quick recovery
    pub async fn save_snapshot(&self) -> Result<()> {
        if let Some(ref cache_path) = self.cache_file_path {
            let snapshot_path = cache_path.with_extension("snapshot.json");
            
            // Clone data while holding locks, then release them immediately
            let snapshot = {
                let metadata = self.metadata.read().unwrap();
                let cursor = self.cursor.read().unwrap();
                let markets = self.markets.read().unwrap();
                
                CacheSnapshot {
                    cursor: cursor.clone(),
                    metadata: metadata.clone(),
                    market_count: markets.len(),
                    timestamp: Utc::now(),
                }
                // Locks are automatically dropped here
            };
            
            let json = serde_json::to_string(&snapshot)?;
            
            // Create parent directory if it doesn't exist
            if let Some(parent) = snapshot_path.parent() {
                tokio_fs::create_dir_all(parent).await?;
            }
            
            tokio_fs::write(&snapshot_path, json).await?;
            
            info!("Snapshot saved: {} markets at position {}", 
                  snapshot.market_count, snapshot.cursor.count);
        }
        Ok(())
    }

    /// Load snapshot if available
    pub fn load_snapshot(&mut self) -> Result<bool> {
        if let Some(ref cache_path) = self.cache_file_path {
            let snapshot_path = cache_path.with_extension("snapshot.json");
            
            if snapshot_path.exists() {
                let json = fs::read_to_string(&snapshot_path)?;
                let snapshot: CacheSnapshot = serde_json::from_str(&json)?;
                
                let cursor_count = snapshot.cursor.count;
                let market_count = snapshot.market_count;
                
                // Update cursor and metadata from snapshot
                *self.metadata.write().unwrap() = snapshot.metadata;
                *self.cursor.write().unwrap() = snapshot.cursor;
                
                info!("Loaded snapshot: position {}, {} markets total", 
                      cursor_count, market_count);
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Invalidate cache if older than TTL
    #[allow(dead_code)]
    pub fn invalidate_if_stale(&self) {
        if self.needs_refresh() {
            warn!("Cache is stale, clearing for refresh");
            self.reset();
        }
    }

    /// Get next batch parameters for API call
    pub fn get_next_batch_params(&self) -> (Option<usize>, Option<String>) {
        let cursor = self.cursor.read().unwrap();
        
        if cursor.is_exhausted {
            return (None, None);
        }

        let offset = Some(cursor.count);
        (offset, cursor.last_market_id.clone())
    }

    // Private helper methods

    fn market_matches_filter(&self, market: &GammaMarket, filter: &MarketFilter) -> bool {
        if let Some(active_only) = filter.active_only {
            if active_only && !market.active {
                return false;
            }
        }

        if let Some(closed_only) = filter.closed_only {
            if closed_only && !market.closed {
                return false;
            }
        }

        if let Some(ref category) = filter.category {
            if market.category.as_ref().map(|c| c.eq_ignore_ascii_case(category)).unwrap_or(false) == false {
                return false;
            }
        }

        if let Some(min_volume) = filter.min_volume {
            if market.volume() < min_volume {
                return false;
            }
        }

        if let Some(max_volume) = filter.max_volume {
            if market.volume() > max_volume {
                return false;
            }
        }

        true
    }
}

/// Filter for querying cached markets
#[derive(Debug, Clone, Default)]
#[allow(dead_code)] // Filter fields kept for future use
pub struct MarketFilter {
    pub active_only: Option<bool>,
    pub closed_only: Option<bool>,
    pub category: Option<String>,
    pub min_volume: Option<rust_decimal::Decimal>,
    pub max_volume: Option<rust_decimal::Decimal>,
}

/// Cache statistics for monitoring
#[derive(Debug)]
pub struct CacheStats {
    pub markets_count: usize,
    pub events_count: usize,
    #[allow(dead_code)]
    pub last_updated: DateTime<Utc>,
    pub is_complete: bool,
    pub cursor_position: usize,
    pub is_exhausted: bool,
}

impl std::fmt::Display for CacheStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Cache Stats: {} markets, {} events, position {}, complete: {}, exhausted: {}",
            self.markets_count,
            self.events_count,
            self.cursor_position,
            self.is_complete,
            self.is_exhausted
        )
    }
}