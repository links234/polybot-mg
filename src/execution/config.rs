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

impl AssetId {
    pub fn _new(id: String) -> Self {
        Self(id)
    }

    pub fn _as_str(&self) -> &str {
        &self.0
    }

    pub fn _into_string(self) -> String {
        self.0
    }
}

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

impl ExecutionConfig {
    /// Create a new real-time configuration
    pub fn _real_time(assets: Vec<AssetId>) -> Self {
        Self {
            mode: ExecutionMode::RealTime {
                assets,
                enable_user_feed: false,
            },
            ..Default::default()
        }
    }

    /// Create a new replay configuration
    pub fn _replay(data_path: PathBuf) -> Self {
        Self {
            mode: ExecutionMode::Replay {
                data_path,
                speed_multiplier: 1.0,
                filter_assets: None,
            },
            ..Default::default()
        }
    }

    /// Create a new simulation configuration
    pub fn _simulation(asset_count: usize) -> Self {
        Self {
            mode: ExecutionMode::Simulation {
                asset_count,
                event_frequency: Duration::from_millis(100),
            },
            ..Default::default()
        }
    }

    /// Enable user feed for real-time mode
    pub fn _with_user_feed(mut self) -> Self {
        if let ExecutionMode::RealTime {
            ref mut enable_user_feed,
            ..
        } = self.mode
        {
            *enable_user_feed = true;
        }
        self
    }

    /// Set playback speed for replay mode
    pub fn _with_playback_speed(mut self, speed: f64) -> Self {
        if let ExecutionMode::Replay {
            ref mut speed_multiplier,
            ..
        } = self.mode
        {
            *speed_multiplier = speed;
        }
        self.replay.playback_speed = speed;
        self
    }

    /// Disable TUI and enable console output
    pub fn _console_mode(mut self) -> Self {
        self.output.enable_tui = false;
        self.output.enable_console = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_id_creation() {
        let asset = AssetId::_new("0x123abc".to_string());
        assert_eq!(asset._as_str(), "0x123abc");

        let asset2 = AssetId::from("0x456def");
        assert_eq!(asset2._as_str(), "0x456def");
    }

    #[test]
    fn test_execution_config_builders() {
        let config = ExecutionConfig::_real_time(vec![AssetId::from("test")])
            ._with_user_feed()
            ._console_mode();

        match config.mode {
            ExecutionMode::RealTime {
                enable_user_feed, ..
            } => {
                assert!(enable_user_feed);
            }
            _ => panic!("Expected RealTime mode"),
        }

        assert!(!config.output.enable_tui);
        assert!(config.output.enable_console);
    }
}
