//! Orderbook types and structures for execution management
//! 
//! This module contains strongly-typed structures for orderbook representation
//! and manipulation, ensuring CLAUDE.md compliance with "no tuples in public APIs".

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

/// Represents a price level in an order book with price and size
/// Replaces all (Decimal, Decimal) tuple usage for price/size pairs
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PriceLevel {
    pub price: Decimal,
    pub size: Decimal,
}

impl PriceLevel {
    /// Creates a new price level
    pub fn new(price: Decimal, size: Decimal) -> Self {
        Self { price, size }
    }

    /// Calculates the total value (price * size)
    pub fn total_value(&self) -> Decimal {
        self.price * self.size
    }

    /// Checks if this level has any size
    pub fn has_size(&self) -> bool {
        self.size > Decimal::ZERO
    }

    /// Checks if this is a valid price level
    pub fn is_valid(&self) -> bool {
        let valid = self.price > Decimal::ZERO && self.size >= Decimal::ZERO;
        if !valid {
            warn!(
                price = %self.price, 
                size = %self.size, 
                "Invalid price level detected"
            );
        }
        valid
    }
    
    /// Creates a validated price level, returning None if invalid
    pub fn try_new(price: Decimal, size: Decimal) -> Option<Self> {
        let level = Self::new(price, size);
        if level.is_valid() {
            debug!(price = %price, size = %size, "Created valid price level");
            Some(level)
        } else {
            warn!(price = %price, size = %size, "Rejected invalid price level");
            None
        }
    }
}

/// Market depth information showing bid and ask level counts
/// Replaces (usize, usize) tuples for depth information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketDepth {
    pub bid_levels: usize,
    pub ask_levels: usize,
}

impl MarketDepth {
    /// Creates new market depth information
    pub fn new(bid_levels: usize, ask_levels: usize) -> Self {
        Self { bid_levels, ask_levels }
    }

    /// Gets total number of levels
    pub fn total_levels(&self) -> usize {
        self.bid_levels + self.ask_levels
    }

    /// Checks if market has any liquidity
    pub fn has_liquidity(&self) -> bool {
        self.bid_levels > 0 || self.ask_levels > 0
    }
}

/// Asset and order book pair for multi-asset operations
/// Replaces (String, OrderBook) tuples
#[derive(Debug, Clone)]
pub struct AssetOrderBook {
    pub asset_id: String,
    pub order_book: crate::ws::OrderBook,
}

impl AssetOrderBook {
    /// Creates new asset order book pair
    pub fn new(asset_id: String, order_book: crate::ws::OrderBook) -> Self {
        Self { asset_id, order_book }
    }
}

/// Spread information with description and mid price
/// Replaces (String, Option<Decimal>) tuples for spread calculations
#[derive(Debug, Clone)]
pub struct SpreadInfo {
    pub description: String,
    pub mid_price: Option<Decimal>,
}

impl SpreadInfo {
    /// Creates new spread information
    pub fn new(description: String, mid_price: Option<Decimal>) -> Self {
        Self { description, mid_price }
    }

    /// Creates spread info for crossed market
    pub fn crossed_market(bid: Decimal, ask: Decimal) -> Self {
        warn!(
            bid = %bid, 
            ask = %ask, 
            spread = %(ask - bid),
            "Crossed market detected - bid >= ask"
        );
        Self {
            description: format!("⚠️ CROSSED MARKET: Bid ${:.4} >= Ask ${:.4}", bid, ask),
            mid_price: None,
        }
    }

    /// Creates spread info for normal market
    pub fn normal_market(bid: Decimal, ask: Decimal, spread_pct: f64) -> Self {
        let mid_price = (bid + ask) / Decimal::from(2);
        let spread = ask - bid;
        
        debug!(
            bid = %bid,
            ask = %ask,
            spread = %spread,
            spread_pct = spread_pct,
            mid_price = %mid_price,
            "Normal market spread calculated"
        );
        
        Self {
            description: format!("Spread: ${:.4} ({:.2}%) | Mid: ${:.4}", spread, spread_pct, mid_price),
            mid_price: Some(mid_price),
        }
    }
}

// Conversion implementations for backward compatibility during migration

impl From<(Decimal, Decimal)> for PriceLevel {
    fn from((price, size): (Decimal, Decimal)) -> Self {
        Self::new(price, size)
    }
}

impl From<PriceLevel> for (Decimal, Decimal) {
    fn from(level: PriceLevel) -> Self {
        (level.price, level.size)
    }
}

impl From<(usize, usize)> for MarketDepth {
    fn from((bid_levels, ask_levels): (usize, usize)) -> Self {
        Self::new(bid_levels, ask_levels)
    }
}

impl From<MarketDepth> for (usize, usize) {
    fn from(depth: MarketDepth) -> Self {
        (depth.bid_levels, depth.ask_levels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_price_level_creation() {
        let level = PriceLevel::new(dec!(0.5), dec!(100.0));
        assert_eq!(level.price, dec!(0.5));
        assert_eq!(level.size, dec!(100.0));
        assert_eq!(level.total_value(), dec!(50.0));
        assert!(level.has_size());
        assert!(level.is_valid());
    }

    #[test]
    fn test_market_depth() {
        let depth = MarketDepth::new(5, 3);
        assert_eq!(depth.bid_levels, 5);
        assert_eq!(depth.ask_levels, 3);
        assert_eq!(depth.total_levels(), 8);
        assert!(depth.has_liquidity());
    }

    #[test]
    fn test_spread_info() {
        let spread = SpreadInfo::normal_market(dec!(0.4), dec!(0.6), 50.0);
        assert!(spread.description.contains("Spread"));
        assert!(spread.mid_price.is_some());
        assert_eq!(spread.mid_price.unwrap(), dec!(0.5));
    }

    #[test]
    fn test_tuple_conversions() {
        let tuple = (dec!(0.5), dec!(100.0));
        let level: PriceLevel = tuple.into();
        let back_to_tuple: (Decimal, Decimal) = level.into();
        assert_eq!(tuple, back_to_tuple);
    }
}