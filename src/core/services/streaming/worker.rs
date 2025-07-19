//! Individual WebSocket worker that handles a subset of tokens

use dashmap::DashMap;
use rand::Rng;
use rust_decimal::Decimal;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::core::ws::{
    WsClient, WsConfig,
    parse_message, PolyEvent, WsMessage,
    OrderBook,
};

/// Status of a worker
#[derive(Debug, Clone, PartialEq)]
pub enum WorkerStatus {
    /// Worker is starting up
    Starting,
    /// Worker is connected and streaming
    Connected,
    /// Worker is attempting to reconnect
    Reconnecting { attempt: u32 },
    /// Worker has failed and stopped
    Failed { error: String },
    /// Worker is shutting down
    Stopping,
    /// Worker has stopped
    Stopped,
}

/// A WebSocket worker that handles a subset of tokens
pub struct StreamerWorker {
    /// Unique worker ID
    worker_id: usize,

    /// Configuration
    config: StreamerWorkerConfig,

    /// Current status
    status: Arc<RwLock<WorkerStatus>>,

    /// Tokens this worker is responsible for
    assigned_tokens: Arc<RwLock<Vec<String>>>,

    /// WebSocket client
    ws_client: Arc<Mutex<Option<WsClient>>>,

    /// Event broadcaster
    event_sender: broadcast::Sender<PolyEvent>,

    /// Order books for assigned tokens
    order_books: Arc<DashMap<String, OrderBook>>,

    /// Last trade prices
    last_trade_prices: Arc<DashMap<String, (Decimal, u64)>>,

    /// Background task handle
    task_handle: Arc<Mutex<Option<JoinHandle<()>>>>,

    /// Statistics
    stats: Arc<RwLock<WorkerStats>>,

    /// Shutdown signal
    shutdown_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
}

/// Configuration for a worker
#[derive(Debug, Clone)]
pub struct StreamerWorkerConfig {
    pub ws_config: WsConfig,
    pub auto_reconnect: bool,
    pub reconnect_delay_ms: u64,
    pub max_reconnect_delay_ms: u64,
    pub max_reconnect_attempts: u32,
    pub event_buffer_size: usize,
}

/// Worker statistics
#[derive(Debug, Default, Clone)]
pub struct WorkerStats {
    pub events_processed: u64,
    pub last_activity: Option<Instant>,
    pub connection_errors: u64,
    pub reconnection_attempts: u64,
    pub uptime_start: Option<Instant>,
    pub last_error: Option<String>,
}

impl StreamerWorker {
    /// Create a new worker
    pub fn new(worker_id: usize, config: StreamerWorkerConfig) -> Self {
        let (event_sender, _) = broadcast::channel(config.event_buffer_size);

        Self {
            worker_id,
            config,
            status: Arc::new(RwLock::new(WorkerStatus::Stopped)),
            assigned_tokens: Arc::new(RwLock::new(Vec::new())),
            ws_client: Arc::new(Mutex::new(None)),
            event_sender,
            order_books: Arc::new(DashMap::new()),
            last_trade_prices: Arc::new(DashMap::new()),
            task_handle: Arc::new(Mutex::new(None)),
            stats: Arc::new(RwLock::new(WorkerStats::default())),
            shutdown_tx: Arc::new(Mutex::new(None)),
        }
    }

    /// Start the worker with assigned tokens
    pub async fn start(&self, tokens: Vec<String>) -> Result<(), anyhow::Error> {
        info!(
            "Starting worker {} with {} tokens",
            self.worker_id,
            tokens.len()
        );

        // Update assigned tokens
        {
            let mut assigned = self.assigned_tokens.write().await;
            *assigned = tokens.clone();
        }

        // Update status
        {
            let mut status = self.status.write().await;
            *status = WorkerStatus::Starting;
        }

        // Initialize stats
        {
            let mut stats = self.stats.write().await;
            stats.uptime_start = Some(Instant::now());
        }

        // If no tokens assigned, just mark as connected but don't start WebSocket
        if tokens.is_empty() {
            let mut status = self.status.write().await;
            *status = WorkerStatus::Connected;
            info!(
                "Worker {} started with no tokens - ready for assignment",
                self.worker_id
            );
            return Ok(());
        }

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        *self.shutdown_tx.lock().await = Some(shutdown_tx);

        // Start the worker task
        let task = self.spawn_worker_task(tokens, shutdown_rx).await?;
        *self.task_handle.lock().await = Some(task);

        Ok(())
    }

    /// Stop the worker
    pub async fn stop(&self) {
        info!("Stopping worker {}", self.worker_id);

        // Update status
        {
            let mut status = self.status.write().await;
            *status = WorkerStatus::Stopping;
        }

        // Send shutdown signal
        if let Some(shutdown_tx) = self.shutdown_tx.lock().await.take() {
            let _ = shutdown_tx.send(());
        }

        // Wait for task to finish
        if let Some(task) = self.task_handle.lock().await.take() {
            let _ = task.await;
        }

        // Close WebSocket client
        {
            let mut client = self.ws_client.lock().await;
            if let Some(ws_client) = client.take() {
                let _ = ws_client.disconnect();
            }
        }

        // Update status
        {
            let mut status = self.status.write().await;
            *status = WorkerStatus::Stopped;
        }

        info!("Worker {} stopped", self.worker_id);
    }

    /// Update assigned tokens
    pub async fn update_tokens(&self, tokens: Vec<String>) -> Result<(), anyhow::Error> {
        info!(
            "Updating worker {} tokens: {} -> {}",
            self.worker_id,
            self.assigned_tokens.read().await.len(),
            tokens.len()
        );

        let mut assigned = self.assigned_tokens.write().await;
        *assigned = tokens.clone();

        // If worker is running, restart with new tokens
        let status = self.status.read().await.clone();
        if matches!(status, WorkerStatus::Connected) {
            drop(assigned); // Release the lock before restarting
            self.restart_with_tokens(tokens).await?;
        }

        Ok(())
    }

    /// Get worker status
    pub async fn get_status(&self) -> WorkerStatus {
        self.status.read().await.clone()
    }

    /// Get assigned tokens
    pub async fn get_assigned_tokens(&self) -> Vec<String> {
        self.assigned_tokens.read().await.clone()
    }

    /// Get event receiver
    pub fn subscribe_events(&self) -> broadcast::Receiver<PolyEvent> {
        self.event_sender.subscribe()
    }

    /// Get order book for a token
    pub async fn get_order_book(&self, token_id: &str) -> Option<OrderBook> {
        self.order_books.get(token_id).map(|entry| entry.clone())
    }

    /// Get last trade price
    pub async fn get_last_trade_price(&self, token_id: &str) -> Option<(Decimal, u64)> {
        self.last_trade_prices.get(token_id).map(|entry| *entry)
    }

    /// Get worker statistics
    pub async fn get_stats(&self) -> WorkerStats {
        self.stats.read().await.clone()
    }


    /// Restart worker with new tokens
    async fn restart_with_tokens(&self, tokens: Vec<String>) -> Result<(), anyhow::Error> {
        // Stop current task
        if let Some(shutdown_tx) = self.shutdown_tx.lock().await.take() {
            let _ = shutdown_tx.send(());
        }

        if let Some(task) = self.task_handle.lock().await.take() {
            let _ = task.await;
        }

        // Start with new tokens
        self.start(tokens).await
    }

    /// Spawn the main worker task
    async fn spawn_worker_task(
        &self,
        tokens: Vec<String>,
        mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    ) -> Result<JoinHandle<()>, anyhow::Error> {
        let worker_id = self.worker_id;
        let config = self.config.clone();
        let status = Arc::clone(&self.status);
        let event_sender = self.event_sender.clone();
        let order_books = Arc::clone(&self.order_books);
        let last_trade_prices = Arc::clone(&self.last_trade_prices);
        let stats = Arc::clone(&self.stats);
        let ws_client = Arc::clone(&self.ws_client);

        let task = tokio::spawn(async move {
            info!("Worker {} task started", worker_id);

            let mut reconnect_attempts = 0;
            let mut reconnect_delay = config.reconnect_delay_ms;

            loop {
                // Check for shutdown signal
                if let Ok(()) = shutdown_rx.try_recv() {
                    info!("Worker {} received shutdown signal", worker_id);
                    break;
                }

                // Try to connect and stream
                match Self::connect_and_stream(
                    worker_id,
                    &config,
                    &tokens,
                    &status,
                    &event_sender,
                    &order_books,
                    &last_trade_prices,
                    &stats,
                    &ws_client,
                    &mut shutdown_rx,
                )
                .await
                {
                    Ok(()) => {
                        // Connection closed normally
                        info!("Worker {} connection closed normally", worker_id);
                        break;
                    }
                    Err(e) => {
                        error!("Worker {} connection failed: {}", worker_id, e);

                        // Update stats
                        {
                            let mut stats = stats.write().await;
                            stats.connection_errors += 1;
                            stats.last_error = Some(e.to_string());
                        }

                        // Check if this is a rate limiting error (429)
                        let is_rate_limited = e.to_string().contains("429")
                            || e.to_string().contains("Too Many Requests");

                        if !config.auto_reconnect
                            || reconnect_attempts >= config.max_reconnect_attempts
                        {
                            error!(
                                "Worker {} giving up after {} attempts",
                                worker_id, reconnect_attempts
                            );
                            let mut status = status.write().await;
                            *status = WorkerStatus::Failed {
                                error: e.to_string(),
                            };
                            break;
                        }

                        // Attempt to reconnect
                        reconnect_attempts += 1;
                        {
                            let mut status = status.write().await;
                            *status = WorkerStatus::Reconnecting {
                                attempt: reconnect_attempts,
                            };
                        }

                        {
                            let mut stats = stats.write().await;
                            stats.reconnection_attempts += 1;
                        }

                        // Use longer delays for rate limiting errors with jitter
                        if is_rate_limited {
                            let base_delay = reconnect_delay.max(5000); // At least 5 seconds for 429 errors
                            let jitter = rand::rng().random_range(0..=2000); // 0-2 second jitter
                            let rate_limit_delay = base_delay + jitter;
                            warn!(
                                "Worker {} rate limited - reconnecting in {}ms (attempt {})",
                                worker_id, rate_limit_delay, reconnect_attempts
                            );
                            tokio::time::sleep(Duration::from_millis(rate_limit_delay)).await;
                        } else {
                            let jitter = rand::rng().random_range(0..=500); // 0-500ms jitter
                            let delay_with_jitter = reconnect_delay + jitter;
                            warn!(
                                "Worker {} reconnecting in {}ms (attempt {})",
                                worker_id, delay_with_jitter, reconnect_attempts
                            );
                            tokio::time::sleep(Duration::from_millis(delay_with_jitter)).await;
                        }

                        // Check for shutdown after wait
                        if let Ok(()) = shutdown_rx.try_recv() {
                            info!("Worker {} shutdown during reconnect wait", worker_id);
                            break;
                        }

                        // Exponential backoff (faster for rate limit errors)
                        if is_rate_limited {
                            reconnect_delay =
                                (reconnect_delay * 3).min(config.max_reconnect_delay_ms);
                        } else {
                            reconnect_delay =
                                (reconnect_delay * 2).min(config.max_reconnect_delay_ms);
                        }
                    }
                }
            }

            info!("Worker {} task ended", worker_id);
        });

        Ok(task)
    }

    /// Connect to WebSocket and stream events
    async fn connect_and_stream(
        worker_id: usize,
        config: &StreamerWorkerConfig,
        tokens: &[String],
        status: &Arc<RwLock<WorkerStatus>>,
        event_sender: &broadcast::Sender<PolyEvent>,
        order_books: &Arc<DashMap<String, OrderBook>>,
        last_trade_prices: &Arc<DashMap<String, (Decimal, u64)>>,
        stats: &Arc<RwLock<WorkerStats>>,
        ws_client: &Arc<Mutex<Option<WsClient>>>,
        shutdown_rx: &mut tokio::sync::oneshot::Receiver<()>,
    ) -> Result<(), anyhow::Error> {
        // Create WebSocket client
        let client = WsClient::new_market(config.ws_config.clone()).await?;

        // Subscribe to tokens
        client.subscribe_market(tokens.to_vec())?;

        // Store client
        {
            let mut ws_client = ws_client.lock().await;
            *ws_client = Some(client);
        }

        // Update status to connected
        {
            let mut status = status.write().await;
            *status = WorkerStatus::Connected;
        }

        info!(
            "Worker {} connected and subscribed to {} tokens",
            worker_id,
            tokens.len()
        );

        // Get message receiver
        let client = ws_client.lock().await;
        let mut messages = client.as_ref().unwrap().messages();
        drop(client); // Release the lock

        // Process messages
        loop {
            // Check for shutdown
            if let Ok(()) = shutdown_rx.try_recv() {
                info!("Worker {} received shutdown during streaming", worker_id);
                break;
            }

            tokio::select! {
                // Process WebSocket messages
                msg_result = messages.recv() => {
                    match msg_result {
                        Ok(ws_message) => {
                            Self::handle_message(
                                worker_id,
                                ws_message,
                                event_sender,
                                order_books,
                                last_trade_prices,
                                stats,
                                config,
                            ).await;
                        }
                        Err(e) => {
                            error!("Worker {} message receive error: {}", worker_id, e);
                            return Err(anyhow::anyhow!("Message receive error: {}", e));
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Handle a WebSocket message
    async fn handle_message(
        worker_id: usize,
        ws_message: WsMessage,
        event_sender: &broadcast::Sender<PolyEvent>,
        order_books: &Arc<DashMap<String, OrderBook>>,
        last_trade_prices: &Arc<DashMap<String, (Decimal, u64)>>,
        stats: &Arc<RwLock<WorkerStats>>,
        config: &StreamerWorkerConfig,
    ) {
        debug!(
            "Worker {} received message: {:?}",
            worker_id, ws_message.event_type
        );

        match parse_message(&ws_message) {
            Ok(events) => {
                for event in events {
                    // Update local state
                    match &event {
                        PolyEvent::Book {
                            asset_id,
                            market,
                            timestamp,
                            bids,
                            asks,
                            hash,
                        } => {
                            let mut book = order_books
                                .entry(asset_id.clone())
                                .or_insert_with(|| OrderBook::new(asset_id.clone()));
                            
                            // Get the skip_hash_verification and quiet_hash_mismatch flags from config
                            let skip_hash = config.ws_config.skip_hash_verification;
                            let quiet_hash_mismatch = config.ws_config.quiet_hash_mismatch;
                            
                            // Apply snapshot based on hash verification setting
                            if skip_hash {
                                book.replace_with_snapshot_no_hash(
                                    market.clone(),
                                    *timestamp,
                                    bids.clone(),
                                    asks.clone(),
                                );
                                debug!(
                                    "Order book snapshot applied (no hash verification) for {}",
                                    asset_id
                                );
                            } else {
                                if let Err(e) = book.replace_with_snapshot(
                                    market.clone(),
                                    *timestamp,
                                    bids.clone(),
                                    asks.clone(),
                                    hash.clone(),
                                ) {
                                    if !quiet_hash_mismatch {
                                        warn!(
                                            "Failed to apply book snapshot with hash verification: {}",
                                            e
                                        );
                                    }
                                    // Fallback to no-hash method if verification fails
                                    book.replace_with_snapshot_no_hash(
                                        market.clone(),
                                        *timestamp,
                                        bids.clone(),
                                        asks.clone(),
                                    );
                                    debug!(
                                        "Order book snapshot applied (fallback no hash) for {}",
                                        asset_id
                                    );
                                } else {
                                    debug!("Order book snapshot applied successfully for {}", asset_id);
                                }
                            }
                        }
                        PolyEvent::PriceChange {
                            asset_id,
                            side,
                            price,
                            size,
                            ..
                        } => {
                            if let Some(mut book) = order_books.get_mut(asset_id) {
                                book.apply_price_change_no_hash(*side, *price, *size);
                            }
                        }
                        PolyEvent::LastTradePrice {
                            asset_id,
                            price,
                            timestamp,
                        } => {
                            last_trade_prices.insert(asset_id.clone(), (*price, *timestamp));
                        }
                        _ => {}
                    }

                    // Broadcast event
                    match event_sender.send(event) {
                        Ok(_) => {
                            // Update stats
                            let mut stats = stats.write().await;
                            stats.events_processed += 1;
                            stats.last_activity = Some(Instant::now());
                        }
                        Err(_) => {
                            debug!("Worker {} no event receivers", worker_id);
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Worker {} failed to parse message: {}", worker_id, e);
            }
        }
    }
}
