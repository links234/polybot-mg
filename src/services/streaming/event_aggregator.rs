//! Event aggregation from multiple workers

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

use crate::ws::events::PolyEvent;

/// Aggregates events from multiple workers into a single stream
pub struct EventAggregator {
    /// Main event sender that clients subscribe to
    main_sender: broadcast::Sender<PolyEvent>,

    /// Worker event receivers: worker_id -> receiver
    worker_receivers: Arc<RwLock<HashMap<usize, broadcast::Receiver<PolyEvent>>>>,

    /// Active aggregation tasks: worker_id -> task_handle
    aggregation_tasks: Arc<RwLock<HashMap<usize, JoinHandle<()>>>>,

    /// Event statistics
    stats: Arc<RwLock<EventStats>>,

    /// Whether the aggregator is running
    is_running: Arc<RwLock<bool>>,
}

/// Statistics about event processing
#[derive(Debug, Default, Clone)]
pub struct EventStats {
    /// Total events processed
    pub total_events: u64,

    /// Events per worker
    pub worker_events: HashMap<usize, u64>,

    /// Events processed in the last second
    pub events_last_second: u64,

    /// Events per second calculation
    pub events_per_second: f64,

    /// Last stats update time
    pub last_update: Option<Instant>,

    /// Dropped events due to buffer overflow
    pub dropped_events: u64,
}

impl EventAggregator {
    /// Create a new event aggregator
    pub fn new(buffer_size: usize) -> Self {
        let (main_sender, _) = broadcast::channel(buffer_size);

        Self {
            main_sender,
            worker_receivers: Arc::new(RwLock::new(HashMap::new())),
            aggregation_tasks: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(EventStats::default())),
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the aggregator
    pub async fn start(&self) {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            warn!("Event aggregator is already running");
            return;
        }

        *is_running = true;
        info!("Event aggregator started");

        // Start stats collection task
        self.start_stats_collection().await;
    }


    /// Add a worker's event receiver
    pub async fn add_worker(&self, worker_id: usize, receiver: broadcast::Receiver<PolyEvent>) {
        // Store the receiver
        {
            let mut receivers = self.worker_receivers.write().await;
            receivers.insert(worker_id, receiver);
        }

        // Start aggregation task for this worker
        let mut receiver = {
            let receivers = self.worker_receivers.read().await;
            receivers.get(&worker_id).unwrap().resubscribe()
        };

        let main_sender = self.main_sender.clone();
        let stats = Arc::clone(&self.stats);
        let is_running = Arc::clone(&self.is_running);

        let task = tokio::spawn(async move {
            info!("Started event aggregation for worker {}", worker_id);

            while *is_running.read().await {
                match receiver.recv().await {
                    Ok(event) => {
                        // Forward event to main channel
                        match main_sender.send(event) {
                            Ok(_) => {
                                // Update stats
                                let mut stats = stats.write().await;
                                stats.total_events += 1;
                                stats.events_last_second += 1;
                                *stats.worker_events.entry(worker_id).or_insert(0) += 1;
                            }
                            Err(_) => {
                                // No receivers on main channel, but that's ok
                                debug!(
                                    "No receivers for aggregated event from worker {}",
                                    worker_id
                                );
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!("Worker {} lagged, skipped {} events", worker_id, skipped);
                        let mut stats = stats.write().await;
                        stats.dropped_events += skipped;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!("Worker {} channel closed, stopping aggregation", worker_id);
                        break;
                    }
                }
            }

            info!("Event aggregation for worker {} stopped", worker_id);
        });

        // Store the task
        {
            let mut tasks = self.aggregation_tasks.write().await;
            tasks.insert(worker_id, task);
        }

        info!("Added worker {} to event aggregator", worker_id);
    }

    /// Remove a worker
    pub async fn remove_worker(&self, worker_id: usize) {
        // Stop the aggregation task
        {
            let mut tasks = self.aggregation_tasks.write().await;
            if let Some(task) = tasks.remove(&worker_id) {
                task.abort();
            }
        }

        // Remove the receiver
        {
            let mut receivers = self.worker_receivers.write().await;
            receivers.remove(&worker_id);
        }

        info!("Removed worker {} from event aggregator", worker_id);
    }

    /// Get a receiver for the aggregated events
    pub fn subscribe(&self) -> broadcast::Receiver<PolyEvent> {
        self.main_sender.subscribe()
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> EventStats {
        self.stats.read().await.clone()
    }

    /// Start statistics collection task
    async fn start_stats_collection(&self) {
        let stats = Arc::clone(&self.stats);
        let is_running = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));

            while *is_running.read().await {
                interval.tick().await;

                let mut stats = stats.write().await;

                // Calculate events per second
                if let Some(last_update) = stats.last_update {
                    let elapsed = last_update.elapsed().as_secs_f64();
                    if elapsed > 0.0 {
                        stats.events_per_second = stats.events_last_second as f64 / elapsed;
                    }
                } else {
                    stats.events_per_second = 0.0;
                }

                // Reset counter for next second
                stats.events_last_second = 0;
                stats.last_update = Some(Instant::now());
            }
        });
    }
}

impl Drop for EventAggregator {
    fn drop(&mut self) {
        // Note: We can't use async in Drop, so this is best effort cleanup
        // The tasks will be cleaned up when the runtime shuts down
        if let Ok(is_running) = self.is_running.try_read() {
            if *is_running {
                eprintln!("EventAggregator dropped while running - tasks may leak");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_event_aggregation() {
        let aggregator = EventAggregator::new(100);
        aggregator.start().await;

        // Create a worker event channel
        let (worker_sender, worker_receiver) = broadcast::channel(10);

        // Add worker to aggregator
        aggregator.add_worker(1, worker_receiver).await;

        // Subscribe to aggregated events
        let mut main_receiver = aggregator.subscribe();

        // Send events from worker
        let test_event = PolyEvent::LastTradePrice {
            asset_id: "test".to_string(),
            price: rust_decimal::Decimal::new(100, 2),
            timestamp: 123456789,
        };

        worker_sender.send(test_event.clone()).unwrap();

        // Should receive the event on main channel
        let received = tokio::time::timeout(Duration::from_millis(100), main_receiver.recv()).await;
        assert!(received.is_ok());

        // Check stats
        let stats = aggregator.get_stats().await;
        assert_eq!(stats.total_events, 1);
        assert_eq!(stats.worker_events.get(&1), Some(&1));

        aggregator.stop().await;
    }
}
