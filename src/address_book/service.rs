//! Address book service implementation with actor pattern
//!
//! This module provides an actor-based service for managing the address book
//! using channels for communication.

use anyhow::{Result, Context};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};
use tracing::{info, error, warn};

use crate::address_book::types::*;
use crate::address_book::storage::*;
use crate::core::portfolio::PortfolioServiceHandle;
use reqwest::Client;
use serde_json::Value;
use std::str::FromStr;
use crate::data::DataPaths;

/// Commands that can be sent to the address book service
#[derive(Debug)]
pub enum AddressBookCommand {
    /// Add a new address
    AddAddress {
        address: String,
        label: Option<String>,
        description: Option<String>,
        address_type: AddressType,
        tags: Vec<String>,
        response: oneshot::Sender<Result<AddressEntry>>,
    },
    
    /// Update an existing address
    UpdateAddress {
        address: String,
        label: Option<String>,
        description: Option<String>,
        tags: Option<Vec<String>>,
        is_active: Option<bool>,
        notes: Option<String>,
        response: oneshot::Sender<Result<AddressEntry>>,
    },
    
    /// Remove an address
    RemoveAddress {
        address: String,
        response: oneshot::Sender<Result<()>>,
    },
    
    /// Query addresses
    QueryAddresses {
        query: AddressQuery,
        response: oneshot::Sender<Result<Vec<AddressEntry>>>,
    },
    
    /// Get specific address
    GetAddress {
        address_or_label: String,
        response: oneshot::Sender<Result<Option<AddressEntry>>>,
    },
    
    /// Set current address
    SetCurrentAddress {
        address: String,
        response: oneshot::Sender<Result<()>>,
    },
    
    /// Get current address
    GetCurrentAddress {
        response: oneshot::Sender<Result<Option<AddressEntry>>>,
    },
    
    /// List all addresses
    ListAddresses {
        limit: Option<usize>,
        response: oneshot::Sender<Result<Vec<AddressEntry>>>,
    },
    
    /// Sync address statistics with portfolio
    SyncAddressStats {
        address: String,
        response: oneshot::Sender<Result<AddressStats>>,
    },
    
    /// Import addresses from CSV
    ImportCsv {
        path: String,
        response: oneshot::Sender<Result<usize>>,
    },
    
    /// Export addresses to CSV
    ExportCsv {
        path: String,
        response: oneshot::Sender<Result<()>>,
    },
    
    /// Get address book stats
    GetStats {
        response: oneshot::Sender<Result<AddressBookMetadata>>,
    },
}

/// Address book service actor
pub struct AddressBookService {
    /// Storage backend
    storage: AddressBookStorage,
    
    /// Data paths for snapshots and storage
    data_paths: DataPaths,
    
    /// In-memory address book
    address_book: Arc<RwLock<AddressBook>>,
    
    /// Portfolio service handle for syncing stats
    portfolio_service: Option<Arc<PortfolioServiceHandle>>,
    
    /// HTTP client for Gamma API
    http_client: Option<Client>,
    
    /// Command receiver
    receiver: mpsc::Receiver<AddressBookCommand>,
}

/// Core portfolio metrics
#[derive(Debug, Clone)]
struct CoreMetrics {
    total_value: f64,
    total_unrealized_pnl: f64,
    active_positions: usize,
    total_positions: usize,
    win_rate: Option<rust_decimal::Decimal>,
}

/// Trading-specific metrics
#[derive(Debug, Clone)]
struct TradingMetrics {
    total_realized_pnl: f64,
    trading_pnl: f64,
    unmatched_wins_pnl: f64,
    total_trades: usize,
    buy_count: usize,
    sell_count: usize,
    avg_trade_size: f64,
    trade_profits: Vec<f64>,
    last_trade: Option<chrono::DateTime<chrono::Utc>>,
    total_volume: f64,
    buy_volume: f64,
    sell_volume: f64,
}

/// Risk and performance metrics
#[derive(Debug, Clone)]
struct RiskMetrics {
    volatility: Option<f64>,
    sharpe_ratio: Option<f64>,
    max_position_percentage: Option<f64>,
}

/// Position tracking for P&L calculation
#[derive(Debug, Clone)]
struct PositionTracker {
    /// Total shares bought
    total_shares: f64,
    /// Total cost basis (USDC spent)
    total_cost: f64,
    /// Total shares sold/redeemed
    total_shares_exited: f64,
    /// Total proceeds from exits (USDC received)
    total_proceeds: f64,
    /// Market info
    market_slug: String,
}

impl PositionTracker {
    fn new(market_slug: String, _outcome: String) -> Self {
        Self {
            total_shares: 0.0,
            total_cost: 0.0,
            total_shares_exited: 0.0,
            total_proceeds: 0.0,
            market_slug,
        }
    }
    
    fn add_buy(&mut self, shares: f64, cost: f64) {
        self.total_shares += shares;
        self.total_cost += cost;
    }
    
    fn add_exit(&mut self, shares: f64, proceeds: f64) {
        self.total_shares_exited += shares;
        self.total_proceeds += proceeds;
    }
    
    fn get_realized_pnl(&self) -> f64 {
        // Calculate P&L only for exited positions
        if self.total_shares_exited > 0.0 && self.total_shares > 0.0 {
            let avg_cost_per_share = self.total_cost / self.total_shares;
            let cost_of_exited = avg_cost_per_share * self.total_shares_exited;
            self.total_proceeds - cost_of_exited
        } else {
            0.0
        }
    }
    
}

impl AddressBookService {
    /// Create new service
    pub async fn new(
        data_paths: DataPaths,
        portfolio_service: Option<Arc<PortfolioServiceHandle>>,
        _gamma_tracker: Option<()>, // Placeholder
        receiver: mpsc::Receiver<AddressBookCommand>,
    ) -> Result<Self> {
        let storage_path = data_paths.root().join("address_book");
        let storage = AddressBookStorage::new(storage_path);
        
        // Initialize storage
        storage.init().await?;
        
        // Load existing address book
        let address_book = storage.load().await?;
        
        // Create HTTP client for Gamma API
        let http_client = Some(Client::new());
        info!("Created HTTP client for Gamma API");
        
        Ok(Self {
            storage,
            data_paths,
            address_book: Arc::new(RwLock::new(address_book)),
            portfolio_service,
            http_client,
            receiver,
        })
    }
    
    /// Run the service
    pub async fn run(mut self) {
        info!("Starting address book service");
        
        while let Some(command) = self.receiver.recv().await {
            match command {
                AddressBookCommand::AddAddress {
                    address,
                    label,
                    description,
                    address_type,
                    tags,
                    response,
                } => {
                    let result = self.handle_add_address(
                        address,
                        label,
                        description,
                        address_type,
                        tags,
                    ).await;
                    let _ = response.send(result);
                }
                
                AddressBookCommand::UpdateAddress {
                    address,
                    label,
                    description,
                    tags,
                    is_active,
                    notes,
                    response,
                } => {
                    let result = self.handle_update_address(
                        address,
                        label,
                        description,
                        tags,
                        is_active,
                        notes,
                    ).await;
                    let _ = response.send(result);
                }
                
                AddressBookCommand::RemoveAddress { address, response } => {
                    let result = self.handle_remove_address(address).await;
                    let _ = response.send(result);
                }
                
                AddressBookCommand::QueryAddresses { query, response } => {
                    let result = self.handle_query_addresses(query).await;
                    let _ = response.send(result);
                }
                
                AddressBookCommand::GetAddress { address_or_label, response } => {
                    let result = self.handle_get_address(address_or_label).await;
                    let _ = response.send(result);
                }
                
                AddressBookCommand::SetCurrentAddress { address, response } => {
                    let result = self.handle_set_current_address(address).await;
                    let _ = response.send(result);
                }
                
                AddressBookCommand::GetCurrentAddress { response } => {
                    let result = self.handle_get_current_address().await;
                    let _ = response.send(result);
                }
                
                AddressBookCommand::ListAddresses { limit, response } => {
                    let result = self.handle_list_addresses(limit).await;
                    let _ = response.send(result);
                }
                
                AddressBookCommand::SyncAddressStats { address, response } => {
                    let result = self.handle_sync_address_stats(address).await;
                    let _ = response.send(result);
                }
                
                AddressBookCommand::ImportCsv { path, response } => {
                    let result = self.handle_import_csv(path).await;
                    let _ = response.send(result);
                }
                
                AddressBookCommand::ExportCsv { path, response } => {
                    let result = self.handle_export_csv(path).await;
                    let _ = response.send(result);
                }
                
                AddressBookCommand::GetStats { response } => {
                    let result = self.handle_get_stats().await;
                    let _ = response.send(result);
                }
            }
        }
        
        // Save before shutdown
        let book = self.address_book.read().await.clone();
        if let Err(e) = self.storage.save(&book).await {
            error!("Failed to save address book on shutdown: {}", e);
        }
    }
    
    /// Handle add address command
    async fn handle_add_address(
        &mut self,
        address: String,
        label: Option<String>,
        description: Option<String>,
        address_type: AddressType,
        tags: Vec<String>,
    ) -> Result<AddressEntry> {
        // Validate address format
        let validated_address = validate_address(&address)?;
        let checksummed = checksum_address(&validated_address);
        
        let mut book = self.address_book.write().await;
        
        // Check if address already exists
        if book.entries.contains_key(&checksummed) {
            return Err(AddressBookError::AddressExists(checksummed).into());
        }
        
        // Check if label is already in use
        if let Some(ref label) = label {
            if book.labels.contains_key(label) {
                return Err(AddressBookError::LabelExists(label.clone()).into());
            }
        }
        
        // Create new entry
        let mut entry = AddressEntry::new(checksummed.clone(), address_type);
        entry.label = label;
        entry.description = description;
        entry.tags = tags;
        
        // If portfolio service is available, sync initial stats
        if let Some(ref portfolio_service) = self.portfolio_service {
            if let Ok(stats) = self.sync_portfolio_stats(&checksummed, portfolio_service).await {
                entry.stats = Some(stats);
            }
        }
        
        // Add to book
        book.upsert_entry(entry.clone());
        
        // Save to disk
        self.storage.save(&book).await?;
        
        info!("Added address {} with label {:?}", checksummed, entry.label);
        Ok(entry)
    }
    
    /// Handle update address command
    async fn handle_update_address(
        &mut self,
        address: String,
        label: Option<String>,
        description: Option<String>,
        tags: Option<Vec<String>>,
        is_active: Option<bool>,
        notes: Option<String>,
    ) -> Result<AddressEntry> {
        let validated_address = validate_address(&address)?;
        let checksummed = checksum_address(&validated_address);
        
        let mut book = self.address_book.write().await;
        
        // Check if entry exists
        if !book.entries.contains_key(&checksummed) {
            return Err(AddressBookError::AddressNotFound(checksummed.clone()).into());
        }
        
        // Check label conflict if changing label
        if let Some(ref new_label) = label {
            if let Some(existing_addr) = book.labels.get(new_label) {
                if existing_addr != &checksummed {
                    return Err(AddressBookError::LabelExists(new_label.clone()).into());
                }
            }
        }
        
        // Get the entry, update it, and handle label changes
        let (old_label, updated_entry) = {
            let entry = book.entries.get_mut(&checksummed).unwrap();
            let old_label = entry.label.clone();
            
            // Update fields
            if let Some(new_label) = label {
                entry.label = Some(new_label);
            }
            
            if description.is_some() {
                entry.description = description;
            }
            
            if let Some(tags) = tags {
                entry.tags = tags;
            }
            
            if let Some(is_active) = is_active {
                entry.is_active = is_active;
            }
            
            if notes.is_some() {
                entry.notes = notes;
            }
            
            entry.metadata.updated_at = chrono::Utc::now();
            
            (old_label, entry.clone())
        };
        
        // Update label index after releasing the mutable borrow
        if let Some(ref new_label) = updated_entry.label {
            // Remove old label from index if changing
            if let Some(ref old) = old_label {
                if old != new_label {
                    book.labels.remove(old);
                }
            }
            book.labels.insert(new_label.clone(), checksummed.clone());
        }
        
        // Save to disk
        self.storage.save(&book).await?;
        
        info!("Updated address {}", checksummed);
        Ok(updated_entry)
    }
    
    /// Handle remove address command
    async fn handle_remove_address(&mut self, address: String) -> Result<()> {
        let validated_address = validate_address(&address)?;
        let checksummed = checksum_address(&validated_address);
        
        let mut book = self.address_book.write().await;
        
        // Remove entry
        let entry = book.remove_entry(&checksummed)
            .ok_or_else(|| AddressBookError::AddressNotFound(checksummed.clone()))?;
        
        // Save to disk
        self.storage.save(&book).await?;
        
        info!("Removed address {} with label {:?}", checksummed, entry.label);
        Ok(())
    }
    
    /// Handle query addresses command
    async fn handle_query_addresses(&self, query: AddressQuery) -> Result<Vec<AddressEntry>> {
        let book = self.address_book.read().await;
        
        let results: Vec<AddressEntry> = book.query(&query)
            .into_iter()
            .cloned()
            .collect();
        
        // Update query counts
        if !results.is_empty() {
            drop(book);
            let mut book = self.address_book.write().await;
            for entry in &results {
                if let Some(e) = book.entries.get_mut(&entry.address) {
                    e.record_query();
                }
            }
            self.storage.save(&book).await?;
        }
        
        Ok(results)
    }
    
    /// Handle get address command
    async fn handle_get_address(&self, address_or_label: String) -> Result<Option<AddressEntry>> {
        let book = self.address_book.read().await;
        
        let entry = book.get_entry(&address_or_label).cloned();
        
        if let Some(ref e) = entry {
            drop(book);
            let mut book = self.address_book.write().await;
            if let Some(entry) = book.entries.get_mut(&e.address) {
                entry.record_query();
            }
            self.storage.save(&book).await?;
        }
        
        Ok(entry)
    }
    
    /// Handle set current address command
    async fn handle_set_current_address(&mut self, address: String) -> Result<()> {
        let validated_address = validate_address(&address)?;
        let checksummed = checksum_address(&validated_address);
        
        let mut book = self.address_book.write().await;
        
        book.set_current(&checksummed)
            .map_err(|e| anyhow::anyhow!(e))?;
        
        // Save to disk
        self.storage.save(&book).await?;
        
        info!("Set current address to {}", checksummed);
        Ok(())
    }
    
    /// Handle get current address command
    async fn handle_get_current_address(&self) -> Result<Option<AddressEntry>> {
        let book = self.address_book.read().await;
        
        if let Some(ref addr) = book.current_address {
            Ok(book.entries.get(addr).cloned())
        } else {
            Ok(None)
        }
    }
    
    /// Handle list addresses command
    async fn handle_list_addresses(&self, limit: Option<usize>) -> Result<Vec<AddressEntry>> {
        let book = self.address_book.read().await;
        
        let mut entries: Vec<AddressEntry> = book.entries.values().cloned().collect();
        
        // Sort by label/address
        entries.sort_by(|a, b| a.display_name().cmp(&b.display_name()));
        
        // Apply limit
        if let Some(limit) = limit {
            entries.truncate(limit);
        }
        
        Ok(entries)
    }
    
    /// Handle sync address stats command
    async fn handle_sync_address_stats(&mut self, address: String) -> Result<AddressStats> {
        let validated_address = validate_address(&address)?;
        let checksummed = checksum_address(&validated_address);
        
        info!("Syncing address stats for: {}", checksummed);
        
        let stats = if let Some(ref http_client) = self.http_client {
            info!("Using Gamma API for sync");
            self.sync_gamma_stats_direct(&checksummed, http_client).await?
        } else if let Some(ref portfolio_service) = self.portfolio_service {
            info!("Using portfolio service for sync");
            self.sync_portfolio_stats(&checksummed, portfolio_service).await?
        } else {
            info!("No sync service available - using default stats");
            AddressStats {
                total_value: rust_decimal::Decimal::ZERO,
                active_positions: 0,
                active_orders: 0,
                total_realized_pnl: rust_decimal::Decimal::ZERO,
                trading_pnl: rust_decimal::Decimal::ZERO,
                unmatched_wins_pnl: rust_decimal::Decimal::ZERO,
                total_unrealized_pnl: rust_decimal::Decimal::ZERO,
                win_rate: None,
                total_trades: 0,
                total_volume: rust_decimal::Decimal::ZERO,
                buy_volume: rust_decimal::Decimal::ZERO,
                sell_volume: rust_decimal::Decimal::ZERO,
                last_trade: None,
                updated_at: chrono::Utc::now(),
            }
        };
        
        info!("Synced stats for {}: total_value=${:.2}, positions={}, trades={}, volume=${:.2}", 
            checksummed, stats.total_value, stats.active_positions, stats.total_trades, stats.total_volume);
        
        // Update in address book
        let mut book = self.address_book.write().await;
        if let Some(entry) = book.entries.get_mut(&checksummed) {
            entry.stats = Some(stats.clone());
            entry.metadata.last_synced = Some(chrono::Utc::now());
        }
        
        // Save to disk
        self.storage.save(&book).await?;
        
        Ok(stats)
    }
    
    /// Handle import CSV command
    async fn handle_import_csv(&mut self, path: String) -> Result<usize> {
        let path = std::path::Path::new(&path);
        let entries = self.storage.import_csv(path).await?;
        
        let mut book = self.address_book.write().await;
        let mut imported = 0;
        
        for entry in entries {
            if !book.entries.contains_key(&entry.address) {
                book.upsert_entry(entry);
                imported += 1;
            }
        }
        
        // Save to disk
        self.storage.save(&book).await?;
        
        info!("Imported {} addresses from CSV", imported);
        Ok(imported)
    }
    
    /// Handle export CSV command
    async fn handle_export_csv(&self, path: String) -> Result<()> {
        let book = self.address_book.read().await;
        let path = std::path::Path::new(&path);
        
        self.storage.export_csv(path, &book).await?;
        
        info!("Exported {} addresses to CSV", book.entries.len());
        Ok(())
    }
    
    /// Handle get stats command
    async fn handle_get_stats(&self) -> Result<AddressBookMetadata> {
        let book = self.address_book.read().await;
        Ok(book.metadata.clone())
    }
    
    /// Sync portfolio statistics for an address
    async fn sync_portfolio_stats(
        &self,
        _address: &str,
        portfolio_service: &PortfolioServiceHandle,
    ) -> Result<AddressStats> {
        // Get portfolio state for address
        let portfolio_state = portfolio_service.get_state().await?;
        
        // Get trade history
        let trades = portfolio_service.get_trade_history(None, None).await.unwrap_or_default();
        
        // Calculate volumes from trades
        let mut total_volume = rust_decimal::Decimal::ZERO;
        let mut buy_volume = rust_decimal::Decimal::ZERO;
        let mut sell_volume = rust_decimal::Decimal::ZERO;
        
        for trade in &trades {
            let volume = trade.size * trade.price;
            total_volume += volume;
            match trade.side {
                crate::core::portfolio::types::OrderSide::Buy => buy_volume += volume,
                crate::core::portfolio::types::OrderSide::Sell => sell_volume += volume,
            }
        }
        
        // Extract stats
        let stats = AddressStats {
            total_value: portfolio_state.balances.total_value,
            active_positions: portfolio_state.positions.len(),
            active_orders: portfolio_state.active_orders.len(),
            total_realized_pnl: portfolio_state.stats.total_realized_pnl,
            trading_pnl: portfolio_state.stats.total_realized_pnl, // TODO: separate matched/unmatched
            unmatched_wins_pnl: rust_decimal::Decimal::ZERO, // TODO: track unmatched wins
            total_unrealized_pnl: portfolio_state.stats.total_unrealized_pnl,
            win_rate: portfolio_state.stats.win_rate,
            total_trades: trades.len(),
            total_volume,
            buy_volume,
            sell_volume,
            last_trade: trades.last()
                .map(|t| t.timestamp),
            updated_at: chrono::Utc::now(),
        };
        
        Ok(stats)
    }
    
    /// Sync address statistics using Gamma API with incremental updates
    async fn sync_gamma_stats_direct(
        &self,
        address: &str,
        http_client: &Client,
    ) -> Result<AddressStats> {
        info!("Starting incremental Gamma API sync for address: {}", address);
        
        // Initialize database
        let db = crate::address_book::db::HistoricalDatabase::new(&self.data_paths);
        db.init_address(address).await?;
        
        // Check existing sync state
        let existing_state = db.get_sync_state(address).await?;
        let stored_summary = db.get_stored_summary(address).await?;
        
        // Display current stats before syncing
        println!("📊 Current stored data for {}:", address);
        println!("     📈 Stored activities: {}", stored_summary.total_activities);
        println!("     📍 Stored positions: {}", stored_summary.total_positions);
        if let Some(last_sync) = stored_summary.last_sync {
            println!("     🕐 Last sync: {}", last_sync.format("%Y-%m-%d %H:%M:%S UTC"));
        } else {
            println!("     🕐 Last sync: Never");
        }
        
        // Mark sync as in progress
        let mut sync_state = existing_state.unwrap_or_else(|| {
            crate::address_book::db::SyncState {
                address: address.to_string(),
                last_activity_timestamp: None,
                last_activity_id: None,
                total_activities_stored: 0,
                total_positions_stored: 0,
                last_sync_completed: None,
                last_sync_started: chrono::Utc::now(),
                sync_in_progress: true,
            }
        });
        sync_state.sync_in_progress = true;
        sync_state.last_sync_started = chrono::Utc::now();
        db.save_sync_state(&sync_state).await?;
        
        // Load previously stored data
        let mut all_positions = db.load_all_positions(address).await?;
        let mut all_activity = db.load_all_activities(address).await?;
        
        println!("     ♻️  Loaded {} existing activities and {} positions from database", 
            all_activity.len(), all_positions.len());
        
        // Fetch new data incrementally
        let _new_positions = self.fetch_all_positions_incremental(
            address, http_client, &db, &mut all_positions
        ).await?;
        
        let _new_activities = self.fetch_all_activities_incremental(
            address, http_client, &db, &mut all_activity, &sync_state
        ).await?;
        
        // Update sync state
        sync_state.total_activities_stored = all_activity.len();
        sync_state.total_positions_stored = all_positions.len();
        sync_state.sync_in_progress = false;
        sync_state.last_sync_completed = Some(chrono::Utc::now());
        
        // Extract latest activity timestamp
        if let Some(latest_activity) = all_activity.iter()
            .filter_map(|a| a.get("timestamp").and_then(|t| t.as_i64()))
            .filter_map(|t| chrono::DateTime::from_timestamp(t, 0))
            .max() {
            sync_state.last_activity_timestamp = Some(latest_activity.with_timezone(&chrono::Utc));
        }
        
        db.save_sync_state(&sync_state).await?;
        
        info!("Incremental sync complete for {}: {} total positions, {} total activities", 
            address, all_positions.len(), all_activity.len());
        
        // Create data snapshot with the complete data
        println!("     💾 Saving data snapshot...");
        self.create_data_snapshot(address, &all_positions, &all_activity).await?;
        
        // Calculate comprehensive portfolio statistics
        println!("     🧮 Calculating P&L and portfolio metrics...");
        let stats = self.calculate_portfolio_stats(address, &all_positions, &all_activity).await?;
        
        info!("Incremental sync complete for {}: {} positions, {} trades, ${:.2} total value", 
            address, stats.active_positions, stats.total_trades, stats.total_value);
        
        Ok(stats)
    }
    
    /// Fetch positions incrementally, loading only new data since last sync
    async fn fetch_all_positions_incremental(
        &self,
        address: &str,
        http_client: &Client,
        db: &crate::address_book::db::HistoricalDatabase,
        existing_positions: &mut Vec<Value>,
    ) -> Result<usize> {
        info!("Fetching incremental positions for {}", address);
        let mut new_positions = Vec::new();
        let mut offset = 0;
        let limit = 500; // API max limit
        let mut batch_count = 0;
        let mut position_ids = std::collections::HashSet::new();
        
        // Build set of existing position identifiers to avoid duplicates
        for pos in existing_positions.iter() {
            if let (Some(condition_id), Some(outcome)) = (
                pos.get("conditionId").and_then(|c| c.as_str()),
                pos.get("outcome").and_then(|o| o.as_str())
            ) {
                let position_key = format!("{}:{}", condition_id, outcome);
                position_ids.insert(position_key);
            }
        }
        
        println!("     📍 Checking for new positions...");
        
        loop {
            let url = format!(
                "https://data-api.polymarket.com/positions?user={}&limit={}&offset={}", 
                address, limit, offset
            );
            
            let positions = match self.fetch_with_retry(&url, http_client, "positions").await {
                Ok(text) => {
                    match serde_json::from_str::<Vec<Value>>(&text) {
                        Ok(pos) => pos,
                        Err(e) => {
                            warn!("Failed to parse positions batch: {}", e);
                            break;
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to fetch positions: {}", e);
                    break;
                }
            };
            
            let batch_size = positions.len();
            let mut new_in_batch = 0;
            
            // Filter out positions we already have
            for position in positions {
                if let (Some(condition_id), Some(outcome)) = (
                    position.get("conditionId").and_then(|c| c.as_str()),
                    position.get("outcome").and_then(|o| o.as_str())
                ) {
                    let position_key = format!("{}:{}", condition_id, outcome);
                    if !position_ids.contains(&position_key) {
                        position_ids.insert(position_key);
                        new_positions.push(position);
                        new_in_batch += 1;
                    }
                }
            }
            
            batch_count += 1;
            
            if new_in_batch > 0 {
                println!("     📈 Found {} new positions (batch {})", new_positions.len(), batch_count);
            }
            
            // If we got 0 results, we've reached the end
            if batch_size == 0 {
                break;
            }
            
            // If all positions in this batch were already known, we might have caught up
            if new_in_batch == 0 && batch_count > 5 {
                // Continue for a few more batches to be sure
                if batch_count > 10 {
                    println!("     ✅ No new positions found after checking {} batches", batch_count);
                    break;
                }
            }
            
            offset += limit;
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        
        // Store new positions in database
        if !new_positions.is_empty() {
            let existing_count = existing_positions.len();
            db.store_positions(address, &new_positions, existing_count).await?;
            existing_positions.extend(new_positions.clone());
            println!("     💾 Stored {} new positions in database", new_positions.len());
        } else {
            println!("     ✅ No new positions to sync");
        }
        
        Ok(new_positions.len())
    }
    
    /// Fetch activities incrementally, loading only new data since last sync
    async fn fetch_all_activities_incremental(
        &self,
        address: &str,
        http_client: &Client,
        db: &crate::address_book::db::HistoricalDatabase,
        existing_activities: &mut Vec<Value>,
        sync_state: &crate::address_book::db::SyncState,
    ) -> Result<usize> {
        info!("Fetching incremental activities for {}", address);
        let mut new_activities = Vec::new();
        let mut offset = 0;
        let limit = 500; // API max limit
        let mut batch_count = 0;
        let mut activity_ids = std::collections::HashSet::new();
        
        // Build set of existing activity transaction hashes to avoid duplicates
        for act in existing_activities.iter() {
            if let Some(tx_hash) = act.get("transactionHash").and_then(|h| h.as_str()) {
                activity_ids.insert(tx_hash.to_string());
            }
        }
        
        // If we have a last activity timestamp, we can use it as a hint
        if let Some(last_timestamp) = &sync_state.last_activity_timestamp {
            println!("     📊 Checking for activities newer than {}", 
                last_timestamp.format("%Y-%m-%d %H:%M:%S UTC"));
        } else {
            println!("     📊 Fetching all activities (first sync)...");
        }
        
        loop {
            let url = format!(
                "https://data-api.polymarket.com/activity?user={}&limit={}&offset={}", 
                address, limit, offset
            );
            
            let activities = match self.fetch_with_retry(&url, http_client, "activity").await {
                Ok(text) => {
                    match serde_json::from_str::<Vec<Value>>(&text) {
                        Ok(act) => act,
                        Err(e) => {
                            warn!("Failed to parse activity batch: {}", e);
                            break;
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to fetch activities: {}", e);
                    break;
                }
            };
            
            let batch_size = activities.len();
            let mut new_in_batch = 0;
            let mut found_old_activity = false;
            
            // Check each activity
            for activity in activities {
                if let Some(tx_hash) = activity.get("transactionHash").and_then(|h| h.as_str()) {
                    if !activity_ids.contains(tx_hash) {
                        // Check if this activity is newer than our last sync
                        if let Some(last_ts) = &sync_state.last_activity_timestamp {
                            if let Some(act_ts_num) = activity.get("timestamp").and_then(|t| t.as_i64()) {
                                let act_ts = chrono::DateTime::from_timestamp(act_ts_num, 0)
                                    .map(|dt| dt.with_timezone(&chrono::Utc));
                                if let Some(act_ts_utc) = act_ts {
                                    if act_ts_utc <= *last_ts {
                                        found_old_activity = true;
                                        continue; // Skip activities we've already seen
                                    }
                                }
                            }
                        }
                        
                        activity_ids.insert(tx_hash.to_string());
                        new_activities.push(activity);
                        new_in_batch += 1;
                    } else {
                        found_old_activity = true;
                    }
                }
            }
            
            batch_count += 1;
            
            if new_in_batch > 0 {
                println!("     📈 Found {} new activities (batch {})", new_activities.len(), batch_count);
            }
            
            // If we got 0 results, we've reached the end
            if batch_size == 0 {
                println!("     ✅ Reached end of activity data");
                break;
            }
            
            // If we found activities we've already seen, we can stop
            if found_old_activity && sync_state.last_activity_timestamp.is_some() {
                println!("     ✅ Caught up to previously synced data");
                break;
            }
            
            // If we got a full batch but all were duplicates, we're likely at the end
            if batch_size == limit && new_in_batch == 0 {
                println!("     ✅ All activities in batch were duplicates - reached end");
                break;
            }
            
            // Progress indicator
            if batch_count % 5 == 0 {
                println!("     📈 Progress: {} new activities found so far", new_activities.len());
            }
            
            offset += limit;
            
            // Safety check - if we've gone through many batches with no new data, stop
            if new_in_batch == 0 && batch_count > 5 {
                println!("     ⚠️  No new activities found after {} batches - stopping", batch_count);
                break;
            }
            
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        
        // Store new activities in database
        if !new_activities.is_empty() {
            // Store in batches of 500
            for (i, chunk) in new_activities.chunks(500).enumerate() {
                let batch_id = existing_activities.len() / 500 + i;
                db.store_activities(address, chunk, batch_id).await?;
            }
            existing_activities.extend(new_activities.clone());
            println!("     💾 Stored {} new activities in database", new_activities.len());
        } else {
            println!("     ✅ No new activities to sync");
        }
        
        Ok(new_activities.len())
    }
    
    /// Create comprehensive data snapshot with error handling
    async fn create_data_snapshot(
        &self,
        address: &str,
        positions: &[Value],
        activity: &[Value],
    ) -> Result<()> {
        let timestamp = chrono::Utc::now();
        let snapshot_name = format!("{}_{}", address, timestamp.format("%Y%m%d_%H%M%S"));
        
        // Create snapshot directory
        let snapshot_dir = self.data_paths.root()
            .join("snapshots")
            .join(address)
            .join(&snapshot_name);
        
        // Ensure directory creation with detailed error handling
        if let Err(e) = tokio::fs::create_dir_all(&snapshot_dir).await {
            error!("Failed to create snapshot directory {}: {}", snapshot_dir.display(), e);
            return Err(e.into());
        }
        
        // Save positions snapshot with atomic write
        let positions_file = snapshot_dir.join("positions.json");
        match self.atomic_write_json(&positions_file, positions).await {
            Ok(_) => info!("Saved {} positions to snapshot", positions.len()),
            Err(e) => {
                error!("Failed to save positions snapshot: {}", e);
                return Err(e);
            }
        }
        
        // Save activity snapshot with atomic write
        let activity_file = snapshot_dir.join("activity.json");
        match self.atomic_write_json(&activity_file, activity).await {
            Ok(_) => info!("Saved {} activities to snapshot", activity.len()),
            Err(e) => {
                error!("Failed to save activity snapshot: {}", e);
                return Err(e);
            }
        }
        
        // Save metadata
        let metadata = serde_json::json!({
            "address": address,
            "timestamp": timestamp,
            "positions_count": positions.len(),
            "activity_count": activity.len(),
            "snapshot_name": snapshot_name,
            "polymarket_url": format!("https://polymarket.com/profile/{}", address),
            "data_integrity": {
                "positions_hash": self.calculate_data_hash(positions),
                "activity_hash": self.calculate_data_hash(activity),
                "created_at": timestamp
            }
        });
        
        let metadata_file = snapshot_dir.join("metadata.json");
        match self.atomic_write_json(&metadata_file, &metadata).await {
            Ok(_) => info!("Saved metadata to snapshot"),
            Err(e) => {
                error!("Failed to save metadata: {}", e);
                return Err(e);
            }
        }
        
        info!("Created comprehensive data snapshot for {} at {}", address, snapshot_dir.display());
        info!("Snapshot contains {} positions and {} activities", positions.len(), activity.len());
        
        Ok(())
    }
    
    /// Atomic write for JSON data with error handling
    async fn atomic_write_json<T: serde::Serialize + ?Sized>(
        &self,
        file_path: &std::path::Path,
        data: &T,
    ) -> Result<()> {
        let json = serde_json::to_string_pretty(data)
            .with_context(|| format!("Failed to serialize data for {}", file_path.display()))?;
        
        // Write to temporary file first
        let temp_path = file_path.with_extension("tmp");
        tokio::fs::write(&temp_path, &json).await
            .with_context(|| format!("Failed to write temporary file {}", temp_path.display()))?;
        
        // Atomic rename
        tokio::fs::rename(&temp_path, file_path).await
            .with_context(|| format!("Failed to rename {} to {}", temp_path.display(), file_path.display()))?;
        
        Ok(())
    }
    
    /// Calculate hash for data integrity verification
    fn calculate_data_hash<T: serde::Serialize + ?Sized>(&self, data: &T) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let json = serde_json::to_string(data).unwrap_or_default();
        let mut hasher = DefaultHasher::new();
        json.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
    
    /// Calculate comprehensive portfolio statistics with advanced analytics
    async fn calculate_portfolio_stats(
        &self,
        address: &str,
        positions: &[Value],
        activity: &[Value],
    ) -> Result<AddressStats> {
        info!("Calculating comprehensive portfolio analytics for {}", address);
        
        // Core metrics
        let core_metrics = self.calculate_core_metrics(positions).await;
        let trading_metrics = self.calculate_trading_metrics(activity).await;
        let risk_metrics = self.calculate_risk_metrics(positions, activity).await;
        
        info!("Portfolio analysis for {}:", address);
        info!("  Core: ${:.2} total, {} active positions, {:.1}% win rate", 
            core_metrics.total_value, core_metrics.active_positions, 
            core_metrics.win_rate.unwrap_or_default());
        info!("  Trading: {} total trades, ${:.2} realized P&L", 
            trading_metrics.total_trades, trading_metrics.total_realized_pnl);
        info!("  Risk: {:.2} sharpe ratio, {:.1}% volatility", 
            risk_metrics.sharpe_ratio.unwrap_or_default(), 
            risk_metrics.volatility.unwrap_or_default());
        
        let stats = AddressStats {
            total_value: rust_decimal::Decimal::from_str(&core_metrics.total_value.to_string()).unwrap_or_default(),
            active_positions: core_metrics.active_positions,
            active_orders: 0, // Gamma API doesn't provide active orders
            total_realized_pnl: rust_decimal::Decimal::from_str(&trading_metrics.total_realized_pnl.to_string()).unwrap_or_default(),
            trading_pnl: rust_decimal::Decimal::from_str(&trading_metrics.trading_pnl.to_string()).unwrap_or_default(),
            unmatched_wins_pnl: rust_decimal::Decimal::from_str(&trading_metrics.unmatched_wins_pnl.to_string()).unwrap_or_default(),
            total_unrealized_pnl: rust_decimal::Decimal::from_str(&core_metrics.total_unrealized_pnl.to_string()).unwrap_or_default(),
            win_rate: core_metrics.win_rate,
            total_trades: trading_metrics.total_trades,
            total_volume: rust_decimal::Decimal::from_str(&trading_metrics.total_volume.to_string()).unwrap_or_default(),
            buy_volume: rust_decimal::Decimal::from_str(&trading_metrics.buy_volume.to_string()).unwrap_or_default(),
            sell_volume: rust_decimal::Decimal::from_str(&trading_metrics.sell_volume.to_string()).unwrap_or_default(),
            last_trade: trading_metrics.last_trade,
            updated_at: chrono::Utc::now(),
        };
        
        // Log detailed analytics
        self.log_detailed_analytics(address, &core_metrics, &trading_metrics, &risk_metrics).await;
        
        Ok(stats)
    }
    
    /// Calculate core portfolio metrics
    async fn calculate_core_metrics(&self, positions: &[Value]) -> CoreMetrics {
        let mut total_value = 0.0;
        let mut total_unrealized_pnl = 0.0;
        let mut active_positions = 0;
        let mut winning_positions = 0;
        let total_positions = positions.len();
        
        for position in positions {
            if let Some(current_val) = position.get("currentValue").and_then(|v| v.as_f64()) {
                total_value += current_val;
                
                if current_val > 0.0 {
                    active_positions += 1;
                }
            }
            
            if let Some(cash_pnl) = position.get("cashPnl").and_then(|v| v.as_f64()) {
                if cash_pnl > 0.0 {
                    winning_positions += 1;
                }
            }
            
            if let Some(unrealized_pnl) = position.get("unrealizedPnl").and_then(|v| v.as_f64()) {
                total_unrealized_pnl += unrealized_pnl;
            }
        }
        
        let win_rate = if total_positions > 0 {
            Some(rust_decimal::Decimal::from(winning_positions * 100) / rust_decimal::Decimal::from(total_positions))
        } else {
            None
        };
        
        CoreMetrics {
            total_value,
            total_unrealized_pnl,
            active_positions,
            total_positions,
            win_rate,
        }
    }
    
    /// Calculate trading-specific metrics with proper P&L computation
    async fn calculate_trading_metrics(&self, activity: &[Value]) -> TradingMetrics {
        // Use HashMap to track positions by conditionId + outcome
        let mut positions: std::collections::HashMap<String, PositionTracker> = std::collections::HashMap::new();
        
        let mut buy_count = 0;
        let mut sell_count = 0;
        let mut redeem_count = 0;
        let mut conversion_count = 0;
        let mut merge_count = 0;
        let mut reward_count = 0;
        let mut trade_volumes = Vec::new();
        let mut trade_cash_flows = Vec::new();
        let mut buy_volume = 0.0;
        let mut sell_volume = 0.0;
        let mut total_volume = 0.0;
        let mut reward_income = 0.0;
        
        let mut last_trade = None;
        
        info!("Analyzing {} activity items for trading metrics", activity.len());
        
        for activity_item in activity {
            let activity_type = activity_item.get("type").and_then(|t| t.as_str()).unwrap_or("");
            let condition_id = activity_item.get("conditionId").and_then(|c| c.as_str()).unwrap_or("");
            let outcome = activity_item.get("outcome").and_then(|o| o.as_str()).unwrap_or("");
            let market_slug = activity_item.get("slug").and_then(|s| s.as_str()).unwrap_or("");
            
            // Create unique position key
            let position_key = format!("{}:{}", condition_id, outcome);
            
            match activity_type {
                "TRADE" => {
                    let side = activity_item.get("side").and_then(|s| s.as_str()).unwrap_or("");
                    let usdc_size = activity_item.get("usdcSize").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let size = activity_item.get("size").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    
                    trade_volumes.push(usdc_size);
                    
                    match side {
                        "BUY" => {
                            buy_count += 1;
                            buy_volume += usdc_size;
                            total_volume += usdc_size;
                            
                            // Track position for P&L calculation
                            let position = positions.entry(position_key.clone())
                                .or_insert_with(|| PositionTracker::new(market_slug.to_string(), outcome.to_string()));
                            position.add_buy(size, usdc_size);
                            
                            info!("BUY: {} shares of {} for ${:.2}", size, market_slug, usdc_size);
                        }
                        "SELL" => {
                            sell_count += 1;
                            sell_volume += usdc_size;
                            total_volume += usdc_size;
                            
                            // Track position exit
                            let position = positions.entry(position_key.clone())
                                .or_insert_with(|| PositionTracker::new(market_slug.to_string(), outcome.to_string()));
                            position.add_exit(size, usdc_size);
                            
                            info!("SELL: {} shares of {} for ${:.2}", size, market_slug, usdc_size);
                        }
                        _ => {
                            warn!("Unknown trade side: {}", side);
                        }
                    }
                }
                "REDEEM" => {
                    redeem_count += 1;
                    let usdc_size = activity_item.get("usdcSize").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let size = activity_item.get("size").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    
                    sell_volume += usdc_size;
                    total_volume += usdc_size;
                    
                    // Track position exit (winning position redeemed)
                    let position = positions.entry(position_key.clone())
                        .or_insert_with(|| PositionTracker::new(market_slug.to_string(), outcome.to_string()));
                    position.add_exit(size, usdc_size);
                    
                    info!("REDEEM: {} shares of {} for ${:.2} (winning position)", size, market_slug, usdc_size);
                }
                "CONVERSION" => {
                    conversion_count += 1;
                    let usdc_size = activity_item.get("usdcSize").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let size = activity_item.get("size").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    
                    sell_volume += usdc_size;
                    total_volume += usdc_size;
                    
                    // Track position exit
                    let position = positions.entry(position_key.clone())
                        .or_insert_with(|| PositionTracker::new(market_slug.to_string(), outcome.to_string()));
                    position.add_exit(size, usdc_size);
                    
                    info!("CONVERSION: {} shares converted for ${:.2}", size, usdc_size);
                }
                "MERGE" => {
                    merge_count += 1;
                    let usdc_size = activity_item.get("usdcSize").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let size = activity_item.get("size").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    
                    sell_volume += usdc_size;
                    total_volume += usdc_size;
                    
                    // MERGE is special - it's market neutral, not a position exit
                    // We'll still track it but separately
                    info!("MERGE: {} shares merged for ${:.2} (market neutral)", size, usdc_size);
                }
                "REWARD" => {
                    reward_count += 1;
                    let usdc_size = activity_item.get("usdcSize").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    
                    // REWARD activities are winning positions where we don't have the corresponding buy
                    // This happens when the buy was before our data collection started
                    reward_income += usdc_size;
                    sell_volume += usdc_size;
                    total_volume += usdc_size;
                    
                    info!("REWARD: ${:.2} (unmatched winning position)", usdc_size);
                }
                _ => continue,
            }
            
            // Update last trade timestamp for all trading/financial activities
            if matches!(activity_type, "TRADE" | "REDEEM" | "CONVERSION" | "MERGE" | "REWARD") {
                if let Some(ts_val) = activity_item.get("timestamp") {
                    let timestamp = if let Some(ts_num) = ts_val.as_i64() {
                        // Unix timestamp
                        chrono::DateTime::from_timestamp(ts_num, 0)
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                    } else if let Some(ts_str) = ts_val.as_str() {
                        // RFC3339 string
                        chrono::DateTime::parse_from_rfc3339(ts_str).ok()
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                    } else {
                        None
                    };
                    
                    if let Some(utc_dt) = timestamp {
                        if last_trade.is_none() || last_trade.unwrap() < utc_dt {
                            last_trade = Some(utc_dt);
                        }
                    }
                }
            }
        }
        
        // Calculate P&L from positions
        let mut total_realized_pnl = 0.0;
        let mut position_count = 0;
        let mut winning_positions = 0;
        
        for (key, position) in &positions {
            let position_pnl = position.get_realized_pnl();
            if position_pnl != 0.0 {
                total_realized_pnl += position_pnl;
                position_count += 1;
                if position_pnl > 0.0 {
                    winning_positions += 1;
                }
                trade_cash_flows.push(position_pnl);
                
                info!("Position {}: {} - P&L: ${:.2} (bought {} @ avg ${:.2}, sold {} @ avg ${:.2})",
                    key, 
                    position.market_slug,
                    position_pnl,
                    position.total_shares,
                    if position.total_shares > 0.0 { position.total_cost / position.total_shares } else { 0.0 },
                    position.total_shares_exited,
                    if position.total_shares_exited > 0.0 { position.total_proceeds / position.total_shares_exited } else { 0.0 }
                );
            }
        }
        
        // Add rewards to total (but keep separate for reporting)
        let total_with_rewards = total_realized_pnl + reward_income;
        
        let total_trades = buy_count + sell_count + redeem_count + conversion_count + merge_count + reward_count;
        let avg_trade_size = if !trade_volumes.is_empty() {
            trade_volumes.iter().sum::<f64>() / trade_volumes.len() as f64
        } else {
            0.0
        };
        
        info!("Trading metrics calculated: {} activities total", total_trades);
        info!("  - TRADE: {} (Buy: {}, Sell: {})", buy_count + sell_count, buy_count, sell_count);
        info!("  - REDEEM: {}, CONVERSION: {}, MERGE: {}, REWARD: {}", 
            redeem_count, conversion_count, merge_count, reward_count);
        info!("  - Matched position P&L: ${:.2}", total_realized_pnl);
        info!("  - Unmatched winning positions: ${:.2}", reward_income);
        info!("  - Total P&L: ${:.2}", total_with_rewards);
        info!("  - Positions with P&L: {} ({} winning)", position_count, winning_positions);
        
        TradingMetrics {
            total_realized_pnl: total_with_rewards,  // Include rewards in final P&L
            trading_pnl: total_realized_pnl,         // Matched position P&L
            unmatched_wins_pnl: reward_income,              // Unmatched winning positions
            total_trades,
            buy_count,
            sell_count: sell_count + redeem_count + conversion_count + merge_count, // Don't include rewards as sells
            avg_trade_size,
            trade_profits: trade_cash_flows,
            last_trade,
            total_volume,
            buy_volume,
            sell_volume,
        }
    }
    
    /// Calculate risk and performance metrics
    async fn calculate_risk_metrics(&self, positions: &[Value], activity: &[Value]) -> RiskMetrics {
        let trading_metrics = self.calculate_trading_metrics(activity).await;
        
        // Calculate volatility from trade profits
        let volatility = if trading_metrics.trade_profits.len() > 1 {
            let mean_profit = trading_metrics.trade_profits.iter().sum::<f64>() / trading_metrics.trade_profits.len() as f64;
            let variance = trading_metrics.trade_profits.iter()
                .map(|profit| (profit - mean_profit).powi(2))
                .sum::<f64>() / trading_metrics.trade_profits.len() as f64;
            Some(variance.sqrt() * 100.0) // Convert to percentage
        } else {
            None
        };
        
        // Calculate Sharpe ratio (simplified - assumes risk-free rate of 0)
        let sharpe_ratio = if let Some(vol) = volatility {
            if vol > 0.0 {
                let avg_return = if !trading_metrics.trade_profits.is_empty() {
                    trading_metrics.trade_profits.iter().sum::<f64>() / trading_metrics.trade_profits.len() as f64
                } else {
                    0.0
                };
                Some(avg_return / (vol / 100.0))
            } else {
                None
            }
        } else {
            None
        };
        
        // Calculate maximum position concentration
        let max_position_percentage = if !positions.is_empty() {
            let total_value: f64 = positions.iter()
                .filter_map(|p| p.get("currentValue").and_then(|v| v.as_f64()))
                .sum();
            
            if total_value > 0.0 {
                let max_position = positions.iter()
                    .filter_map(|p| p.get("currentValue").and_then(|v| v.as_f64()))
                    .fold(0.0, f64::max);
                Some((max_position / total_value) * 100.0)
            } else {
                None
            }
        } else {
            None
        };
        
        RiskMetrics {
            volatility,
            sharpe_ratio,
            max_position_percentage,
        }
    }
    
    /// Log detailed analytics for debugging and monitoring
    async fn log_detailed_analytics(
        &self,
        address: &str,
        core: &CoreMetrics,
        trading: &TradingMetrics,
        risk: &RiskMetrics,
    ) {
        info!("📊 Detailed Analytics for {}:", address);
        info!("  💰 Portfolio Value: ${:.2}", core.total_value);
        info!("  📈 Active Positions: {} of {} total", core.active_positions, core.total_positions);
        info!("  🎯 Win Rate: {:.1}%", core.win_rate.map_or(0.0, |wr| wr.to_string().parse::<f64>().unwrap_or(0.0)));
        info!("  💹 Realized P&L: ${:.2}", trading.total_realized_pnl);
        info!("  🔄 Total Trades: {} (Buy: {}, Sell: {})", trading.total_trades, trading.buy_count, trading.sell_count);
        info!("  📊 Total Volume: ${:.2} (Buy: ${:.2}, Sell: ${:.2})", 
            trading.total_volume, trading.buy_volume, trading.sell_volume);
        
        if let Some(avg_size) = (trading.avg_trade_size > 0.0).then_some(trading.avg_trade_size) {
            info!("  📊 Avg Trade Size: ${:.2}", avg_size);
        }
        
        if let Some(vol) = risk.volatility {
            info!("  📉 Volatility: {:.1}%", vol);
        }
        
        if let Some(sharpe) = risk.sharpe_ratio {
            info!("  ⚖️ Sharpe Ratio: {:.2}", sharpe);
        }
        
        if let Some(concentration) = risk.max_position_percentage {
            info!("  🎯 Max Position: {:.1}% of portfolio", concentration);
        }
        
        if let Some(last_trade) = trading.last_trade {
            info!("  🕐 Last Trade: {}", last_trade.format("%Y-%m-%d %H:%M:%S UTC"));
        }
    }
    
    /// Fetch with exponential backoff retry logic
    async fn fetch_with_retry(
        &self,
        url: &str,
        http_client: &Client,
        data_type: &str,
    ) -> Result<String> {
        let max_retries = 3;
        let mut retry_count = 0;
        let mut backoff_ms = 1000; // Start with 1 second
        
        loop {
            match http_client.get(url).send().await {
                Ok(response) => {
                    let status = response.status();
                    
                    if status.is_success() {
                        match response.text().await {
                            Ok(text) => {
                                if retry_count > 0 {
                                    info!("Successfully fetched {} after {} retries", data_type, retry_count);
                                }
                                return Ok(text);
                            }
                            Err(e) => {
                                warn!("Failed to read response text for {}: {}", data_type, e);
                                if retry_count >= max_retries {
                                    return Err(e.into());
                                }
                            }
                        }
                    } else if status.as_u16() == 429 {
                        // Rate limited - use longer backoff
                        warn!("Rate limited when fetching {}, backing off for {}ms", data_type, backoff_ms * 2);
                        tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms * 2)).await;
                    } else if status.is_server_error() {
                        warn!("Server error {} when fetching {}, retrying in {}ms", status, data_type, backoff_ms);
                        if retry_count >= max_retries {
                            return Err(anyhow::anyhow!("Server error after {} retries: {}", max_retries, status));
                        }
                    } else {
                        return Err(anyhow::anyhow!("HTTP error {}: {}", status, url));
                    }
                }
                Err(e) => {
                    warn!("Network error fetching {}: {}", data_type, e);
                    if retry_count >= max_retries {
                        return Err(e.into());
                    }
                }
            }
            
            retry_count += 1;
            if retry_count <= max_retries {
                info!("Retrying {} fetch (attempt {}/{}) in {}ms", data_type, retry_count + 1, max_retries + 1, backoff_ms);
                tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
                backoff_ms *= 2; // Exponential backoff
            }
        }
    }
}

/// Handle to communicate with address book service
#[derive(Clone)]
pub struct AddressBookServiceHandle {
    sender: mpsc::Sender<AddressBookCommand>,
}

impl AddressBookServiceHandle {
    pub fn new(sender: mpsc::Sender<AddressBookCommand>) -> Self {
        Self { sender }
    }
    
    pub async fn send(&self, command: AddressBookCommand) -> Result<()> {
        self.sender.send(command).await
            .context("Failed to send command to address book service")
    }
}

/// Start address book service
pub async fn start_address_book_service(
    data_paths: DataPaths,
    portfolio_service: Option<Arc<PortfolioServiceHandle>>,
    _gamma_tracker: Option<()>, // Placeholder
) -> Result<AddressBookServiceHandle> {
    let (tx, rx) = mpsc::channel(100);
    
    let service = AddressBookService::new(data_paths, portfolio_service, None, rx).await?;
    
    tokio::spawn(async move {
        service.run().await;
    });
    
    Ok(AddressBookServiceHandle::new(tx))
}

/// Global address book service instance
static ADDRESS_BOOK_SERVICE: tokio::sync::OnceCell<Arc<AddressBookServiceHandle>> = tokio::sync::OnceCell::const_new();

/// Get or create address book service handle
pub async fn get_address_book_service(
    data_paths: DataPaths,
    portfolio_service: Option<Arc<PortfolioServiceHandle>>,
    _gamma_tracker: Option<()>, // Placeholder
) -> Result<Arc<AddressBookServiceHandle>> {
    ADDRESS_BOOK_SERVICE.get_or_try_init(|| async {
        let handle = start_address_book_service(data_paths, portfolio_service, None).await?;
        Ok(Arc::new(handle))
    }).await.cloned()
}