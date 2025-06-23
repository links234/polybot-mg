//! Strategy system for processing execution events
//!
//! Provides a flexible framework for implementing different
//! strategies that can process and react to market events.

// COMMENTED OUT: This module is not currently used and depends on other commented code
// Will be re-enabled when the execution engine is fully implemented

/*
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use rust_decimal::prelude::ToPrimitive;

use tracing::{debug, info, error};

use super::events::{ExecutionEvent, EventData, MarketEvent};
use super::config::AssetId;
use crate::ws::OrderBook;

/// Strategy trait for processing execution events
#[async_trait]
pub trait Strategy: Send + Sync {
    /// Process an execution event
    async fn process_event(&mut self, event: &ExecutionEvent) -> Result<StrategyResult, StrategyError>;

    /// Get strategy name
    fn name(&self) -> &str;


    /// Initialize strategy
    async fn initialize(&mut self) -> Result<(), StrategyError>;

    /// Cleanup strategy resources
    async fn shutdown(&mut self) -> Result<(), StrategyError>;

    /// Get strategy metrics
    fn metrics(&self) -> StrategyMetrics;

    /// Check if strategy is ready to process events
    fn is_ready(&self) -> bool;
}

/// Strategy processing result
#[derive(Debug, Clone)]
pub enum StrategyResult {
    /// No action taken
    NoAction,
    /// Action taken with description
    Action(ActionResult),
    /// Multiple actions taken
    MultipleActions(Vec<ActionResult>),
    }

/// Individual action result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    /// Action type
    pub action_type: ActionType,
    /// Asset involved (if applicable)
    pub asset_id: Option<AssetId>,
    /// Action description
    pub description: String,
    /// Action metadata
    pub metadata: HashMap<String, String>,
    /// Action timestamp
    pub timestamp: std::time::SystemTime,
}

/// Types of actions a strategy can take
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionType {
    /// Log information
    Log,
    /// Send alert/notification
    Alert,
    /// Place order (if trading enabled)
    PlaceOrder,
    /// Cancel order
    CancelOrder,
    /// Update strategy parameters
    UpdateParameters,
    /// Save data/state
    SaveData,
    /// Custom action type
    Custom(String),
}

/// Strategy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    /// Strategy name
    pub name: String,
    /// Whether strategy is enabled
    pub enabled: bool,
    /// Assets to process (None = all assets)
    pub filter_assets: Option<Vec<AssetId>>,
    /// Event types to process
    pub event_types: Vec<EventTypeFilter>,
    /// Strategy parameters
    pub parameters: HashMap<String, StrategyParameter>,
    /// Output configuration
    pub output: StrategyOutputConfig,
}

/// Event type filters
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventTypeFilter {
    Market,
    User,
    System,
    Metrics,
    All,
}

/// Strategy parameter value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StrategyParameter {
    String(String),
    Number(f64),
    Boolean(bool),
    Duration(Duration),
    List(Vec<String>),
}

/// Strategy output configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyOutputConfig {
    /// Enable console output
    pub console: bool,
    /// Log file path
    pub log_file: Option<String>,
    /// Alert configuration
    pub alerts: AlertConfig,
}

/// Alert configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertConfig {
    /// Enable alerts
    pub enabled: bool,
    /// Alert thresholds
    pub thresholds: HashMap<String, f64>,
    /// Alert destinations
    pub destinations: Vec<AlertDestination>,
}

/// Alert destinations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertDestination {
    Console,
    File(String),
    Webhook(String),
    Email(String),
}

/// Strategy metrics
#[derive(Debug, Clone)]
pub struct StrategyMetrics {
    /// Events processed
    pub events_processed: usize,
    /// Actions taken
    pub actions_taken: usize,
    /// Errors encountered
    pub _errors: usize,
    /// Processing duration
    pub total_processing_time: Duration,
    /// Average processing time per event
    pub avg_processing_time: Duration,
    /// Last event processed
    pub last_event_time: Option<Instant>,
}

impl Default for StrategyMetrics {
    fn default() -> Self {
        Self {
            events_processed: 0,
            actions_taken: 0,
            _errors: 0,
            total_processing_time: Duration::from_secs(0),
            avg_processing_time: Duration::from_secs(0),
            last_event_time: None,
        }
    }
}

/// Strategy errors
#[derive(Debug, thiserror::Error)]
pub enum StrategyError {
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    #[error("Processing error: {0}")]
    ProcessingError(String),
    #[error("Strategy disabled")]
    Disabled,
}

/// Market analysis strategy - analyzes spread and liquidity
pub struct MarketAnalysisStrategy {
    config: StrategyConfig,
    metrics: StrategyMetrics,
    order_books: Arc<RwLock<HashMap<AssetId, OrderBook>>>,
    spread_threshold: f64,
    liquidity_threshold: f64,
    is_ready: bool,
}

impl MarketAnalysisStrategy {
    pub fn new(config: StrategyConfig) -> Self {
        let spread_threshold = config.parameters.get("spread_threshold")
            .and_then(|p| match p {
                StrategyParameter::Number(n) => Some(*n),
                _ => None,
            })
            .unwrap_or(2.0); // 2% default

        let liquidity_threshold = config.parameters.get("liquidity_threshold")
            .and_then(|p| match p {
                StrategyParameter::Number(n) => Some(*n),
                _ => None,
            })
            .unwrap_or(1000.0); // $1000 default

        info!(
            strategy = %config.name,
            spread_threshold = spread_threshold,
            liquidity_threshold = liquidity_threshold,
            "Creating market analysis strategy"
        );

        Self {
            config,
            metrics: StrategyMetrics::default(),
            order_books: Arc::new(RwLock::new(HashMap::new())),
            spread_threshold,
            liquidity_threshold,
            is_ready: false,
        }
    }

    async fn analyze_spread(&self, asset_id: &AssetId, book: &OrderBook) -> Option<ActionResult> {
        let best_bid = book.best_bid()?;
        let best_ask = book.best_ask()?;

        let spread = best_ask.price - best_bid.price;
        let mid_price = (best_bid.price + best_ask.price) / rust_decimal::Decimal::from(2);
        let spread_pct = (spread / mid_price * rust_decimal::Decimal::from(100))
            .to_f64().unwrap_or(0.0);

        if spread_pct > self.spread_threshold {
            debug!(
                asset_id = %asset_id.as_str(),
                spread_pct = spread_pct,
                threshold = self.spread_threshold,
                "Wide spread detected"
            );

            Some(ActionResult {
                action_type: ActionType::Alert,
                asset_id: Some(asset_id.clone()),
                description: format!(
                    "Wide spread detected: {:.2}% (threshold: {:.2}%)",
                    spread_pct, self.spread_threshold
                ),
                metadata: {
                    let mut meta = HashMap::new();
                    meta.insert("spread_percentage".to_string(), spread_pct.to_string());
                    meta.insert("bid_price".to_string(), best_bid.price.to_string());
                    meta.insert("ask_price".to_string(), best_ask.price.to_string());
                    meta.insert("mid_price".to_string(), mid_price.to_string());
                    meta
                },
                timestamp: std::time::SystemTime::now(),
            })
        } else {
            None
        }
    }

    async fn analyze_liquidity(&self, asset_id: &AssetId, book: &OrderBook) -> Option<ActionResult> {
        let total_bid_liquidity = book.bids.values().sum::<rust_decimal::Decimal>();
        let total_ask_liquidity = book.asks.values().sum::<rust_decimal::Decimal>();
        let total_liquidity = (total_bid_liquidity + total_ask_liquidity).to_f64().unwrap_or(0.0);

        if total_liquidity < self.liquidity_threshold {
            debug!(
                asset_id = %asset_id.as_str(),
                total_liquidity = total_liquidity,
                threshold = self.liquidity_threshold,
                "Low liquidity detected"
            );

            Some(ActionResult {
                action_type: ActionType::Alert,
                asset_id: Some(asset_id.clone()),
                description: format!(
                    "Low liquidity: ${:.2} (threshold: ${:.2})",
                    total_liquidity, self.liquidity_threshold
                ),
                metadata: {
                    let mut meta = HashMap::new();
                    meta.insert("total_liquidity".to_string(), total_liquidity.to_string());
                    meta.insert("bid_liquidity".to_string(), total_bid_liquidity.to_string());
                    meta.insert("ask_liquidity".to_string(), total_ask_liquidity.to_string());
                    meta
                },
                timestamp: std::time::SystemTime::now(),
            })
        } else {
            None
        }
    }
}

#[async_trait]
impl Strategy for MarketAnalysisStrategy {
    async fn process_event(&mut self, event: &ExecutionEvent) -> Result<StrategyResult, StrategyError> {
        if !self.config.enabled {
            return Err(StrategyError::Disabled);
        }

        let start_time = Instant::now();
        self.metrics.events_processed += 1;
        self.metrics.last_event_time = Some(start_time);

        let mut actions = Vec::new();

        if let EventData::Market(market_event) = &event.data {
            match market_event {
                MarketEvent::OrderBookSnapshot { asset_id, bids, asks, .. } => {
                    // Update our order book copy
                    let mut order_book = OrderBook::new(asset_id.as_str().to_string());
                    order_book.replace_with_snapshot_no_hash(bids.clone(), asks.clone());

                    // Analyze the order book
                    if let Some(action) = self.analyze_spread(asset_id, &order_book).await {
                        actions.push(action);
                    }

                    if let Some(action) = self.analyze_liquidity(asset_id, &order_book).await {
                        actions.push(action);
                    }

                    // Store updated order book
                    {
                        let mut books = self.order_books.write().await;
                        books.insert(asset_id.clone(), order_book);
                    }
                }
                MarketEvent::PriceChange { asset_id, side, price, size, .. } => {
                    // Update order book with price change
                    {
                        let mut books = self.order_books.write().await;
                        if let Some(book) = books.get_mut(asset_id) {
                            // Apply the price change
                            let _ = book.apply_price_change(*side, *price, *size, "".to_string());

                            // Re-analyze if significant change
                            if *size == rust_decimal::Decimal::ZERO || *size > rust_decimal::Decimal::from(100) {
                                if let Some(action) = self.analyze_spread(asset_id, book).await {
                                    actions.push(action);
                                }
                            }
                        }
                    }
                }
                _ => {
                    // Other market events - log for now
                    debug!(
                        strategy = %self.config.name,
                        event_type = ?market_event,
                        "Processed market event"
                    );
                }
            }
        }

        // Update metrics
        let processing_time = start_time.elapsed();
        self.metrics.total_processing_time += processing_time;
        self.metrics.avg_processing_time = self.metrics.total_processing_time / self.metrics.events_processed as u32;

        if !actions.is_empty() {
            self.metrics.actions_taken += actions.len();
            Ok(StrategyResult::MultipleActions(actions))
        } else {
            Ok(StrategyResult::NoAction)
        }
    }

    fn name(&self) -> &str {
        &self.config.name
    }


    async fn initialize(&mut self) -> Result<(), StrategyError> {
        info!(strategy = %self.config.name, "Initializing market analysis strategy");
        self.is_ready = true;
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), StrategyError> {
        info!(strategy = %self.config.name, "Shutting down market analysis strategy");
        self.is_ready = false;
        Ok(())
    }

    fn metrics(&self) -> StrategyMetrics {
        self.metrics.clone()
    }

    fn is_ready(&self) -> bool {
        self.is_ready
    }
}

/// Simple logging strategy
pub struct LoggingStrategy {
    config: StrategyConfig,
    metrics: StrategyMetrics,
    is_ready: bool,
}

impl LoggingStrategy {
    pub fn new(config: StrategyConfig) -> Self {
        Self {
            config,
            metrics: StrategyMetrics::default(),
            is_ready: false,
        }
    }
}

#[async_trait]
impl Strategy for LoggingStrategy {
    async fn process_event(&mut self, event: &ExecutionEvent) -> Result<StrategyResult, StrategyError> {
        if !self.config.enabled {
            return Err(StrategyError::Disabled);
        }

        let start_time = Instant::now();
        self.metrics.events_processed += 1;
        self.metrics.last_event_time = Some(start_time);

        // Log event information
        match &event.data {
            EventData::Market(market_event) => {
                info!(
                    strategy = %self.config.name,
                    event_id = %event.id.0,
                    event_type = "market",
                    asset_id = ?market_event,
                    "Processed market event"
                );
            }
            EventData::User(user_event) => {
                info!(
                    strategy = %self.config.name,
                    event_id = %event.id.0,
                    event_type = "user",
                    asset_id = ?user_event,
                    "Processed user event"
                );
            }
            EventData::System(_) => {
                info!(
                    strategy = %self.config.name,
                    event_id = %event.id.0,
                    event_type = "system",
                    "Processed system event"
                );
            }
            EventData::Metrics(_) => {
                debug!(
                    strategy = %self.config.name,
                    event_id = %event.id.0,
                    event_type = "metrics",
                    "Processed metrics event"
                );
            }
        }

        // Update metrics
        let processing_time = start_time.elapsed();
        self.metrics.total_processing_time += processing_time;
        if self.metrics.events_processed > 0 {
            self.metrics.avg_processing_time = self.metrics.total_processing_time / self.metrics.events_processed as u32;
        }

        Ok(StrategyResult::NoAction)
    }

    fn name(&self) -> &str {
        &self.config.name
    }


    async fn initialize(&mut self) -> Result<(), StrategyError> {
        info!(strategy = %self.config.name, "Initializing logging strategy");
        self.is_ready = true;
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), StrategyError> {
        info!(strategy = %self.config.name, "Shutting down logging strategy");
        self.is_ready = false;
        Ok(())
    }

    fn metrics(&self) -> StrategyMetrics {
        self.metrics.clone()
    }

    fn is_ready(&self) -> bool {
        self.is_ready
    }
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            enabled: true,
            filter_assets: None,
            event_types: vec![EventTypeFilter::All],
            parameters: HashMap::new(),
            output: StrategyOutputConfig {
                console: true,
                log_file: None,
                alerts: AlertConfig {
                    enabled: false,
                    thresholds: HashMap::new(),
                    destinations: vec![AlertDestination::Console],
                },
            },
        }
    }
}

impl StrategyConfig {
    /// Create a market analysis strategy config
    pub fn market_analysis() -> Self {
        let mut config = Self::default();
        config.name = "market_analysis".to_string();
        config.event_types = vec![EventTypeFilter::Market];

        // Add default parameters
        config.parameters.insert(
            "spread_threshold".to_string(),
            StrategyParameter::Number(2.0)
        );
        config.parameters.insert(
            "liquidity_threshold".to_string(),
            StrategyParameter::Number(1000.0)
        );

        config
    }

    /// Create a logging strategy config
    pub fn logging() -> Self {
        let mut config = Self::default();
        config.name = "logging".to_string();
        config.event_types = vec![EventTypeFilter::All];
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::events::{EventSource, MarketEvent, FeedType};
    use crate::execution::orderbook::PriceLevel;
    use crate::ws::Side;

    #[tokio::test]
    async fn test_market_analysis_strategy() {
        let config = StrategyConfig::market_analysis();
        let mut strategy = MarketAnalysisStrategy::new(config);

        assert!(strategy.initialize().await.is_ok());
        assert!(strategy.is_ready());

        // Create a test event
        let market_event = MarketEvent::OrderBookSnapshot {
            asset_id: AssetId::from("test_asset"),
            bids: vec![PriceLevel::new(rust_decimal::Decimal::new(4000, 4), rust_decimal::Decimal::new(100, 0))],
            asks: vec![PriceLevel::new(rust_decimal::Decimal::new(6000, 4), rust_decimal::Decimal::new(100, 0))],
            hash: "test_hash".to_string(),
        };

        let event = ExecutionEvent::market(
            market_event,
            EventSource::WebSocket {
                connection_id: "test".to_string(),
                feed_type: FeedType::Market,
            }
        );

        let result = strategy.process_event(&event).await;
        assert!(result.is_ok());

        // Should detect wide spread (40% vs 2% threshold)
        match result.unwrap() {
            StrategyResult::MultipleActions(actions) => {
                assert!(!actions.is_empty());
                assert!(actions.iter().any(|a| a.action_type == ActionType::Alert));
            }
            _ => panic!("Expected multiple actions"),
        }

        assert!(strategy.shutdown().await.is_ok());
    }

    #[tokio::test]
    async fn test_logging_strategy() {
        let config = StrategyConfig::logging();
        let mut strategy = LoggingStrategy::new(config);

        assert!(strategy.initialize().await.is_ok());

        let market_event = MarketEvent::Trade {
            asset_id: AssetId::from("test_asset"),
            price: rust_decimal::Decimal::new(5000, 4),
            size: rust_decimal::Decimal::new(100, 0),
            side: Side::Buy,
            trade_id: Some("trade_123".to_string()),
        };

        let event = ExecutionEvent::market(
            market_event,
            EventSource::WebSocket {
                connection_id: "test".to_string(),
                feed_type: FeedType::Market,
            }
        );

        let result = strategy.process_event(&event).await;
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), StrategyResult::NoAction));

        let metrics = strategy.metrics();
        assert_eq!(metrics.events_processed, 1);
    }
}
*/

// Placeholder exports to avoid breaking other modules
#[derive(Debug, Clone)]
pub struct StrategyMetrics;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct StrategyError;
