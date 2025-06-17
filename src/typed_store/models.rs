use serde::{Serialize, Deserialize};
use crate::typed_store::codec::{RocksDbValue, CodecError};
use crate::typed_store::table::{Table, TypedCf};
use crate::define_typed_cf;
use crate::markets::fetcher::{Market as FetchedMarket, MarketToken as FetchedMarketToken};
use std::collections::HashMap;

/// RocksDB-optimized market structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RocksDbMarket {
    pub id: Option<String>,
    pub condition_id: Option<String>,
    pub question: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub tags: Option<Vec<String>>,
    pub tokens: Vec<RocksDbMarketToken>,
    pub active: bool,
    pub closed: bool,
    pub archived: Option<bool>,
    pub accepting_orders: bool,
    pub minimum_order_size: Option<f64>,
    pub minimum_tick_size: Option<f64>,
    pub end_date_iso: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub volume: Option<f64>,
    pub volume_24hr: Option<f64>,
    pub liquidity: Option<f64>,
    pub outcomes: Option<Vec<String>>,
    pub outcome_prices: Option<Vec<f64>>,
    pub market_slug: Option<String>,
    pub creator: Option<String>,
    pub fee_rate: Option<f64>,
    pub additional_fields: HashMap<String, serde_json::Value>,
}

/// RocksDB-optimized market token structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RocksDbMarketToken {
    pub token_id: String,
    pub outcome: String,
    pub price: f64,
    pub winner: Option<bool>,
    pub volume: Option<f64>,
    pub volume_24hr: Option<f64>,
    pub supply: Option<f64>,
    pub market_cap: Option<f64>,
    pub additional_fields: HashMap<String, serde_json::Value>,
}

/// Condition metadata (extracted from markets)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub id: String,
    pub question: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub tags: Option<Vec<String>>,
    pub outcomes: Option<Vec<String>>,
    pub creator: Option<String>,
    pub created_at: Option<String>,
    pub market_count: usize, // Number of markets with this condition
}

/// Token metadata (extracted from market tokens)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub id: String,
    pub outcome: String,
    pub condition_id: Option<String>,
    pub market_id: Option<String>,
    pub current_price: f64,
    pub volume: Option<f64>,
    pub volume_24hr: Option<f64>,
    pub supply: Option<f64>,
    pub market_cap: Option<f64>,
    pub winner: Option<bool>,
    pub last_updated: Option<String>,
}

/// Market search index entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketIndex {
    pub market_id: String,
    pub condition_id: String,
    pub question_lower: String, // Lowercase for search
    pub category_lower: Option<String>, // Lowercase for search
    pub tags_lower: Option<Vec<String>>, // Lowercase for search
    pub active: bool,
    pub closed: bool,
    pub volume: Option<f64>,
    pub volume_24hr: Option<f64>,
    pub created_at: Option<String>,
}

// Conversion implementations

impl From<FetchedMarket> for RocksDbMarket {
    fn from(market: FetchedMarket) -> Self {
        Self {
            id: market.id,
            condition_id: market.condition_id,
            question: market.question,
            description: market.description,
            category: market.category,
            tags: market.tags,
            tokens: market.tokens.into_iter().map(RocksDbMarketToken::from).collect(),
            active: market.active,
            closed: market.closed,
            archived: market.archived,
            accepting_orders: market.accepting_orders,
            minimum_order_size: market.minimum_order_size,
            minimum_tick_size: market.minimum_tick_size,
            end_date_iso: market.end_date_iso,
            created_at: market.created_at,
            updated_at: market.updated_at,
            volume: market.volume,
            volume_24hr: market.volume_24hr,
            liquidity: market.liquidity,
            outcomes: market.outcomes,
            outcome_prices: market.outcome_prices,
            market_slug: market.market_slug,
            creator: market.creator,
            fee_rate: market.fee_rate,
            additional_fields: market.additional_fields,
        }
    }
}

impl From<FetchedMarketToken> for RocksDbMarketToken {
    fn from(token: FetchedMarketToken) -> Self {
        Self {
            token_id: token.token_id,
            outcome: token.outcome,
            price: token.price,
            winner: token.winner,
            volume: token.volume,
            volume_24hr: token.volume_24hr,
            supply: token.supply,
            market_cap: token.market_cap,
            additional_fields: token.additional_fields,
        }
    }
}

impl RocksDbMarket {
    /// Extract condition data from market
    pub fn extract_condition(&self) -> Option<Condition> {
        let condition_id = self.condition_id.as_ref()?.clone();
        
        Some(Condition {
            id: condition_id,
            question: self.question.clone(),
            description: self.description.clone(),
            category: self.category.clone(),
            tags: self.tags.clone(),
            outcomes: self.outcomes.clone(),
            creator: self.creator.clone(),
            created_at: self.created_at.clone(),
            market_count: 1, // Will be aggregated when indexing
        })
    }

    /// Extract tokens for separate indexing
    pub fn extract_tokens(&self) -> Vec<Token> {
        self.tokens.iter().map(|token| Token {
            id: token.token_id.clone(),
            outcome: token.outcome.clone(),
            condition_id: self.condition_id.clone(),
            market_id: self.id.clone(),
            current_price: token.price,
            volume: token.volume,
            volume_24hr: token.volume_24hr,
            supply: token.supply,
            market_cap: token.market_cap,
            winner: token.winner,
            last_updated: self.updated_at.clone(),
        }).collect()
    }

    /// Create search index entry
    pub fn create_index(&self) -> Option<MarketIndex> {
        let market_id = self.id.as_ref()?.clone();
        let condition_id = self.condition_id.as_ref()?.clone();

        Some(MarketIndex {
            market_id,
            condition_id,
            question_lower: self.question.to_lowercase(),
            category_lower: self.category.as_ref().map(|c| c.to_lowercase()),
            tags_lower: self.tags.as_ref().map(|tags| {
                tags.iter().map(|t| t.to_lowercase()).collect()
            }),
            active: self.active,
            closed: self.closed,
            volume: self.volume,
            volume_24hr: self.volume_24hr,
            created_at: self.created_at.clone(),
        })
    }
}

// RocksDbValue implementations

impl RocksDbValue for RocksDbMarket {
    fn encode_value(&self) -> Result<Vec<u8>, CodecError> {
        serde_json::to_vec(self).map_err(|e| CodecError::SerializationError(e.to_string()))
    }
    
    fn decode_value(data: &[u8]) -> Result<Self, CodecError> {
        serde_json::from_slice(data).map_err(|e| CodecError::DeserializationError(e.to_string()))
    }
}

impl RocksDbValue for Condition {
    fn encode_value(&self) -> Result<Vec<u8>, CodecError> {
        serde_json::to_vec(self).map_err(|e| CodecError::SerializationError(e.to_string()))
    }
    
    fn decode_value(data: &[u8]) -> Result<Self, CodecError> {
        serde_json::from_slice(data).map_err(|e| CodecError::DeserializationError(e.to_string()))
    }
}

impl RocksDbValue for Token {
    fn encode_value(&self) -> Result<Vec<u8>, CodecError> {
        serde_json::to_vec(self).map_err(|e| CodecError::SerializationError(e.to_string()))
    }
    
    fn decode_value(data: &[u8]) -> Result<Self, CodecError> {
        serde_json::from_slice(data).map_err(|e| CodecError::DeserializationError(e.to_string()))
    }
}

impl RocksDbValue for MarketIndex {
    fn encode_value(&self) -> Result<Vec<u8>, CodecError> {
        serde_json::to_vec(self).map_err(|e| CodecError::SerializationError(e.to_string()))
    }
    
    fn decode_value(data: &[u8]) -> Result<Self, CodecError> {
        serde_json::from_slice(data).map_err(|e| CodecError::DeserializationError(e.to_string()))
    }
}

// Table definitions with unique prefixes

pub struct MarketTable;
impl Table for MarketTable {
    type Key = String; // market_id
    type Value = RocksDbMarket;
    const PREFIX: u8 = 0x01;
}

pub struct MarketByConditionTable;
impl Table for MarketByConditionTable {
    type Key = String; // condition_id  
    type Value = RocksDbMarket;
    const PREFIX: u8 = 0x02;
}

pub struct ConditionTable;
impl Table for ConditionTable {
    type Key = String; // condition_id
    type Value = Condition;
    const PREFIX: u8 = 0x03;
}

pub struct TokenTable;
impl Table for TokenTable {
    type Key = String; // token_id
    type Value = Token;
    const PREFIX: u8 = 0x04;
}

pub struct TokensByConditionTable;
impl Table for TokensByConditionTable {
    type Key = String; // condition_id
    type Value = Vec<Token>; // All tokens for a condition
    const PREFIX: u8 = 0x05;
}

pub struct MarketIndexTable;
impl Table for MarketIndexTable {
    type Key = String; // market_id
    type Value = MarketIndex;
    const PREFIX: u8 = 0x06;
}

// Implement Vec<Token> serialization
impl RocksDbValue for Vec<Token> {
    fn encode_value(&self) -> Result<Vec<u8>, CodecError> {
        serde_json::to_vec(self).map_err(|e| CodecError::SerializationError(e.to_string()))
    }
    
    fn decode_value(data: &[u8]) -> Result<Self, CodecError> {
        serde_json::from_slice(data).map_err(|e| CodecError::DeserializationError(e.to_string()))
    }
}

// Implement Vec<String> serialization for ConditionIndexCf
impl RocksDbValue for Vec<String> {
    fn encode_value(&self) -> Result<Vec<u8>, CodecError> {
        serde_json::to_vec(self).map_err(|e| CodecError::SerializationError(e.to_string()))
    }
    
    fn decode_value(data: &[u8]) -> Result<Self, CodecError> {
        serde_json::from_slice(data).map_err(|e| CodecError::DeserializationError(e.to_string()))
    }
}

// TypedCf implementations for typed RocksDB operations
// These provide column family definitions for the new TypedDbContext

define_typed_cf!(MarketCf, String, RocksDbMarket, "markets", 0x01);
define_typed_cf!(MarketByConditionCf, String, RocksDbMarket, "markets_by_condition", 0x02);
define_typed_cf!(ConditionCf, String, Condition, "conditions", 0x03);
define_typed_cf!(TokenCf, String, Token, "tokens", 0x04);
define_typed_cf!(TokensByConditionCf, String, Vec<Token>, "tokens_by_condition", 0x05);
define_typed_cf!(MarketIndexCf, String, MarketIndex, "market_index", 0x06);

// Separate indices for fast lookups
define_typed_cf!(TokenIndexCf, String, String, "token_index", 0x07); // token_id -> condition_id
define_typed_cf!(ConditionIndexCf, String, Vec<String>, "condition_index", 0x08); // condition_id -> [token_ids]

/// All column family names for database initialization
pub const ALL_COLUMN_FAMILIES: &[&str] = &[
    MarketCf::NAME,
    MarketByConditionCf::NAME,
    ConditionCf::NAME,
    TokenCf::NAME,
    TokensByConditionCf::NAME,
    MarketIndexCf::NAME,
    TokenIndexCf::NAME,
    ConditionIndexCf::NAME,
];