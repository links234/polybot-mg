//! Individual market storage implementation
//! 
//! This module saves each market individually by condition_id and token_id
//! in both JSON and RocksDB formats for efficient querying.

use anyhow::{Context, Result};
use rocksdb::{DB, Options};
use serde_json;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn, error};

use super::types::*;

/// Storage paths for individual market data
pub struct IndividualStoragePaths {
    /// Root directory for gamma database
    #[allow(dead_code)]
    pub root: PathBuf,
    /// JSON storage directory
    pub json_dir: PathBuf,
    /// RocksDB storage directory
    pub rocksdb_dir: PathBuf,
    /// By condition ID directory
    pub by_condition_dir: PathBuf,
    /// By token ID directory
    pub by_token_dir: PathBuf,
    /// By market ID directory
    pub by_market_dir: PathBuf,
}

impl IndividualStoragePaths {
    pub fn new(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        let json_dir = root.join("jsons");
        let rocksdb_dir = root.join("rocksdb");
        
        Self {
            by_condition_dir: json_dir.join("by_condition"),
            by_token_dir: json_dir.join("by_token"),
            by_market_dir: json_dir.join("by_market"),
            json_dir,
            rocksdb_dir,
            root,
        }
    }
    
    /// Create all necessary directories
    pub fn create_directories(&self) -> Result<()> {
        fs::create_dir_all(&self.json_dir)?;
        fs::create_dir_all(&self.rocksdb_dir)?;
        fs::create_dir_all(&self.by_condition_dir)?;
        fs::create_dir_all(&self.by_token_dir)?;
        fs::create_dir_all(&self.by_market_dir)?;
        Ok(())
    }
}

/// Individual market storage manager
pub struct IndividualMarketStorage {
    paths: IndividualStoragePaths,
    /// RocksDB instance for condition_id -> market mapping
    condition_db: Option<DB>,
    /// RocksDB instance for token_id -> market mapping
    token_db: Option<DB>,
    /// RocksDB instance for market_id -> market mapping
    market_db: Option<DB>,
}

impl IndividualMarketStorage {
    /// Create new storage instance
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let paths = IndividualStoragePaths::new(root);
        paths.create_directories()
            .context("Failed to create storage directories")?;
        
        // Open RocksDB instances
        let mut opts = Options::default();
        opts.create_if_missing(true);
        
        let condition_db = DB::open(&opts, paths.rocksdb_dir.join("conditions"))
            .context("Failed to open conditions RocksDB")?;
        let token_db = DB::open(&opts, paths.rocksdb_dir.join("tokens"))
            .context("Failed to open tokens RocksDB")?;
        let market_db = DB::open(&opts, paths.rocksdb_dir.join("markets"))
            .context("Failed to open markets RocksDB")?;
        
        Ok(Self {
            paths,
            condition_db: Some(condition_db),
            token_db: Some(token_db),
            market_db: Some(market_db),
        })
    }
    
    /// Save a single market to all storage locations
    pub fn save_market(&self, market: &GammaMarket) -> Result<()> {
        // Get token IDs from the market
        let token_ids = self.get_token_ids(market);
        
        // Save to JSON files
        self.save_market_json(market, &token_ids)?;
        
        // Save to RocksDB
        self.save_market_rocksdb(market, &token_ids)?;
        
        Ok(())
    }
    
    /// Save multiple markets in batch
    #[allow(dead_code)] // Batch storage API kept for future use
    pub fn save_markets_batch(&self, markets: &[GammaMarket]) -> Result<()> {
        let mut success_count = 0;
        let mut error_count = 0;
        
        for market in markets {
            match self.save_market(market) {
                Ok(_) => success_count += 1,
                Err(e) => {
                    error_count += 1;
                    warn!("Failed to save market {}: {}", market.id.0, e);
                }
            }
        }
        
        info!("Saved {} markets individually ({} errors)", success_count, error_count);
        Ok(())
    }
    
    /// Get token IDs from market
    fn get_token_ids(&self, market: &GammaMarket) -> Vec<String> {
        market.clob_token_ids
            .iter()
            .map(|token_id| token_id.0.clone())
            .collect()
    }
    
    /// Save market as JSON files
    fn save_market_json(&self, market: &GammaMarket, token_ids: &[String]) -> Result<()> {
        let market_json = serde_json::to_string_pretty(market)
            .context("Failed to serialize market to JSON")?;
        
        // Save by condition ID
        let condition_file = self.paths.by_condition_dir
            .join(format!("{}.json", &market.condition_id.0));
        fs::write(&condition_file, &market_json)
            .with_context(|| format!("Failed to write condition file: {:?}", condition_file))?;
        
        // Save by market ID
        let market_file = self.paths.by_market_dir
            .join(format!("{}.json", market.id.0));
        fs::write(&market_file, &market_json)
            .with_context(|| format!("Failed to write market file: {:?}", market_file))?;
        
        // Save by token IDs
        for token_id in token_ids {
            // Clean token ID (remove any special characters that might cause filesystem issues)
            let safe_token_id = token_id.replace("/", "_").replace("\\", "_");
            let token_file = self.paths.by_token_dir
                .join(format!("{}.json", safe_token_id));
            fs::write(&token_file, &market_json)
                .with_context(|| format!("Failed to write token file: {:?}", token_file))?;
        }
        
        debug!("Saved market {} to JSON files", market.id.0);
        Ok(())
    }
    
    /// Save market to RocksDB
    fn save_market_rocksdb(&self, market: &GammaMarket, token_ids: &[String]) -> Result<()> {
        let market_bytes = serde_json::to_vec(market)
            .context("Failed to serialize market for RocksDB")?;
        
        // Save by condition ID
        if let Some(ref db) = self.condition_db {
            db.put(market.condition_id.0.as_bytes(), &market_bytes)
                .context("Failed to save to condition DB")?;
        }
        
        // Save by market ID
        if let Some(ref db) = self.market_db {
            db.put(market.id.0.to_string().as_bytes(), &market_bytes)
                .context("Failed to save to market DB")?;
        }
        
        // Save by token IDs
        if let Some(ref db) = self.token_db {
            for token_id in token_ids {
                db.put(token_id.as_bytes(), &market_bytes)
                    .context("Failed to save to token DB")?;
            }
        }
        
        debug!("Saved market {} to RocksDB", market.id.0);
        Ok(())
    }
    
    /// Get market by condition ID
    #[allow(dead_code)]
    pub fn get_market_by_condition(&self, condition_id: &str) -> Result<Option<GammaMarket>> {
        // Try RocksDB first
        if let Some(ref db) = self.condition_db {
            if let Some(bytes) = db.get(condition_id.as_bytes())? {
                let market: GammaMarket = serde_json::from_slice(&bytes)
                    .context("Failed to deserialize market from RocksDB")?;
                return Ok(Some(market));
            }
        }
        
        // Fallback to JSON file
        let json_path = self.paths.by_condition_dir.join(format!("{}.json", condition_id));
        if json_path.exists() {
            let json = fs::read_to_string(&json_path)
                .context("Failed to read JSON file")?;
            let market: GammaMarket = serde_json::from_str(&json)
                .context("Failed to parse market JSON")?;
            return Ok(Some(market));
        }
        
        Ok(None)
    }
    
    /// Get market by token ID
    #[allow(dead_code)]
    pub fn get_market_by_token(&self, token_id: &str) -> Result<Option<GammaMarket>> {
        // Try RocksDB first
        if let Some(ref db) = self.token_db {
            if let Some(bytes) = db.get(token_id.as_bytes())? {
                let market: GammaMarket = serde_json::from_slice(&bytes)
                    .context("Failed to deserialize market from RocksDB")?;
                return Ok(Some(market));
            }
        }
        
        // Fallback to JSON file
        let safe_token_id = token_id.replace("/", "_").replace("\\", "_");
        let json_path = self.paths.by_token_dir.join(format!("{}.json", safe_token_id));
        if json_path.exists() {
            let json = fs::read_to_string(&json_path)
                .context("Failed to read JSON file")?;
            let market: GammaMarket = serde_json::from_str(&json)
                .context("Failed to parse market JSON")?;
            return Ok(Some(market));
        }
        
        Ok(None)
    }
    
    /// Get market by market ID
    #[allow(dead_code)]
    pub fn get_market_by_id(&self, market_id: u64) -> Result<Option<GammaMarket>> {
        // Try RocksDB first
        if let Some(ref db) = self.market_db {
            if let Some(bytes) = db.get(market_id.to_string().as_bytes())? {
                let market: GammaMarket = serde_json::from_slice(&bytes)
                    .context("Failed to deserialize market from RocksDB")?;
                return Ok(Some(market));
            }
        }
        
        // Fallback to JSON file
        let json_path = self.paths.by_market_dir.join(format!("{}.json", market_id));
        if json_path.exists() {
            let json = fs::read_to_string(&json_path)
                .context("Failed to read JSON file")?;
            let market: GammaMarket = serde_json::from_str(&json)
                .context("Failed to parse market JSON")?;
            return Ok(Some(market));
        }
        
        Ok(None)
    }
    
    /// Process raw API response and save all markets individually
    pub fn process_raw_response(&self, raw_json_path: &Path) -> Result<()> {
        info!("Processing raw API response: {:?}", raw_json_path);
        
        let json_str = fs::read_to_string(raw_json_path)
            .context("Failed to read raw JSON file")?;
        
        let raw_markets: Vec<serde_json::Value> = serde_json::from_str(&json_str)
            .context("Failed to parse raw JSON")?;
        
        info!("Found {} markets in raw response", raw_markets.len());
        
        let mut success_count = 0;
        let mut error_count = 0;
        
        for (i, raw_market) in raw_markets.into_iter().enumerate() {
            match serde_json::from_value::<GammaMarket>(raw_market) {
                Ok(market) => {
                    if let Err(e) = self.save_market(&market) {
                        error_count += 1;
                        warn!("Failed to save market at index {}: {}", i, e);
                    } else {
                        success_count += 1;
                    }
                }
                Err(e) => {
                    error_count += 1;
                    error!("Failed to parse market at index {}: {}", i, e);
                }
            }
        }
        
        info!("Processed raw response: {} saved, {} errors", success_count, error_count);
        Ok(())
    }
    
    /// Process all raw responses in the gamma_raw_responses directory
    pub fn process_all_raw_responses(&self) -> Result<()> {
        let raw_dir = PathBuf::from("./data/gamma_raw_responses");
        if !raw_dir.exists() {
            warn!("Raw responses directory does not exist: {:?}", raw_dir);
            return Ok(());
        }
        
        let entries = fs::read_dir(&raw_dir)
            .context("Failed to read raw responses directory")?;
        
        let mut file_count = 0;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                info!("Processing file {}: {:?}", file_count + 1, path.file_name());
                if let Err(e) = self.process_raw_response(&path) {
                    error!("Failed to process {:?}: {}", path, e);
                }
                file_count += 1;
            }
        }
        
        info!("Processed {} raw response files", file_count);
        Ok(())
    }
}

impl Drop for IndividualMarketStorage {
    fn drop(&mut self) {
        // Ensure RocksDB instances are properly closed
        self.condition_db.take();
        self.token_db.take();
        self.market_db.take();
    }
}