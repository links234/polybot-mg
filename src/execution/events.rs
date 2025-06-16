//! Execution event system with strongly typed events
//! 
//! Defines the unified event model for the execution engine,
//! supporting both real-time and replay scenarios.

use std::time::{Duration, Instant, SystemTime};
use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;
use tracing::{debug, info, error};

use crate::types::PriceLevel;
use crate::ws::{PolyEvent, Side, OrderStatus};
use super::config::AssetId;

/// Unified execution event that wraps all possible event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionEvent {
    /// Unique event identifier
    pub id: EventId,
    /// Event timestamp (when it occurred)
    pub timestamp: SystemTime,
    /// Event processing time (when it was processed)
    #[serde(skip)]
    pub processed_at: Option<Instant>,
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
    
    pub fn from_string(id: String) -> Self {
        Self(id)
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
        generator_id: String,
    },
    /// Internal system event
    System {
        component: String,
    },
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
    BalanceUpdate {
        asset_id: AssetId,
        balance: Decimal,
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
        error: String,
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
        debug!(event_id = %id.as_str(), event_type = "market", "Creating market event");
        
        Self {
            id,
            timestamp: SystemTime::now(),
            processed_at: None,
            source,
            data: EventData::Market(data),
            metadata: EventMetadata::default(),
        }
    }
    
    /// Create a new user event
    pub fn user(data: UserEvent, source: EventSource) -> Self {
        let id = EventId::new();
        debug!(event_id = %id.as_str(), event_type = "user", "Creating user event");
        
        Self {
            id,
            timestamp: SystemTime::now(),
            processed_at: None,
            source,
            data: EventData::User(data),
            metadata: EventMetadata::default(),
        }
    }
    
    /// Create a new system event
    pub fn system(data: SystemEvent, source: EventSource) -> Self {
        let id = EventId::new();
        info!(event_id = %id.as_str(), event_type = "system", "Creating system event");
        
        Self {
            id,
            timestamp: SystemTime::now(),
            processed_at: None,
            source,
            data: EventData::System(data),
            metadata: EventMetadata::default(),
        }
    }
    
    /// Create a new metrics event
    pub fn metrics(data: MetricsEvent, source: EventSource) -> Self {
        let id = EventId::new();
        debug!(event_id = %id.as_str(), event_type = "metrics", "Creating metrics event");
        
        Self {
            id,
            timestamp: SystemTime::now(),
            processed_at: None,
            source,
            data: EventData::Metrics(data),
            metadata: EventMetadata::default(),
        }
    }
    
    /// Mark event as processed
    pub fn mark_processed(&mut self) {
        self.processed_at = Some(Instant::now());
        
        if let Some(start_time) = self.processed_at {
            // This is a placeholder - in real implementation we'd track from creation
            self.metadata.processing_duration = Some(start_time.elapsed());
        }
    }
    
    /// Add a tag to the event
    pub fn with_tag(mut self, key: String, value: String) -> Self {
        self.metadata.tags.insert(key, value);
        self
    }
    
    /// Set event priority
    pub fn with_priority(mut self, priority: EventPriority) -> Self {
        self.metadata.priority = priority;
        self
    }
    
    /// Get the asset ID if this is a market event
    pub fn asset_id(&self) -> Option<&AssetId> {
        match &self.data {
            EventData::Market(market_event) => Some(market_event.asset_id()),
            EventData::User(user_event) => Some(user_event.asset_id()),
            _ => None,
        }
    }
}

impl MarketEvent {
    /// Get the asset ID for this market event
    pub fn asset_id(&self) -> &AssetId {
        match self {
            MarketEvent::OrderBookSnapshot { asset_id, .. } => asset_id,
            MarketEvent::PriceChange { asset_id, .. } => asset_id,
            MarketEvent::Trade { asset_id, .. } => asset_id,
            MarketEvent::TickSizeChange { asset_id, .. } => asset_id,
            MarketEvent::MarketStatus { asset_id, .. } => asset_id,
        }
    }
}

impl UserEvent {
    /// Get the asset ID for this user event
    pub fn asset_id(&self) -> &AssetId {
        match self {
            UserEvent::OrderUpdate { asset_id, .. } => asset_id,
            UserEvent::UserTrade { asset_id, .. } => asset_id,
            UserEvent::BalanceUpdate { asset_id, .. } => asset_id,
        }
    }
}

/// Convert from WebSocket PolyEvent to ExecutionEvent
impl From<PolyEvent> for ExecutionEvent {
    fn from(poly_event: PolyEvent) -> Self {
        let source = EventSource::WebSocket {
            connection_id: "primary".to_string(),
            feed_type: FeedType::Market,
        };
        
        let market_event = match poly_event {
            PolyEvent::Book { asset_id, bids, asks, hash } => {
                MarketEvent::OrderBookSnapshot {
                    asset_id: AssetId::from(asset_id),
                    bids,
                    asks,
                    hash,
                }
            }
            PolyEvent::PriceChange { asset_id, side, price, size, hash } => {
                MarketEvent::PriceChange {
                    asset_id: AssetId::from(asset_id),
                    side,
                    price,
                    size,
                    hash,
                }
            }
            PolyEvent::Trade { asset_id, price, size, side } => {
                MarketEvent::Trade {
                    asset_id: AssetId::from(asset_id),
                    price,
                    size,
                    side,
                    trade_id: None,
                }
            }
            PolyEvent::TickSizeChange { asset_id, tick_size } => {
                MarketEvent::TickSizeChange {
                    asset_id: AssetId::from(asset_id),
                    tick_size,
                }
            }
            PolyEvent::MyOrder { asset_id, side, price, size, status } => {
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
            PolyEvent::MyTrade { asset_id, side, price, size } => {
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
        };
        
        ExecutionEvent::market(market_event, source)
    }
}

/// Event handler trait for processing execution events
pub trait EventHandler: Send + Sync {
    /// Handle an execution event
    fn handle_event(&mut self, event: &ExecutionEvent) -> Result<(), EventHandlerError>;
    
    /// Get handler name for logging
    fn name(&self) -> &str;
    
    /// Check if handler can process this event type
    fn can_handle(&self, event: &ExecutionEvent) -> bool;
}

/// Event handler errors
#[derive(Debug, thiserror::Error)]
pub enum EventHandlerError {
    #[error("Processing error: {0}")]
    ProcessingError(String),
    #[error("Unsupported event type")]
    UnsupportedEvent,
    #[error("Handler not ready")]
    NotReady,
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
        
        let event = ExecutionEvent::market(market_event, source)
            .with_tag("test".to_string(), "value".to_string())
            .with_priority(EventPriority::High);
        
        assert_eq!(event.metadata.priority, EventPriority::High);
        assert_eq!(event.metadata.tags.get("test"), Some(&"value".to_string()));
        assert!(event.asset_id().is_some());
    }
    
    #[test]
    fn test_poly_event_conversion() {
        let poly_event = PolyEvent::Trade {
            asset_id: "0x123".to_string(),
            price: Decimal::new(5000, 4),
            size: Decimal::new(100, 0),
            side: Side::Buy,
        };
        
        let exec_event: ExecutionEvent = poly_event.into();
        
        match exec_event.data {
            EventData::Market(MarketEvent::Trade { asset_id, .. }) => {
                assert_eq!(asset_id.as_str(), "0x123");
            }
            _ => panic!("Expected market trade event"),
        }
    }
}