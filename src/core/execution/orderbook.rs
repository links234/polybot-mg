//! Orderbook types and structures for execution management
//!
//! This module contains strongly-typed structures for orderbook representation
//! and manipulation, ensuring CLAUDE.md compliance with "no tuples in public APIs".

use rust_decimal::Decimal;
use crate::core::types::market::PriceLevel;

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
        Self {
            bid_levels,
            ask_levels,
        }
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
    pub order_book: crate::core::ws::OrderBook,
}

impl AssetOrderBook {
    /// Creates new asset order book pair
    pub fn new(asset_id: String, order_book: crate::core::ws::OrderBook) -> Self {
        Self {
            asset_id,
            order_book,
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
    fn test_tuple_conversions() {
        let tuple = (dec!(0.5), dec!(100.0));
        let level: PriceLevel = tuple.into();
        let back_to_tuple: (Decimal, Decimal) = level.into();
        assert_eq!(tuple, back_to_tuple);
    }
}
