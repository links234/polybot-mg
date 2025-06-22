//! Main streaming service implementation

use async_trait::async_trait;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use super::{
    config::StreamingServiceConfig,
    event_aggregator::EventAggregator,
    token_distributor::{DistributionUpdate, TokenDistributor},
    traits::{StreamingServiceTrait, StreamingStats, WorkerStatus as TraitWorkerStatus},
    worker::{StreamerWorker, StreamerWorkerConfig, WorkerStatus},
};
use crate::ws::{events::PolyEvent, state::OrderBook};

/// Main streaming service that manages multiple WebSocket workers
pub struct StreamingService {
    /// Service configuration
    config: StreamingServiceConfig,

    /// Token distributor
    distributor: Arc<Mutex<TokenDistributor>>,

    /// Event aggregator
    aggregator: Arc<EventAggregator>,

    /// Active workers: worker_id -> worker
    workers: Arc<RwLock<HashMap<usize, Arc<StreamerWorker>>>>,

    /// Service statistics
    stats: Arc<RwLock<StreamingStats>>,

    /// Service start time
    start_time: Instant,

    /// Health check task
    health_check_task: Arc<Mutex<Option<JoinHandle<()>>>>,

    /// Statistics collection task
    stats_task: Arc<Mutex<Option<JoinHandle<()>>>>,

    /// Whether the service is running
    is_running: Arc<RwLock<bool>>,
}

impl StreamingService {
    /// Create a new streaming service
    pub fn new(config: StreamingServiceConfig) -> Arc<Self> {
        let distributor = Arc::new(Mutex::new(TokenDistributor::new(config.tokens_per_worker)));
        let aggregator = Arc::new(EventAggregator::new(config.event_buffer_size));

        Arc::new(Self {
            config,
            distributor,
            aggregator,
            workers: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(StreamingStats::default())),
            start_time: Instant::now(),
            health_check_task: Arc::new(Mutex::new(None)),
            stats_task: Arc::new(Mutex::new(None)),
            is_running: Arc::new(RwLock::new(false)),
        })
    }

    /// Start the streaming service
    pub async fn start(self: &Arc<Self>) -> Result<(), anyhow::Error> {
        info!("Starting StreamingService");

        let mut is_running = self.is_running.write().await;
        if *is_running {
            warn!("StreamingService is already running");
            return Ok(());
        }

        *is_running = true;

        // Start the event aggregator
        self.aggregator.start().await;

        // Start background tasks
        self.start_health_check_task().await;
        self.start_stats_collection_task().await;

        info!("StreamingService started successfully");
        Ok(())
    }

    /// Apply distribution updates by creating/removing/updating workers
    async fn apply_distribution_update(
        &self,
        update: DistributionUpdate,
    ) -> Result<(), anyhow::Error> {
        if !update.has_changes() {
            return Ok(());
        }

        info!(
            "Applying distribution update: {} workers to add, {} to remove, {} to shutdown",
            update.workers_to_add.len(),
            update.workers_to_remove.len(),
            update.workers_to_shutdown.len()
        );

        let mut workers = self.workers.write().await;

        // Handle workers that need new tokens with connection throttling
        let new_workers_count = update
            .workers_to_add
            .iter()
            .filter(|(worker_id, _)| !workers.contains_key(worker_id))
            .count();

        if new_workers_count > 0 {
            info!("🚦 Rate limiting enabled: {} new workers will connect with max {} concurrent connections and {}ms delays",
                  new_workers_count, self.config.max_concurrent_connections, self.config.worker_connection_delay_ms);
        }

        let connection_semaphore = Arc::new(tokio::sync::Semaphore::new(
            self.config.max_concurrent_connections,
        ));
        let mut new_worker_tasks = Vec::new();

        for (worker_id, tokens) in update.workers_to_add {
            if let Some(worker) = workers.get(&worker_id) {
                // Update existing worker (no connection needed)
                debug!("Updating worker {} with {} tokens", worker_id, tokens.len());
                worker.update_tokens(tokens).await?;
            } else {
                // Create new worker with throttling
                debug!(
                    "Creating new worker {} with {} tokens",
                    worker_id,
                    tokens.len()
                );

                let worker = self.create_worker(worker_id).await?;
                let aggregator = Arc::clone(&self.aggregator);
                let delay_ms = self.config.worker_connection_delay_ms;
                let permit = Arc::clone(&connection_semaphore).acquire_owned().await?;

                // Start worker in a controlled manner
                let task = tokio::spawn(async move {
                    // Hold the permit to limit concurrent connections
                    let _permit = permit;

                    match worker.start(tokens).await {
                        Ok(_) => {
                            // Add to aggregator
                            let receiver = worker.subscribe_events();
                            aggregator.add_worker(worker_id, receiver).await;
                            info!("✅ Worker {} started and connected successfully", worker_id);
                            Ok((worker_id, worker))
                        }
                        Err(e) => {
                            error!("❌ Worker {} failed to start: {}", worker_id, e);
                            Err((worker_id, e))
                        }
                    }
                });

                new_worker_tasks.push(task);

                // Add delay between connection attempts only for new workers
                if new_worker_tasks.len() < new_workers_count {
                    info!(
                        "Delaying {}ms before next worker connection attempt",
                        delay_ms
                    );
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                }
            }
        }

        // Wait for all new workers to complete
        for task in new_worker_tasks {
            match task.await? {
                Ok((worker_id, worker)) => {
                    workers.insert(worker_id, worker);
                    info!("Worker {} added to worker pool", worker_id);
                }
                Err((worker_id, e)) => {
                    warn!("Failed to start worker {}: {}", worker_id, e);
                    // Continue with other workers even if one fails
                }
            }
        }

        // Handle workers that need tokens removed
        for (worker_id, tokens) in update.workers_to_remove {
            if let Some(worker) = workers.get(&worker_id) {
                debug!("Removing {} tokens from worker {}", tokens.len(), worker_id);
                let current_tokens = worker.get_assigned_tokens().await;
                let remaining_tokens: Vec<String> = current_tokens
                    .into_iter()
                    .filter(|t| !tokens.contains(t))
                    .collect();
                worker.update_tokens(remaining_tokens).await?;
            }
        }

        // Handle workers to shutdown
        for worker_id in update.workers_to_shutdown {
            if let Some(worker) = workers.remove(&worker_id) {
                debug!("Shutting down worker {}", worker_id);
                worker.stop().await;
                self.aggregator.remove_worker(worker_id).await;
                info!("Shut down worker {}", worker_id);
            }
        }

        Ok(())
    }

    /// Create a new worker
    async fn create_worker(&self, worker_id: usize) -> Result<Arc<StreamerWorker>, anyhow::Error> {
        let worker_config = StreamerWorkerConfig {
            ws_config: self.config.ws_config.clone(),
            auto_reconnect: self.config.auto_reconnect,
            reconnect_delay_ms: self.config.reconnect_delay_ms,
            max_reconnect_delay_ms: self.config.max_reconnect_delay_ms,
            max_reconnect_attempts: self.config.max_reconnect_attempts,
            event_buffer_size: self.config.worker_event_buffer_size,
        };

        Ok(Arc::new(StreamerWorker::new(worker_id, worker_config)))
    }

    /// Start health check background task
    async fn start_health_check_task(&self) {
        let workers = Arc::clone(&self.workers);
        let is_running = Arc::clone(&self.is_running);
        let interval_secs = self.config.health_check_interval_secs;

        let task = tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));

            while *is_running.read().await {
                interval.tick().await;

                let workers = workers.read().await;
                let mut unhealthy_workers = Vec::new();

                for (worker_id, worker) in workers.iter() {
                    let status = worker.get_status().await;
                    if matches!(status, WorkerStatus::Failed { .. }) {
                        unhealthy_workers.push(*worker_id);
                    }
                }

                if !unhealthy_workers.is_empty() {
                    warn!(
                        "Found {} unhealthy workers: {:?}",
                        unhealthy_workers.len(),
                        unhealthy_workers
                    );
                    // TODO: Implement recovery logic
                }
            }
        });

        *self.health_check_task.lock().await = Some(task);
    }

    /// Start statistics collection background task
    async fn start_stats_collection_task(&self) {
        let workers = Arc::clone(&self.workers);
        let stats = Arc::clone(&self.stats);
        let aggregator = Arc::clone(&self.aggregator);
        let start_time = self.start_time;
        let is_running = Arc::clone(&self.is_running);
        let interval_secs = self.config.stats_interval_secs;

        let task = tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));

            while *is_running.read().await {
                interval.tick().await;

                let workers = workers.read().await;
                let mut new_stats = StreamingStats::default();

                // Collect worker statistics
                new_stats.active_connections = workers.len();

                let mut total_events = 0;
                let mut total_errors = 0;
                let mut total_reconnects = 0;
                let mut total_tokens = 0;

                for worker in workers.values() {
                    let worker_stats = worker.get_stats().await;
                    let worker_tokens = worker.get_assigned_tokens().await;

                    total_events += worker_stats.events_processed;
                    total_errors += worker_stats.connection_errors;
                    total_reconnects += worker_stats.reconnection_attempts;
                    total_tokens += worker_tokens.len();
                }

                new_stats.total_tokens = total_tokens;
                new_stats.total_events_received = total_events;
                new_stats.connection_errors = total_errors;
                new_stats.reconnection_attempts = total_reconnects;
                new_stats.uptime_seconds = start_time.elapsed().as_secs();

                // Get events per second from aggregator
                let aggregator_stats = aggregator.get_stats().await;
                new_stats.events_per_second = aggregator_stats.events_per_second;

                // Update stats
                *stats.write().await = new_stats;
            }
        });

        *self.stats_task.lock().await = Some(task);
    }
}

#[async_trait]
impl StreamingServiceTrait for StreamingService {
    async fn add_tokens(&self, tokens: Vec<String>) -> Result<(), anyhow::Error> {
        info!("Adding {} tokens to streaming service", tokens.len());

        let update = {
            let mut distributor = self.distributor.lock().await;
            distributor.add_tokens(tokens)
        };

        self.apply_distribution_update(update).await?;

        info!("Successfully added tokens");
        Ok(())
    }

    async fn get_streaming_tokens(&self) -> Vec<String> {
        let workers = self.workers.read().await;
        let mut all_tokens = Vec::new();

        for worker in workers.values() {
            let tokens = worker.get_assigned_tokens().await;
            all_tokens.extend(tokens);
        }

        all_tokens
    }

    async fn get_order_book(&self, token_id: &str) -> Option<OrderBook> {
        let distributor = self.distributor.lock().await;
        let worker_id = distributor.get_worker_for_token(token_id)?;
        drop(distributor);

        let workers = self.workers.read().await;
        let worker = workers.get(&worker_id)?;
        worker.get_order_book(token_id).await
    }

    async fn get_last_trade_price(&self, token_id: &str) -> Option<(Decimal, u64)> {
        let distributor = self.distributor.lock().await;
        let worker_id = distributor.get_worker_for_token(token_id)?;
        drop(distributor);

        let workers = self.workers.read().await;
        let worker = workers.get(&worker_id)?;
        worker.get_last_trade_price(token_id).await
    }

    fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<PolyEvent> {
        self.aggregator.subscribe()
    }

    async fn get_stats(&self) -> StreamingStats {
        self.stats.read().await.clone()
    }

    async fn get_worker_statuses(&self) -> Vec<TraitWorkerStatus> {
        let workers = self.workers.read().await;
        let mut statuses = Vec::new();

        for (worker_id, worker) in workers.iter() {
            let status = worker.get_status().await;
            let tokens = worker.get_assigned_tokens().await;
            let worker_stats = worker.get_stats().await;

            let trait_status = TraitWorkerStatus {
                worker_id: *worker_id,
                assigned_tokens: tokens,
                is_connected: matches!(status, WorkerStatus::Connected),
                events_processed: worker_stats.events_processed,
                last_error: worker_stats.last_error,
                last_activity: worker_stats.last_activity.unwrap_or_else(Instant::now),
            };

            statuses.push(trait_status);
        }

        statuses
    }

}
