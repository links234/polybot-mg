//! Execution configuration with strong typing
//!
//! Defines all configuration structures for the execution engine,
//! following CLAUDE.md principles of strong typing over primitives.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Execution modes supported by the engine
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExecutionMode {
    /// Live WebSocket streaming from Polymarket
    RealTime {
        /// Assets to subscribe to
        assets: Vec<AssetId>,
        /// Enable user feed (requires authentication)
        enable_user_feed: bool,
    },
    /// Replay historical data from files
    Replay {
        /// Path to replay data directory
        data_path: PathBuf,
        /// Playback speed multiplier (1.0 = real-time)
        speed_multiplier: f64,
        /// Assets to filter (None = all assets)
        filter_assets: Option<Vec<AssetId>>,
    },
    /// Simulation mode with synthetic data
    Simulation {
        /// Number of synthetic assets to create
        asset_count: usize,
        /// Event generation frequency
        event_frequency: Duration,
    },
}

/// Strongly typed asset identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AssetId(pub String);


impl From<String> for AssetId {
    fn from(id: String) -> Self {
        Self(id)
    }
}

impl From<&str> for AssetId {
    fn from(id: &str) -> Self {
        Self(id.to_string())
    }
}

/// WebSocket connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketConfig {
    /// Market data WebSocket URL
    pub market_url: String,
    /// User data WebSocket URL (optional)
    pub user_url: Option<String>,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Heartbeat interval
    pub heartbeat_interval: Duration,
    /// Maximum reconnection attempts (None = infinite)
    pub max_reconnections: Option<usize>,
    /// Initial reconnection delay
    pub initial_reconnection_delay: Duration,
    /// Maximum reconnection delay
    pub max_reconnection_delay: Duration,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            market_url: "wss://ws-subscriptions-clob.polymarket.com/ws/market".to_string(),
            user_url: Some("wss://ws-subscriptions-clob.polymarket.com/ws/user".to_string()),
            connection_timeout: Duration::from_secs(30),
            heartbeat_interval: Duration::from_secs(10),
            max_reconnections: None,
            initial_reconnection_delay: Duration::from_millis(1000),
            max_reconnection_delay: Duration::from_secs(30),
        }
    }
}

/// Replay configuration options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayConfig {
    /// Data source directory
    pub data_directory: PathBuf,
    /// Playback speed (1.0 = real-time, 2.0 = 2x speed)
    pub playback_speed: f64,
    /// Start time filter (ISO 8601)
    pub start_time: Option<String>,
    /// End time filter (ISO 8601)
    pub end_time: Option<String>,
    /// Loop replay when finished
    pub loop_replay: bool,
    /// Preserve original timestamps
    pub preserve_timestamps: bool,
}

impl Default for ReplayConfig {
    fn default() -> Self {
        Self {
            data_directory: PathBuf::from("./data/replay"),
            playback_speed: 1.0,
            start_time: None,
            end_time: None,
            loop_replay: false,
            preserve_timestamps: false,
        }
    }
}

/// Processing configuration for events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingConfig {
    /// Enable order book validation
    pub validate_orderbooks: bool,
    /// Auto-clean crossed markets
    pub auto_clean_crossed_markets: bool,
    /// Event buffer size
    pub event_buffer_size: usize,
    /// Maximum events per second (rate limiting)
    pub max_events_per_second: Option<usize>,
    /// Enable metrics collection
    pub collect_metrics: bool,
    /// Metrics update interval
    pub metrics_interval: Duration,
}

impl Default for ProcessingConfig {
    fn default() -> Self {
        Self {
            validate_orderbooks: true,
            auto_clean_crossed_markets: true,
            event_buffer_size: 10000,
            max_events_per_second: None,
            collect_metrics: true,
            metrics_interval: Duration::from_secs(1),
        }
    }
}

/// Main execution configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    /// Execution mode and parameters
    pub mode: ExecutionMode,
    /// WebSocket configuration (used for real-time mode)
    pub websocket: WebSocketConfig,
    /// Replay configuration (used for replay mode)
    pub replay: ReplayConfig,
    /// Event processing configuration
    pub processing: ProcessingConfig,
    /// Output configuration
    pub output: OutputConfig,
}

/// Output configuration for execution results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Enable TUI display
    pub enable_tui: bool,
    /// Enable console logging
    pub enable_console: bool,
    /// Log level filter
    pub log_level: LogLevel,
    /// Save events to file
    pub save_events: Option<PathBuf>,
    /// Event save format
    pub save_format: EventFormat,
}

/// Log level enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// Event save formats
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventFormat {
    Json,
    Binary,
    Csv,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            enable_tui: true,
            enable_console: false,
            log_level: LogLevel::Info,
            save_events: None,
            save_format: EventFormat::Json,
        }
    }
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            mode: ExecutionMode::RealTime {
                assets: vec![],
                enable_user_feed: false,
            },
            websocket: WebSocketConfig::default(),
            replay: ReplayConfig::default(),
            processing: ProcessingConfig::default(),
            output: OutputConfig::default(),
        }
    }
}


