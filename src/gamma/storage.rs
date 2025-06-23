//! Comprehensive storage layer for Gamma API data
//! 
//! This module provides dual storage:
//! - RocksDB for efficient typed queries and indexing
//! - JSON files for debugging and human-readable storage

use anyhow::{Context, Result};
use rocksdb::{DB, Options, ColumnFamily, ColumnFamilyDescriptor};
use serde::{Serialize, Deserialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, info, warn};

use super::types::*;

/// Column family names for RocksDB storage
pub const CF_MARKETS: &str = "markets";
pub const CF_EVENTS: &str = "events";
pub const CF_TRADES: &str = "trades";
pub const CF_POSITIONS: &str = "positions";
pub const CF_PRICE_HISTORY: &str = "price_history";
pub const CF_MARKET_BY_CONDITION: &str = "market_by_condition";
pub const CF_MARKET_BY_TAG: &str = "market_by_tag";
pub const CF_TRADE_BY_USER: &str = "trade_by_user";
pub const CF_TRADE_BY_MARKET: &str = "trade_by_market";
pub const CF_POSITION_BY_USER: &str = "position_by_user";

/// All column families for Gamma data
pub const GAMMA_COLUMN_FAMILIES: &[&str] = &[
    CF_MARKETS,
    CF_EVENTS,
    CF_TRADES,
    CF_POSITIONS,
    CF_PRICE_HISTORY,
    CF_MARKET_BY_CONDITION,
    CF_MARKET_BY_TAG,
    CF_TRADE_BY_USER,
    CF_TRADE_BY_MARKET,
    CF_POSITION_BY_USER,
];

/// Gamma-specific database context
#[derive(Clone)]
pub struct GammaDbContext {
    pub db: Arc<DB>,
    pub _data_dir: PathBuf,
    pub json_dir: PathBuf,
}

impl GammaDbContext {
    /// Open or create the Gamma database
    pub fn open<P: AsRef<Path>>(data_dir: P) -> Result<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();
        let db_path = data_dir.join("rocksdb");
        let json_dir = data_dir.join("jsons");

        // Create directories if they don't exist
        std::fs::create_dir_all(&db_path)
            .context("Failed to create RocksDB directory")?;
        std::fs::create_dir_all(&json_dir)
            .context("Failed to create JSON directory")?;

        // Configure RocksDB options
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        opts.set_max_open_files(1000);
        opts.set_compression_type(rocksdb::DBCompressionType::Lz4);

        // Create column family descriptors
        let cf_descriptors: Vec<ColumnFamilyDescriptor> = GAMMA_COLUMN_FAMILIES
            .iter()
            .map(|name| ColumnFamilyDescriptor::new(*name, Options::default()))
            .collect();

        // Open database
        let db = DB::open_cf_descriptors(&opts, &db_path, cf_descriptors)
            .context("Failed to open RocksDB")?;

        info!("Opened Gamma database at {:?}", db_path);

        Ok(Self {
            db: Arc::new(db),
            _data_dir: data_dir,
            json_dir,
        })
    }

    /// Get column family handle
    pub fn cf_handle(&self, name: &str) -> Result<&ColumnFamily> {
        self.db.cf_handle(name)
            .context(format!("Column family '{}' not found", name))
    }
}

/// Primary storage interface for Gamma data
#[derive(Clone)]
pub struct GammaStorage {
    pub ctx: GammaDbContext,
}

impl GammaStorage {
    /// Create new storage instance
    pub fn new<P: AsRef<Path>>(data_dir: P) -> Result<Self> {
        let ctx = GammaDbContext::open(data_dir)?;

        Ok(Self {
            ctx,
        })
    }

    // ============================================================================
    // MARKET STORAGE
    // ============================================================================

    /// Store a market with all indexes
    pub fn store_market(&self, market: &GammaMarket) -> Result<()> {
        let cf = self.ctx.cf_handle(CF_MARKETS)?;
        
        // Store main market record
        let key = market.id.0.to_be_bytes();
        let value = serde_json::to_vec(market)
            .context("Failed to serialize market")?;
        
        self.ctx.db.put_cf(cf, key, value)
            .context("Failed to store market")?;

        // Store condition ID index
        let condition_cf = self.ctx.cf_handle(CF_MARKET_BY_CONDITION)?;
        let condition_key = market.condition_id.0.as_bytes();
        self.ctx.db.put_cf(condition_cf, condition_key, key)
            .context("Failed to store market condition index")?;

        // Store tag indexes - TODO: Add tags support when available
        // let tag_cf = self.ctx.cf_handle(CF_MARKET_BY_TAG)?;
        // for tag in &market.tags {
        //     let tag_key = format!("{}:{}", tag.label, market.id.0);
        //     self.ctx.db.put_cf(tag_cf, tag_key.as_bytes(), key)
        //         .context("Failed to store market tag index")?;
        // }

        // Store JSON copy
        self.store_market_json(market)?;

        debug!("Stored market: {} ({})", market.id.0, market.question);
        Ok(())
    }

    /// Retrieve a market by ID
    pub fn get_market(&self, id: &MarketId) -> Result<Option<GammaMarket>> {
        let cf = self.ctx.cf_handle(CF_MARKETS)?;
        let key = id.0.to_be_bytes();
        
        match self.ctx.db.get_cf(cf, key)? {
            Some(value) => {
                let market: GammaMarket = serde_json::from_slice(&value)
                    .context("Failed to deserialize market")?;
                Ok(Some(market))
            }
            None => Ok(None),
        }
    }

    /// Get markets by condition ID
    pub fn _get_markets_by_condition(&self, condition_id: &ConditionId) -> Result<Vec<GammaMarket>> {
        let condition_cf = self.ctx.cf_handle(CF_MARKET_BY_CONDITION)?;
        let markets_cf = self.ctx.cf_handle(CF_MARKETS)?;
        
        let mut markets = Vec::new();
        let iter = self.ctx.db.prefix_iterator_cf(condition_cf, condition_id.0.as_bytes());
        
        for item in iter {
            let (_key, market_id_bytes) = item?;
            if let Some(value) = self.ctx.db.get_cf(markets_cf, &market_id_bytes)? {
                let market: GammaMarket = serde_json::from_slice(&value)
                    .context("Failed to deserialize market")?;
                markets.push(market);
            }
        }
        
        Ok(markets)
    }

    /// Search markets by tag
    pub fn search_markets_by_tag(&self, tag: &str) -> Result<Vec<GammaMarket>> {
        let tag_cf = self.ctx.cf_handle(CF_MARKET_BY_TAG)?;
        let markets_cf = self.ctx.cf_handle(CF_MARKETS)?;
        
        let mut markets = Vec::new();
        let prefix = format!("{}:", tag);
        let iter = self.ctx.db.prefix_iterator_cf(tag_cf, prefix.as_bytes());
        
        for item in iter {
            let (_key, market_id_bytes) = item?;
            if let Some(value) = self.ctx.db.get_cf(markets_cf, &market_id_bytes)? {
                let market: GammaMarket = serde_json::from_slice(&value)
                    .context("Failed to deserialize market")?;
                markets.push(market);
            }
        }
        
        Ok(markets)
    }

    /// Store market as JSON
    fn store_market_json(&self, market: &GammaMarket) -> Result<()> {
        let file_path = self.ctx.json_dir.join(format!("market_{}.json", market.id.0));
        let json = serde_json::to_string_pretty(market)
            .context("Failed to serialize market to JSON")?;
        
        std::fs::write(&file_path, json)
            .context("Failed to write market JSON file")?;
        
        Ok(())
    }

    // ============================================================================
    // EVENT STORAGE
    // ============================================================================

    /// Store an event with all its markets
    pub fn store_event(&self, event: &GammaEvent) -> Result<()> {
        let cf = self.ctx.cf_handle(CF_EVENTS)?;
        
        // Store main event record
        let key = event.id.0.to_be_bytes();
        let value = serde_json::to_vec(event)
            .context("Failed to serialize event")?;
        
        self.ctx.db.put_cf(cf, key, value)
            .context("Failed to store event")?;

        // Store all markets in the event
        for market in &event.markets {
            self.store_market(market)?;
        }

        // Store JSON copy
        self.store_event_json(event)?;

        debug!("Stored event: {} ({})", event.id.0, event.title);
        Ok(())
    }

    /// Retrieve an event by ID
    pub fn _get_event(&self, id: &EventId) -> Result<Option<GammaEvent>> {
        let cf = self.ctx.cf_handle(CF_EVENTS)?;
        let key = id.0.to_be_bytes();
        
        match self.ctx.db.get_cf(cf, key)? {
            Some(value) => {
                let event: GammaEvent = serde_json::from_slice(&value)
                    .context("Failed to deserialize event")?;
                Ok(Some(event))
            }
            None => Ok(None),
        }
    }

    /// Store event as JSON
    fn store_event_json(&self, event: &GammaEvent) -> Result<()> {
        let file_path = self.ctx.json_dir.join(format!("event_{}.json", event.id.0));
        let json = serde_json::to_string_pretty(event)
            .context("Failed to serialize event to JSON")?;
        
        std::fs::write(&file_path, json)
            .context("Failed to write event JSON file")?;
        
        Ok(())
    }

    // ============================================================================
    // TRADE STORAGE
    // ============================================================================

    /// Store a trade with indexes
    pub fn store_trade(&self, trade: &GammaTrade) -> Result<()> {
        let cf = self.ctx.cf_handle(CF_TRADES)?;
        
        // Create composite key: timestamp + transaction_hash
        let key = format!("{}:{}", trade.timestamp.timestamp(), trade.transaction_hash.0);
        let value = serde_json::to_vec(trade)
            .context("Failed to serialize trade")?;
        
        self.ctx.db.put_cf(cf, key.as_bytes(), value)
            .context("Failed to store trade")?;

        // Store user index
        let user_cf = self.ctx.cf_handle(CF_TRADE_BY_USER)?;
        let user_key = format!("{}:{}", trade.proxy_wallet.0, key);
        self.ctx.db.put_cf(user_cf, user_key.as_bytes(), key.as_bytes())
            .context("Failed to store trade user index")?;

        // Store market index
        let market_cf = self.ctx.cf_handle(CF_TRADE_BY_MARKET)?;
        let market_key = format!("{}:{}", trade.condition_id.0, key);
        self.ctx.db.put_cf(market_cf, market_key.as_bytes(), key.as_bytes())
            .context("Failed to store trade market index")?;

        debug!("Stored trade: {} {:?} {} @ {}", 
               trade.proxy_wallet.0, trade.side, trade.outcome, trade.price);
        Ok(())
    }

    /// Get trades by user
    pub fn _get_trades_by_user(&self, user: &UserAddress, limit: Option<usize>) -> Result<Vec<GammaTrade>> {
        let user_cf = self.ctx.cf_handle(CF_TRADE_BY_USER)?;
        let trades_cf = self.ctx.cf_handle(CF_TRADES)?;
        
        let mut trades = Vec::new();
        let prefix = format!("{}:", user.0);
        let iter = self.ctx.db.prefix_iterator_cf(user_cf, prefix.as_bytes());
        
        for (count, item) in iter.enumerate() {
            if let Some(limit) = limit {
                if count >= limit {
                    break;
                }
            }
            
            let (_key, trade_key) = item?;
            if let Some(value) = self.ctx.db.get_cf(trades_cf, &trade_key)? {
                let trade: GammaTrade = serde_json::from_slice(&value)
                    .context("Failed to deserialize trade")?;
                trades.push(trade);
            }
        }
        
        Ok(trades)
    }

    /// Get trades by market (condition ID)  
    pub fn _get_trades_by_market(&self, condition_id: &ConditionId, limit: Option<usize>) -> Result<Vec<GammaTrade>> {
        let market_cf = self.ctx.cf_handle(CF_TRADE_BY_MARKET)?;
        let trades_cf = self.ctx.cf_handle(CF_TRADES)?;
        
        let mut trades = Vec::new();
        let prefix = format!("{}:", condition_id.0);
        let iter = self.ctx.db.prefix_iterator_cf(market_cf, prefix.as_bytes());
        
        for (count, item) in iter.enumerate() {
            if let Some(limit) = limit {
                if count >= limit {
                    break;
                }
            }
            
            let (_key, trade_key) = item?;
            if let Some(value) = self.ctx.db.get_cf(trades_cf, &trade_key)? {
                let trade: GammaTrade = serde_json::from_slice(&value)
                    .context("Failed to deserialize trade")?;
                trades.push(trade);
            }
        }
        
        Ok(trades)
    }

    // ============================================================================
    // POSITION STORAGE
    // ============================================================================

    /// Store a position with indexes
    pub fn store_position(&self, position: &GammaPosition) -> Result<()> {
        let cf = self.ctx.cf_handle(CF_POSITIONS)?;
        
        // Create composite key: user + condition_id + outcome_index
        let key = format!("{}:{}:{}", 
                         position.proxy_wallet.0, 
                         position.condition_id.0, 
                         position.outcome_index);
        let value = serde_json::to_vec(position)
            .context("Failed to serialize position")?;
        
        self.ctx.db.put_cf(cf, key.as_bytes(), value)
            .context("Failed to store position")?;

        // Store user index
        let user_cf = self.ctx.cf_handle(CF_POSITION_BY_USER)?;
        let user_key = format!("{}:{}", position.proxy_wallet.0, key);
        self.ctx.db.put_cf(user_cf, user_key.as_bytes(), key.as_bytes())
            .context("Failed to store position user index")?;

        debug!("Stored position: {} {} {} shares", 
               position.proxy_wallet.0, position.outcome, position.size);
        Ok(())
    }

    /// Get positions by user
    pub fn _get_positions_by_user(&self, user: &UserAddress) -> Result<Vec<GammaPosition>> {
        let user_cf = self.ctx.cf_handle(CF_POSITION_BY_USER)?;
        let positions_cf = self.ctx.cf_handle(CF_POSITIONS)?;
        
        let mut positions = Vec::new();
        let prefix = format!("{}:", user.0);
        let iter = self.ctx.db.prefix_iterator_cf(user_cf, prefix.as_bytes());
        
        for item in iter {
            let (_key, position_key) = item?;
            if let Some(value) = self.ctx.db.get_cf(positions_cf, &position_key)? {
                let position: GammaPosition = serde_json::from_slice(&value)
                    .context("Failed to deserialize position")?;
                positions.push(position);
            }
        }
        
        Ok(positions)
    }

    // ============================================================================
    // PRICE HISTORY STORAGE
    // ============================================================================

    /// Store price history for a token
    pub fn _store_price_history(&self, history: &PriceHistory) -> Result<()> {
        let cf = self.ctx.cf_handle(CF_PRICE_HISTORY)?;
        
        let key = history.token_id.0.as_bytes();
        let value = serde_json::to_vec(history)
            .context("Failed to serialize price history")?;
        
        self.ctx.db.put_cf(cf, key, value)
            .context("Failed to store price history")?;

        // Store JSON copy
        let file_path = self.ctx.json_dir.join(format!("price_history_{}.json", history.token_id.0));
        let json = serde_json::to_string_pretty(history)
            .context("Failed to serialize price history to JSON")?;
        
        std::fs::write(&file_path, json)
            .context("Failed to write price history JSON file")?;

        debug!("Stored price history for token: {} ({} points)", 
               history.token_id.0, history.history.len());
        Ok(())
    }

    /// Get price history for a token
    pub fn _get_price_history(&self, token_id: &ClobTokenId) -> Result<Option<PriceHistory>> {
        let cf = self.ctx.cf_handle(CF_PRICE_HISTORY)?;
        let key = token_id.0.as_bytes();
        
        match self.ctx.db.get_cf(cf, key)? {
            Some(value) => {
                let history: PriceHistory = serde_json::from_slice(&value)
                    .context("Failed to deserialize price history")?;
                Ok(Some(history))
            }
            None => Ok(None),
        }
    }

    // ============================================================================
    // BULK STORAGE OPERATIONS
    // ============================================================================

    /// Store multiple markets in batch
    pub fn store_markets_batch(&self, markets: &[GammaMarket]) -> Result<()> {
        info!("Storing {} markets in batch", markets.len());
        
        for market in markets {
            self.store_market(market)?;
        }
        
        info!("Successfully stored {} markets", markets.len());
        Ok(())
    }

    /// Store multiple events in batch
    pub fn store_events_batch(&self, events: &[GammaEvent]) -> Result<()> {
        info!("Storing {} events in batch", events.len());
        
        for event in events {
            self.store_event(event)?;
        }
        
        info!("Successfully stored {} events", events.len());
        Ok(())
    }

    /// Store multiple trades in batch
    pub fn store_trades_batch(&self, trades: &[GammaTrade]) -> Result<()> {
        info!("Storing {} trades in batch", trades.len());
        
        for trade in trades {
            self.store_trade(trade)?;
        }
        
        info!("Successfully stored {} trades", trades.len());
        Ok(())
    }

    /// Store multiple positions in batch
    pub fn store_positions_batch(&self, positions: &[GammaPosition]) -> Result<()> {
        info!("Storing {} positions in batch", positions.len());
        
        for position in positions {
            self.store_position(position)?;
        }
        
        info!("Successfully stored {} positions", positions.len());
        Ok(())
    }

    // ============================================================================
    // STATISTICS AND ANALYTICS
    // ============================================================================

    /// Get storage statistics
    pub fn get_stats(&self) -> Result<GammaStorageStats> {
        let mut stats = GammaStorageStats::default();
        
        // Count markets
        let markets_cf = self.ctx.cf_handle(CF_MARKETS)?;
        let iter = self.ctx.db.iterator_cf(markets_cf, rocksdb::IteratorMode::Start);
        stats.total_markets = iter.count() as u64;

        // Count events
        let events_cf = self.ctx.cf_handle(CF_EVENTS)?;
        let iter = self.ctx.db.iterator_cf(events_cf, rocksdb::IteratorMode::Start);
        stats.total_events = iter.count() as u64;

        // Count trades
        let trades_cf = self.ctx.cf_handle(CF_TRADES)?;
        let iter = self.ctx.db.iterator_cf(trades_cf, rocksdb::IteratorMode::Start);
        stats.total_trades = iter.count() as u64;

        // Count positions
        let positions_cf = self.ctx.cf_handle(CF_POSITIONS)?;
        let iter = self.ctx.db.iterator_cf(positions_cf, rocksdb::IteratorMode::Start);
        stats.total_positions = iter.count() as u64;

        Ok(stats)
    }

    /// Clear all data (for development/testing)
    pub fn clear_all(&self) -> Result<()> {
        warn!("Clearing all Gamma data");
        
        for cf_name in GAMMA_COLUMN_FAMILIES {
            let cf = self.ctx.cf_handle(cf_name)?;
            let iter = self.ctx.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
            
            let keys: Vec<Vec<u8>> = iter.map(|item| item.unwrap().0.to_vec()).collect();
            for key in keys {
                self.ctx.db.delete_cf(cf, &key)?;
            }
        }
        
        // Clear JSON directory
        if self.ctx.json_dir.exists() {
            std::fs::remove_dir_all(&self.ctx.json_dir)?;
            std::fs::create_dir_all(&self.ctx.json_dir)?;
        }
        
        info!("Cleared all Gamma data");
        Ok(())
    }
}

/// Storage statistics
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct GammaStorageStats {
    pub total_markets: u64,
    pub total_events: u64,
    pub total_trades: u64,
    pub total_positions: u64,
    pub total_price_histories: u64,
    pub database_size_bytes: u64,
    pub json_files_count: u64,
}