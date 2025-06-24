//! Type definitions for Gamma API responses

#[allow(dead_code)]

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Gamma API error types
#[allow(dead_code)]
#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum GammaError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),
    
    #[error("API error: {0}")]
    ApiError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Address not found: {0}")]
    AddressNotFound(String),
}

/// Position data from Gamma API
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GammaPosition {
    /// Market ID
    pub market: String,
    
    /// Market slug
    pub market_slug: Option<String>,
    
    /// Asset ID (token ID)
    pub asset_id: String,
    
    /// Market question
    pub question: String,
    
    /// Outcome name
    pub outcome: String,
    
    /// Position size
    pub size: Decimal,
    
    /// Realized profit/loss
    pub realized_pnl: Decimal,
    
    /// Unrealized profit/loss  
    pub unrealized_pnl: Decimal,
    
    /// Average entry price
    pub average_price: Decimal,
    
    /// Current market price
    pub current_price: Option<Decimal>,
    
    /// Position value
    pub value: Decimal,
    
    /// Last trade timestamp
    pub last_trade_time: Option<DateTime<Utc>>,
    
    /// Position state
    pub state: PositionState,
    
    /// Associated market metadata
    pub market_info: Option<MarketInfo>,
}

/// Activity/trade data from Gamma API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GammaActivity {
    /// Activity ID
    pub id: String,
    
    /// Market ID
    pub market: String,
    
    /// Asset ID (token ID)
    pub asset_id: String,
    
    /// Activity type
    pub activity_type: ActivityType,
    
    /// Transaction side (buy/sell)
    pub side: String,
    
    /// Trade size
    pub size: Decimal,
    
    /// Trade price
    pub price: Decimal,
    
    /// Transaction fee
    pub fee: Option<Decimal>,
    
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    
    /// Transaction hash
    pub tx_hash: Option<String>,
    
    /// Block number
    pub block_number: Option<u64>,
    
    /// Market question
    pub question: Option<String>,
    
    /// Outcome name
    pub outcome: Option<String>,
    
    /// Market slug
    pub market_slug: Option<String>,
}

/// Holder information from Gamma API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GammaHolder {
    /// User address
    pub address: String,
    
    /// Market ID
    pub market: String,
    
    /// Asset ID (token ID)
    pub asset_id: String,
    
    /// Holdings amount
    pub amount: Decimal,
    
    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
    
    /// Market information
    pub market_info: Option<MarketInfo>,
}

/// Position state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionState {
    Open,
    Closed,
    Expired,
}

/// Activity type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivityType {
    Trade,
    Mint,
    Burn,
    Transfer,
    Claim,
}

/// Market information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketInfo {
    /// Market ID
    pub id: String,
    
    /// Market slug
    pub slug: String,
    
    /// Market question
    pub question: String,
    
    /// Market description
    pub description: Option<String>,
    
    /// Market end date
    pub end_date: Option<DateTime<Utc>>,
    
    /// Market creation date
    pub created_at: DateTime<Utc>,
    
    /// Market status
    pub status: String,
    
    /// Total volume
    pub volume: Option<Decimal>,
    
    /// Market outcomes
    pub outcomes: Vec<String>,
}

/// Gamma API response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GammaResponse<T> {
    pub data: T,
    pub next_cursor: Option<String>,
    pub count: Option<usize>,
}

/// User metadata for tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GammaMetadata {
    /// User address
    pub address: String,
    
    /// Whether this is our own address
    pub is_own_address: bool,
    
    /// Address label from address book
    pub label: Option<String>,
    
    /// Address type from address book
    pub address_type: Option<String>,
    
    /// Last sync timestamps
    pub last_positions_sync: Option<DateTime<Utc>>,
    pub last_activity_sync: Option<DateTime<Utc>>,
    pub last_holders_sync: Option<DateTime<Utc>>,
    
    /// Sync statistics
    pub total_positions: usize,
    pub total_activities: usize,
    pub total_unique_markets: usize,
    
    /// Account statistics
    pub total_volume: Decimal,
    pub total_pnl: Decimal,
    pub active_positions: usize,
    
    /// Creation and update timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// User state summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserState {
    /// Metadata
    pub metadata: GammaMetadata,
    
    /// Current positions
    pub positions: Vec<GammaPosition>,
    
    /// Recent activity (last 100)
    pub recent_activity: Vec<GammaActivity>,
    
    /// Portfolio summary
    pub portfolio_summary: PortfolioSummary,
    
    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
}

/// Portfolio summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioSummary {
    /// Total portfolio value
    pub total_value: Decimal,
    
    /// Total realized PnL
    pub total_realized_pnl: Decimal,
    
    /// Total unrealized PnL
    pub total_unrealized_pnl: Decimal,
    
    /// Total trading volume
    pub total_volume: Decimal,
    
    /// Number of active positions
    pub active_positions: usize,
    
    /// Number of closed positions
    pub closed_positions: usize,
    
    /// Number of unique markets traded
    pub unique_markets: usize,
    
    /// Win rate (percentage)
    pub win_rate: Option<Decimal>,
    
    /// Total trades count
    pub total_trades: usize,
    
    /// Last trade timestamp
    pub last_trade: Option<DateTime<Utc>>,
}

/// Query parameters for Gamma API
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct GammaQuery {
    /// Limit number of results
    pub limit: Option<usize>,
    
    /// Cursor for pagination
    pub cursor: Option<String>,
    
    /// Start date filter
    pub start_date: Option<DateTime<Utc>>,
    
    /// End date filter
    pub end_date: Option<DateTime<Utc>>,
    
    /// Market filter
    pub market: Option<String>,
    
    /// Asset filter
    pub asset_id: Option<String>,
    
    /// Activity type filter
    pub activity_type: Option<ActivityType>,
}

#[allow(dead_code)]
impl GammaMetadata {
    /// Create new metadata for an address
    pub fn new(address: String, is_own_address: bool) -> Self {
        let now = Utc::now();
        Self {
            address,
            is_own_address,
            label: None,
            address_type: None,
            last_positions_sync: None,
            last_activity_sync: None,
            last_holders_sync: None,
            total_positions: 0,
            total_activities: 0,
            total_unique_markets: 0,
            total_volume: Decimal::ZERO,
            total_pnl: Decimal::ZERO,
            active_positions: 0,
            created_at: now,
            updated_at: now,
        }
    }
    
    /// Update sync timestamp for specific data type
    pub fn update_sync_time(&mut self, data_type: &str) {
        self.updated_at = Utc::now();
        match data_type {
            "positions" => self.last_positions_sync = Some(self.updated_at),
            "activity" => self.last_activity_sync = Some(self.updated_at),
            "holders" => self.last_holders_sync = Some(self.updated_at),
            _ => {}
        }
    }
}