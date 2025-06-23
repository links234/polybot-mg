//! Fast Search Service - A background service that manages the search engine lifecycle
//! 
//! This service handles:
//! - Loading markets from database without locking issues
//! - Building the search index in the background
//! - Progress tracking and status reporting
//! - Thread-safe access to the search engine

use std::sync::Arc;
use std::path::PathBuf;
use tokio::sync::{RwLock, Mutex, watch};
use anyhow::Result;
use chrono::{DateTime, Utc};
use tracing::{info, error};
use serde::{Serialize, Deserialize};

use super::fast_search::{FastSearchEngine, SearchParams};
use super::database::GammaDatabase;
use super::types::GammaMarket;

/// Status of the search service
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ServiceStatus {
    /// Service is not initialized
    NotStarted,
    /// Connecting to database
    Connecting,
    /// Loading markets from database
    LoadingMarkets {
        loaded: usize,
        total: usize,
        rate: f64,
    },
    /// Building search index
    BuildingIndex {
        markets: usize,
        elapsed_ms: u64,
    },
    /// Search engine is ready
    Ready {
        markets: usize,
        patterns: usize,
        categories: usize,
        build_time_ms: u64,
    },
    /// Service failed with error
    Failed {
        error: String,
        timestamp: DateTime<Utc>,
    },
}

/// Progress information for the service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceProgress {
    pub status: ServiceStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub last_update: DateTime<Utc>,
}

/// Fast Search Service that manages the search engine lifecycle
pub struct FastSearchService {
    /// The search engine (once built)
    engine: Arc<RwLock<Option<FastSearchEngine>>>,
    /// Current service status
    status: Arc<RwLock<ServiceStatus>>,
    /// Progress tracking
    progress: Arc<RwLock<ServiceProgress>>,
    /// Status broadcast channel
    status_tx: watch::Sender<ServiceStatus>,
    status_rx: watch::Receiver<ServiceStatus>,
    /// Build task handle
    build_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Database path
    db_path: PathBuf,
    /// Index path
    index_path: PathBuf,
}

impl FastSearchService {
    /// Create a new search service
    pub fn new(db_path: PathBuf, index_path: PathBuf) -> Self {
        let (status_tx, status_rx) = watch::channel(ServiceStatus::NotStarted);
        
        Self {
            engine: Arc::new(RwLock::new(None)),
            status: Arc::new(RwLock::new(ServiceStatus::NotStarted)),
            progress: Arc::new(RwLock::new(ServiceProgress {
                status: ServiceStatus::NotStarted,
                started_at: None,
                completed_at: None,
                last_update: Utc::now(),
            })),
            status_tx,
            status_rx,
            build_handle: Arc::new(Mutex::new(None)),
            db_path,
            index_path,
        }
    }
    
    /// Start building the search index in the background
    pub async fn start(&self) -> Result<()> {
        // Check if already building
        let mut handle_guard = self.build_handle.lock().await;
        if handle_guard.is_some() {
            return Ok(()); // Already building
        }
        
        // Update status
        self.update_status(ServiceStatus::Connecting).await;
        self.update_progress_started().await;
        
        // Clone Arc references for the background task
        let engine = self.engine.clone();
        let status = self.status.clone();
        let progress = self.progress.clone();
        let status_tx = self.status_tx.clone();
        let db_path = self.db_path.clone();
        let index_path = self.index_path.clone();
        
        // Spawn background build task
        let handle = tokio::spawn(async move {
            match Self::build_engine_task(
                engine,
                status,
                progress,
                status_tx,
                db_path,
                index_path,
            ).await {
                Ok(()) => info!("Fast search engine built successfully"),
                Err(e) => error!("Failed to build fast search engine: {}", e),
            }
        });
        
        *handle_guard = Some(handle);
        Ok(())
    }
    
    /// Background task that builds the search engine
    async fn build_engine_task(
        engine: Arc<RwLock<Option<FastSearchEngine>>>,
        status: Arc<RwLock<ServiceStatus>>,
        progress: Arc<RwLock<ServiceProgress>>,
        status_tx: watch::Sender<ServiceStatus>,
        db_path: PathBuf,
        _index_path: PathBuf,
    ) -> Result<()> {
        let build_start = std::time::Instant::now();
        
        // Initialize database
        let database = match GammaDatabase::new(&db_path).await {
            Ok(db) => db,
            Err(e) => {
                let error_status = ServiceStatus::Failed {
                    error: format!("Failed to connect to database: {}", e),
                    timestamp: Utc::now(),
                };
                Self::update_status_static(&status, &progress, &status_tx, error_status).await;
                return Err(e);
            }
        };
        
        // Get market count
        let total_count = match database.get_market_count().await {
            Ok(count) => count as usize,
            Err(e) => {
                let error_status = ServiceStatus::Failed {
                    error: format!("Failed to get market count: {}", e),
                    timestamp: Utc::now(),
                };
                Self::update_status_static(&status, &progress, &status_tx, error_status).await;
                return Err(e);
            }
        };
        
        info!("Starting to load {} markets from database", total_count);
        
        // Load markets with progress tracking
        let load_start = std::time::Instant::now();
        let mut all_markets = Vec::new();
        let batch_size = 10000;
        let mut offset = 0;
        
        while offset < total_count {
            match database.get_all_markets(Some(batch_size as u64)).await {
                Ok(batch) => {
                    let batch_len = batch.len();
                    all_markets.extend(batch);
                    offset += batch_len;
                    
                    let elapsed = load_start.elapsed().as_secs_f64();
                    let rate = if elapsed > 0.0 { offset as f64 / elapsed } else { 0.0 };
                    
                    let loading_status = ServiceStatus::LoadingMarkets {
                        loaded: offset,
                        total: total_count,
                        rate,
                    };
                    Self::update_status_static(&status, &progress, &status_tx, loading_status).await;
                    
                    info!("Loaded {}/{} markets (rate: {:.0} markets/s)", 
                          offset, total_count, rate);
                    
                    if batch_len < batch_size {
                        break; // Last batch
                    }
                }
                Err(e) => {
                    let error_status = ServiceStatus::Failed {
                        error: format!("Failed to load markets at offset {}: {}", offset, e),
                        timestamp: Utc::now(),
                    };
                    Self::update_status_static(&status, &progress, &status_tx, error_status).await;
                    return Err(e);
                }
            }
            
            // Small delay to avoid overwhelming the database
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
        
        info!("Loaded all {} markets, building index", all_markets.len());
        
        // Update status to building
        let building_status = ServiceStatus::BuildingIndex {
            markets: all_markets.len(),
            elapsed_ms: build_start.elapsed().as_millis() as u64,
        };
        Self::update_status_static(&status, &progress, &status_tx, building_status).await;
        
        // Build the search engine
        match FastSearchEngine::build(all_markets).await {
            Ok(search_engine) => {
                let stats = search_engine.stats();
                
                // Get stats before moving the engine
                let ready_status = ServiceStatus::Ready {
                    markets: stats.total_documents,
                    patterns: stats.total_patterns,
                    categories: stats.total_categories,
                    build_time_ms: build_start.elapsed().as_millis() as u64,
                };
                
                // Clone values we need before the move
                let doc_count = stats.total_documents;
                let pattern_count = stats.total_patterns;
                
                // Store the engine
                *engine.write().await = Some(search_engine);
                Self::update_status_static(&status, &progress, &status_tx, ready_status).await;
                
                // Update progress completed
                let mut progress_guard = progress.write().await;
                progress_guard.completed_at = Some(Utc::now());
                
                info!("Fast search service ready: {} markets, {} patterns", 
                      doc_count, pattern_count);
                
                Ok(())
            }
            Err(e) => {
                let error_status = ServiceStatus::Failed {
                    error: format!("Failed to build search engine: {}", e),
                    timestamp: Utc::now(),
                };
                Self::update_status_static(&status, &progress, &status_tx, error_status).await;
                Err(e)
            }
        }
    }
    
    /// Update status (instance method)
    async fn update_status(&self, new_status: ServiceStatus) {
        *self.status.write().await = new_status.clone();
        let _ = self.status_tx.send(new_status.clone());
        
        let mut progress = self.progress.write().await;
        progress.status = new_status;
        progress.last_update = Utc::now();
    }
    
    /// Update status (static method for background task)
    async fn update_status_static(
        status: &Arc<RwLock<ServiceStatus>>,
        progress: &Arc<RwLock<ServiceProgress>>,
        status_tx: &watch::Sender<ServiceStatus>,
        new_status: ServiceStatus,
    ) {
        *status.write().await = new_status.clone();
        let _ = status_tx.send(new_status.clone());
        
        let mut progress_guard = progress.write().await;
        progress_guard.status = new_status;
        progress_guard.last_update = Utc::now();
    }
    
    /// Update progress started timestamp
    async fn update_progress_started(&self) {
        let mut progress = self.progress.write().await;
        progress.started_at = Some(Utc::now());
        progress.completed_at = None;
    }
    
    /// Get current status
    pub async fn get_status(&self) -> ServiceStatus {
        self.status.read().await.clone()
    }
    
    /// Get current progress
    pub async fn get_progress(&self) -> ServiceProgress {
        self.progress.read().await.clone()
    }
    
    /// Subscribe to status updates
    pub fn subscribe_status(&self) -> watch::Receiver<ServiceStatus> {
        self.status_rx.clone()
    }
    
    /// Search using the engine (if ready)
    pub async fn search(&self, params: &SearchParams) -> Result<Vec<Arc<GammaMarket>>> {
        let engine_guard = self.engine.read().await;
        match &*engine_guard {
            Some(engine) => Ok(engine.search(params)),
            None => {
                let status = self.get_status().await;
                match status {
                    ServiceStatus::Failed { error, .. } => {
                        Err(anyhow::anyhow!("Search service failed: {}", error))
                    }
                    ServiceStatus::Ready { .. } => {
                        Err(anyhow::anyhow!("Search engine not available despite ready status"))
                    }
                    _ => {
                        Err(anyhow::anyhow!("Search service not ready: {:?}", status))
                    }
                }
            }
        }
    }
    
    /// Check if the service is ready
    #[allow(dead_code)] // Service API kept for future use
    pub async fn is_ready(&self) -> bool {
        matches!(self.get_status().await, ServiceStatus::Ready { .. })
    }
    
    /// Stop the service and cancel any ongoing build
    #[allow(dead_code)] // Service API kept for future use
    pub async fn stop(&self) -> Result<()> {
        let mut handle_guard = self.build_handle.lock().await;
        if let Some(handle) = handle_guard.take() {
            handle.abort();
            info!("Fast search service build task cancelled");
        }
        Ok(())
    }
}

/// Global instance of the search service
static SEARCH_SERVICE: tokio::sync::OnceCell<Arc<FastSearchService>> = tokio::sync::OnceCell::const_new();

/// Initialize the global search service
pub async fn init_search_service(db_path: PathBuf, index_path: PathBuf) -> Result<Arc<FastSearchService>> {
    let service = SEARCH_SERVICE.get_or_init(|| async {
        Arc::new(FastSearchService::new(db_path, index_path))
    }).await;
    
    Ok(service.clone())
}

/// Get the global search service (must be initialized first)
#[allow(dead_code)] // Service API kept for future use
pub fn get_search_service() -> Option<Arc<FastSearchService>> {
    SEARCH_SERVICE.get().cloned()
}