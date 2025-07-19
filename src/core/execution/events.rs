//! Execution event system with strongly typed events
//!
//! Defines the unified event model for the execution engine,
//! supporting both real-time and replay scenarios.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant, SystemTime};
use tracing::debug;

use super::config::AssetId;
use crate::core::types::market::PriceLevel;
use crate::core::types::common::{OrderStatus, Side};
use crate::core::ws::PolyEvent;

/// Unified execution event that wraps all possible event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionEvent {
    /// Unique event identifier
    pub id: EventId,
    /// Event timestamp (when it occurred)
    pub timestamp: SystemTime,
    /// Event processing time (when it was processed)
    #[serde(skip)]
    pub _processed_at: Option<Instant>,
    /// Event source information
    pub source: EventSource,
    /// The actual event data
    pub data: EventData,
    /// Event metadata
    pub metadata: EventMetadata,
}

/// Strongly typed event identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId(pub String);

impl EventId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

}

impl Default for EventId {
    fn default() -> Self {
        Self::new()
    }
}

/// Event source information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventSource {
    /// Real-time WebSocket data
    WebSocket {
        connection_id: String,
        feed_type: FeedType,
    },
    /// Replay from file
    Replay {
        file_path: String,
        original_timestamp: SystemTime,
    },
    /// Synthetic/simulation data
    Simulation { generator_id: String },
    /// Internal system event
    System { component: String },
}

/// WebSocket feed types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedType {
    Market,
    User,
}

/// Event data payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventData {
    /// Market data events
    Market(MarketEvent),
    /// User-specific events
    User(UserEvent),
    /// System control events
    System(SystemEvent),
    /// Metrics and statistics
    Metrics(MetricsEvent),
}

/// Market-related events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarketEvent {
    /// Order book snapshot
    OrderBookSnapshot {
        asset_id: AssetId,
        bids: Vec<PriceLevel>,
        asks: Vec<PriceLevel>,
        hash: String,
    },
    /// Price level change
    PriceChange {
        asset_id: AssetId,
        side: Side,
        price: Decimal,
        size: Decimal,
        hash: String,
    },
    /// Trade execution
    Trade {
        asset_id: AssetId,
        price: Decimal,
        size: Decimal,
        side: Side,
        trade_id: Option<String>,
    },
    /// Tick size change
    TickSizeChange {
        asset_id: AssetId,
        tick_size: Decimal,
    },
    /// Market status change
    MarketStatus {
        asset_id: AssetId,
        status: MarketStatus,
    },
}

/// User-specific events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserEvent {
    /// User order update
    OrderUpdate {
        order_id: String,
        asset_id: AssetId,
        side: Side,
        price: Decimal,
        size: Decimal,
        status: OrderStatus,
    },
    /// User trade
    UserTrade {
        trade_id: String,
        order_id: String,
        asset_id: AssetId,
        side: Side,
        price: Decimal,
        size: Decimal,
    },
    /// Balance update
    BalanceUpdate { asset_id: AssetId, balance: Decimal },
}

/// System control events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemEvent {
    /// Execution started
    ExecutionStarted {
        mode: String,
        config_summary: String,
    },
    /// Execution stopped
    ExecutionStopped {
        reason: StopReason,
        duration: Duration,
    },
    /// Connection established
    ConnectionEstablished {
        endpoint: String,
        feed_type: FeedType,
    },
    /// Connection lost
    ConnectionLost { endpoint: String, error: String },
    /// Error occurred
    Error {
        component: String,
        error: String,
        recoverable: bool,
    },
    /// Health check
    HealthCheck {
        component: String,
        status: HealthStatus,
    },
}

/// Metrics and statistics events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsEvent {
    /// Events processed per second
    pub events_per_second: f64,
    /// Total events processed
    pub total_events: usize,
    /// Active connections
    pub active_connections: usize,
    /// Order book metrics
    pub orderbook_metrics: OrderBookMetrics,
    /// Memory usage
    pub memory_usage: MemoryMetrics,
}

/// Order book specific metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookMetrics {
    /// Number of active order books
    pub active_books: usize,
    /// Average spread across all books
    pub average_spread: Option<Decimal>,
    /// Books with crossed markets
    pub crossed_markets: usize,
    /// Total liquidity (sum of all bid/ask sizes)
    pub total_liquidity: Decimal,
}

/// Memory usage metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetrics {
    /// Heap memory usage in bytes
    pub heap_bytes: usize,
    /// Order book memory usage
    pub orderbook_bytes: usize,
    /// Event buffer usage
    pub buffer_bytes: usize,
}

/// Market status enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketStatus {
    Active,
    Paused,
    Closed,
    Settling,
    Resolved,
}

/// Execution stop reasons
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopReason {
    UserRequested,
    Error,
    ReplayFinished,
    Timeout,
    ConfigChange,
}

/// Component health status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Warning,
    Error,
    Unknown,
}

/// Event metadata for additional context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    /// Processing duration
    pub processing_duration: Option<Duration>,
    /// Event sequence number
    pub sequence_number: Option<u64>,
    /// Related event IDs
    pub related_events: Vec<EventId>,
    /// Custom tags
    pub tags: std::collections::HashMap<String, String>,
    /// Event priority
    pub priority: EventPriority,
}

/// Event priority levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EventPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl Default for EventMetadata {
    fn default() -> Self {
        Self {
            processing_duration: None,
            sequence_number: None,
            related_events: Vec::new(),
            tags: std::collections::HashMap::new(),
            priority: EventPriority::Normal,
        }
    }
}

impl ExecutionEvent {
    /// Create a new market event
    pub fn market(data: MarketEvent, source: EventSource) -> Self {
        let id = EventId::new();
        debug!(event_id = %id.0, event_type = "market", "Creating market event");

        Self {
            id,
            timestamp: SystemTime::now(),
            _processed_at: None,
            source,
            data: EventData::Market(data),
            metadata: EventMetadata::default(),
        }
    }

    /// Create a new user event
    pub fn user(data: UserEvent, source: EventSource) -> Self {
        let id = EventId::new();
        debug!(event_id = %id.0, event_type = "user", "Creating user event");

        Self {
            id,
            timestamp: SystemTime::now(),
            _processed_at: None,
            source,
            data: EventData::User(data),
            metadata: EventMetadata::default(),
        }
    }

    // /// Create a new system event
    // pub fn system(data: SystemEvent, source: EventSource) -> Self {
    //     let id = EventId::new();
    //     info!(event_id = %id._as_str(), event_type = "system", "Creating system event");
    //
    //     Self {
    //         id,
    //         timestamp: SystemTime::now(),
    //         _processed_at: None,
    //         source,
    //         data: EventData::System(data),
    //         metadata: EventMetadata::default(),
    //     }
    // }
    //
    // /// Create a new metrics event
    // pub fn metrics(data: MetricsEvent, source: EventSource) -> Self {
    //     let id = EventId::new();
    //     debug!(event_id = %id._as_str(), event_type = "metrics", "Creating metrics event");
    //
    //     Self {
    //         id,
    //         timestamp: SystemTime::now(),
    //         _processed_at: None,
    //         source,
    //         data: EventData::Metrics(data),
    //         metadata: EventMetadata::default(),
    //     }
    // }

}


/// Convert from WebSocket PolyEvent to ExecutionEvent
impl From<PolyEvent> for ExecutionEvent {
    fn from(poly_event: PolyEvent) -> Self {
        let source = EventSource::WebSocket {
            connection_id: "primary".to_string(),
            feed_type: FeedType::Market,
        };

        let market_event = match poly_event {
            PolyEvent::Book {
                asset_id,
                bids,
                asks,
                hash,
                .. // Ignore market and timestamp
            } => MarketEvent::OrderBookSnapshot {
                asset_id: AssetId::from(asset_id),
                bids,
                asks,
                hash,
            },
            PolyEvent::PriceChange {
                asset_id,
                side,
                price,
                size,
                hash,
            } => MarketEvent::PriceChange {
                asset_id: AssetId::from(asset_id),
                side,
                price,
                size,
                hash,
            },
            PolyEvent::Trade {
                asset_id,
                price,
                size,
                side,
            } => MarketEvent::Trade {
                asset_id: AssetId::from(asset_id),
                price,
                size,
                side,
                trade_id: None,
            },
            PolyEvent::TickSizeChange {
                asset_id,
                tick_size,
            } => MarketEvent::TickSizeChange {
                asset_id: AssetId::from(asset_id),
                tick_size,
            },
            PolyEvent::MyOrder {
                asset_id,
                side,
                price,
                size,
                status,
            } => {
                let user_event = UserEvent::OrderUpdate {
                    order_id: "unknown".to_string(), // PolyEvent doesn't have order_id
                    asset_id: AssetId::from(asset_id),
                    side,
                    price,
                    size,
                    status,
                };
                return ExecutionEvent::user(user_event, source);
            }
            PolyEvent::MyTrade {
                asset_id,
                side,
                price,
                size,
            } => {
                let user_event = UserEvent::UserTrade {
                    trade_id: "unknown".to_string(),
                    order_id: "unknown".to_string(),
                    asset_id: AssetId::from(asset_id),
                    side,
                    price,
                    size,
                };
                return ExecutionEvent::user(user_event, source);
            }
            PolyEvent::LastTradePrice {
                asset_id,
                price,
                timestamp: _,
            } => {
                // Convert LastTradePrice to a Trade event
                // We don't have size or side information, so we'll use defaults
                MarketEvent::Trade {
                    asset_id: AssetId::from(asset_id),
                    price,
                    size: Decimal::ZERO, // No size information available
                    side: Side::Buy,     // Default to Buy since we don't know
                    trade_id: Some("last_trade".to_string()),
                }
            }
            PolyEvent::Unknown { .. } => {
                // Unknown events can't be converted to market events
                // Return a placeholder event
                MarketEvent::Trade {
                    asset_id: AssetId::from("unknown"),
                    price: Decimal::ZERO,
                    size: Decimal::ZERO,
                    side: Side::Buy,
                    trade_id: Some("unknown".to_string()),
                }
            }
        };

        ExecutionEvent::market(market_event, source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_creation() {
        let market_event = MarketEvent::Trade {
            asset_id: AssetId::from("test_asset"),
            price: Decimal::new(5000, 4), // 0.5000
            size: Decimal::new(100, 0),   // 100
            side: Side::Buy,
            trade_id: Some("trade_123".to_string()),
        };

        let source = EventSource::WebSocket {
            connection_id: "test_connection".to_string(),
            feed_type: FeedType::Market,
        };

        let event = ExecutionEvent::market(market_event, source);

        assert_eq!(event.metadata.priority, EventPriority::Normal);
        assert!(event.metadata.tags.is_empty());
    }
}
