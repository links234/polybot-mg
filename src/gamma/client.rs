//! Comprehensive Gamma API client implementation
//! 
//! This module provides a unified client for all Polymarket data APIs:
//! - Gamma Markets API (markets, events)
//! - Data API (trades, positions)
//! - CLOB price history

use anyhow::{Context, Result};
use chrono::DateTime;
use reqwest::Client;
use serde_json::{self, Value};
use tracing::{debug, info, error, warn};

use super::types::*;
use super::cache::GammaCache;
use super::individual_storage::IndividualMarketStorage;

/// Comprehensive Gamma API client with intelligent caching
pub struct GammaClient {
    client: Client,
    gamma_base_url: String,
    data_base_url: String,
    _clob_base_url: String,
    cache: GammaCache,
    _individual_storage: Option<IndividualMarketStorage>,
}

impl GammaClient {
    /// Create a new Gamma API client with default endpoints
    pub fn new() -> Self {
        Self::new_with_cache_path(None)
    }

    /// Create a new Gamma API client with persistent cache
    pub fn new_with_cache_path(cache_path: Option<std::path::PathBuf>) -> Self {
        let cache = if let Some(path) = cache_path {
            info!("Initializing GammaClient with cache at: {}", path.display());
            GammaCache::new_with_path(0, Some(path)) // 0 = no expiration for immutable data
        } else {
            // Use default cache location in data directory
            let default_path = std::path::PathBuf::from("./data/gamma_cache.json");
            info!("Initializing GammaClient with default cache at: {}", default_path.display());
            // Use TTL = 0 for indefinite caching of immutable data
            GammaCache::new_with_path(0, Some(default_path))
        };

        // Initialize individual storage
        let _individual_storage = match IndividualMarketStorage::new("./data/database/gamma") {
            Ok(storage) => {
                info!("Initialized individual market storage");
                Some(storage)
            }
            Err(e) => {
                warn!("Failed to initialize individual market storage: {}", e);
                None
            }
        };
        
        Self {
            client: Client::new(),
            gamma_base_url: "https://gamma-api.polymarket.com".to_string(),
            data_base_url: "https://data-api.polymarket.com".to_string(),
            _clob_base_url: "https://clob.polymarket.com".to_string(),
            cache,
            _individual_storage,
        }
    }

    /// Create a new client with custom endpoints (for testing)
    pub fn _with_endpoints(gamma_url: String, data_url: String, clob_url: String) -> Self {
        Self {
            client: Client::new(),
            gamma_base_url: gamma_url,
            data_base_url: data_url,
            _clob_base_url: clob_url,
            cache: GammaCache::default(),
            _individual_storage: None,
        }
    }

    // ============================================================================
    // CACHE MANAGEMENT
    // ============================================================================

    /// Get cache statistics
    /// Reset cache and force refresh
    #[allow(dead_code)]
    pub fn reset_cache(&self) {
        self.cache.reset();
    }

    /// Check if cache needs refresh
    #[allow(dead_code)]
    pub fn cache_needs_refresh(&self) -> bool {
        self.cache.needs_refresh()
    }

    // ============================================================================
    // MARKETS API  
    // ============================================================================

    pub async fn fetch_markets(&self, query: &MarketQuery) -> Result<PaginatedResponse<GammaMarket>> {
        let url = format!("{}/markets", self.gamma_base_url);
        let params = self.build_market_query_params(query);
        
        info!("Fetching markets from URL: {}", url);
        debug!("Query parameters: {:?}", params);
        
        let response = self.client
            .get(&url)
            .query(&params)
            .send()
            .await
            .context("Failed to fetch markets from Gamma API")?;

        let status = response.status();
        info!("API response status: {}", status);

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!("Markets API error response: {}", error_text);
            return Err(anyhow::anyhow!(
                "Markets API returned status {}: {}",
                status,
                error_text
            ));
        }

        let response_text = response.text().await
            .context("Failed to get response text")?;
        
        debug!("Raw API response (first 1000 chars): {}", &response_text.chars().take(1000).collect::<String>());
        
        // IMMEDIATE SAVE: Save raw API response to prevent data loss
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S_%3f");
        let raw_response_path = format!("./data/gamma_raw_responses/markets_{}_offset_{}.json", 
                                       timestamp, query.offset.unwrap_or(0));
        
        // Create directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all("./data/gamma_raw_responses") {
            warn!("Failed to create raw responses directory: {}", e);
        } else {
            // Save raw response
            if let Err(e) = std::fs::write(&raw_response_path, &response_text) {
                error!("Failed to save raw API response to {}: {}", raw_response_path, e);
            } else {
                info!("Saved raw API response to {}", raw_response_path);
            }
        }
        
        // Parse as JSON array
        let markets_json: Vec<Value> = serde_json::from_str(&response_text)
            .context("Failed to parse markets response as JSON array")?;
        
        info!("Received {} markets from API", markets_json.len());

        let mut markets = Vec::new();
        for (i, market_json) in markets_json.into_iter().enumerate() {
            debug!("Parsing market {}: {:?}", i, market_json);
            match self.parse_market(market_json) {
                Ok(market) => markets.push(market),
                Err(e) => {
                    error!("Failed to parse market {}: {} - skipping", i, e);
                    // Continue parsing other markets instead of failing entirely
                }
            }
        }

        let offset = query.offset.unwrap_or(0);
        let limit = query.limit.unwrap_or(50);
        
        Ok(PaginatedResponse::new(markets, offset, limit, None))
    }

    /// Fetch a single market by ID
    pub async fn _fetch_market(&self, id: &MarketId) -> Result<Option<GammaMarket>> {
        let query = MarketQuery {
            ids: vec![id.clone()],
            limit: Some(1),
            ..Default::default()
        };
        
        let response = self.fetch_markets(&query).await?;
        Ok(response.data.into_iter().next())
    }

    /// Fetch markets by condition IDs
    pub async fn _fetch_markets_by_condition(&self, condition_ids: &[ConditionId]) -> Result<Vec<GammaMarket>> {
        let query = MarketQuery {
            condition_ids: condition_ids.to_vec(),
            limit: Some(500),
            ..Default::default()
        };
        
        let response = self.fetch_markets(&query).await?;
        Ok(response.data)
    }

    /// Fetch all markets from the API using pagination
    #[allow(dead_code)]
    pub async fn fetch_all_markets(&self, query: &MarketQuery) -> Result<Vec<GammaMarket>> {
        let mut all_markets = Vec::new();
        let mut offset = query.offset.unwrap_or(0);
        let batch_size = 500; // Always use 500 as batch size for efficiency
        let mut consecutive_empty_batches = 0;
        
        info!("Starting to fetch all markets with batch size {}", batch_size);
        
        loop {
            let current_query = MarketQuery {
                limit: Some(batch_size),
                offset: Some(offset),
                order: query.order.clone(),
                ascending: query.ascending,
                archived: query.archived,
                active: query.active,
                closed: query.closed,
                tags: query.tags.clone(),
                volume_min: query.volume_min,
                volume_max: query.volume_max,
                liquidity_min: query.liquidity_min,
                liquidity_max: query.liquidity_max,
                ..Default::default()
            };
            
            info!("Fetching batch at offset {} with limit {}", offset, batch_size);
            let response = self.fetch_markets(&current_query).await?;
            
            if response.data.is_empty() {
                consecutive_empty_batches += 1;
                info!("No markets returned in this batch (consecutive empty: {})", consecutive_empty_batches);
                if consecutive_empty_batches >= 2 {
                    info!("Two consecutive empty batches, stopping pagination");
                    break;
                }
                // Continue to next offset even if this batch was empty
                offset += batch_size;
                continue;
            } else {
                consecutive_empty_batches = 0; // Reset counter when we get data
            }
            
            let batch_count = response.data.len();
            all_markets.extend(response.data);
            info!("Fetched {} markets in this batch, total so far: {}", batch_count, all_markets.len());
            
            // Show progress to user
            if all_markets.len() % 1000 == 0 || batch_count < batch_size as usize {
                println!("📊 Loaded {} markets so far...", all_markets.len());
            }
            
            // Continue fetching until we get an empty response
            // Some batches may have fewer markets due to parsing errors
            
            offset += batch_size;
            
            // Safety checks to prevent infinite loops
            if all_markets.len() > 1_000_000 {
                info!("Reached safety limit of 1,000,000 markets, stopping");
                break;
            }
            
            if offset > 2_000_000 {
                info!("Reached maximum offset of 2,000,000, stopping");
                break;
            }
        }
        
        info!("Finished fetching all markets, total: {}", all_markets.len());
        Ok(all_markets)
    }

    // ============================================================================
    // EVENTS API
    // ============================================================================

    /// Fetch events with comprehensive filtering options
    pub async fn fetch_events(&self, query: &EventQuery) -> Result<PaginatedResponse<GammaEvent>> {
        let url = format!("{}/events", self.gamma_base_url);
        let params = self.build_event_query_params(query);
        
        debug!("Fetching events with query: {:?}", query);
        
        let response = self.client
            .get(&url)
            .query(&params)
            .send()
            .await
            .context("Failed to fetch events from Gamma API")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Events API returned status {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            ));
        }

        let raw_json = response.text().await
            .context("Failed to read response text")?;
        
        debug!("Raw API response (first 1000 chars): {}", &raw_json.chars().take(1000).collect::<String>());
        
        let events: Vec<GammaEvent> = serde_json::from_str(&raw_json)
            .context("Failed to parse events from JSON")?;

        let offset = query.offset.unwrap_or(0);
        let limit = query.limit.unwrap_or(50);
        
        Ok(PaginatedResponse::new(events, offset, limit, None))
    }

    /// Fetch a single event by ID
    pub async fn _fetch_event(&self, id: &EventId) -> Result<Option<GammaEvent>> {
        let query = EventQuery {
            ids: vec![id.clone()],
            limit: Some(1),
            ..Default::default()
        };
        
        let response = self.fetch_events(&query).await?;
        Ok(response.data.into_iter().next())
    }

    // ============================================================================
    // TRADES API (Data API)
    // ============================================================================

    /// Fetch trade history with filtering options
    pub async fn fetch_trades(&self, query: &TradeQuery) -> Result<PaginatedResponse<GammaTrade>> {
        let url = format!("{}/trades", self.data_base_url);
        let params = self.build_trade_query_params(query);
        
        debug!("Fetching trades with query: {:?}", query);
        
        let response = self.client
            .get(&url)
            .query(&params)
            .send()
            .await
            .context("Failed to fetch trades from Data API")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Trades API returned status {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            ));
        }

        let raw_json = response.text().await
            .context("Failed to read response text")?;
        
        debug!("Raw API response (first 1000 chars): {}", &raw_json.chars().take(1000).collect::<String>());
        
        let trades: Vec<GammaTrade> = serde_json::from_str(&raw_json)
            .context("Failed to parse trades from JSON")?;

        let offset = query.offset.unwrap_or(0);
        let limit = query.limit.unwrap_or(100);
        
        Ok(PaginatedResponse::new(trades, offset, limit, None))
    }

    /// Fetch trades for a specific market
    pub async fn _fetch_market_trades(&self, condition_id: &ConditionId, limit: Option<u32>) -> Result<Vec<GammaTrade>> {
        let query = TradeQuery {
            market: Some(condition_id.clone()),
            limit: limit.or(Some(500)),
            taker_only: Some(true),
            ..Default::default()
        };
        
        let response = self.fetch_trades(&query).await?;
        Ok(response.data)
    }

    /// Fetch trades for a specific user
    pub async fn _fetch_user_trades(&self, user: &UserAddress, limit: Option<u32>) -> Result<Vec<GammaTrade>> {
        let query = TradeQuery {
            user: Some(user.clone()),
            limit: limit.or(Some(500)),
            ..Default::default()
        };
        
        let response = self.fetch_trades(&query).await?;
        Ok(response.data)
    }

    // ============================================================================
    // POSITIONS API (Data API)
    // ============================================================================

    /// Fetch user positions with filtering options
    pub async fn fetch_positions(&self, query: &PositionQuery) -> Result<PaginatedResponse<GammaPosition>> {
        let url = format!("{}/positions", self.data_base_url);
        let params = self.build_position_query_params(query);
        
        debug!("Fetching positions with query: {:?}", query);
        
        let response = self.client
            .get(&url)
            .query(&params)
            .send()
            .await
            .context("Failed to fetch positions from Data API")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Positions API returned status {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            ));
        }

        let positions_json: Vec<Value> = response.json().await
            .context("Failed to parse positions response as JSON")?;

        let positions = positions_json.into_iter()
            .map(|v| self.parse_position(v))
            .collect::<Result<Vec<_>>>()?;

        let offset = query.offset.unwrap_or(0);
        let limit = query.limit.unwrap_or(50);
        
        Ok(PaginatedResponse::new(positions, offset, limit, None))
    }

    /// Fetch all positions for a user
    pub async fn _fetch_user_positions(&self, user: &UserAddress) -> Result<Vec<GammaPosition>> {
        let query = PositionQuery {
            user: user.clone(),
            limit: Some(500),
            size_threshold: Some(rust_decimal::Decimal::new(1, 0)), // >= 1.0
            ..Default::default()
        };
        
        let response = self.fetch_positions(&query).await?;
        Ok(response.data)
    }

    // ============================================================================
    // PRICE HISTORY API (CLOB)
    // ============================================================================

    /// Fetch price history for a token
    pub async fn _fetch_price_history(&self, query: &PriceHistoryQuery) -> Result<PriceHistory> {
        let url = format!("{}/prices-history", self._clob_base_url);
        let params = self._build_price_history_params(query);
        
        debug!("Fetching price history with query: {:?}", query);
        
        let response = self.client
            .get(&url)
            .query(&params)
            .send()
            .await
            .context("Failed to fetch price history from CLOB API")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Price history API returned status {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            ));
        }

        let history_json: Value = response.json().await
            .context("Failed to parse price history response as JSON")?;

        self._parse_price_history(history_json, query.market.clone())
    }

    // ============================================================================
    // HELPER METHODS
    // ============================================================================

    fn build_market_query_params(&self, query: &MarketQuery) -> Vec<(&str, String)> {
        let mut params = Vec::new();
        
        if let Some(limit) = query.limit {
            params.push(("limit", limit.to_string()));
        }
        if let Some(offset) = query.offset {
            params.push(("offset", offset.to_string()));
        }
        if let Some(ref order) = query.order {
            params.push(("order", order.clone()));
        }
        if let Some(ascending) = query.ascending {
            params.push(("ascending", ascending.to_string()));
        }
        if let Some(archived) = query.archived {
            params.push(("archived", archived.to_string()));
        }
        if let Some(active) = query.active {
            params.push(("active", active.to_string()));
        }
        if let Some(closed) = query.closed {
            params.push(("closed", closed.to_string()));
        }
        
        // Add vector parameters
        for id in &query.ids {
            params.push(("id", id.0.to_string()));
        }
        for slug in &query.slugs {
            params.push(("slug", slug.0.clone()));
        }
        
        // Add numeric ranges
        if let Some(min) = query.liquidity_min {
            params.push(("liquidity_num_min", min.to_string()));
        }
        if let Some(max) = query.liquidity_max {
            params.push(("liquidity_num_max", max.to_string()));
        }
        if let Some(min) = query.volume_min {
            params.push(("volume_num_min", min.to_string()));
        }
        if let Some(max) = query.volume_max {
            params.push(("volume_num_max", max.to_string()));
        }
        
        // Add date ranges
        if let Some(min) = query.start_date_min {
            params.push(("start_date_min", min.to_rfc3339()));
        }
        if let Some(max) = query.start_date_max {
            params.push(("start_date_max", max.to_rfc3339()));
        }
        if let Some(min) = query.end_date_min {
            params.push(("end_date_min", min.to_rfc3339()));
        }
        if let Some(max) = query.end_date_max {
            params.push(("end_date_max", max.to_rfc3339()));
        }
        
        // Add tag filters
        for tag in &query.tags {
            params.push(("tag", tag.clone()));
        }
        for tag_id in &query.tag_ids {
            params.push(("tag_id", tag_id.0.to_string()));
        }
        
        params
    }

    fn build_event_query_params(&self, query: &EventQuery) -> Vec<(&str, String)> {
        let mut params = Vec::new();
        
        if let Some(limit) = query.limit {
            params.push(("limit", limit.to_string()));
        }
        if let Some(offset) = query.offset {
            params.push(("offset", offset.to_string()));
        }
        if let Some(ref order) = query.order {
            params.push(("order", order.clone()));
        }
        if let Some(ascending) = query.ascending {
            params.push(("ascending", ascending.to_string()));
        }
        if let Some(archived) = query.archived {
            params.push(("archived", archived.to_string()));
        }
        if let Some(active) = query.active {
            params.push(("active", active.to_string()));
        }
        if let Some(closed) = query.closed {
            params.push(("closed", closed.to_string()));
        }
        
        // Note: Events API uses different parameter names (no _num suffix)
        if let Some(min) = query.liquidity_min {
            params.push(("liquidity_min", min.to_string()));
        }
        if let Some(max) = query.liquidity_max {
            params.push(("liquidity_max", max.to_string()));
        }
        if let Some(min) = query.volume_min {
            params.push(("volume_min", min.to_string()));
        }
        if let Some(max) = query.volume_max {
            params.push(("volume_max", max.to_string()));
        }
        
        params
    }

    fn build_trade_query_params(&self, query: &TradeQuery) -> Vec<(&str, String)> {
        let mut params = Vec::new();
        
        if let Some(ref user) = query.user {
            params.push(("user", user.0.clone()));
        }
        if let Some(ref market) = query.market {
            params.push(("market", market.0.clone()));
        }
        if let Some(limit) = query.limit {
            params.push(("limit", limit.to_string()));
        }
        if let Some(offset) = query.offset {
            params.push(("offset", offset.to_string()));
        }
        if let Some(taker_only) = query.taker_only {
            params.push(("takerOnly", taker_only.to_string()));
        }
        if let Some(ref filter_type) = query.filter_type {
            let filter_str = match filter_type {
                FilterType::Cash => "CASH",
                FilterType::Tokens => "TOKENS",
            };
            params.push(("filterType", filter_str.to_string()));
        }
        if let Some(filter_amount) = query.filter_amount {
            params.push(("filterAmount", filter_amount.to_string()));
        }
        if let Some(ref side) = query.side {
            let side_str = match side {
                TradeSide::Buy => "BUY",
                TradeSide::Sell => "SELL",
            };
            params.push(("side", side_str.to_string()));
        }
        
        params
    }

    fn build_position_query_params(&self, query: &PositionQuery) -> Vec<(&str, String)> {
        let mut params = Vec::new();
        
        params.push(("user", query.user.0.clone()));
        
        if !query.markets.is_empty() {
            let markets_str = query.markets.iter()
                .map(|m| m.0.clone())
                .collect::<Vec<_>>()
                .join(",");
            params.push(("market", markets_str));
        }
        
        if let Some(ref event_id) = query.event_id {
            params.push(("eventId", event_id.0.to_string()));
        }
        if let Some(threshold) = query.size_threshold {
            params.push(("sizeThreshold", threshold.to_string()));
        }
        if let Some(redeemable) = query.redeemable {
            params.push(("redeemable", redeemable.to_string()));
        }
        if let Some(mergeable) = query.mergeable {
            params.push(("mergeable", mergeable.to_string()));
        }
        if let Some(ref title) = query.title {
            params.push(("title", title.clone()));
        }
        if let Some(limit) = query.limit {
            params.push(("limit", limit.to_string()));
        }
        if let Some(offset) = query.offset {
            params.push(("offset", offset.to_string()));
        }
        if let Some(ref sort_by) = query.sort_by {
            params.push(("sortBy", sort_by.clone()));
        }
        if let Some(ref sort_direction) = query.sort_direction {
            params.push(("sortDirection", sort_direction.clone()));
        }
        
        params
    }

    fn _build_price_history_params(&self, query: &PriceHistoryQuery) -> Vec<(&str, String)> {
        let mut params = Vec::new();
        
        params.push(("market", query.market.0.clone()));
        
        if let Some(start) = query.start_ts {
            params.push(("startTs", start.timestamp().to_string()));
        }
        if let Some(end) = query.end_ts {
            params.push(("endTs", end.timestamp().to_string()));
        }
        if let Some(ref interval) = query.interval {
            params.push(("interval", interval.clone()));
        }
        if let Some(fidelity) = query.fidelity {
            params.push(("fidelity", fidelity.to_string()));
        }
        
        params
    }

    // ============================================================================
    // PARSING METHODS
    // ============================================================================

    fn parse_market(&self, value: Value) -> Result<GammaMarket> {
        debug!("Attempting to parse market JSON: {}", serde_json::to_string_pretty(&value).unwrap_or_else(|_| "Failed to serialize".to_string()));
        
        match serde_json::from_value::<GammaMarket>(value.clone()) {
            Ok(market) => {
                debug!("Successfully parsed market: {}", market.id.0);
                Ok(market)
            }
            Err(e) => {
                error!("Failed to parse market from JSON: {}", e);
                error!("Problematic JSON: {}", serde_json::to_string_pretty(&value).unwrap_or_else(|_| "Failed to serialize".to_string()));
                
                // Try to identify specific field causing issue
                if let Some(obj) = value.as_object() {
                    for (key, val) in obj {
                        let val_type = match val {
                            serde_json::Value::Null => "null",
                            serde_json::Value::Bool(_) => "bool",
                            serde_json::Value::Number(_) => "number",
                            serde_json::Value::String(s) => {
                                if s.starts_with('[') && s.ends_with(']') {
                                    "string(json_array)"
                                } else {
                                    "string"
                                }
                            },
                            serde_json::Value::Array(_) => "array",
                            serde_json::Value::Object(_) => "object",
                        };
                        debug!("Field '{}': type={}, value={}", key, val_type, 
                            if let serde_json::Value::String(s) = val {
                                if s.len() > 100 {
                                    format!("{}...", &s[..100])
                                } else {
                                    s.clone()
                                }
                            } else {
                                val.to_string()
                            }
                        );
                    }
                }
                
                // Try parsing individual fields to find the problematic one
                debug!("Attempting to parse GammaMarket fields individually...");
                
                // Test parsing the id field
                if let Some(id_val) = value.get("id") {
                    match serde_json::from_value::<MarketId>(id_val.clone()) {
                        Ok(id) => debug!("✓ Successfully parsed id: {:?}", id),
                        Err(e) => error!("✗ Failed to parse id: {}", e),
                    }
                }
                
                Err(anyhow::anyhow!("Failed to parse market from JSON: {}", e))
            }
        }
    }


    fn parse_position(&self, value: Value) -> Result<GammaPosition> {
        serde_json::from_value(value)
            .context("Failed to parse position from JSON")
    }

    fn _parse_price_history(&self, value: Value, token_id: ClobTokenId) -> Result<PriceHistory> {
        let history_array = value.get("history")
            .and_then(|h| h.as_array())
            .context("Expected 'history' field as array")?;

        let mut price_points = Vec::new();
        for point in history_array {
            let t = point.get("t")
                .and_then(|t| t.as_i64())
                .context("Expected 't' field as timestamp")?;
            let p = point.get("p")
                .and_then(|p| p.as_f64())
                .context("Expected 'p' field as price")?;

            let timestamp = DateTime::from_timestamp(t, 0)
                .context("Invalid timestamp")?;
            let price = rust_decimal::Decimal::try_from(p)
                .context("Invalid price decimal")?;

            price_points.push(PricePoint { timestamp, price });
        }

        Ok(PriceHistory {
            token_id,
            history: price_points,
        })
    }
}

impl Default for GammaClient {
    fn default() -> Self {
        Self::new()
    }
}