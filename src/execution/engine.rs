//! Main execution engine that orchestrates data sources, strategies, and event processing
//!
//! The ExecutionEngine is the central component that coordinates all execution activities,
//! managing the lifecycle of data sources, event streams, and strategy execution.

use std::collections::HashMap;
use std::time::{Duration, Instant};
// use tracing::error;

use super::events::{MemoryMetrics, OrderBookMetrics};
use super::sources::SourceHealth;
use super::strategies::StrategyMetrics;

// /// Main execution engine
// pub struct _ExecutionEngine {
//     /// Engine configuration
//     config: ExecutionConfig,
//     /// Data source providing events
//     data_source: Option<Box<dyn DataSource>>,
//     /// Registered strategies
//     strategies: Vec<Box<dyn Strategy>>,
//     /// Order book state
//     order_books: Arc<RwLock<HashMap<AssetId, OrderBook>>>,
//     /// Event broadcaster for external consumers
//     event_tx: broadcast::Sender<ExecutionEvent>,
//     /// Engine state
//     state: EngineState,
//     /// Engine metrics
//     metrics: EngineMetrics,
//     /// Shutdown signal
//     shutdown_tx: Option<mpsc::Sender<()>>,
// }

// /// Execution engine state
// #[derive(Debug, Clone, PartialEq, Eq)]
// pub enum EngineState {
//     Stopped,
//     Starting,
//     Running,
//     Stopping,
// }

/// Engine execution metrics
#[derive(Debug, Clone)]
pub struct EngineMetrics {
    /// Total events processed
    pub _total_events: usize,
    /// Events processed per second
    pub _events_per_second: f64,
    /// Engine uptime
    pub _uptime: Duration,
    /// Start time
    pub _start_time: Option<Instant>,
    /// Data source health
    pub _source_health: SourceHealth,
    /// Strategy metrics
    pub _strategy_metrics: HashMap<String, StrategyMetrics>,
    /// Order book metrics
    pub _orderbook_metrics: OrderBookMetrics,
    /// Memory metrics
    pub _memory_metrics: MemoryMetrics,
}

impl Default for EngineMetrics {
    fn default() -> Self {
        Self {
            _total_events: 0,
            _events_per_second: 0.0,
            _uptime: Duration::from_secs(0),
            _start_time: None,
            _source_health: SourceHealth::Disconnected,
            _strategy_metrics: HashMap::new(),
            _orderbook_metrics: OrderBookMetrics {
                active_books: 0,
                average_spread: None,
                crossed_markets: 0,
                total_liquidity: rust_decimal::Decimal::ZERO,
            },
            _memory_metrics: MemoryMetrics {
                heap_bytes: 0,
                orderbook_bytes: 0,
                buffer_bytes: 0,
            },
        }
    }
}

/*
impl ExecutionEngine {
    /// Create a new execution engine
    pub fn new(config: ExecutionConfig) -> Self {
        let (event_tx, _) = broadcast::channel(10000);

        info!(
            mode = ?config.mode,
            tui_enabled = config.output.enable_tui,
            console_enabled = config.output.enable_console,
            "Creating execution engine"
        );

        Self {
            config,
            data_source: None,
            strategies: Vec::new(),
            order_books: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            state: EngineState::Stopped,
            metrics: EngineMetrics::default(),
            shutdown_tx: None,
        }
    }

    /// Add a strategy to the engine
    pub fn add_strategy(&mut self, strategy: Box<dyn Strategy>) {
        let strategy_name = strategy.name().to_string();
        info!(strategy = %strategy_name, "Adding strategy to execution engine");

        self.strategies.push(strategy);
    }

    /// Add default strategies based on configuration
    pub fn add_default_strategies(&mut self) {
        info!("Adding default strategies");

        // Always add logging strategy
        let logging_config = StrategyConfig::logging();
        let logging_strategy = LoggingStrategy::new(logging_config);
        self.add_strategy(Box::new(logging_strategy));

        // Add market analysis strategy for real-time and replay modes
        match &self.config.mode {
            super::config::ExecutionMode::RealTime { .. } |
            super::config::ExecutionMode::Replay { .. } => {
                let analysis_config = StrategyConfig::market_analysis();
                let analysis_strategy = MarketAnalysisStrategy::new(analysis_config);
                self.add_strategy(Box::new(analysis_strategy));
            }
            _ => {}
        }
    }

    /// Start the execution engine
    pub async fn start(&mut self) -> Result<(), ExecutionError> {
        if self.state != EngineState::Stopped {
            return Err(ExecutionError::InvalidState(format!(
                "Cannot start engine in state: {:?}", self.state
            )));
        }

        info!("Starting execution engine");
        self.state = EngineState::Starting;

        // Create data source based on configuration
        self.data_source = Some(self.create_data_source()?);

        // Initialize strategies
        for strategy in &mut self.strategies {
            if let Err(e) = strategy.initialize().await {
                error!(
                    strategy = strategy.name(),
                    error = %e,
                    "Failed to initialize strategy"
                );
                return Err(ExecutionError::StrategyError(e));
            }
        }

        // Start data source
        if let Some(ref mut source) = self.data_source {
            source.start().await.map_err(ExecutionError::DataSourceError)?;
        }

        // Start metrics collection
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);

        // Clone shutdown receiver for both tasks
        let shutdown_rx = Arc::new(tokio::sync::Mutex::new(shutdown_rx));
        let shutdown_rx_clone = shutdown_rx.clone();

        // Start main event processing loop
        let event_tx = self.event_tx.clone();
        let order_books = self.order_books.clone();
        let mut strategies = std::mem::take(&mut self.strategies);

        let _engine_task = tokio::spawn(async move {
            let mut rx = shutdown_rx_clone.lock().await;
            Self::run_event_loop(
                event_tx,
                order_books,
                &mut strategies,
                &mut rx,
            ).await
        });

        // Start metrics collection task
        let metrics_tx = self.event_tx.clone();
        let _metrics_task = tokio::spawn(async move {
            // Create a dummy receiver for metrics - we'll improve this later
            let (_tx, mut rx) = mpsc::channel(1);
            Self::run_metrics_loop(metrics_tx, &mut rx).await
        });

        self.state = EngineState::Running;
        self.metrics.start_time = Some(Instant::now());

        // Send startup event
        let startup_event = ExecutionEvent::system(
            SystemEvent::ExecutionStarted {
                mode: format!("{:?}", self.config.mode),
                config_summary: "Execution engine started".to_string(),
            },
            EventSource::System {
                component: "execution_engine".to_string(),
            }
        );

        let _ = self.event_tx.send(startup_event);

        info!("Execution engine started successfully");
        Ok(())
    }

    /// Stop the execution engine
    pub async fn stop(&mut self) -> Result<(), ExecutionError> {
        if self.state != EngineState::Running {
            return Err(ExecutionError::InvalidState(format!(
                "Cannot stop engine in state: {:?}", self.state
            )));
        }

        info!("Stopping execution engine");
        self.state = EngineState::Stopping;

        // Send shutdown signal
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(()).await;
        }

        // Stop data source
        if let Some(ref mut source) = self.data_source {
            source.stop().await.map_err(ExecutionError::DataSourceError)?;
        }

        // Shutdown strategies
        for strategy in &mut self.strategies {
            if let Err(e) = strategy.shutdown().await {
                warn!(
                    strategy = strategy.name(),
                    error = %e,
                    "Error shutting down strategy"
                );
            }
        }

        // Calculate final metrics
        let uptime = self.metrics.start_time.map_or(Duration::from_secs(0), |start| start.elapsed());

        // Send shutdown event
        let shutdown_event = ExecutionEvent::system(
            SystemEvent::ExecutionStopped {
                reason: StopReason::UserRequested,
                duration: uptime,
            },
            EventSource::System {
                component: "execution_engine".to_string(),
            }
        );

        let _ = self.event_tx.send(shutdown_event);

        self.state = EngineState::Stopped;
        info!(uptime = ?uptime, "Execution engine stopped");
        Ok(())
    }

    /// Get current engine state
    pub fn state(&self) -> &EngineState {
        &self.state
    }

    /// Get engine metrics
    pub fn metrics(&self) -> &EngineMetrics {
        &self.metrics
    }

    /// Get event stream for external consumers
    pub fn event_stream(&self) -> broadcast::Receiver<ExecutionEvent> {
        self.event_tx.subscribe()
    }

    /// Create data source based on configuration
    fn create_data_source(&self) -> Result<Box<dyn DataSource>, ExecutionError> {
        match &self.config.mode {
            super::config::ExecutionMode::RealTime { assets, .. } => {
                let source = WebSocketDataSource::new(
                    self.config.websocket.clone(),
                    assets.clone(),
                );
                Ok(Box::new(source))
            }
            super::config::ExecutionMode::Replay { filter_assets, .. } => {
                let source = ReplayDataSource::new(
                    self.config.replay.clone(),
                    filter_assets.clone(),
                );
                Ok(Box::new(source))
            }
            super::config::ExecutionMode::Simulation { asset_count, event_frequency } => {
                let source = SimulationDataSource::new(*asset_count, *event_frequency);
                Ok(Box::new(source))
            }
        }
    }

    /// Main event processing loop
    async fn run_event_loop(
        _event_tx: broadcast::Sender<ExecutionEvent>,
        _order_books: Arc<RwLock<HashMap<AssetId, OrderBook>>>,
        strategies: &mut [Box<dyn Strategy>],
        shutdown_rx: &mut mpsc::Receiver<()>,
    ) {
        info!("Starting event processing loop");

        let mut events_processed = 0;
        let start_time = Instant::now();

        loop {
            tokio::select! {
                // Check for shutdown signal
                _ = shutdown_rx.recv() => {
                    info!("Received shutdown signal, stopping event loop");
                    break;
                }

                // Process events (placeholder - would integrate with actual data source)
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    // TODO: Get events from data source and process them
                    events_processed += 1;

                    // Update order books and run strategies
                    for strategy in strategies.iter_mut() {
                        if !strategy.is_ready() {
                            continue;
                        }

                        // TODO: Process actual events through strategies
                    }
                }
            }
        }

        let duration = start_time.elapsed();
        info!(
            events_processed = events_processed,
            duration = ?duration,
            "Event processing loop finished"
        );
    }

    /// Metrics collection loop
    async fn run_metrics_loop(
        event_tx: broadcast::Sender<ExecutionEvent>,
        shutdown_rx: &mut mpsc::Receiver<()>,
    ) {
        info!("Starting metrics collection loop");

        let mut metrics_interval = interval(Duration::from_secs(5));

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!("Received shutdown signal, stopping metrics loop");
                    break;
                }

                _ = metrics_interval.tick() => {
                    // Collect and send metrics
                    let metrics_event = MetricsEvent {
                        events_per_second: 0.0, // TODO: Calculate actual rate
                        total_events: 0,        // TODO: Get actual count
                        active_connections: 1,  // TODO: Get actual connection count
                        orderbook_metrics: OrderBookMetrics {
                            active_books: 0,
                            average_spread: None,
                            crossed_markets: 0,
                            total_liquidity: rust_decimal::Decimal::ZERO,
                        },
                        memory_usage: MemoryMetrics {
                            heap_bytes: 0,      // TODO: Get actual memory usage
                            orderbook_bytes: 0,
                            buffer_bytes: 0,
                        },
                    };

                    let event = ExecutionEvent::metrics(
                        metrics_event,
                        EventSource::System {
                            component: "metrics_collector".to_string(),
                        }
                    );

                    let _ = event_tx.send(event);
                }
            }
        }

        info!("Metrics collection loop finished");
    }
}
*/

// /// Execution engine errors
// #[derive(Debug, thiserror::Error)]
// pub enum ExecutionError {
//     #[error("Invalid engine state: {0}")]
//     InvalidState(String),
//     #[error("Data source error: {0}")]
//     DataSourceError(#[from] super::sources::DataSourceError),
//     #[error("Strategy error: {0}")]
//     StrategyError(#[from] StrategyError),
// }

/*
#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::config::ExecutionConfig;

    #[tokio::test]
    async fn test_engine_lifecycle() {
        let config = ExecutionConfig::simulation(5);
        let mut engine = ExecutionEngine::new(config);

        // Should start in stopped state
        assert_eq!(engine.state(), &EngineState::Stopped);

        // Add default strategies
        engine.add_default_strategies();
        assert!(!engine.strategies.is_empty());

        // Start should succeed
        let result = engine.start().await;
        assert!(result.is_ok());
        assert_eq!(engine.state(), &EngineState::Running);

        // Stop should succeed
        let result = engine.stop().await;
        assert!(result.is_ok());
        assert_eq!(engine.state(), &EngineState::Stopped);
    }

    #[tokio::test]
    async fn test_invalid_state_transitions() {
        let config = ExecutionConfig::simulation(5);
        let mut engine = ExecutionEngine::new(config);

        // Cannot stop when not running
        let result = engine.stop().await;
        assert!(matches!(result, Err(ExecutionError::InvalidState(_))));

        // Start and then try to start again
        engine.add_default_strategies();
        assert!(engine.start().await.is_ok());

        let result = engine.start().await;
        assert!(matches!(result, Err(ExecutionError::InvalidState(_))));

        // Clean shutdown
        assert!(engine.stop().await.is_ok());
    }
}
*/
