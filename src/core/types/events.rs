//! Event type definitions for the polybot system

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

/// Strongly typed event identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId(pub String);

impl EventId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
    
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for EventId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
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
    Simulation { 
        generator_id: String 
    },
    /// Internal system event
    System { 
        component: String 
    },
}

/// WebSocket feed types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedType {
    Market,
    User,
}

/// Event data payload - unified across all event sources
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

/// Market-related events (moved from execution module)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarketEvent {
    /// Order book snapshot
    OrderBookSnapshot {
        asset_id: String,
        bids: Vec<(Decimal, Decimal)>, // Will be replaced with PriceLevel when moving execution
        asks: Vec<(Decimal, Decimal)>,
        hash: String,
    },
    /// Price level change
    PriceChange {
        asset_id: String,
        side: super::common::Side,
        price: Decimal,
        size: Decimal,
        hash: String,
    },
    /// Trade execution
    Trade {
        asset_id: String,
        price: Decimal,
        size: Decimal,
        side: super::common::Side,
        trade_id: Option<String>,
    },
    /// Tick size change
    TickSizeChange {
        asset_id: String,
        tick_size: Decimal,
    },
    /// Market status change
    MarketStatus {
        asset_id: String,
        status: super::common::MarketStatus,
    },
}

/// User-specific events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserEvent {
    /// User order update
    OrderUpdate {
        order_id: String,
        asset_id: String,
        side: super::common::Side,
        price: Decimal,
        size: Decimal,
        status: super::common::OrderStatus,
    },
    /// User trade
    UserTrade {
        trade_id: String,
        order_id: String,
        asset_id: String,
        side: super::common::Side,
        price: Decimal,
        size: Decimal,
    },
    /// Balance update
    BalanceUpdate { 
        asset_id: String, 
        balance: Decimal 
    },
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
    ConnectionLost { 
        endpoint: String, 
        error: String 
    },
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
    pub tags: HashMap<String, String>,
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
            tags: HashMap::new(),
            priority: EventPriority::Normal,
        }
    }
}