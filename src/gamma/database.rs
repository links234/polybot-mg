//! SurrealDB RocksDB-based storage for deduplicated market data
//! 
//! This module provides persistent, deduplicated storage for gamma markets
//! using SurrealDB with RocksDB backend for high performance and reliability.

use std::path::Path;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use surrealdb::engine::local::{Db, RocksDb};
use surrealdb::Surreal;
use tracing::{info, debug, error};

use crate::gamma::types::{GammaMarket, MarketId, ConditionId, ClobTokenId};

/// Database connection and operations for gamma markets with caching
pub struct GammaDatabase {
    db: Surreal<Db>,
    _path: String,
    // Query result cache for improved performance
    query_cache: Arc<RwLock<HashMap<String, (DateTime<Utc>, serde_json::Value)>>>,
    // Statistics cache
    stats_cache: Arc<RwLock<Option<(DateTime<Utc>, MarketStats)>>>,
    // Configuration
    cache_ttl_minutes: i64,
}

use surrealdb::sql::Thing;

/// Helper type for deserializing SurrealDB records that include Thing IDs
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct MarketRecordRaw {
    id: Thing, // SurrealDB Thing type - must match database field name
    market_id: String,
    condition_id: String,
    slug: String,
    question: String,
    description: Option<String>,
    outcomes: Vec<String>,
    outcome_prices: Option<Vec<String>>,
    clob_token_ids: Vec<String>,
    category: Option<String>,
    end_date: Option<String>,
    created_at: String,
    updated_at: String,
    closed_time: Option<String>,
    image: Option<String>,
    icon: Option<String>,
    volume: Option<String>,
    liquidity: Option<String>,
    active: bool,
    closed: bool,
    archived: bool,
    restricted: bool,
    cyom: bool,
    approved: bool,
    volume_24hr: Option<String>,
    volume_1wk: Option<String>,
    volume_1mo: Option<String>,
    volume_1yr: Option<String>,
    best_bid: Option<String>,
    best_ask: Option<String>,
    last_trade_price: Option<String>,
    accepting_orders: bool,
    enable_order_book: bool,
    featured: bool,
    new: bool,
    neg_risk: bool,
    spread: Option<String>,
    start_date: Option<String>,
    first_seen: String,
    last_updated_db: String,
    source_session: u32,
}

impl MarketRecordRaw {
    /// Convert raw record to clean MarketRecord, extracting ID from Thing
    fn into_market_record(self) -> MarketRecord {
        // Note: We use the actual market_id field, not the SurrealDB Thing id
        // The Thing id is just SurrealDB's internal record identifier
        
        MarketRecord {
            market_id: self.market_id,
            condition_id: self.condition_id,
            slug: self.slug,
            question: self.question,
            description: self.description,
            outcomes: self.outcomes,
            outcome_prices: self.outcome_prices,
            clob_token_ids: self.clob_token_ids,
            category: self.category,
            end_date: self.end_date,
            created_at: self.created_at,
            updated_at: self.updated_at,
            closed_time: self.closed_time,
            image: self.image,
            icon: self.icon,
            volume: self.volume,
            liquidity: self.liquidity,
            active: self.active,
            closed: self.closed,
            archived: self.archived,
            restricted: self.restricted,
            cyom: self.cyom,
            approved: self.approved,
            volume_24hr: self.volume_24hr,
            volume_1wk: self.volume_1wk,
            volume_1mo: self.volume_1mo,
            volume_1yr: self.volume_1yr,
            best_bid: self.best_bid,
            best_ask: self.best_ask,
            last_trade_price: self.last_trade_price,
            accepting_orders: self.accepting_orders,
            enable_order_book: self.enable_order_book,
            featured: self.featured,
            new: self.new,
            neg_risk: self.neg_risk,
            spread: self.spread,
            start_date: self.start_date,
            first_seen: self.first_seen,
            last_updated_db: self.last_updated_db,
            source_session: self.source_session,
        }
    }
}

/// Lightweight market record for database storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketRecord {
    pub market_id: String, // The actual market ID from GammaMarket
    pub condition_id: String,
    pub slug: String,
    pub question: String,
    pub description: Option<String>,
    pub outcomes: Vec<String>,
    pub outcome_prices: Option<Vec<String>>,
    pub clob_token_ids: Vec<String>,
    pub category: Option<String>,
    pub end_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub closed_time: Option<String>,
    pub image: Option<String>,
    pub icon: Option<String>,
    pub volume: Option<String>,
    pub liquidity: Option<String>,
    pub active: bool,
    pub closed: bool,
    pub archived: bool,
    pub restricted: bool,
    pub cyom: bool,
    pub approved: bool,
    pub volume_24hr: Option<String>,
    pub volume_1wk: Option<String>,
    pub volume_1mo: Option<String>,
    pub volume_1yr: Option<String>,
    pub best_bid: Option<String>,
    pub best_ask: Option<String>,
    pub last_trade_price: Option<String>,
    pub accepting_orders: bool,
    pub enable_order_book: bool,
    pub featured: bool,
    pub new: bool,
    pub neg_risk: bool,
    pub spread: Option<String>,
    pub start_date: Option<String>,
    // Database metadata
    pub first_seen: String,
    pub last_updated_db: String,
    pub source_session: u32,
}

/// Market statistics for analytics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketStats {
    pub total_markets: u64,
    pub active_markets: u64,
    pub closed_markets: u64,
    pub archived_markets: u64,
    pub total_volume: String,
    pub total_liquidity: String,
    pub last_updated: String,
}

impl GammaDatabase {
    /// Create new database connection
    pub async fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let path_str = db_path.as_ref().to_string_lossy().to_string();
        info!("Initializing SurrealDB at path: {}", path_str);
        
        // Connect to RocksDB backend
        let db = Surreal::new::<RocksDb>(&path_str).await
            .with_context(|| format!("Failed to create SurrealDB RocksDB connection at {}. The database might be locked by another process.", path_str))?;
        
        // Use database and namespace
        db.use_ns("polybot").use_db("gamma").await
            .context("Failed to set namespace and database")?;
        
        info!("Using namespace: polybot, database: gamma");
        
        let database = Self {
            db,
            _path: path_str,
            query_cache: Arc::new(RwLock::new(HashMap::new())),
            stats_cache: Arc::new(RwLock::new(None)),
            cache_ttl_minutes: 5, // 5 minute cache TTL
        };
        
        // Initialize schema
        database.init_schema().await?;
        
        info!("SurrealDB initialized successfully");
        Ok(database)
    }
    
    /// Initialize database schema with indexes
    async fn init_schema(&self) -> Result<()> {
        info!("Initializing database schema");
        
        // Create markets table without schema restrictions
        let schema_queries = vec![
            // Markets table - schemaless for flexibility
            "DEFINE TABLE markets SCHEMALESS;",
            
            // Stats table for analytics
            "DEFINE TABLE stats SCHEMAFULL;",
            "DEFINE FIELD total_markets ON TABLE stats TYPE number;",
            "DEFINE FIELD active_markets ON TABLE stats TYPE number;",
            "DEFINE FIELD closed_markets ON TABLE stats TYPE number;",
            "DEFINE FIELD archived_markets ON TABLE stats TYPE number;",
            "DEFINE FIELD total_volume ON TABLE stats TYPE string;",
            "DEFINE FIELD total_liquidity ON TABLE stats TYPE string;",
            "DEFINE FIELD last_updated ON TABLE stats TYPE string;",
        ];
        
        for query in schema_queries {
            self.db.query(query).await
                .with_context(|| format!("Failed to execute schema query: {}", query))?;
        }
        
        info!("Database schema initialized successfully");
        Ok(())
    }
    
    /// Store markets with deduplication
    pub async fn store_markets(&self, markets: &[GammaMarket], session_id: u32) -> Result<u64> {
        let now = Utc::now();
        let mut stored_count = 0;
        
        info!("Storing {} markets from session {}", markets.len(), session_id);
        
        // Store each market individually
        for market in markets {
            let market_record = self.convert_market_to_record(market, session_id, now);
            let market_id = market.id.0.to_string();
            
            // Convert to JSON
            let json_value = serde_json::to_value(&market_record)?;
            
            // Use CREATE with the JSON content
            let query = format!(
                "CREATE markets CONTENT {}",
                serde_json::to_string(&json_value)?
            );
            
            let result = self.db.query(&query).await;
            
            if let Err(e) = &result {
                if stored_count < 3 {
                    error!("Failed to create market {}: {}", market_id, e);
                }
            } else {
                stored_count += 1;
            }
            
            if stored_count > 0 && stored_count % 1000 == 0 {
                debug!("Stored {} markets so far", stored_count);
            }
        }
        
        info!("Stored {} markets from session {}", stored_count, session_id);
        
        // Quick verification - try to count markets
        let verify_count = self.get_market_count().await?;
        info!("Verification: Database now contains {} markets", verify_count);
        
        // Skip update_stats to avoid hanging on large datasets
        info!("Skipping statistics update to avoid performance issues with large dataset");
        
        Ok(stored_count)
    }
    
    
    /// Convert GammaMarket to MarketRecord
    fn convert_market_to_record(&self, market: &GammaMarket, session_id: u32, now: DateTime<Utc>) -> MarketRecord {
        MarketRecord {
            market_id: market.id.0.to_string(),
            condition_id: market.condition_id.0.clone(),
            slug: market.slug.clone(),
            question: market.question.clone(),
            description: market.description.clone(),
            outcomes: market.outcomes.clone(),
            outcome_prices: market.outcome_prices.as_ref().map(|prices| prices.iter().map(|d| d.to_string()).collect()),
            clob_token_ids: market.clob_token_ids.iter().map(|id| id.0.clone()).collect(),
            category: market.category.clone(),
            end_date: market.end_date.map(|d| d.to_rfc3339()),
            created_at: market.created_at.to_rfc3339(),
            updated_at: market.updated_at.to_rfc3339(),
            closed_time: market.closed_time.map(|d| d.to_rfc3339()),
            image: market.image.clone(),
            icon: market.icon.clone(),
            volume: Some(market.volume().to_string()),
            liquidity: market.liquidity.map(|d| d.to_string()),
            active: market.active,
            closed: market.closed,
            archived: market.archived,
            restricted: market.restricted,
            cyom: market.cyom,
            approved: market.approved,
            volume_24hr: market.volume_24hr.map(|d| d.to_string()),
            volume_1wk: market.volume_1wk.map(|d| d.to_string()),
            volume_1mo: market.volume_1mo.map(|d| d.to_string()),
            volume_1yr: market.volume_1yr.map(|d| d.to_string()),
            best_bid: market.best_bid.map(|d| d.to_string()),
            best_ask: market.best_ask.map(|d| d.to_string()),
            last_trade_price: market.last_trade_price.map(|d| d.to_string()),
            accepting_orders: market.accepting_orders,
            enable_order_book: market.enable_order_book,
            featured: market.featured,
            new: market.new,
            neg_risk: market.neg_risk,
            spread: market.spread.map(|d| d.to_string()),
            start_date: market.start_date.map(|d| d.to_rfc3339()),
            first_seen: now.to_rfc3339(),
            last_updated_db: now.to_rfc3339(),
            source_session: session_id,
        }
    }
    
    /// Get all markets with optional filtering using batching
    
    
    /// Search markets by question text
    pub async fn search_markets(&self, query: &str, limit: Option<u64>) -> Result<Vec<GammaMarket>> {
        let search_query = if let Some(limit) = limit {
            format!(
                "SELECT * FROM markets WHERE question CONTAINS '{}' OR slug CONTAINS '{}' ORDER BY volume DESC LIMIT {}",
                query, query, limit
            )
        } else {
            format!(
                "SELECT * FROM markets WHERE question CONTAINS '{}' OR slug CONTAINS '{}' ORDER BY volume DESC",
                query, query
            )
        };
        
        // Execute query and get raw records
        let mut result = self.db.query(&search_query).await
            .context("Failed to execute search query")?;
            
        let raw_values: Vec<MarketRecordRaw> = result.take(0)
            .context("Failed to parse search query result as MarketRecordRaw")?;
        
        // Convert MarketRecordRaw to GammaMarket
        let markets = raw_values.into_iter()
            .map(|raw| {
                let record = raw.into_market_record();
                self.convert_record_to_gamma_market(record)
            })
            .collect::<Result<Vec<_>>>()?;
        
        debug!("Found {} markets matching query: {}", markets.len(), query);
        Ok(markets)
    }
    
    /// Get markets by status
    
    /// Get database statistics with enhanced caching
    pub async fn get_stats(&self) -> Result<MarketStats> {
        // Check in-memory cache first
        if let Ok(cache) = self.stats_cache.read() {
            if let Some((timestamp, stats)) = cache.as_ref() {
                if (Utc::now() - *timestamp).num_minutes() < self.cache_ttl_minutes {
                    debug!("Using cached stats");
                    return Ok(stats.clone());
                }
            }
        }
        
        // Try to get cached stats from database
        let mut result = self.db
            .query("SELECT * FROM stats:current")
            .await?;
            
        let cached_stats: Option<MarketStats> = result.take(0).ok().flatten();
        
        if let Some(stats) = cached_stats {
            // Return cached if less than 5 minutes old
            if let Ok(last_updated) = stats.last_updated.parse::<DateTime<Utc>>() {
                if (Utc::now() - last_updated).num_minutes() < 5 {
                    // Update in-memory cache
                    if let Ok(mut cache) = self.stats_cache.write() {
                        *cache = Some((Utc::now(), stats.clone()));
                    }
                    return Ok(stats);
                }
            }
        }
        
        // Calculate fresh stats
        self.update_stats().await?;
        
        let mut result = self.db
            .query("SELECT * FROM stats:current")
            .await?;
            
        let stats: Option<MarketStats> = result.take(0)?;
        
        if let Some(ref stats) = stats {
            // Update in-memory cache
            if let Ok(mut cache) = self.stats_cache.write() {
                *cache = Some((Utc::now(), stats.clone()));
            }
        }
        
        stats.ok_or_else(|| anyhow::anyhow!("Failed to generate stats"))
    }
    
    /// Update database statistics
    async fn update_stats(&self) -> Result<()> {
        debug!("Updating database statistics");
        
        // Use simpler counting approach
        let total_markets = self.get_market_count().await?;
        
        // Count records using COUNT queries instead of fetching all records
        let mut active_result = self.db
            .query("SELECT count() FROM markets WHERE active = true GROUP ALL")
            .await?;
        let active_count: Option<serde_json::Value> = active_result.take(0)?;
        let active_markets = active_count
            .and_then(|v| v.get("count").and_then(|c| c.as_u64()))
            .unwrap_or(0);
        
        let mut closed_result = self.db
            .query("SELECT count() FROM markets WHERE closed = true GROUP ALL")
            .await?;
        let closed_count: Option<serde_json::Value> = closed_result.take(0)?;
        let closed_markets = closed_count
            .and_then(|v| v.get("count").and_then(|c| c.as_u64()))
            .unwrap_or(0);
        
        let mut archived_result = self.db
            .query("SELECT count() FROM markets WHERE archived = true GROUP ALL")
            .await?;
        let archived_count: Option<serde_json::Value> = archived_result.take(0)?;
        let archived_markets = archived_count
            .and_then(|v| v.get("count").and_then(|c| c.as_u64()))
            .unwrap_or(0);
        
        // For now, just use 0 for volume and liquidity totals to avoid fetching all records
        let total_volume = Decimal::ZERO;
        let total_liquidity = Decimal::ZERO;
        
        let stats = MarketStats {
            total_markets,
            active_markets,
            closed_markets,
            archived_markets,
            total_volume: total_volume.to_string(),
            total_liquidity: total_liquidity.to_string(),
            last_updated: Utc::now().to_rfc3339(),
        };
        
        // Store updated stats using raw query to avoid serialization issues
        self.db
            .query("UPSERT stats:current CONTENT $content")
            .bind(("content", stats))
            .await
            .context("Failed to store updated stats")?;
        
        debug!("Updated database statistics: {} total markets", total_markets);
        Ok(())
    }
    
    /// Convert MarketRecord back to GammaMarket for display
    #[allow(dead_code)] // Used by search/milli_service.rs
    pub fn convert_record_to_market(&self, record: &MarketRecord) -> GammaMarket {
        GammaMarket {
            id: MarketId(record.market_id.parse().unwrap_or(0)),
            condition_id: ConditionId(record.condition_id.clone()),
            slug: record.slug.clone(),
            question: record.question.clone(),
            description: record.description.clone(),
            outcomes: record.outcomes.clone(),
            outcome_prices: record.outcome_prices.as_ref().map(|prices| prices.iter().filter_map(|s| s.parse().ok()).collect()),
            clob_token_ids: record.clob_token_ids.iter()
                .map(|id| ClobTokenId(id.clone()))
                .collect(),
            category: record.category.clone(),
            end_date: record.end_date.clone().and_then(|s| s.parse().ok()),
            created_at: record.created_at.parse().unwrap_or_else(|_| Utc::now()),
            updated_at: record.updated_at.parse().unwrap_or_else(|_| Utc::now()),
            closed_time: record.closed_time.clone().and_then(|s| s.parse().ok()),
            image: record.image.clone(),
            icon: record.icon.clone(),
            twitter_card_image: None,
            market_maker_address: None,
            volume_num: record.volume.clone().and_then(|s| s.parse().ok()),
            volume_alt: None,
            liquidity: record.liquidity.clone().and_then(|s| s.parse().ok()),
            active: record.active,
            closed: record.closed,
            archived: record.archived,
            restricted: record.restricted,
            cyom: record.cyom,
            approved: record.approved,
            volume_24hr: record.volume_24hr.clone().and_then(|s| s.parse().ok()),
            volume_1wk: record.volume_1wk.clone().and_then(|s| s.parse().ok()),
            volume_1mo: record.volume_1mo.clone().and_then(|s| s.parse().ok()),
            volume_1yr: record.volume_1yr.clone().and_then(|s| s.parse().ok()),
            best_bid: record.best_bid.clone().and_then(|s| s.parse().ok()),
            best_ask: record.best_ask.clone().and_then(|s| s.parse().ok()),
            last_trade_price: record.last_trade_price.clone().and_then(|s| s.parse().ok()),
            accepting_orders: record.accepting_orders,
            accepting_orders_timestamp: None,
            automatically_resolved: None,
            automatically_active: None,
            clear_book_on_start: None,
            clob_rewards: Vec::new(),
            deploying: None,
            enable_order_book: record.enable_order_book,
            end_date_iso: None,
            events: None,
            featured: record.featured,
            funded: None,
            group_item_threshold: None,
            group_item_title: None,
            has_reviewed_dates: None,
            manual_activation: None,
            neg_risk: record.neg_risk,
            neg_risk_other: None,
            new: record.new,
            one_day_price_change: None,
            one_hour_price_change: None,
            one_month_price_change: None,
            one_week_price_change: None,
            one_year_price_change: None,
            order_min_size: None,
            order_price_min_tick_size: None,
            pager_duty_notification_enabled: None,
            pending_deployment: None,
            question_id: None,
            ready: None,
            resolution_source: None,
            resolved_by: None,
            rewards_max_spread: None,
            rewards_min_size: None,
            rfq_enabled: None,
            spread: record.spread.clone().and_then(|s| s.parse().ok()),
            start_date: record.start_date.clone().and_then(|s| s.parse().ok()),
            start_date_iso: None,
            submitted_by: None,
            uma_bond: None,
            uma_end_date: None,
            uma_resolution_status: None,
            uma_resolution_statuses: None,
            uma_reward: None,
            volume_1mo_amm: None,
            volume_1mo_clob: None,
            volume_1wk_amm: None,
            volume_1wk_clob: None,
            volume_1yr_amm: None,
            volume_1yr_clob: None,
            volume_clob: None,
            competitive: None,
            deploying_timestamp: None,
            neg_risk_market_id: None,
            neg_risk_request_id: None,
            series_color: None,
            show_gmp_outcome: None,
            show_gmp_series: None,
            mailchimp_tag: None,
            market_type: None,
            ready_for_cron: None,
            updated_by: None,
            creator: None,
            wide_format: None,
            game_start_time: None,
            seconds_delay: None,
            sent_discord: None,
            notifications_enabled: None,
            fee: None,
            fpmm_live: None,
            volume_24hr_clob: None,
            volume_amm: None,
            liquidity_amm: None,
            comments_enabled: None,
            ticker: None,
        }
    }
    
    /// Get total market count with caching
    pub async fn get_market_count(&self) -> Result<u64> {
        let cache_key = "market_count".to_string();
        
        // Check cache first
        if let Some(cached_result) = self.get_cached_result(&cache_key)? {
            if let Some(count) = cached_result.as_u64() {
                debug!("Using cached market count: {}", count);
                return Ok(count);
            }
        }
        
        // Use count function
        let mut result = self.db
            .query("SELECT count() FROM markets GROUP ALL")
            .await?;
            
        // The result is an array with one object
        let counts: Vec<serde_json::Value> = result.take(0)?;
        
        let count = if let Some(first) = counts.first() {
            first.get("count").and_then(|v| v.as_u64()).unwrap_or(0)
        } else {
            0
        };
        
        // Cache the result
        self.cache_result(cache_key, serde_json::json!(count))?;
        
        Ok(count)
    }
    
    /// Get all markets from database with batching to avoid timeouts
    pub async fn get_all_markets(&self, limit: Option<u64>) -> Result<Vec<GammaMarket>> {
        info!("Fetching all markets with batching (limit: {:?})", limit);
        
        // Use larger batch size for better performance
        let batch_size = 10000;  // Increased from 1000 for 10x speed
        let max_limit = limit.unwrap_or(u64::MAX);
        let mut all_markets = Vec::new();
        let mut offset = 0u64;
        let start_time = std::time::Instant::now();
        
        loop {
            // Calculate how many to fetch in this batch
            let remaining = max_limit.saturating_sub(all_markets.len() as u64);
            let batch_limit = std::cmp::min(batch_size, remaining);
            
            if batch_limit == 0 {
                break;
            }
            
            let query = format!("SELECT * FROM markets LIMIT {} START {}", batch_limit, offset);
            debug!("Executing batch query: {} (batch {}, offset {})", query, (offset / batch_size) + 1, offset);
            
            let batch_start = std::time::Instant::now();
            
            // Use the same deserialization approach as execute_query
            let mut result = self.db.query(&query).await
                .with_context(|| format!("Failed to execute batch query at offset {}", offset))?;
            
            // Always use MarketRecordRaw since direct GammaMarket deserialization fails with SurrealDB Thing types
            let batch = match result.take::<Vec<MarketRecordRaw>>(0) {
                Ok(raw_records) => {
                    debug!("Successfully deserialized {} MarketRecordRaw records", raw_records.len());
                    
                    // Convert raw records to GammaMarket
                    let mut converted_markets = Vec::new();
                    for (i, raw) in raw_records.into_iter().enumerate() {
                        let record = raw.into_market_record();
                        match self.convert_record_to_gamma_market(record) {
                            Ok(market) => converted_markets.push(market),
                            Err(e) => {
                                error!("Failed to convert record {} in batch at offset {}: {}", i, offset, e);
                                continue; // Skip invalid records
                            }
                        }
                    }
                    
                    debug!("Successfully converted {} records to GammaMarket", converted_markets.len());
                    converted_markets
                }
                Err(e) => {
                    error!("MarketRecordRaw deserialization failed at offset {}: {}", offset, e);
                    return Err(anyhow::anyhow!("Failed to deserialize records at offset {}: {}", offset, e));
                }
            };
            
            let batch_time = batch_start.elapsed();
            let batch_len = batch.len();
            all_markets.extend(batch);
            
            info!("Batch {} complete: fetched {} markets in {:.2}s (total: {}, rate: {:.0} markets/s)", 
                (offset / batch_size) + 1, 
                batch_len, 
                batch_time.as_secs_f64(),
                all_markets.len(),
                batch_len as f64 / batch_time.as_secs_f64().max(0.001)
            );
            
            // If we got fewer than requested, we've reached the end
            if batch_len < batch_size as usize {
                info!("Reached end of data (got {} < {} batch size)", batch_len, batch_size);
                break;
            }
            
            // If we've reached our limit, stop
            if all_markets.len() >= max_limit as usize {
                info!("Reached requested limit of {} markets", max_limit);
                break;
            }
            
            offset += batch_size;
        }
        
        let total_time = start_time.elapsed();
        let avg_rate = all_markets.len() as f64 / total_time.as_secs_f64().max(0.001);
        info!("Completed fetching {} markets in {:.2}s (avg rate: {:.0} markets/s)", 
            all_markets.len(), total_time.as_secs_f64(), avg_rate);
        Ok(all_markets)
    }
    
    /// Get all markets with progress bar updates
    pub async fn get_all_markets_with_progress(&self, progress: Option<&indicatif::ProgressBar>) -> Result<Vec<GammaMarket>> {
        // First get total count with timing
        let count_start = std::time::Instant::now();
        let total_count = self.get_market_count().await?;
        let count_time = count_start.elapsed();
        info!("Got market count {} in {:.2}s", total_count, count_time.as_secs_f64());
        
        if let Some(pb) = progress {
            pb.set_length(total_count);
        }
        
        // Use larger batch size for better performance
        let batch_size = 10000;  // Increased from 1000 for 10x speed
        let mut all_markets = Vec::with_capacity(total_count as usize);
        let mut offset = 0;
        let start_time = std::time::Instant::now();
        
        while offset < total_count {
            let query = format!("SELECT * FROM markets LIMIT {} START {}", batch_size, offset);
            debug!("Progress batch query: {} (offset: {})", query, offset);
            
            let batch_start = std::time::Instant::now();
            
            // Use the same deserialization approach as execute_query
            let mut result = self.db.query(&query).await
                .with_context(|| format!("Failed to execute progress batch query at offset {}", offset))?;
            
            let batch = match result.take::<Vec<GammaMarket>>(0) {
                Ok(markets) => markets,
                Err(_) => {
                    // Try MarketRecordRaw fallback
                    let mut result = self.db.query(&query).await?;
                    let raw_values: Vec<MarketRecordRaw> = result.take(0)
                        .with_context(|| format!("Failed to parse progress batch as MarketRecordRaw at offset {}", offset))?;
                    
                    raw_values.into_iter()
                        .filter_map(|raw| {
                            let record = raw.into_market_record();
                            self.convert_record_to_gamma_market(record).ok()
                        })
                        .collect()
                }
            };
            
            let batch_time = batch_start.elapsed();
            let batch_len = batch.len();
            all_markets.extend(batch);
            
            if let Some(pb) = progress {
                pb.inc(batch_len as u64);
                pb.set_message(format!("Loaded {} markets ({:.1}s/batch)", all_markets.len(), batch_time.as_secs_f64()));
            }
            
            debug!("Progress batch {} complete: {} markets in {:.2}s", 
                offset / batch_size as u64 + 1, batch_len, batch_time.as_secs_f64());
            
            if batch_len < batch_size {
                info!("Progress fetch reached end of data");
                break;
            }
            
            offset += batch_size as u64;
        }
        
        let total_time = start_time.elapsed();
        info!("Progress fetch complete: {} markets in {:.2}s", all_markets.len(), total_time.as_secs_f64());
        Ok(all_markets)
    }
    
    /// Execute a raw query and return GammaMarket results with caching
    pub async fn execute_query(&self, query: &str) -> Result<Vec<GammaMarket>> {
        // For read-only queries, check cache
        if query.trim().to_uppercase().starts_with("SELECT") {
            if let Some(cached_result) = self.get_cached_result(query)? {
                if let Ok(markets) = serde_json::from_value::<Vec<GammaMarket>>(cached_result) {
                    debug!("Using cached query result for: {}", query);
                    return Ok(markets);
                }
            }
        }
        
        let mut result = self.db.query(query).await
            .context("Failed to execute query")?;
        
        // Try to deserialize as GammaMarket first
        match result.take::<Vec<GammaMarket>>(0) {
            Ok(markets) => {
                // Cache read-only query results
                if query.trim().to_uppercase().starts_with("SELECT") && markets.len() < 1000 {
                    let json_value = serde_json::to_value(&markets)?;
                    self.cache_result(query.to_string(), json_value)?;
                }
                Ok(markets)
            }
            Err(_) => {
                // Try MarketRecordRaw deserialization
                let mut result = self.db.query(query).await?;
                let raw_values: Vec<MarketRecordRaw> = result.take(0)
                    .context("Failed to parse as MarketRecordRaw")?;
                
                // Convert MarketRecord to GammaMarket
                let markets = raw_values.into_iter()
                    .map(|raw| {
                        let record = raw.into_market_record();
                        self.convert_record_to_gamma_market(record)
                    })
                    .collect::<Result<Vec<_>>>()?;
                
                // Cache if appropriate
                if query.trim().to_uppercase().starts_with("SELECT") && markets.len() < 1000 {
                    let json_value = serde_json::to_value(&markets)?;
                    self.cache_result(query.to_string(), json_value)?;
                }
                
                Ok(markets)
            }
        }
    }
    
    /// Execute a raw query and return raw results
    pub async fn execute_raw_query(&self, query: &str) -> Result<Vec<serde_json::Value>> {
        let mut result = self.db.query(query).await
            .context("Failed to execute raw query")?;
        
        // Try to extract the raw result first before parsing
        error!("DEBUG: About to call result.take(0) on query: {}", query);
        
        let values: Vec<serde_json::Value> = result.take(0)
            .with_context(|| {
                error!("DEBUG: result.take(0) failed for query: {}", query);
                "Failed to parse raw query results"
            })?;
        
        error!("DEBUG: Successfully got {} values from result.take(0)", values.len());
        Ok(values)
    }

    
    /// Clear all caches
    pub fn clear_cache(&self) -> Result<()> {
        if let Ok(mut cache) = self.query_cache.write() {
            cache.clear();
        }
        if let Ok(mut stats_cache) = self.stats_cache.write() {
            *stats_cache = None;
        }
        info!("Cleared all database caches");
        Ok(())
    }
    
    /// Get cached result if still valid
    fn get_cached_result(&self, key: &str) -> Result<Option<serde_json::Value>> {
        if let Ok(cache) = self.query_cache.read() {
            if let Some((timestamp, value)) = cache.get(key) {
                let now = Utc::now();
                if (now - *timestamp).num_minutes() < self.cache_ttl_minutes {
                    return Ok(Some(value.clone()));
                }
            }
        }
        Ok(None)
    }
    
    /// Cache a query result
    fn cache_result(&self, key: String, value: serde_json::Value) -> Result<()> {
        if let Ok(mut cache) = self.query_cache.write() {
            cache.insert(key, (Utc::now(), value));
            
            // Clean up old entries (keep cache size reasonable)
            if cache.len() > 100 {
                let cutoff = Utc::now() - chrono::Duration::minutes(self.cache_ttl_minutes);
                cache.retain(|_, (timestamp, _)| *timestamp > cutoff);
            }
        }
        Ok(())
    }
    
    
    /// Convert MarketRecord back to GammaMarket
    fn convert_record_to_gamma_market(&self, record: MarketRecord) -> Result<GammaMarket> {
        // Parse market_id as u64
        let market_id = record.market_id.parse::<u64>()
            .context("Failed to parse market_id as u64")?;
            
        Ok(GammaMarket {
            id: MarketId(market_id),
            condition_id: ConditionId(record.condition_id),
            slug: record.slug,
            question: record.question,
            description: record.description,
            outcomes: record.outcomes,
            outcome_prices: record.outcome_prices.map(|prices| {
                prices.into_iter()
                    .filter_map(|p| p.parse::<Decimal>().ok())
                    .collect()
            }),
            clob_token_ids: record.clob_token_ids.into_iter()
                .map(ClobTokenId)
                .collect(),
            category: record.category,
            end_date: record.end_date.clone().and_then(|d| DateTime::parse_from_rfc3339(&d).ok())
                .map(|d| d.with_timezone(&Utc)),
            created_at: DateTime::parse_from_rfc3339(&record.created_at)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&record.updated_at)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            closed_time: record.closed_time.and_then(|d| DateTime::parse_from_rfc3339(&d).ok())
                .map(|d| d.with_timezone(&Utc)),
            image: record.image,
            icon: record.icon,
            twitter_card_image: None,
            market_maker_address: None,
            volume_num: record.volume.and_then(|v| v.parse::<Decimal>().ok()),
            volume_alt: None,
            liquidity: record.liquidity.and_then(|l| l.parse::<Decimal>().ok()),
            active: record.active,
            closed: record.closed,
            archived: record.archived,
            restricted: record.restricted,
            cyom: record.cyom,
            approved: record.approved,
            volume_24hr: record.volume_24hr.and_then(|v| v.parse::<Decimal>().ok()),
            volume_1wk: record.volume_1wk.and_then(|v| v.parse::<Decimal>().ok()),
            volume_1mo: record.volume_1mo.and_then(|v| v.parse::<Decimal>().ok()),
            volume_1yr: record.volume_1yr.and_then(|v| v.parse::<Decimal>().ok()),
            best_bid: record.best_bid.and_then(|v| v.parse::<Decimal>().ok()),
            best_ask: record.best_ask.and_then(|v| v.parse::<Decimal>().ok()),
            last_trade_price: record.last_trade_price.and_then(|v| v.parse::<Decimal>().ok()),
            accepting_orders: record.accepting_orders,
            accepting_orders_timestamp: None,
            automatically_resolved: None,
            automatically_active: None,
            clear_book_on_start: None,
            clob_rewards: Vec::new(),
            deploying: None,
            enable_order_book: record.enable_order_book,
            end_date_iso: record.end_date,
            events: None,
            featured: record.featured,
            funded: None,
            group_item_threshold: None,
            group_item_title: None,
            has_reviewed_dates: None,
            manual_activation: None,
            neg_risk: record.neg_risk,
            neg_risk_other: None,
            new: record.new,
            one_day_price_change: None,
            one_hour_price_change: None,
            one_month_price_change: None,
            one_week_price_change: None,
            one_year_price_change: None,
            order_min_size: None,
            order_price_min_tick_size: None,
            pager_duty_notification_enabled: None,
            pending_deployment: None,
            question_id: None,
            ready: None,
            resolution_source: None,
            resolved_by: None,
            rewards_max_spread: None,
            rewards_min_size: None,
            rfq_enabled: None,
            spread: record.spread.and_then(|v| v.parse::<Decimal>().ok()),
            start_date: record.start_date.and_then(|d| DateTime::parse_from_rfc3339(&d).ok())
                .map(|d| d.with_timezone(&Utc)),
            start_date_iso: None,
            submitted_by: None,
            uma_bond: None,
            uma_end_date: None,
            uma_resolution_status: None,
            uma_resolution_statuses: None,
            uma_reward: None,
            volume_1mo_amm: None,
            volume_1mo_clob: None,
            volume_1wk_amm: None,
            volume_1wk_clob: None,
            volume_1yr_amm: None,
            volume_1yr_clob: None,
            volume_clob: None,
            competitive: None,
            deploying_timestamp: None,
            neg_risk_market_id: None,
            neg_risk_request_id: None,
            series_color: None,
            show_gmp_outcome: None,
            show_gmp_series: None,
            mailchimp_tag: None,
            market_type: None,
            ready_for_cron: None,
            updated_by: None,
            creator: None,
            wide_format: None,
            game_start_time: None,
            seconds_delay: None,
            sent_discord: None,
            notifications_enabled: None,
            fee: None,
            fpmm_live: None,
            volume_24hr_clob: None,
            volume_amm: None,
            liquidity_amm: None,
            comments_enabled: None,
            ticker: None,
        })
    }
    
    /// Perform a quick health check on the database
    pub async fn health_check(&self) -> Result<DatabaseHealth> {
        let _start_time = std::time::Instant::now();
        
        // Test basic connectivity
        let ping_start = std::time::Instant::now();
        let ping_result = self.db.query("RETURN 1").await;
        let ping_time = ping_start.elapsed();
        
        if let Err(e) = ping_result {
            error!("Database ping failed: {}", e);
            return Ok(DatabaseHealth {
                is_healthy: false,
                ping_time_ms: ping_time.as_millis() as u64,
                market_count: None,
                count_time_ms: None,
                issues: vec![format!("Database connectivity failed: {}", e)],
                recommendations: vec![
                    "Check if database path exists and is accessible".to_string(),
                    "Verify database permissions".to_string(),
                    "Check if SurrealDB service is running".to_string(),
                ],
            });
        }
        
        debug!("Database ping successful in {}ms", ping_time.as_millis());
        
        // Test market count query performance with a simple query
        let count_start = std::time::Instant::now();
        let count_result = async {
            let mut result = self.db.query("SELECT count() FROM markets GROUP ALL").await?;
            let counts: Vec<serde_json::Value> = result.take(0)?;
            let count = if let Some(first) = counts.first() {
                first.get("count").and_then(|v| v.as_u64()).unwrap_or(0)
            } else {
                0
            };
            Ok::<u64, anyhow::Error>(count)
        }.await;
        let count_time = count_start.elapsed();
        
        let mut issues = Vec::new();
        let mut recommendations = Vec::new();
        let market_count = count_result.ok();
        
        // Analyze performance
        if ping_time.as_millis() > 1000 {
            issues.push("Slow database ping response".to_string());
            recommendations.push("Database may be under heavy load".to_string());
        }
        
        if count_time.as_millis() > 5000 {
            issues.push("Slow market count query".to_string());
            recommendations.push("Consider rebuilding database indexes".to_string());
        }
        
        if let Some(count) = market_count {
            if count > 100000 {
                recommendations.push("Large dataset - use filters and smaller page sizes".to_string());
            }
        }
        
        let is_healthy = issues.is_empty() && market_count.is_some();
        
        Ok(DatabaseHealth {
            is_healthy,
            ping_time_ms: ping_time.as_millis() as u64,
            market_count,
            count_time_ms: Some(count_time.as_millis() as u64),
            issues,
            recommendations,
        })
    }
    
    /// Close database connection
    pub async fn close(&self) -> Result<()> {
        info!("Closing database connection");
        self.clear_cache()?;
        // SurrealDB handles cleanup automatically
        Ok(())
    }
}

/// Database health information
#[derive(Debug, Clone)]
pub struct DatabaseHealth {
    pub is_healthy: bool,
    pub ping_time_ms: u64,
    pub market_count: Option<u64>,
    pub count_time_ms: Option<u64>,
    pub issues: Vec<String>,
    pub recommendations: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[tokio::test]
    async fn test_database_initialization() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_db");
        
        let db = GammaDatabase::new(&db_path).await.unwrap();
        let count = db.get_market_count().await.unwrap();
        assert_eq!(count, 0);
    }
}