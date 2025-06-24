//! Streaming service trait definitions

use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;
use std::time::Instant;
use tokio::sync::broadcast;

/// Statistics about the streaming service
#[derive(Debug, Clone, Default)]
pub struct StreamingStats {
    /// Total number of active WebSocket connections
    pub active_connections: usize,

    /// Total number of tokens being streamed
    pub total_tokens: usize,

    /// Number of events received per second
    pub events_per_second: f64,

    /// Total events received since start
    pub total_events_received: u64,

    /// Number of connection errors
    pub connection_errors: u64,

    /// Number of reconnection attempts
    pub reconnection_attempts: u64,

    /// Uptime in seconds
    pub uptime_seconds: u64,
}

/// Worker status information
#[derive(Debug, Clone)]
pub struct WorkerStatus {
    /// Unique worker ID
    pub worker_id: usize,

    /// Tokens assigned to this worker
    pub assigned_tokens: Vec<String>,

    /// Connection status
    pub is_connected: bool,

    /// Events processed by this worker
    pub events_processed: u64,

    /// Last error if any
    pub last_error: Option<String>,

    /// Last activity timestamp
    pub last_activity: Instant,
}

/// Main trait for the streaming service
/// 
/// This trait will be implemented by concrete streaming services
/// and used throughout the application for real-time data access.
#[async_trait]
pub trait StreamingServiceTrait: Send + Sync {
    /// The event type produced by this streaming service
    type Event: Send + Sync + Clone;
    
    /// The orderbook type used by this streaming service
    type OrderBook: Send + Sync + Clone;

    /// Add tokens to stream (will be distributed across workers)
    async fn add_tokens(&self, tokens: Vec<String>) -> Result<()>;

    /// Get all currently streaming tokens
    async fn get_streaming_tokens(&self) -> Vec<String>;

    /// Get orderbook for a specific token
    async fn get_order_book(&self, token_id: &str) -> Option<Self::OrderBook>;

    /// Get last trade price for a token
    async fn get_last_trade_price(&self, token_id: &str) -> Option<(Decimal, u64)>;

    /// Get event receiver for all events from all workers
    fn subscribe_events(&self) -> broadcast::Receiver<Self::Event>;

    /// Get current statistics
    async fn get_stats(&self) -> StreamingStats;

    /// Get status of all workers
    async fn get_worker_statuses(&self) -> Vec<WorkerStatus>;
}