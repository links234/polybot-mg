//! Market-related type definitions

use serde::{Deserialize, Serialize};
use std::fmt;

/// Strongly typed asset identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AssetId(String);

impl AssetId {
    // new and as_str methods removed as unused
}

impl From<String> for AssetId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for AssetId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Strongly typed market identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MarketId(String);

impl MarketId {
    // new and as_str methods removed as unused
}

impl From<String> for MarketId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for MarketId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl fmt::Display for MarketId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Strongly typed token identifier (alias for AssetId)
pub type TokenId = AssetId;

/// Market information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketInfo {
    pub market_id: MarketId,
    pub question: String,
    pub outcome_tokens: Vec<TokenInfo>,
    pub status: super::common::MarketStatus,
    pub created_at: super::common::Timestamp,
    pub resolved_at: Option<super::common::Timestamp>,
}

/// Token information within a market
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub token_id: TokenId,
    pub outcome: String,
    pub outcome_index: usize,
}

/// Market resolution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketResolution {
    pub market_id: MarketId,
    pub winning_outcome: Option<usize>,
    pub resolved_at: super::common::Timestamp,
    pub resolution_source: String,
}

/// Represents a price level in an order book with price and size
/// Replaces all (Decimal, Decimal) tuple usage for price/size pairs
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PriceLevel {
    pub price: rust_decimal::Decimal,
    pub size: rust_decimal::Decimal,
}

impl PriceLevel {
    /// Creates a new price level
    pub fn new(price: rust_decimal::Decimal, size: rust_decimal::Decimal) -> Self {
        Self { price, size }
    }

    /// Calculates the total value (price * size)
    pub fn total_value(&self) -> rust_decimal::Decimal {
        self.price * self.size
    }

    /// Checks if this level has any size
    pub fn has_size(&self) -> bool {
        self.size > rust_decimal::Decimal::ZERO
    }

    /// Checks if this is a valid price level
    pub fn is_valid(&self) -> bool {
        let valid = self.price > rust_decimal::Decimal::ZERO && self.size >= rust_decimal::Decimal::ZERO;
        if !valid {
            tracing::warn!(
                price = %self.price,
                size = %self.size,
                "Invalid price level detected"
            );
        }
        valid
    }

    /// Creates a validated price level, returning None if invalid
    pub fn try_new(price: rust_decimal::Decimal, size: rust_decimal::Decimal) -> Option<Self> {
        let level = Self::new(price, size);
        if level.is_valid() {
            Some(level)
        } else {
            None
        }
    }
}