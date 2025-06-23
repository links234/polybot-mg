//! Background Index Service - Independent thread for managing Aho-Corasick search index
//! 
//! This service runs in a background thread and provides:
//! - Async status and progress queries
//! - Non-blocking index building
//! - Real-time progress updates
//! - Thread-safe access to the search engine

use std::sync::Arc;
use tokio::sync::{Mutex, watch, mpsc};
use anyhow::Result;
use chrono::{DateTime, Utc};
use tracing::{info, error};
use serde::{Serialize, Deserialize};

use super::fast_search::{FastSearchEngine, SearchParams};
use super::types::GammaMarket;

/// Current status of the index service
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IndexStatus {
    /// Service not started
    NotStarted,
    /// Initializing database connection
    Initializing,
    /// Loading markets from database
    LoadingMarkets {
        loaded: usize,
        total: usize,
        rate: f64,
        elapsed_seconds: f64,
    },
    /// Building Aho-Corasick index
    BuildingIndex {
        markets: usize,
        patterns: usize,
        elapsed_seconds: f64,
    },
    /// Index ready for use
    Ready {
        markets: usize,
        patterns: usize,
        categories: usize,
        build_time_seconds: f64,
        memory_usage_mb: f64,
    },
    /// Service failed
    Failed {
        error: String,
        timestamp: DateTime<Utc>,
    },
}

/// Progress information with detailed metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexProgress {
    pub status: IndexStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub last_update: DateTime<Utc>,
    pub estimated_completion: Option<DateTime<Utc>>,
}

/// Commands that can be sent to the index service
#[derive(Debug)]
pub enum IndexCommand {
    /// Start building the index with optional markets data (empty vec means load from DB)
    StartBuild {
        markets: Vec<GammaMarket>,
        #[allow(dead_code)] // Fields kept for future use
        db_path: Option<std::path::PathBuf>, // Add database path for loading
        #[allow(dead_code)] // Fields kept for future use
        force_rebuild: bool,
    },
    /// Stop the service
    #[allow(dead_code)] // Variants kept for future use
    Stop,
    /// Get current status (response via channel)
    #[allow(dead_code)] // Variants kept for future use
    GetStatus {
        response: tokio::sync::oneshot::Sender<IndexProgress>,
    },
}

/// Background Index Service
pub struct IndexService {
    /// Command channel for sending requests to the service
    command_tx: mpsc::UnboundedSender<IndexCommand>,
    /// Progress updates broadcast channel
    progress_rx: watch::Receiver<IndexProgress>,
    /// Handle to the background task
    #[allow(dead_code)] // Field kept for future use
    task_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl IndexService {
    /// Create and start the background index service
    pub async fn new() -> Self {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (progress_tx, progress_rx) = watch::channel(IndexProgress {
            status: IndexStatus::NotStarted,
            started_at: None,
            completed_at: None,
            last_update: Utc::now(),
            estimated_completion: None,
        });

        // Spawn background task
        let task_handle = tokio::spawn(Self::service_task(command_rx, progress_tx));

        Self {
            command_tx,
            progress_rx,
            task_handle: Arc::new(Mutex::new(Some(task_handle))),
        }
    }

    /// Start building the index with markets data (empty vec means load from database)
    pub async fn start_build(&self, markets: Vec<GammaMarket>, force_rebuild: bool) -> Result<()> {
        let db_path = if markets.is_empty() {
            Some(std::path::PathBuf::from("./data/database/gamma"))
        } else {
            None
        };
        
        self.command_tx.send(IndexCommand::StartBuild {
            markets,
            db_path,
            force_rebuild,
        })?;
        Ok(())
    }

    /// Get current progress
    pub async fn get_progress(&self) -> IndexProgress {
        self.progress_rx.borrow().clone()
    }

    /// Subscribe to progress updates
    #[allow(dead_code)] // Service API kept for future use
    pub fn subscribe_progress(&self) -> watch::Receiver<IndexProgress> {
        self.progress_rx.clone()
    }

    /// Check if the index is ready
    pub async fn is_ready(&self) -> bool {
        matches!(self.get_progress().await.status, IndexStatus::Ready { .. })
    }

    /// Search using the index (if ready)
    #[allow(dead_code)] // Service API kept for future use
    pub async fn search(&self, _params: &SearchParams) -> Result<Vec<Arc<GammaMarket>>> {
        // For now, we'll need to store the engine somewhere accessible
        // This is a simplified implementation - in practice, we'd need to
        // store the engine in a shared location or use a different approach
        Err(anyhow::anyhow!("Search not yet implemented in service - use direct engine access"))
    }

    /// Stop the service
    #[allow(dead_code)] // Service API kept for future use
    pub async fn stop(&self) -> Result<()> {
        self.command_tx.send(IndexCommand::Stop)?;
        
        // Wait for task to complete
        if let Some(handle) = self.task_handle.lock().await.take() {
            handle.abort();
        }
        
        Ok(())
    }

    /// Background service task
    async fn service_task(
        mut command_rx: mpsc::UnboundedReceiver<IndexCommand>,
        progress_tx: watch::Sender<IndexProgress>,
    ) {
        info!("Index service background task started");
        let mut _search_engine: Option<FastSearchEngine> = None;

        while let Some(command) = command_rx.recv().await {
            match command {
                IndexCommand::StartBuild { markets, db_path: _, force_rebuild: _ } => {
                    info!("Starting index build with {} markets", markets.len());
                    
                    // Update status to initializing
                    let mut progress = IndexProgress {
                        status: IndexStatus::Initializing,
                        started_at: Some(Utc::now()),
                        completed_at: None,
                        last_update: Utc::now(),
                        estimated_completion: None,
                    };
                    let _ = progress_tx.send(progress.clone());

                    match Self::build_index_with_markets(markets, &progress_tx).await {
                            Ok(engine) => {
                                let stats = engine.stats();
                                let build_time = progress.started_at
                                    .map(|start| (Utc::now() - start).num_milliseconds() as f64 / 1000.0)
                                    .unwrap_or(0.0);

                                // Estimate memory usage (rough approximation)
                                let memory_mb = (stats.total_documents * 500 + stats.total_patterns * 100) as f64 / 1_000_000.0;

                                let doc_count = stats.total_documents;
                                let pattern_count = stats.total_patterns;
                                let category_count = stats.total_categories;

                                progress.status = IndexStatus::Ready {
                                    markets: doc_count,
                                    patterns: pattern_count,
                                    categories: category_count,
                                    build_time_seconds: build_time,
                                    memory_usage_mb: memory_mb,
                                };
                                progress.completed_at = Some(Utc::now());
                                progress.last_update = Utc::now();

                                _search_engine = Some(engine);
                                let _ = progress_tx.send(progress);
                                
                                info!("Index build completed successfully: {} markets, {} patterns", 
                                      doc_count, pattern_count);
                            }
                            Err(e) => {
                                error!("Index build failed: {}", e);
                                progress.status = IndexStatus::Failed {
                                    error: e.to_string(),
                                    timestamp: Utc::now(),
                                };
                                progress.last_update = Utc::now();
                                let _ = progress_tx.send(progress);
                            }
                        }
                }
                IndexCommand::Stop => {
                    info!("Index service stopping");
                    break;
                }
                IndexCommand::GetStatus { response } => {
                    let progress = progress_tx.borrow().clone();
                    let _ = response.send(progress);
                }
            }
        }

        info!("Index service background task ended");
    }

    /// Build the search index with progress tracking using provided markets data
    async fn build_index_with_markets(
        markets: Vec<GammaMarket>,
        progress_tx: &watch::Sender<IndexProgress>,
    ) -> Result<FastSearchEngine> {
        let start_time = Utc::now();
        let total_count = markets.len();

        info!("Starting index build with {} markets", total_count);

        // Update to building status (no loading needed since we already have the data)
        let progress = IndexProgress {
            status: IndexStatus::BuildingIndex {
                markets: total_count,
                patterns: 0, // Will be updated during build
                elapsed_seconds: 0.0,
            },
            started_at: Some(start_time),
            completed_at: None,
            last_update: Utc::now(),
            estimated_completion: None,
        };
        let _ = progress_tx.send(progress.clone());

        // Build the search engine
        info!("Building search engine for {} markets", total_count);
        let engine = FastSearchEngine::build(markets).await?;

        info!("Index build completed successfully");
        Ok(engine)
    }

}

/// Global instance management
static INDEX_SERVICE: tokio::sync::OnceCell<Arc<IndexService>> = tokio::sync::OnceCell::const_new();

/// Initialize the global index service
pub async fn init_index_service() -> Result<Arc<IndexService>> {
    let service = INDEX_SERVICE.get_or_init(|| async {
        Arc::new(IndexService::new().await)
    }).await;
    
    Ok(service.clone())
}

/// Get the global index service (must be initialized first)
#[allow(dead_code)] // Service API kept for future use
pub fn get_index_service() -> Option<Arc<IndexService>> {
    INDEX_SERVICE.get().cloned()
}