//! Configuration for the streaming service

use crate::data_paths::DataPaths;
use crate::core::ws::WsConfig;

/// Configuration for the streaming service
#[derive(Debug, Clone)]
pub struct StreamingServiceConfig {
    /// Base WebSocket configuration
    pub ws_config: WsConfig,

    /// Maximum tokens per worker connection
    pub tokens_per_worker: usize,

    /// Event buffer size for the main event channel
    pub event_buffer_size: usize,

    /// Buffer size for worker-specific event channels
    pub worker_event_buffer_size: usize,

    /// Auto-reconnect on connection failure
    pub auto_reconnect: bool,

    /// Initial reconnect delay in milliseconds
    pub reconnect_delay_ms: u64,

    /// Maximum reconnect delay in milliseconds (exponential backoff)
    pub max_reconnect_delay_ms: u64,

    /// Maximum number of reconnect attempts before giving up
    pub max_reconnect_attempts: u32,

    /// Data paths for configuration and logging
    pub _data_paths: DataPaths,

    /// Host for API connections
    pub _host: String,

    /// Health check interval in seconds
    pub health_check_interval_secs: u64,

    /// Statistics collection interval in seconds
    pub stats_interval_secs: u64,

    /// Delay between worker connection attempts in milliseconds
    pub worker_connection_delay_ms: u64,

    /// Maximum number of concurrent connection attempts
    pub max_concurrent_connections: usize,
}

impl Default for StreamingServiceConfig {
    fn default() -> Self {
        Self {
            ws_config: WsConfig::default(),
            tokens_per_worker: 20,          // Increased to reduce worker count
            event_buffer_size: 10000,       // Large buffer for main channel
            worker_event_buffer_size: 1000, // Smaller buffer per worker
            auto_reconnect: true,
            reconnect_delay_ms: 1000,      // Start with 1 second
            max_reconnect_delay_ms: 30000, // Cap at 30 seconds
            max_reconnect_attempts: 10,
            _data_paths: DataPaths::new("./data"),
            _host: "https://clob.polymarket.com".to_string(),
            health_check_interval_secs: 30,
            stats_interval_secs: 10,
            worker_connection_delay_ms: 250, // 250ms delay between connections
            max_concurrent_connections: 3,   // Only 3 concurrent connection attempts
        }
    }
}

