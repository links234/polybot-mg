//! Order book state management with hash verification

use crate::ws::events::Side;
use crate::execution::orderbook::PriceLevel;
use blake3::Hasher;
use rust_decimal::Decimal;
use std::collections::BTreeMap;
use thiserror::Error;
use tracing::{debug, warn};

#[derive(Error, Debug)]
pub enum StateError {
    #[error("Hash verification failed: expected {expected}, got {computed}")]
    HashMismatch { expected: String, computed: String },
}

/// Level-2 order book with hash verification
#[derive(Debug, Clone)]
pub struct OrderBook {
    /// Asset ID this order book represents
    pub asset_id: String,
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
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            last_hash: None,
            tick_size: None,
        }
    }

    /// Replace order book with new snapshot
    pub fn replace_with_snapshot(
        &mut self,
        bids: Vec<PriceLevel>,
        asks: Vec<PriceLevel>,
        hash: String,
    ) -> Result<(), StateError> {
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

        // Verify hash
        let computed_hash = self.compute_hash();
        if computed_hash != hash {
            warn!(
                asset_id = %self.asset_id,
                expected = %hash,
                computed = %computed_hash,
                "Hash mismatch on snapshot"
            );
            return Err(StateError::HashMismatch {
                expected: hash,
                computed: computed_hash,
            });
        }

        self.last_hash = Some(hash);
        debug!(asset_id = %self.asset_id, "Order book snapshot applied");
        Ok(())
    }

    /// Replace order book with new snapshot without hash validation
    pub fn replace_with_snapshot_no_hash(
        &mut self,
        bids: Vec<PriceLevel>,
        asks: Vec<PriceLevel>,
    ) {
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

        debug!(asset_id = %self.asset_id, "Order book snapshot applied without hash validation");
    }

    /// Apply a price change (add/update/remove level)
    pub fn apply_price_change(
        &mut self,
        side: Side,
        price: Decimal,
        size: Decimal,
        expected_hash: String,
    ) -> Result<(), StateError> {
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

        // Verify hash after applying change
        let computed_hash = self.compute_hash();
        if computed_hash != expected_hash {
            warn!(
                asset_id = %self.asset_id,
                side = ?side,
                price = %price,
                size = %size,
                expected = %expected_hash,
                computed = %computed_hash,
                "Hash mismatch on price change"
            );
            return Err(StateError::HashMismatch {
                expected: expected_hash,
                computed: computed_hash,
            });
        }

        self.last_hash = Some(expected_hash);
        debug!(
            asset_id = %self.asset_id,
            side = ?side,
            price = %price,
            size = %size,
            "Price change applied"
        );
        Ok(())
    }

    /// Apply a price change without hash validation
    pub fn apply_price_change_no_hash(
        &mut self,
        side: Side,
        price: Decimal,
        size: Decimal,
    ) {
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
        self.bids.iter().rev().map(|(&price, &size)| PriceLevel::new(price, size)).collect()
    }
    
    /// Get all asks as a vector (lowest to highest)
    pub fn get_asks(&self) -> Vec<PriceLevel> {
        self.asks.iter().map(|(&price, &size)| PriceLevel::new(price, size)).collect()
    }

    /// Get best bid (highest bid price)
    pub fn best_bid(&self) -> Option<PriceLevel> {
        self.bids.iter().next_back().map(|(&price, &size)| PriceLevel::new(price, size))
    }

    /// Get best ask (lowest ask price)
    pub fn best_ask(&self) -> Option<PriceLevel> {
        self.asks.iter().next().map(|(&price, &size)| PriceLevel::new(price, size))
    }







    /// Compute Blake3 hash of current order book state
    pub fn compute_hash(&self) -> String {
        let mut hasher = Hasher::new();
        
        // Hash asset ID
        hasher.update(self.asset_id.as_bytes());
        
        // Hash bids (in price-descending order)
        for (&price, &size) in self.bids.iter().rev() {
            hasher.update(b"bid");
            hasher.update(price.to_string().as_bytes());
            hasher.update(size.to_string().as_bytes());
        }
        
        // Hash asks (in price-ascending order)
        for (&price, &size) in self.asks.iter() {
            hasher.update(b"ask");
            hasher.update(price.to_string().as_bytes());
            hasher.update(size.to_string().as_bytes());
        }
        
        // Return hex-encoded hash
        hasher.finalize().to_hex().to_string()
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
                format!("{}: bid ${} ({}), no asks", self.asset_id, bid.price, bid.size)
            }
            (None, Some(ask)) => {
                format!("{}: ask ${} ({}), no bids", self.asset_id, ask.price, ask.size)
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
                let bids_to_remove: Vec<Decimal> = self.bids
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
                    let asks_to_remove: Vec<Decimal> = self.asks
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

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_order_book_creation() {
        let book = OrderBook::new("test_asset".to_string());
        assert_eq!(book.asset_id, "test_asset");
        assert!(book.is_empty());
        assert!(book.best_bid().is_none());
        assert!(book.best_ask().is_none());
    }

    #[test]
    fn test_snapshot_application() {
        let mut book = OrderBook::new("test_asset".to_string());
        
        let bids = vec![(dec!(0.95), dec!(100)), (dec!(0.94), dec!(200))];
        let asks = vec![(dec!(0.96), dec!(150)), (dec!(0.97), dec!(250))];
        
        // Compute expected hash
        book.bids.insert(dec!(0.95), dec!(100));
        book.bids.insert(dec!(0.94), dec!(200));
        book.asks.insert(dec!(0.96), dec!(150));
        book.asks.insert(dec!(0.97), dec!(250));
        let expected_hash = book.compute_hash();
        
        // Clear and apply snapshot
        book.bids.clear();
        book.asks.clear();
        book.replace_with_snapshot(bids, asks, expected_hash).unwrap();
        
        assert_eq!(book.best_bid(), Some((dec!(0.95), dec!(100))));
        assert_eq!(book.best_ask(), Some((dec!(0.96), dec!(150))));
        assert_eq!(book.spread(), Some(dec!(0.01)));
    }

    #[test]
    fn test_price_change_application() {
        let mut book = OrderBook::new("test_asset".to_string());
        
        // Set initial state
        book.bids.insert(dec!(0.95), dec!(100));
        book.asks.insert(dec!(0.96), dec!(150));
        
        // Apply bid update
        book.bids.insert(dec!(0.95), dec!(200)); // Update existing
        let expected_hash = book.compute_hash();
        book.bids.insert(dec!(0.95), dec!(100)); // Revert for test
        
        book.apply_price_change(Side::Buy, dec!(0.95), dec!(200), expected_hash).unwrap();
        assert_eq!(book.best_bid(), Some((dec!(0.95), dec!(200))));
        
        // Apply bid removal
        book.bids.remove(&dec!(0.95)); // Remove for hash calculation
        let expected_hash = book.compute_hash();
        book.bids.insert(dec!(0.95), dec!(200)); // Revert for test
        
        book.apply_price_change(Side::Buy, dec!(0.95), dec!(0), expected_hash).unwrap();
        assert!(book.best_bid().is_none());
    }

    #[test]
    fn test_hash_verification() {
        let mut book = OrderBook::new("test_asset".to_string());
        
        let bids = vec![(dec!(0.95), dec!(100))];
        let asks = vec![(dec!(0.96), dec!(150))];
        
        // Try with wrong hash
        let result = book.replace_with_snapshot(bids, asks, "wrong_hash".to_string());
        assert!(matches!(result, Err(StateError::HashMismatch { .. })));
    }
} 