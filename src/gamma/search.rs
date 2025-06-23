//! Advanced search and filtering capabilities for Gamma data
//! 
//! This module provides sophisticated search functionality including:
//! - Full-text search across titles, descriptions, and tags
//! - Multi-criteria filtering with numeric ranges
//! - Tag-based categorization and filtering
//! - Time-based range queries
//! - Performance analytics and ranking

use anyhow::Result;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use tracing::{debug, info};

use super::types::*;
use super::storage::GammaStorage;

/// Advanced search engine for Gamma data
pub struct GammaSearchEngine {
    storage: GammaStorage,
}

impl GammaSearchEngine {
    /// Create new search engine
    pub fn new(storage: GammaStorage) -> Self {
        Self { storage }
    }

    /// Comprehensive market search with multiple filters
    pub fn search_markets(&self, filters: &SearchFilters) -> Result<Vec<GammaMarket>> {
        debug!("Searching markets with filters: {:?}", filters);
        
        let mut results = Vec::new();
        
        // Start with all markets or filter by specific criteria
        let candidate_markets = if let Some(ref keyword) = filters.keyword {
            self.search_markets_by_keyword(keyword)?
        } else if !filters.tags.is_empty() {
            self.search_markets_by_tags(&filters.tags)?
        } else {
            self.get_all_markets()?
        };

        // Apply additional filters
        for market in candidate_markets {
            if self.market_matches_filters(&market, filters) {
                results.push(market);
            }
        }

        // Sort results by relevance/volume
        results.sort_by(|a, b| {
            b.volume().cmp(&a.volume())
        });

        info!("Found {} markets matching search criteria", results.len());
        Ok(results)
    }

    /// Search markets by keyword in title, description, or outcomes
    pub fn search_markets_by_keyword(&self, keyword: &str) -> Result<Vec<GammaMarket>> {
        let all_markets = self.get_all_markets()?;
        let keyword_lower = keyword.to_lowercase();
        
        let mut results = Vec::new();
        for market in all_markets {
            if self.market_contains_keyword(&market, &keyword_lower) {
                results.push(market);
            }
        }
        
        Ok(results)
    }

    /// Search markets by tags
    pub fn search_markets_by_tags(&self, tags: &[String]) -> Result<Vec<GammaMarket>> {
        let mut all_results = HashSet::new();
        
        for tag in tags {
            let tag_results = self.storage.search_markets_by_tag(tag)?;
            for market in tag_results {
                all_results.insert(market.id.clone());
            }
        }
        
        // Fetch full market objects
        let mut results = Vec::new();
        for market_id in all_results {
            if let Some(market) = self.storage.get_market(&market_id)? {
                results.push(market);
            }
        }
        
        Ok(results)
    }

    /// Get markets by category
    pub fn _search_markets_by_category(&self, category: &str) -> Result<Vec<GammaMarket>> {
        let all_markets = self.get_all_markets()?;
        
        let results = all_markets.into_iter()
            .filter(|market| {
                market.category.as_ref()
                    .map(|c| c.eq_ignore_ascii_case(category))
                    .unwrap_or(false)
            })
            .collect();
        
        Ok(results)
    }

    /// Get markets by volume range
    pub fn _search_markets_by_volume(&self, min: Option<Decimal>, max: Option<Decimal>) -> Result<Vec<GammaMarket>> {
        let all_markets = self.get_all_markets()?;
        
        let results = all_markets.into_iter()
            .filter(|market| {
                let volume = market.volume();
                
                if let Some(min_vol) = min {
                    if volume < min_vol {
                        return false;
                    }
                }
                
                if let Some(max_vol) = max {
                    if volume > max_vol {
                        return false;
                    }
                }
                
                true
            })
            .collect();
        
        Ok(results)
    }

    /// Get top markets by volume
    pub fn get_top_markets_by_volume(&self, limit: usize) -> Result<Vec<GammaMarket>> {
        let mut markets = self.get_all_markets()?;
        
        markets.sort_by(|a, b| b.volume().cmp(&a.volume()));
        markets.truncate(limit);
        
        Ok(markets)
    }

    /// Get top markets by liquidity
    pub fn _get_top_markets_by_liquidity(&self, limit: usize) -> Result<Vec<GammaMarket>> {
        let mut markets = self.get_all_markets()?;
        
        markets.sort_by(|a, b| b.liquidity.cmp(&a.liquidity));
        markets.truncate(limit);
        
        Ok(markets)
    }

    /// Get recently created markets
    pub fn _get_recent_markets(&self, hours: u32, limit: usize) -> Result<Vec<GammaMarket>> {
        let cutoff_time = Utc::now() - chrono::Duration::hours(hours as i64);
        let all_markets = self.get_all_markets()?;
        
        let mut recent_markets: Vec<GammaMarket> = all_markets.into_iter()
            .filter(|market| market.created_at > cutoff_time)
            .collect();
        
        recent_markets.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        recent_markets.truncate(limit);
        
        Ok(recent_markets)
    }

    /// Get markets closing soon
    pub fn _get_markets_closing_soon(&self, hours: u32, limit: usize) -> Result<Vec<GammaMarket>> {
        let cutoff_time = Utc::now() + chrono::Duration::hours(hours as i64);
        let all_markets = self.get_all_markets()?;
        
        let mut closing_soon: Vec<GammaMarket> = all_markets.into_iter()
            .filter(|market| {
                market.active && 
                !market.closed && 
                market.end_date.map(|d| d <= cutoff_time).unwrap_or(false)
            })
            .collect();
        
        closing_soon.sort_by(|a, b| {
            match (a.end_date, b.end_date) {
                (Some(a_date), Some(b_date)) => a_date.cmp(&b_date),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        });
        closing_soon.truncate(limit);
        
        Ok(closing_soon)
    }

    /// Get market analytics and statistics
    pub fn get_market_analytics(&self) -> Result<MarketAnalytics> {
        let all_markets = self.get_all_markets()?;
        
        let mut analytics = MarketAnalytics::default();
        let mut total_volume = Decimal::ZERO;
        let mut total_liquidity = Decimal::ZERO;
        let mut category_counts: HashMap<String, u32> = HashMap::new();
        let tag_counts: HashMap<String, u32> = HashMap::new();
        
        for market in &all_markets {
            // Count by status
            if market.active {
                analytics.active_markets += 1;
            }
            if market.closed {
                analytics.closed_markets += 1;
            }
            if market.archived {
                analytics.archived_markets += 1;
            }
            
            // Accumulate volume and liquidity
            total_volume += market.volume();
            total_liquidity += market.liquidity.unwrap_or_default();
            
            // Count categories
            if let Some(ref category) = market.category {
                *category_counts.entry(category.clone()).or_insert(0) += 1;
            }
            
            // Tags are not available in simplified structure for now
            // TODO: Add tags support when needed
        }
        
        analytics.total_markets = all_markets.len() as u64;
        analytics.total_volume = total_volume;
        analytics.total_liquidity = total_liquidity;
        
        if !all_markets.is_empty() {
            analytics.avg_volume = total_volume / Decimal::from(all_markets.len());
            analytics.avg_liquidity = total_liquidity / Decimal::from(all_markets.len());
        }
        
        // Get top categories and tags
        analytics.top_categories = self.get_top_entries(category_counts, 10);
        analytics.top_tags = self.get_top_entries(tag_counts, 20);
        
        Ok(analytics)
    }

    /// Search trade history with filters
    pub fn _search_trades(&self, filters: &TradeSearchFilters) -> Result<Vec<GammaTrade>> {
        let mut results = Vec::new();
        
        if let Some(ref user) = filters._user {
            let user_trades = self.storage._get_trades_by_user(user, filters._limit)?;
            results.extend(user_trades);
        }
        
        if let Some(ref condition_id) = filters._condition_id {
            let market_trades = self.storage._get_trades_by_market(condition_id, filters._limit)?;
            results.extend(market_trades);
        }
        
        // Apply additional filters
        results.retain(|trade| self._trade_matches_filters(trade, filters));
        
        // Sort by timestamp (newest first)
        results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        
        if let Some(limit) = filters._limit {
            results.truncate(limit);
        }
        
        Ok(results)
    }

    /// Get trade analytics for a user
    pub fn _get_user_trade_analytics(&self, user: &UserAddress) -> Result<UserTradeAnalytics> {
        let trades = self.storage._get_trades_by_user(user, None)?;
        
        let mut analytics = UserTradeAnalytics::default();
        let mut total_volume = Decimal::ZERO;
        let mut buy_count = 0;
        let mut sell_count = 0;
        let mut markets_traded: HashSet<ConditionId> = HashSet::new();
        
        for trade in &trades {
            total_volume += trade.price * trade.size;
            markets_traded.insert(trade.condition_id.clone());
            
            match trade.side {
                TradeSide::Buy => buy_count += 1,
                TradeSide::Sell => sell_count += 1,
            }
        }
        
        analytics._total_trades = trades.len() as u64;
        analytics._total_volume = total_volume;
        analytics._buy_trades = buy_count;
        analytics._sell_trades = sell_count;
        analytics._unique_markets = markets_traded.len() as u64;
        
        if !trades.is_empty() {
            analytics._avg_trade_size = total_volume / Decimal::from(trades.len());
            
            // Find first and last trade times
            let mut timestamps: Vec<_> = trades.iter().map(|t| t.timestamp).collect();
            timestamps.sort();
            analytics._first_trade = timestamps.first().copied();
            analytics._last_trade = timestamps.last().copied();
        }
        
        Ok(analytics)
    }

    // ============================================================================
    // HELPER METHODS
    // ============================================================================

    /// Get all markets (this would be optimized in production)
    fn get_all_markets(&self) -> Result<Vec<GammaMarket>> {
        // In a real implementation, this would use an iterator to avoid loading all data at once
        // For now, we'll implement a basic version
        let markets = Vec::new();
        
        // This is a placeholder - in reality we'd iterate through the RocksDB efficiently
        // For the MVP, we can return an empty vec and implement proper iteration later
        
        Ok(markets)
    }

    /// Check if market matches search filters
    fn market_matches_filters(&self, market: &GammaMarket, filters: &SearchFilters) -> bool {
        // Category filter
        if let Some(ref category) = filters.category {
            if market.category.as_ref().map(|c| c.eq_ignore_ascii_case(category)).unwrap_or(false) == false {
                return false;
            }
        }
        
        // Volume range filters
        if let Some(min_vol) = filters.min_volume {
            if market.volume() < min_vol {
                return false;
            }
        }
        if let Some(max_vol) = filters.max_volume {
            if market.volume() > max_vol {
                return false;
            }
        }
        
        // Liquidity range filters
        if let Some(min_liq) = filters.min_liquidity {
            if market.liquidity.unwrap_or_default() < min_liq {
                return false;
            }
        }
        if let Some(max_liq) = filters.max_liquidity {
            if market.liquidity.unwrap_or_default() > max_liq {
                return false;
            }
        }
        
        // Status filters
        if filters.active_only && !market.active {
            return false;
        }
        if filters.closed_only && !market.closed {
            return false;
        }
        if filters.archived_only && !market.archived {
            return false;
        }
        
        // Market type filter - TODO: Add market type field when available
        // if let Some(ref market_type) = filters.market_type {
        //     if market.market_type != *market_type {
        //         return false;
        //     }
        // }
        
        // Date range filter - only end_date is available in simplified structure
        if let Some((start, end)) = filters.date_range {
            if market.created_at < start || market.end_date.map(|d| d > end).unwrap_or(false) {
                return false;
            }
        }
        
        // Tag filter - TODO: Add tags support when available
        // if !filters.tags.is_empty() {
        //     // Tags not available in simplified structure for now
        //     return false;
        // }
        
        true
    }

    /// Check if market contains keyword
    fn market_contains_keyword(&self, market: &GammaMarket, keyword: &str) -> bool {
        // Search in question/title
        if market.question.to_lowercase().contains(keyword) {
            return true;
        }
        
        // Search in description
        if let Some(ref desc) = market.description {
            if desc.to_lowercase().contains(keyword) {
                return true;
            }
        }
        
        // Search in outcomes
        for outcome in &market.outcomes {
            if outcome.to_lowercase().contains(keyword) {
                return true;
            }
        }
        
        // Search in tags - TODO: Add tags support when available
        // for tag in &market.tags {
        //     if tag.label.to_lowercase().contains(keyword) {
        //         return true;
        //     }
        // }
        
        // Search in category
        if let Some(ref category) = market.category {
            if category.to_lowercase().contains(keyword) {
                return true;
            }
        }
        
        false
    }

    /// Check if trade matches filters
    fn _trade_matches_filters(&self, trade: &GammaTrade, filters: &TradeSearchFilters) -> bool {
        // Side filter
        if let Some(ref side) = filters._side {
            if trade.side != *side {
                return false;
            }
        }
        
        // Minimum size filter
        if let Some(min_size) = filters._min_size {
            if trade.size < min_size {
                return false;
            }
        }
        
        // Date range filter
        if let Some((start, end)) = filters._date_range {
            if trade.timestamp < start || trade.timestamp > end {
                return false;
            }
        }
        
        true
    }

    /// Get top entries from a count map
    fn get_top_entries(&self, counts: HashMap<String, u32>, limit: usize) -> Vec<(String, u64)> {
        let mut entries: Vec<(String, u32)> = counts.into_iter().collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries.truncate(limit);
        
        entries.into_iter()
            .map(|(k, v)| (k, v as u64))
            .collect()
    }
}

/// Trade search filters
#[derive(Debug, Clone, Default)]
pub struct TradeSearchFilters {
    pub _user: Option<UserAddress>,
    pub _condition_id: Option<ConditionId>,
    pub _side: Option<TradeSide>,
    pub _min_size: Option<Decimal>,
    pub _date_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
    pub _limit: Option<usize>,
}

/// Market analytics data
#[derive(Debug, Default, Serialize)]
pub struct MarketAnalytics {
    pub total_markets: u64,
    pub active_markets: u64,
    pub closed_markets: u64,
    pub archived_markets: u64,
    pub total_volume: Decimal,
    pub total_liquidity: Decimal,
    pub avg_volume: Decimal,
    pub avg_liquidity: Decimal,
    pub top_categories: Vec<(String, u64)>,
    pub top_tags: Vec<(String, u64)>,
}

/// User trade analytics
#[derive(Debug, Default)]
pub struct UserTradeAnalytics {
    pub _total_trades: u64,
    pub _buy_trades: u64,
    pub _sell_trades: u64,
    pub _total_volume: Decimal,
    pub _avg_trade_size: Decimal,
    pub _unique_markets: u64,
    pub _first_trade: Option<DateTime<Utc>>,
    pub _last_trade: Option<DateTime<Utc>>,
}