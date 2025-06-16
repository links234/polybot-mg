//! Data source abstractions for the execution engine
//! 
//! Provides unified interfaces for different data sources:
//! - Real-time WebSocket streams
//! - Historical replay from files
//! - Synthetic simulation data

use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::mpsc;
use async_trait::async_trait;
use tracing::{info, warn, error};

use super::events::ExecutionEvent;
use super::config::{AssetId, WebSocketConfig, ReplayConfig};
use crate::ws::WsClient;

/// Unified data source trait
#[async_trait]
pub trait DataSource: Send + Sync {
    /// Start the data source
    async fn start(&mut self) -> Result<(), DataSourceError>;
    
    /// Stop the data source
    async fn stop(&mut self) -> Result<(), DataSourceError>;
    
    /// Get the event stream
    fn event_stream(&self) -> Box<dyn EventStream>;
    
    /// Get source name for logging
    fn name(&self) -> &str;
    
    /// Check if source is currently active
    fn is_active(&self) -> bool;
    
    /// Get source health status
    fn health_status(&self) -> SourceHealth;
}

/// Event stream trait for receiving events
#[async_trait]
pub trait EventStream: Send + Sync {
    /// Receive the next event
    async fn next_event(&mut self) -> Option<Result<ExecutionEvent, StreamError>>;
    
    /// Check if stream has more events
    fn has_more(&self) -> bool;
    
    /// Get stream statistics
    fn stats(&self) -> StreamStats;
}

/// Data source errors
#[derive(Debug, thiserror::Error)]
pub enum DataSourceError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    #[error("Authentication failed")]
    AuthenticationFailed,
    #[error("Source already running")]
    AlreadyRunning,
    #[error("Source not running")]
    NotRunning,
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Stream errors
#[derive(Debug, thiserror::Error)]
pub enum StreamError {
    #[error("Connection lost: {0}")]
    ConnectionLost(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    #[error("Stream ended")]
    StreamEnded,
}

/// Source health status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceHealth {
    Healthy,
    Warning(String),
    Error(String),
    Disconnected,
}

/// Stream statistics
#[derive(Debug, Clone)]
pub struct StreamStats {
    pub events_received: usize,
    pub events_per_second: f64,
    pub bytes_received: usize,
    pub last_event_time: Option<SystemTime>,
    pub connection_uptime: Duration,
    pub reconnection_count: usize,
}

impl Default for StreamStats {
    fn default() -> Self {
        Self {
            events_received: 0,
            events_per_second: 0.0,
            bytes_received: 0,
            last_event_time: None,
            connection_uptime: Duration::from_secs(0),
            reconnection_count: 0,
        }
    }
}

/// Real-time WebSocket data source
pub struct WebSocketDataSource {
    _config: WebSocketConfig,
    _assets: Vec<AssetId>,
    client: Option<Arc<WsClient>>,
    event_tx: Option<mpsc::UnboundedSender<ExecutionEvent>>,
    is_running: bool,
    health: SourceHealth,
}

impl WebSocketDataSource {
    /// Create a new WebSocket data source
    pub fn new(config: WebSocketConfig, assets: Vec<AssetId>) -> Self {
        info!(
            assets_count = assets.len(),
            market_url = %config.market_url,
            "Creating WebSocket data source"
        );
        
        Self {
            _config: config,
            _assets: assets,
            client: None,
            event_tx: None,
            is_running: false,
            health: SourceHealth::Disconnected,
        }
    }
}

#[async_trait]
impl DataSource for WebSocketDataSource {
    async fn start(&mut self) -> Result<(), DataSourceError> {
        if self.is_running {
            warn!("WebSocket data source already running");
            return Err(DataSourceError::AlreadyRunning);
        }
        
        info!("Starting WebSocket data source");
        
        // Create event channel
        let (event_tx, _event_rx) = mpsc::unbounded_channel();
        self.event_tx = Some(event_tx.clone());
        
        // TODO: Initialize WebSocket client and connect
        // This would integrate with the existing WsClient
        
        self.is_running = true;
        self.health = SourceHealth::Healthy;
        
        info!("WebSocket data source started successfully");
        Ok(())
    }
    
    async fn stop(&mut self) -> Result<(), DataSourceError> {
        if !self.is_running {
            return Err(DataSourceError::NotRunning);
        }
        
        info!("Stopping WebSocket data source");
        
        // Close client connection
        if let Some(_client) = self.client.take() {
            // TODO: Implement graceful shutdown
        }
        
        self.is_running = false;
        self.health = SourceHealth::Disconnected;
        
        info!("WebSocket data source stopped");
        Ok(())
    }
    
    fn event_stream(&self) -> Box<dyn EventStream> {
        Box::new(WebSocketEventStream::new())
    }
    
    fn name(&self) -> &str {
        "WebSocket"
    }
    
    fn is_active(&self) -> bool {
        self.is_running
    }
    
    fn health_status(&self) -> SourceHealth {
        self.health.clone()
    }
}

/// WebSocket event stream implementation
pub struct WebSocketEventStream {
    _receiver: Option<mpsc::UnboundedReceiver<ExecutionEvent>>,
    stats: StreamStats,
}

impl WebSocketEventStream {
    fn new() -> Self {
        Self {
            _receiver: None,
            stats: StreamStats::default(),
        }
    }
}

#[async_trait]
impl EventStream for WebSocketEventStream {
    async fn next_event(&mut self) -> Option<Result<ExecutionEvent, StreamError>> {
        // TODO: Implement actual event receiving
        None
    }
    
    fn has_more(&self) -> bool {
        true // WebSocket streams are continuous
    }
    
    fn stats(&self) -> StreamStats {
        self.stats.clone()
    }
}

/// Replay data source for historical data
pub struct ReplayDataSource {
    config: ReplayConfig,
    _filter_assets: Option<Vec<AssetId>>,
    event_tx: Option<mpsc::UnboundedSender<ExecutionEvent>>,
    is_running: bool,
    health: SourceHealth,
    current_position: usize,
    total_events: usize,
}

impl ReplayDataSource {
    /// Create a new replay data source
    pub fn new(config: ReplayConfig, filter_assets: Option<Vec<AssetId>>) -> Self {
        info!(
            data_directory = %config.data_directory.display(),
            playback_speed = config.playback_speed,
            "Creating replay data source"
        );
        
        Self {
            config,
            _filter_assets: filter_assets,
            event_tx: None,
            is_running: false,
            health: SourceHealth::Disconnected,
            current_position: 0,
            total_events: 0,
        }
    }
    
    /// Get replay progress (0.0 to 1.0)
    pub fn progress(&self) -> f64 {
        if self.total_events == 0 {
            0.0
        } else {
            self.current_position as f64 / self.total_events as f64
        }
    }
}

#[async_trait]
impl DataSource for ReplayDataSource {
    async fn start(&mut self) -> Result<(), DataSourceError> {
        if self.is_running {
            return Err(DataSourceError::AlreadyRunning);
        }
        
        info!("Starting replay data source");
        
        // Validate data directory exists
        if !self.config.data_directory.exists() {
            let error_msg = format!("Data directory does not exist: {}", self.config.data_directory.display());
            error!("{}", error_msg);
            return Err(DataSourceError::ConfigurationError(error_msg));
        }
        
        // Create event channel
        let (event_tx, _event_rx) = mpsc::unbounded_channel();
        self.event_tx = Some(event_tx);
        
        // TODO: Scan directory for replay files and count total events
        self.total_events = 0; // Placeholder
        
        self.is_running = true;
        self.health = SourceHealth::Healthy;
        
        info!("Replay data source started successfully");
        Ok(())
    }
    
    async fn stop(&mut self) -> Result<(), DataSourceError> {
        if !self.is_running {
            return Err(DataSourceError::NotRunning);
        }
        
        info!("Stopping replay data source");
        
        self.is_running = false;
        self.health = SourceHealth::Disconnected;
        
        info!("Replay data source stopped");
        Ok(())
    }
    
    fn event_stream(&self) -> Box<dyn EventStream> {
        Box::new(ReplayEventStream::new(self.config.clone()))
    }
    
    fn name(&self) -> &str {
        "Replay"
    }
    
    fn is_active(&self) -> bool {
        self.is_running
    }
    
    fn health_status(&self) -> SourceHealth {
        self.health.clone()
    }
}

/// Replay event stream implementation
pub struct ReplayEventStream {
    _config: ReplayConfig,
    stats: StreamStats,
    finished: bool,
}

impl ReplayEventStream {
    fn new(config: ReplayConfig) -> Self {
        Self {
            _config: config,
            stats: StreamStats::default(),
            finished: false,
        }
    }
}

#[async_trait]
impl EventStream for ReplayEventStream {
    async fn next_event(&mut self) -> Option<Result<ExecutionEvent, StreamError>> {
        if self.finished {
            return None;
        }
        
        // TODO: Implement actual replay event reading from files
        // For now, return None to indicate end of stream
        self.finished = true;
        None
    }
    
    fn has_more(&self) -> bool {
        !self.finished
    }
    
    fn stats(&self) -> StreamStats {
        self.stats.clone()
    }
}

/// Simulation data source for synthetic data
pub struct SimulationDataSource {
    _asset_count: usize,
    event_frequency: Duration,
    event_tx: Option<mpsc::UnboundedSender<ExecutionEvent>>,
    is_running: bool,
    health: SourceHealth,
}

impl SimulationDataSource {
    /// Create a new simulation data source
    pub fn new(asset_count: usize, event_frequency: Duration) -> Self {
        info!(
            asset_count = asset_count,
            event_frequency = ?event_frequency,
            "Creating simulation data source"
        );
        
        Self {
            _asset_count: asset_count,
            event_frequency,
            event_tx: None,
            is_running: false,
            health: SourceHealth::Disconnected,
        }
    }
}

#[async_trait]
impl DataSource for SimulationDataSource {
    async fn start(&mut self) -> Result<(), DataSourceError> {
        if self.is_running {
            return Err(DataSourceError::AlreadyRunning);
        }
        
        info!("Starting simulation data source");
        
        // Create event channel
        let (event_tx, _event_rx) = mpsc::unbounded_channel();
        self.event_tx = Some(event_tx);
        
        // TODO: Start synthetic event generation
        
        self.is_running = true;
        self.health = SourceHealth::Healthy;
        
        info!("Simulation data source started successfully");
        Ok(())
    }
    
    async fn stop(&mut self) -> Result<(), DataSourceError> {
        if !self.is_running {
            return Err(DataSourceError::NotRunning);
        }
        
        info!("Stopping simulation data source");
        
        self.is_running = false;
        self.health = SourceHealth::Disconnected;
        
        info!("Simulation data source stopped");
        Ok(())
    }
    
    fn event_stream(&self) -> Box<dyn EventStream> {
        Box::new(SimulationEventStream::new(self.event_frequency))
    }
    
    fn name(&self) -> &str {
        "Simulation"
    }
    
    fn is_active(&self) -> bool {
        self.is_running
    }
    
    fn health_status(&self) -> SourceHealth {
        self.health.clone()
    }
}

/// Simulation event stream implementation
pub struct SimulationEventStream {
    event_frequency: Duration,
    stats: StreamStats,
}

impl SimulationEventStream {
    fn new(event_frequency: Duration) -> Self {
        Self {
            event_frequency,
            stats: StreamStats::default(),
        }
    }
}

#[async_trait]
impl EventStream for SimulationEventStream {
    async fn next_event(&mut self) -> Option<Result<ExecutionEvent, StreamError>> {
        // TODO: Generate synthetic events
        tokio::time::sleep(self.event_frequency).await;
        None
    }
    
    fn has_more(&self) -> bool {
        true // Simulation can run indefinitely
    }
    
    fn stats(&self) -> StreamStats {
        self.stats.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_websocket_source_lifecycle() {
        let config = WebSocketConfig::default();
        let assets = vec![AssetId::from("test_asset")];
        let mut source = WebSocketDataSource::new(config, assets);
        
        assert!(!source.is_active());
        assert_eq!(source.health_status(), SourceHealth::Disconnected);
        
        // Start should succeed
        let result = source.start().await;
        assert!(result.is_ok());
        assert!(source.is_active());
        
        // Starting again should fail
        let result = source.start().await;
        assert!(matches!(result, Err(DataSourceError::AlreadyRunning)));
        
        // Stop should succeed
        let result = source.stop().await;
        assert!(result.is_ok());
        assert!(!source.is_active());
    }
    
    #[tokio::test]
    async fn test_replay_source_validation() {
        let config = ReplayConfig {
            data_directory: PathBuf::from("/nonexistent/path"),
            ..Default::default()
        };
        let mut source = ReplayDataSource::new(config, None);
        
        // Should fail due to nonexistent directory
        let result = source.start().await;
        assert!(matches!(result, Err(DataSourceError::ConfigurationError(_))));
    }
}