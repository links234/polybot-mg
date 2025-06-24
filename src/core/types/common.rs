//! Common type definitions used across the polybot system

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Trading side
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Hash)]
pub enum Side {
    Buy,
    Sell,
}

impl<'de> serde::Deserialize<'de> for Side {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "buy" => Ok(Side::Buy),
            "sell" => Ok(Side::Sell),
            _ => Err(serde::de::Error::unknown_variant(&s, &["buy", "sell"])),
        }
    }
}

impl std::fmt::Display for Side {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Side::Buy => write!(f, "Buy"),
            Side::Sell => write!(f, "Sell"),
        }
    }
}

/// Order status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "UPPERCASE")]
pub enum OrderStatus {
    Open,
    Filled,
    Cancelled,
    PartiallyFilled,
}

impl std::fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderStatus::Open => write!(f, "Open"),
            OrderStatus::Filled => write!(f, "Filled"),
            OrderStatus::Cancelled => write!(f, "Cancelled"),
            OrderStatus::PartiallyFilled => write!(f, "PartiallyFilled"),
        }
    }
}

/// Market status for tracking market state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketStatus {
    Active,
    Closed,
    Resolved,
    Paused,
}

/// Common result type for market operations
pub type MarketResult<T> = Result<T, MarketError>;

/// Common error type for market operations
#[derive(Debug, thiserror::Error)]
pub enum MarketError {
    #[error("Invalid market data: {0}")]
    InvalidData(String),
    
    #[error("Market not found: {0}")]
    NotFound(String),
    
    #[error("WebSocket error: {0}")]
    WebSocket(String),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Authentication error: {0}")]
    Auth(String),
    
    #[error("Rate limit exceeded")]
    RateLimit,
    
    #[error("Unknown error: {0}")]
    Unknown(String),
}

/// Time interval for data aggregation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeInterval {
    Second(u32),
    Minute(u32),
    Hour(u32),
    Day(u32),
}

/// Price and size representation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceSize {
    pub price: Decimal,
    pub size: Decimal,
}

impl PriceSize {
    pub fn new(price: Decimal, size: Decimal) -> Self {
        Self { price, size }
    }
}

/// Timestamp wrapper for consistent time handling
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Timestamp(pub u64);

impl Timestamp {
    pub fn now() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");
        Self(duration.as_millis() as u64)
    }
    
    pub fn as_millis(&self) -> u64 {
        self.0
    }
    
    pub fn as_secs(&self) -> u64 {
        self.0 / 1000
    }
}

impl From<u64> for Timestamp {
    fn from(millis: u64) -> Self {
        Self(millis)
    }
}

impl std::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}