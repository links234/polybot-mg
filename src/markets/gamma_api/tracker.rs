//! Gamma tracker service with actor pattern

#[allow(dead_code)]

use anyhow::{Result, Context};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{mpsc, oneshot, RwLock};
use tracing::{info, debug, warn};
use chrono::Utc;

use super::types::*;
use super::client::GammaApiClient;
use super::storage::GammaStorage;
use crate::data::DataPaths;
use crate::address_book::{AddressBookServiceHandle, AddressBookCommand};

/// Commands that can be sent to the gamma tracker service
#[allow(dead_code)]
#[derive(Debug)]
#[allow(dead_code)]
pub enum TrackerCommand {
    /// Track a new address
    TrackAddress {
        address: String,
        is_own_address: bool,
        response: oneshot::Sender<Result<GammaMetadata>>,
    },
    
    /// Untrack an address
    UntrackAddress {
        address: String,
        response: oneshot::Sender<Result<()>>,
    },
    
    /// Sync data for an address
    SyncAddress {
        address: String,
        sync_positions: bool,
        sync_activity: bool,
        response: oneshot::Sender<Result<UserState>>,
    },
    
    /// Sync all tracked addresses
    SyncAll {
        response: oneshot::Sender<Result<Vec<String>>>,
    },
    
    /// Get user state
    GetUserState {
        address: String,
        response: oneshot::Sender<Result<Option<UserState>>>,
    },
    
    /// Get user metadata
    GetUserMetadata {
        address: String,
        response: oneshot::Sender<Result<Option<GammaMetadata>>>,
    },
    
    /// List all tracked addresses
    ListTracked {
        response: oneshot::Sender<Result<Vec<GammaMetadata>>>,
    },
    
    /// Update address metadata from address book
    RefreshAddressBook {
        response: oneshot::Sender<Result<usize>>,
    },
    
    /// Create backup for an address
    CreateBackup {
        address: String,
        response: oneshot::Sender<Result<()>>,
    },
    
    /// Get storage statistics
    GetStorageStats {
        response: oneshot::Sender<Result<HashMap<String, crate::markets::gamma_api::storage::UserStorageStats>>>,
    },
    
    /// Shutdown tracker
    Shutdown,
}

/// Gamma tracker service
#[allow(dead_code)]
pub struct GammaTracker {
    /// Gamma API client
    client: GammaApiClient,
    
    /// Storage manager
    storage: GammaStorage,
    
    /// Address book service handle (optional)
    address_book: Option<Arc<AddressBookServiceHandle>>,
    
    /// In-memory cache of tracked addresses
    tracked_addresses: Arc<RwLock<HashMap<String, GammaMetadata>>>,
    
    /// Command receiver
    receiver: mpsc::Receiver<TrackerCommand>,
}

#[allow(dead_code)]
impl GammaTracker {
    /// Create new gamma tracker
    pub async fn new(
        data_paths: DataPaths,
        address_book: Option<Arc<AddressBookServiceHandle>>,
        receiver: mpsc::Receiver<TrackerCommand>,
    ) -> Result<Self> {
        let client = GammaApiClient::new()
            .context("Failed to create Gamma API client")?;
        
        let storage = GammaStorage::new(data_paths);
        
        // Load existing tracked addresses
        let mut tracked = HashMap::new();
        for address in storage.list_users().await? {
            if let Some(metadata) = storage.load_metadata(&address).await? {
                tracked.insert(address, metadata);
            }
        }
        
        info!("Loaded {} tracked addresses", tracked.len());
        
        Ok(Self {
            client,
            storage,
            address_book,
            tracked_addresses: Arc::new(RwLock::new(tracked)),
            receiver,
        })
    }
    
    /// Run the tracker service
    pub async fn run(mut self) {
        info!("Starting Gamma tracker service");
        
        while let Some(command) = self.receiver.recv().await {
            match command {
                TrackerCommand::TrackAddress { address, is_own_address, response } => {
                    let result = self.handle_track_address(address, is_own_address).await;
                    let _ = response.send(result);
                }
                
                TrackerCommand::UntrackAddress { address, response } => {
                    let result = self.handle_untrack_address(address).await;
                    let _ = response.send(result);
                }
                
                TrackerCommand::SyncAddress { address, sync_positions, sync_activity, response } => {
                    let result = self.handle_sync_address(address, sync_positions, sync_activity).await;
                    let _ = response.send(result);
                }
                
                TrackerCommand::SyncAll { response } => {
                    let result = self.handle_sync_all().await;
                    let _ = response.send(result);
                }
                
                TrackerCommand::GetUserState { address, response } => {
                    let result = self.handle_get_user_state(address).await;
                    let _ = response.send(result);
                }
                
                TrackerCommand::GetUserMetadata { address, response } => {
                    let result = self.handle_get_user_metadata(address).await;
                    let _ = response.send(result);
                }
                
                TrackerCommand::ListTracked { response } => {
                    let result = self.handle_list_tracked().await;
                    let _ = response.send(result);
                }
                
                TrackerCommand::RefreshAddressBook { response } => {
                    let result = self.handle_refresh_address_book().await;
                    let _ = response.send(result);
                }
                
                TrackerCommand::CreateBackup { address, response } => {
                    let result = self.handle_create_backup(address).await;
                    let _ = response.send(result);
                }
                
                TrackerCommand::GetStorageStats { response } => {
                    let result = self.handle_get_storage_stats().await;
                    let _ = response.send(result);
                }
                
                TrackerCommand::Shutdown => {
                    info!("Shutting down Gamma tracker service");
                    break;
                }
            }
        }
    }
    
    /// Handle track address command
    async fn handle_track_address(&mut self, address: String, is_own_address: bool) -> Result<GammaMetadata> {
        // Validate address format
        if !address.starts_with("0x") || address.len() != 42 {
            return Err(anyhow::anyhow!("Invalid Ethereum address format"));
        }
        
        // Check if already tracked
        {
            let tracked = self.tracked_addresses.read().await;
            if tracked.contains_key(&address) {
                return Ok(tracked[&address].clone());
            }
        }
        
        // Initialize storage for this user
        self.storage.init_user(&address).await
            .context("Failed to initialize user storage")?;
        
        // Create metadata
        let mut metadata = GammaMetadata::new(address.clone(), is_own_address);
        
        // Update metadata from address book if available
        if let Some(ref address_book) = self.address_book {
            if let Ok(entry) = self.get_address_book_entry(address_book, &address).await {
                metadata.label = entry.label;
                metadata.address_type = Some(entry.address_type.to_string());
            }
        }
        
        // Save metadata
        self.storage.save_metadata(&address, &metadata).await
            .context("Failed to save metadata")?;
        
        // Add to tracked addresses
        {
            let mut tracked = self.tracked_addresses.write().await;
            tracked.insert(address.clone(), metadata.clone());
        }
        
        info!("Started tracking address: {}", address);
        Ok(metadata)
    }
    
    /// Handle untrack address command
    async fn handle_untrack_address(&mut self, address: String) -> Result<()> {
        // Remove from tracked addresses
        {
            let mut tracked = self.tracked_addresses.write().await;
            tracked.remove(&address);
        }
        
        info!("Stopped tracking address: {}", address);
        Ok(())
    }
    
    /// Handle sync address command
    async fn handle_sync_address(&mut self, address: String, sync_positions: bool, sync_activity: bool) -> Result<UserState> {
        // Check if address is tracked
        let metadata = {
            let tracked = self.tracked_addresses.read().await;
            tracked.get(&address).cloned()
                .ok_or_else(|| anyhow::anyhow!("Address not tracked: {}", address))?
        };
        
        debug!("Syncing data for address: {}", address);
        
        let mut positions = Vec::new();
        let mut activity = Vec::new();
        
        // Fetch ALL data from Gamma API with pagination
        if sync_positions || sync_activity {
            info!("Fetching complete historical data for {}", address);
            match self.client.get_all_user_data(&address, None).await {
                Ok((pos, act)) => {
                    if sync_positions {
                        positions = pos;
                        info!("Fetched {} positions for {}", positions.len(), address);
                    }
                    if sync_activity {
                        activity = act;
                        info!("Fetched {} activities for {}", activity.len(), address);
                    }
                }
                Err(e) => {
                    warn!("Failed to fetch complete data for {}: {}", address, e);
                    // Try fallback to limited data
                    match self.client.get_user_data(&address, None).await {
                        Ok((pos, act)) => {
                            if sync_positions {
                                positions = pos;
                                warn!("Fallback: fetched only {} positions for {}", positions.len(), address);
                            }
                            if sync_activity {
                                activity = act;
                                warn!("Fallback: fetched only {} activities for {}", activity.len(), address);
                            }
                        }
                        Err(e2) => {
                            warn!("Fallback also failed for {}: {}", address, e2);
                            // Continue with empty data rather than failing
                        }
                    }
                }
            }
        }
        
        // Calculate portfolio summary
        let portfolio_summary = self.calculate_portfolio_summary(&positions, &activity);
        
        // Update metadata
        let mut updated_metadata = metadata;
        if sync_positions {
            updated_metadata.update_sync_time("positions");
            updated_metadata.total_positions = positions.len();
            updated_metadata.active_positions = positions.iter()
                .filter(|p| p.state == PositionState::Open)
                .count();
        }
        if sync_activity {
            updated_metadata.update_sync_time("activity");
            updated_metadata.total_activities = activity.len();
            updated_metadata.total_volume = portfolio_summary.total_volume;
            updated_metadata.total_pnl = portfolio_summary.total_realized_pnl + portfolio_summary.total_unrealized_pnl;
        }
        
        // Calculate unique markets
        let mut markets = std::collections::HashSet::new();
        for pos in &positions {
            markets.insert(&pos.market);
        }
        for act in &activity {
            markets.insert(&act.market);
        }
        updated_metadata.total_unique_markets = markets.len();
        
        // Create user state
        let user_state = UserState {
            metadata: updated_metadata.clone(),
            positions: positions.clone(),
            recent_activity: activity.clone(),
            portfolio_summary,
            last_updated: Utc::now(),
        };
        
        // Save data
        self.storage.save_metadata(&address, &updated_metadata).await?;
        self.storage.save_state(&address, &user_state).await?;
        
        if sync_positions {
            self.storage.save_positions(&address, &positions).await?;
        }
        if sync_activity {
            self.storage.save_activity(&address, &activity).await?;
        }
        
        // Update in-memory cache
        {
            let mut tracked = self.tracked_addresses.write().await;
            tracked.insert(address.clone(), updated_metadata);
        }
        
        info!("Synced data for address: {} (positions: {}, activities: {})", 
            address, positions.len(), activity.len());
        
        Ok(user_state)
    }
    
    /// Handle sync all command
    async fn handle_sync_all(&mut self) -> Result<Vec<String>> {
        let addresses: Vec<String> = {
            let tracked = self.tracked_addresses.read().await;
            tracked.keys().cloned().collect()
        };
        
        let mut synced = Vec::new();
        
        for address in addresses {
            match self.handle_sync_address(address.clone(), true, true).await {
                Ok(_) => synced.push(address),
                Err(e) => warn!("Failed to sync {}: {}", address, e),
            }
        }
        
        info!("Synced {} addresses", synced.len());
        Ok(synced)
    }
    
    /// Handle get user state command
    async fn handle_get_user_state(&self, address: String) -> Result<Option<UserState>> {
        self.storage.load_state(&address).await
    }
    
    /// Handle get user metadata command
    async fn handle_get_user_metadata(&self, address: String) -> Result<Option<GammaMetadata>> {
        let tracked = self.tracked_addresses.read().await;
        Ok(tracked.get(&address).cloned())
    }
    
    /// Handle list tracked command
    async fn handle_list_tracked(&self) -> Result<Vec<GammaMetadata>> {
        let tracked = self.tracked_addresses.read().await;
        Ok(tracked.values().cloned().collect())
    }
    
    /// Handle refresh address book command
    async fn handle_refresh_address_book(&mut self) -> Result<usize> {
        let Some(ref address_book) = self.address_book else {
            return Ok(0);
        };
        
        let mut updated = 0;
        let addresses: Vec<String> = {
            let tracked = self.tracked_addresses.read().await;
            tracked.keys().cloned().collect()
        };
        
        for address in addresses {
            if let Ok(entry) = self.get_address_book_entry(address_book, &address).await {
                let mut metadata = {
                    let tracked = self.tracked_addresses.read().await;
                    tracked.get(&address).cloned()
                        .unwrap_or_else(|| GammaMetadata::new(address.clone(), false))
                };
                
                metadata.label = entry.label;
                metadata.address_type = Some(entry.address_type.to_string());
                metadata.is_own_address = entry.address_type == crate::address_book::types::AddressType::Own;
                
                self.storage.save_metadata(&address, &metadata).await?;
                
                {
                    let mut tracked = self.tracked_addresses.write().await;
                    tracked.insert(address, metadata);
                }
                
                updated += 1;
            }
        }
        
        info!("Updated metadata for {} addresses from address book", updated);
        Ok(updated)
    }
    
    /// Handle create backup command
    async fn handle_create_backup(&self, address: String) -> Result<()> {
        self.storage.create_backup(&address).await
    }
    
    /// Handle get storage stats command
    async fn handle_get_storage_stats(&self) -> Result<HashMap<String, crate::markets::gamma_api::storage::UserStorageStats>> {
        let addresses = self.storage.list_users().await?;
        let mut stats = HashMap::new();
        
        for address in addresses {
            if let Ok(user_stats) = self.storage.get_user_stats(&address).await {
                stats.insert(address, user_stats);
            }
        }
        
        Ok(stats)
    }
    
    /// Get address book entry for an address
    async fn get_address_book_entry(
        &self,
        address_book: &AddressBookServiceHandle,
        address: &str,
    ) -> Result<crate::address_book::types::AddressEntry> {
        let (tx, rx) = oneshot::channel();
        address_book.send(AddressBookCommand::GetAddress {
            address_or_label: address.to_string(),
            response: tx,
        }).await?;
        
        match rx.await?? {
            Some(entry) => Ok(entry),
            None => Err(anyhow::anyhow!("Address not found in address book")),
        }
    }
    
    /// Calculate portfolio summary from positions and activity
    fn calculate_portfolio_summary(&self, positions: &[GammaPosition], activity: &[GammaActivity]) -> PortfolioSummary {
        let total_value = positions.iter()
            .map(|p| p.value)
            .sum();
        
        let total_realized_pnl = positions.iter()
            .map(|p| p.realized_pnl)
            .sum();
        
        let total_unrealized_pnl = positions.iter()
            .map(|p| p.unrealized_pnl)
            .sum();
        
        let total_volume = activity.iter()
            .map(|a| a.size * a.price)
            .sum();
        
        let active_positions = positions.iter()
            .filter(|p| p.state == PositionState::Open)
            .count();
        
        let closed_positions = positions.iter()
            .filter(|p| p.state == PositionState::Closed)
            .count();
        
        let unique_markets = positions.iter()
            .map(|p| &p.market)
            .collect::<std::collections::HashSet<_>>()
            .len();
        
        let total_trades = activity.iter()
            .filter(|a| a.activity_type == ActivityType::Trade)
            .count();
        
        let last_trade = activity.iter()
            .filter(|a| a.activity_type == ActivityType::Trade)
            .map(|a| a.timestamp)
            .max();
        
        // Calculate win rate
        let winning_positions = positions.iter()
            .filter(|p| p.realized_pnl > rust_decimal::Decimal::ZERO)
            .count();
        
        let total_closed = closed_positions;
        let win_rate = if total_closed > 0 {
            Some(rust_decimal::Decimal::from(winning_positions * 100) / rust_decimal::Decimal::from(total_closed))
        } else {
            None
        };
        
        PortfolioSummary {
            total_value,
            total_realized_pnl,
            total_unrealized_pnl,
            total_volume,
            active_positions,
            closed_positions,
            unique_markets,
            win_rate,
            total_trades,
            last_trade,
        }
    }
}

/// Handle to communicate with gamma tracker service
#[derive(Clone)]
#[allow(dead_code)]
pub struct GammaTrackerHandle {
    sender: mpsc::Sender<TrackerCommand>,
}

#[allow(dead_code)]
impl GammaTrackerHandle {
    pub fn new(sender: mpsc::Sender<TrackerCommand>) -> Self {
        Self { sender }
    }
    
    pub async fn send(&self, command: TrackerCommand) -> Result<()> {
        self.sender.send(command).await
            .context("Failed to send command to gamma tracker service")
    }
}

/// Start gamma tracker service
#[allow(dead_code)]
pub async fn start_gamma_tracker(
    data_paths: DataPaths,
    address_book: Option<Arc<AddressBookServiceHandle>>,
) -> Result<GammaTrackerHandle> {
    let (tx, rx) = mpsc::channel(100);
    
    let tracker = GammaTracker::new(data_paths, address_book, rx).await?;
    
    tokio::spawn(async move {
        tracker.run().await;
    });
    
    Ok(GammaTrackerHandle::new(tx))
}

/// Global gamma tracker service instance
#[allow(dead_code)]
static GAMMA_TRACKER: tokio::sync::OnceCell<Arc<GammaTrackerHandle>> = tokio::sync::OnceCell::const_new();

/// Get or create gamma tracker service handle
#[allow(dead_code)]
pub async fn get_gamma_tracker(
    data_paths: DataPaths,
    address_book: Option<Arc<AddressBookServiceHandle>>,
) -> Result<Arc<GammaTrackerHandle>> {
    GAMMA_TRACKER.get_or_try_init(|| async {
        let handle = start_gamma_tracker(data_paths, address_book).await?;
        Ok(Arc::new(handle))
    }).await.cloned()
}