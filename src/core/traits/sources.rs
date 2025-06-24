//! Data source trait definitions

use anyhow::Result;
use async_trait::async_trait;
use std::pin::Pin;
use std::time::Duration;
use tokio_stream::Stream;

use crate::core::types::events::{EventData, EventMetadata, EventSource};

/// Trait for unified data sources (WebSocket, Replay, Simulation)
#[async_trait]
pub trait DataSource: Send + Sync {
    /// The event type produced by this data source
    type Event: Send + Sync + Clone;

    /// Connect to the data source
    async fn connect(&mut self) -> Result<()>;

    /// Disconnect from the data source
    async fn disconnect(&mut self) -> Result<()>;

    /// Check if the source is connected
    fn is_connected(&self) -> bool;

    /// Get the source type for event metadata
    fn source_type(&self) -> EventSource;

    /// Subscribe to specific assets/tokens
    async fn subscribe(&mut self, assets: Vec<String>) -> Result<()>;

    /// Unsubscribe from specific assets/tokens
    async fn unsubscribe(&mut self, assets: Vec<String>) -> Result<()>;

    /// Get the event stream
    fn event_stream(&self) -> Pin<Box<dyn Stream<Item = Self::Event> + Send>>;

    /// Get source statistics
    async fn get_stats(&self) -> DataSourceStats;
}

/// Statistics for a data source
#[derive(Debug, Clone, Default)]
pub struct DataSourceStats {
    /// Number of events produced
    pub events_produced: u64,
    
    /// Number of errors encountered
    pub errors_count: u64,
    
    /// Current subscriptions
    pub active_subscriptions: usize,
    
    /// Connection uptime
    pub uptime: Option<Duration>,
    
    /// Last event timestamp
    pub last_event_time: Option<std::time::SystemTime>,
}

/// Trait for event streams with unified event types
pub trait EventStream: Stream<Item = Result<(EventData, EventMetadata)>> + Send + Unpin {
    /// Get the source information for this stream
    fn source(&self) -> &EventSource;
    
    /// Check if the stream is still active
    fn is_active(&self) -> bool;
    
    /// Get stream statistics
    fn stats(&self) -> StreamStats;
}

/// Statistics for an event stream
#[derive(Debug, Clone, Default)]
pub struct StreamStats {
    /// Total events emitted
    pub total_events: u64,
    
    /// Events per second (moving average)
    pub events_per_second: f64,
    
    /// Buffer size (if applicable)
    pub buffer_size: Option<usize>,
    
    /// Dropped events (if buffer overflow)
    pub dropped_events: u64,
}