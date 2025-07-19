//! Order book state management with proper Polymarket SHA-1 hash verification

use crate::core::types::common::Side;
use crate::core::types::market::PriceLevel;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use thiserror::Error;
use tracing::{debug, error, warn};

#[derive(Error, Debug)]
pub enum StateError {
    #[error("Hash verification failed: expected {expected}, got {computed}")]
    HashMismatch { expected: String, computed: String },
}

/// One aggregated price level (price + size) for hash calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderSummary {
    pub price: String,
    pub size: String,
}

impl OrderSummary {
    /// Create new OrderSummary from Decimal values
    pub fn new(price: Decimal, size: Decimal) -> Self {
        // Format numbers to match Polymarket's format
        // Integers are shown without decimals, decimals keep their precision
        let format_price = |d: Decimal| -> String {
            // Always keep price as-is
            d.to_string()
        };
        
        let format_size = |d: Decimal| -> String {
            // For size, if it's a whole number, show without decimal
            if d.fract().is_zero() {
                d.trunc().to_string()
            } else {
                d.to_string()
            }
        };
        
        Self {
            price: format_price(price),
            size: format_size(size),
        }
    }

    /// Parse the `price` as f64 for numeric sorting.
    #[inline]
    fn price_num(&self) -> f64 {
        // Accepts ".48" as shorthand for "0.48".
        self.price
            .strip_prefix('.')
            .map(|p| format!("0.{p}"))
            .unwrap_or_else(|| self.price.clone())
            .parse::<f64>()
            .unwrap_or(0.0)
    }
}

/// Full book for hash calculation - matches Polymarket Python client exactly
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolymarketOrderBook {
    pub market: String,
    pub asset_id: String,
    pub bids: Vec<OrderSummary>,
    pub asks: Vec<OrderSummary>,
    pub timestamp: u64,
    pub hash: String,
}

impl PolymarketOrderBook {
    /// Create a new book matching Polymarket Python client field order.
    pub fn new(market: String, asset_id: String, timestamp: u64, bids: Vec<OrderSummary>, asks: Vec<OrderSummary>) -> Self {
        Self { 
            market,
            asset_id,
            bids, 
            asks,
            timestamp,
            hash: String::new(),
        }
    }

    /// Compute the Polymarket SHA-1 order-book hash.
    pub fn hash(&self) -> String {
        // 1 — sort bids ↓, asks ↑
        let mut bids = self.bids.clone();
        bids.sort_by(|a, b| {
            b.price_num()
                .partial_cmp(&a.price_num())
                .unwrap_or(Ordering::Equal)
        });
        let mut asks = self.asks.clone();
        asks.sort_by(|a, b| {
            a.price_num()
                .partial_cmp(&b.price_num())
                .unwrap_or(Ordering::Equal)
        });

        // 2 — Create orderbook with exact field order as Python implementation
        let canonical = PolymarketOrderBook { 
            market: self.market.clone(),
            asset_id: self.asset_id.clone(),
            bids, 
            asks,
            timestamp: self.timestamp,
            hash: String::new(),
        };
        
        // 3 — Serialize to JSON with compact separators (matching Python)
        // Use to_vec() and then String::from_utf8() to ensure no extra spaces
        let json_bytes = serde_json::to_vec(&canonical).expect("serialising order‑book to JSON");
        let json = String::from_utf8(json_bytes).expect("valid UTF-8");
        
        // 4 — SHA‑1 digest → hex
        let digest = Sha1::digest(json.as_bytes());
        format!("{:x}", digest) // 40‑char lower‑case hex
    }
}

/// Level-2 order book with proper hash verification
#[derive(Debug, Clone)]
pub struct OrderBook {
    /// Asset ID this order book represents
    pub asset_id: String,
    /// Market this order book belongs to
    pub market: String,
    /// Last update timestamp
    pub timestamp: u64,
    /// Bid levels (price -> size), sorted descending by price
    pub bids: BTreeMap<Decimal, Decimal>,
    /// Ask levels (price -> size), sorted ascending by price
    pub asks: BTreeMap<Decimal, Decimal>,
    /// Last known hash from WebSocket feed
    pub last_hash: Option<String>,
    /// Tick size for this asset
    pub tick_size: Option<Decimal>,
}

impl OrderBook {
    /// Create new empty order book
    pub fn new(asset_id: String) -> Self {
        Self {
            asset_id,
            market: String::new(),
            timestamp: 0,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            last_hash: None,
            tick_size: None,
        }
    }

    /// Replace order book with new snapshot
    pub fn replace_with_snapshot(
        &mut self,
        market: String,
        timestamp: u64,
        bids: Vec<PriceLevel>,
        asks: Vec<PriceLevel>,
        hash: String,
    ) -> Result<(), StateError> {
        // Update market and timestamp
        self.market = market;
        self.timestamp = timestamp;
        // Clear existing data
        self.bids.clear();
        self.asks.clear();

        // Insert new levels
        for level in bids {
            if level.size > Decimal::ZERO {
                self.bids.insert(level.price, level.size);
            }
        }

        for level in asks {
            if level.size > Decimal::ZERO {
                self.asks.insert(level.price, level.size);
            }
        }

        // Verify hash using Polymarket-compatible calculation
        let computed_hash = self.compute_polymarket_hash();
        if computed_hash != hash {
            // Debug: show what we're hashing
            let debug_json = self.debug_hash_json(
                &self.bids.iter().map(|(&p, &s)| (p, s)).collect::<Vec<_>>(),
                &self.asks.iter().map(|(&p, &s)| (p, s)).collect::<Vec<_>>(),
            );
            warn!(
                asset_id = %self.asset_id,
                expected = %hash,
                computed = %computed_hash,
                bids_count = self.bids.len(),
                asks_count = self.asks.len(),
                market = %self.market,
                timestamp = self.timestamp,
                json_hashed = %debug_json,
                "Hash mismatch on snapshot"
            );
            return Err(StateError::HashMismatch {
                expected: hash,
                computed: computed_hash,
            });
        }

        self.last_hash = Some(hash);
        debug!(
            asset_id = %self.asset_id,
            bids_count = self.bids.len(),
            asks_count = self.asks.len(),
            "Order book snapshot applied successfully"
        );
        Ok(())
    }

    /// Replace order book with new snapshot without hash validation
    pub fn replace_with_snapshot_no_hash(
        &mut self,
        market: String,
        timestamp: u64,
        bids: Vec<PriceLevel>,
        asks: Vec<PriceLevel>,
    ) {
        // Update market and timestamp
        self.market = market;
        self.timestamp = timestamp;
        // Clear existing data
        self.bids.clear();
        self.asks.clear();

        // Insert new levels
        for level in bids {
            if level.size > Decimal::ZERO {
                self.bids.insert(level.price, level.size);
            }
        }

        for level in asks {
            if level.size > Decimal::ZERO {
                self.asks.insert(level.price, level.size);
            }
        }

        debug!(
            asset_id = %self.asset_id,
            bids_count = self.bids.len(),
            asks_count = self.asks.len(),
            "Order book snapshot applied without hash validation"
        );
    }


    /// Apply a price change (add/update/remove level)
    pub fn apply_price_change(
        &mut self,
        side: Side,
        price: Decimal,
        size: Decimal,
        expected_hash: String,
    ) -> Result<(), StateError> {
        // CAPTURE FULL DIAGNOSTIC STATE BEFORE CHANGE
        let before_hash = self.compute_polymarket_hash();
        let before_bids: Vec<(Decimal, Decimal)> =
            self.bids.iter().map(|(&p, &s)| (p, s)).collect();
        let before_asks: Vec<(Decimal, Decimal)> =
            self.asks.iter().map(|(&p, &s)| (p, s)).collect();

        // Get the previous value at this price level (if any)
        let previous_value = match side {
            Side::Buy => self.bids.get(&price).cloned(),
            Side::Sell => self.asks.get(&price).cloned(),
        };

        // Apply the change
        match side {
            Side::Buy => {
                if size == Decimal::ZERO {
                    self.bids.remove(&price);
                } else {
                    self.bids.insert(price, size);
                }
            }
            Side::Sell => {
                if size == Decimal::ZERO {
                    self.asks.remove(&price);
                } else {
                    self.asks.insert(price, size);
                }
            }
        }

        // CAPTURE FULL DIAGNOSTIC STATE AFTER CHANGE
        let after_hash = self.compute_polymarket_hash();
        let after_bids: Vec<(Decimal, Decimal)> = self.bids.iter().map(|(&p, &s)| (p, s)).collect();
        let after_asks: Vec<(Decimal, Decimal)> = self.asks.iter().map(|(&p, &s)| (p, s)).collect();

        // Verify hash after applying change using Polymarket-compatible calculation
        if after_hash != expected_hash {
            // COMPREHENSIVE HASH MISMATCH DIAGNOSTICS
            error!("🚨 HASH MISMATCH DETECTED - STOPPING FOR FULL DIAGNOSTIC 🚨");
            error!("Asset ID: {}", self.asset_id);
            error!("═══════════════════════════════════════════════════════════════");

            // Event details
            error!("📋 UPDATE EVENT:");
            error!("  Side: {:?}", side);
            error!("  Price: {}", price);
            error!("  Size: {}", size);
            error!("  Previous value at price: {:?}", previous_value);
            error!(
                "  Operation: {}",
                if size == Decimal::ZERO {
                    "REMOVE"
                } else if previous_value.is_some() {
                    "UPDATE"
                } else {
                    "ADD"
                }
            );

            // Hash comparison
            error!("🔍 HASH COMPARISON:");
            error!("  Expected: {}", expected_hash);
            error!("  Computed: {}", after_hash);
            error!("  Before:   {}", before_hash);

            // Full orderbook state comparison
            error!("📊 ORDERBOOK STATE COMPARISON:");
            error!(
                "  BEFORE - Bids count: {}, Asks count: {}",
                before_bids.len(),
                before_asks.len()
            );
            error!(
                "  AFTER  - Bids count: {}, Asks count: {}",
                after_bids.len(),
                after_asks.len()
            );

            // Show actual orderbook data
            error!("📈 BIDS BEFORE:");
            for (i, (price, size)) in before_bids.iter().enumerate() {
                error!("  [{}] ${} → {}", i, price, size);
                if i >= 10 {
                    error!("  ... ({} more)", before_bids.len() - 10);
                    break;
                }
            }

            error!("📈 BIDS AFTER:");
            for (i, (price, size)) in after_bids.iter().enumerate() {
                error!("  [{}] ${} → {}", i, price, size);
                if i >= 10 {
                    error!("  ... ({} more)", after_bids.len() - 10);
                    break;
                }
            }

            error!("📉 ASKS BEFORE:");
            for (i, (price, size)) in before_asks.iter().enumerate() {
                error!("  [{}] ${} → {}", i, price, size);
                if i >= 10 {
                    error!("  ... ({} more)", before_asks.len() - 10);
                    break;
                }
            }

            error!("📉 ASKS AFTER:");
            for (i, (price, size)) in after_asks.iter().enumerate() {
                error!("  [{}] ${} → {}", i, price, size);
                if i >= 10 {
                    error!("  ... ({} more)", after_asks.len() - 10);
                    break;
                }
            }

            // Show the exact JSON being hashed
            let before_json = self.debug_hash_json(&before_bids, &before_asks);
            let after_json = self.debug_hash_json(&after_bids, &after_asks);

            error!("🔍 JSON BEING HASHED:");
            error!("  BEFORE: {}", before_json);
            error!("  AFTER:  {}", after_json);

            // Show differences
            error!("🔄 CHANGES DETECTED:");
            match side {
                Side::Buy => {
                    if let Some(prev) = previous_value {
                        if size == Decimal::ZERO {
                            error!("  BID REMOVED: ${} (was {})", price, prev);
                        } else {
                            error!("  BID UPDATED: ${} {} → {}", price, prev, size);
                        }
                    } else if size != Decimal::ZERO {
                        error!("  BID ADDED: ${} → {}", price, size);
                    }
                }
                Side::Sell => {
                    if let Some(prev) = previous_value {
                        if size == Decimal::ZERO {
                            error!("  ASK REMOVED: ${} (was {})", price, prev);
                        } else {
                            error!("  ASK UPDATED: ${} {} → {}", price, prev, size);
                        }
                    } else if size != Decimal::ZERO {
                        error!("  ASK ADDED: ${} → {}", price, size);
                    }
                }
            }

            error!("═══════════════════════════════════════════════════════════════");
            error!("🛑 STOPPING FURTHER PROCESSING DUE TO HASH MISMATCH");

            return Err(StateError::HashMismatch {
                expected: expected_hash,
                computed: after_hash,
            });
        }

        self.last_hash = Some(expected_hash);
        debug!(
            asset_id = %self.asset_id,
            side = ?side,
            price = %price,
            size = %size,
            "Price change applied successfully"
        );
        Ok(())
    }

    /// Apply a price change without hash validation
    pub fn apply_price_change_no_hash(&mut self, side: Side, price: Decimal, size: Decimal) {
        // Apply the change
        match side {
            Side::Buy => {
                if size == Decimal::ZERO {
                    self.bids.remove(&price);
                } else {
                    self.bids.insert(price, size);
                }
            }
            Side::Sell => {
                if size == Decimal::ZERO {
                    self.asks.remove(&price);
                } else {
                    self.asks.insert(price, size);
                }
            }
        }

        debug!(
            asset_id = %self.asset_id,
            side = ?side,
            price = %price,
            size = %size,
            "Price change applied without hash validation"
        );
    }

    /// Update tick size
    pub fn set_tick_size(&mut self, tick_size: Decimal) {
        self.tick_size = Some(tick_size);
        debug!(asset_id = %self.asset_id, tick_size = %tick_size, "Tick size updated");
    }

    /// Get all bids as a vector (highest to lowest)
    pub fn get_bids(&self) -> Vec<PriceLevel> {
        self.bids
            .iter()
            .rev()
            .map(|(&price, &size)| PriceLevel::new(price, size))
            .collect()
    }

    /// Get all asks as a vector (lowest to highest)
    pub fn get_asks(&self) -> Vec<PriceLevel> {
        self.asks
            .iter()
            .map(|(&price, &size)| PriceLevel::new(price, size))
            .collect()
    }

    /// Get best bid (highest bid price)
    pub fn best_bid(&self) -> Option<PriceLevel> {
        self.bids
            .iter()
            .next_back()
            .map(|(&price, &size)| PriceLevel::new(price, size))
    }

    /// Get best ask (lowest ask price)
    pub fn best_ask(&self) -> Option<PriceLevel> {
        self.asks
            .iter()
            .next()
            .map(|(&price, &size)| PriceLevel::new(price, size))
    }

    /// Compute Polymarket-compatible SHA-1 hash of current order book state
    pub fn compute_polymarket_hash(&self) -> String {
        // Convert BTreeMaps to OrderSummary vectors
        let bids: Vec<OrderSummary> = self
            .bids
            .iter()
            .map(|(&price, &size)| OrderSummary::new(price, size))
            .collect();

        let asks: Vec<OrderSummary> = self
            .asks
            .iter()
            .map(|(&price, &size)| OrderSummary::new(price, size))
            .collect();

        // Create Polymarket-compatible order book and compute hash using the exact algorithm
        let polymarket_book = PolymarketOrderBook::new(
            self.market.clone(),
            self.asset_id.clone(),
            self.timestamp,
            bids,
            asks,
        );
        polymarket_book.hash()
    }

    /// Debug helper to show exact JSON being hashed
    fn debug_hash_json(&self, bids: &[(Decimal, Decimal)], asks: &[(Decimal, Decimal)]) -> String {
        let bids_summary: Vec<OrderSummary> = bids
            .iter()
            .map(|(price, size)| OrderSummary::new(*price, *size))
            .collect();

        let asks_summary: Vec<OrderSummary> = asks
            .iter()
            .map(|(price, size)| OrderSummary::new(*price, *size))
            .collect();

        let polymarket_book = PolymarketOrderBook::new(
            self.market.clone(),
            self.asset_id.clone(),
            self.timestamp,
            bids_summary,
            asks_summary,
        );

        // Show the sorted JSON that gets hashed
        let mut bids = polymarket_book.bids.clone();
        bids.sort_by(|a, b| {
            b.price_num()
                .partial_cmp(&a.price_num())
                .unwrap_or(Ordering::Equal)
        });
        let mut asks = polymarket_book.asks.clone();
        asks.sort_by(|a, b| {
            a.price_num()
                .partial_cmp(&b.price_num())
                .unwrap_or(Ordering::Equal)
        });

        let canonical = PolymarketOrderBook { 
            market: self.market.clone(),
            asset_id: self.asset_id.clone(),
            bids, 
            asks,
            timestamp: self.timestamp,
            hash: String::new(),
        };
        serde_json::to_string(&canonical).unwrap_or_else(|_| "ERROR".to_string())
    }

    /// Get order book summary for logging
    pub fn summary(&self) -> String {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => {
                format!(
                    "{}: bid ${} ({}), ask ${} ({}), spread ${}",
                    self.asset_id,
                    bid.price,
                    bid.size,
                    ask.price,
                    ask.size,
                    ask.price - bid.price
                )
            }
            (Some(bid), None) => {
                format!(
                    "{}: bid ${} ({}), no asks",
                    self.asset_id, bid.price, bid.size
                )
            }
            (None, Some(ask)) => {
                format!(
                    "{}: ask ${} ({}), no bids",
                    self.asset_id, ask.price, ask.size
                )
            }
            (None, None) => {
                format!("{}: empty order book", self.asset_id)
            }
        }
    }

    /// Validate and clean the orderbook to ensure no crossed markets
    pub fn validate_and_clean(&mut self) -> bool {
        // Get best bid and ask
        let best_bid = self.bids.keys().rev().next().cloned();
        let best_ask = self.asks.keys().next().cloned();

        if let (Some(bid), Some(ask)) = (best_bid, best_ask) {
            if bid >= ask {
                // Crossed market detected - remove invalid levels
                warn!(
                    asset_id = %self.asset_id,
                    best_bid = %bid,
                    best_ask = %ask,
                    "Crossed market detected, cleaning orderbook"
                );

                // Remove all bids >= best ask
                let bids_to_remove: Vec<Decimal> = self
                    .bids
                    .keys()
                    .filter(|&&price| price >= ask)
                    .cloned()
                    .collect();

                for price in bids_to_remove {
                    self.bids.remove(&price);
                }

                // Remove all asks <= best bid after the removals above
                let new_best_bid = self.bids.keys().rev().next().cloned();
                if let Some(new_bid) = new_best_bid {
                    let asks_to_remove: Vec<Decimal> = self
                        .asks
                        .keys()
                        .filter(|&&price| price <= new_bid)
                        .cloned()
                        .collect();

                    for price in asks_to_remove {
                        self.asks.remove(&price);
                    }
                }

                return true; // Orderbook was cleaned
            }
        }

        false // Orderbook was valid
    }
}
