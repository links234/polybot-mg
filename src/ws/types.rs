//! WebSocket event types for storage serialization

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Market event types that can be stored
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarketEvent {
    /// Order book change event
    BookChange(BookChange),
    /// Trade event
    Trade(Trade),
    /// Tick size change event
    TickSizeChange(TickSizeChange),
}

/// Book change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookChange {
    pub side: String,
    pub changes: Vec<(Decimal, Decimal)>,
}

/// Trade event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub id: String,
    pub price: Decimal,
    pub size: Decimal,
    pub side: String,
}

/// Tick size change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickSizeChange {
    pub tick_size: Decimal,
    pub min_tick_size: Option<Decimal>,
}
