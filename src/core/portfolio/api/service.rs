//! Portfolio service actor with channel-based communication
//!
//! This service manages all portfolio operations through a message-passing interface.
//! It handles trades, orders, balances, and provides a unified interface for all
//! portfolio-related operations across the application.

use anyhow::Result;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::auth;
use crate::core::portfolio::api::orders::PolymarketOrder;
use crate::core::portfolio::api::types::*;
use crate::core::portfolio::storage::{
    AccountBalances, PortfolioSnapshot, PortfolioStorage, RawDataStorage, SnapshotMetadata,
    SnapshotReason,
};
use crate::core::portfolio::types::*;
use crate::data_paths::DataPaths;

/// Portfolio service actor
pub struct PortfolioService {
    /// Data storage directory
    data_paths: DataPaths,
    /// User address
    address: String,
    /// API host
    host: String,
    /// Portfolio storage
    storage: PortfolioStorage,
    /// In-memory state
    state: RwLock<PortfolioState>,
    /// Command receiver
    command_rx: mpsc::Receiver<PortfolioCommand>,
    /// Raw storage for trades and orders
    raw_storage: RawDataStorage,
}

impl PortfolioService {
    /// Create new portfolio service
    pub fn new(
        data_paths: DataPaths,
        address: String,
        host: String,
        command_rx: mpsc::Receiver<PortfolioCommand>,
    ) -> Self {
        let storage = PortfolioStorage::new(data_paths.root(), &address);
        let raw_storage = RawDataStorage::new(data_paths.root());

        let state = RwLock::new(PortfolioState {
            positions: Vec::new(),
            active_orders: Vec::new(),
            stats: PortfolioStats {
                total_balance: Decimal::ZERO,
                available_balance: Decimal::ZERO,
                locked_balance: Decimal::ZERO,
                total_positions: 0,
                open_positions: 0,
                total_realized_pnl: Decimal::ZERO,
                total_unrealized_pnl: Decimal::ZERO,
                total_fees_paid: Decimal::ZERO,
                win_rate: None,
                average_win: None,
                average_loss: None,
                sharpe_ratio: None,
                last_updated: Utc::now(),
            },
            balances: AccountBalances::default(),
            last_updated: Utc::now(),
            is_synced: false,
        });

        Self {
            data_paths,
            address,
            host,
            storage,
            state,
            command_rx,
            raw_storage,
        }
    }

    /// Start the portfolio service actor
    pub async fn run(mut self) -> Result<()> {
        info!("Starting portfolio service for address: {}", self.address);

        // Initialize storage
        self.storage.init_directories().await?;
        self.raw_storage.init_directories().await?;

        // Load initial state
        if let Err(e) = self.load_initial_state().await {
            warn!("Failed to load initial state: {}", e);
        }

        // Start periodic refresh timer
        let mut refresh_interval = interval(Duration::from_secs(30));

        loop {
            tokio::select! {
                // Handle commands
                command = self.command_rx.recv() => {
                    match command {
                        Some(cmd) => {
                            if let Err(e) = self.handle_command(cmd).await {
                                error!("Failed to handle command: {}", e);
                            }
                        }
                        None => {
                            info!("Command channel closed, stopping portfolio service");
                            break;
                        }
                    }
                }

                // Periodic refresh
                _ = refresh_interval.tick() => {
                    if let Err(e) = self.refresh_data().await {
                        warn!("Periodic refresh failed: {}", e);
                    }
                }
            }
        }

        info!("Portfolio service stopped");
        Ok(())
    }

    /// Handle incoming commands
    async fn handle_command(&mut self, command: PortfolioCommand) -> Result<()> {
        match command {
            PortfolioCommand::Buy {
                market_id,
                token_id,
                price,
                size,
                response,
            } => {
                let result = self.handle_buy(&market_id, &token_id, price, size).await;
                let _ = response.send(result);
            }

            PortfolioCommand::Sell {
                market_id,
                token_id,
                price,
                size,
                response,
            } => {
                let result = self.handle_sell(&market_id, &token_id, price, size).await;
                let _ = response.send(result);
            }

            PortfolioCommand::Cancel { order_id, response } => {
                let result = self.handle_cancel(&order_id).await;
                let _ = response.send(result);
            }

            PortfolioCommand::GetPortfolioState { response } => {
                let state = self.state.read().await.clone();
                let _ = response.send(state);
            }

            PortfolioCommand::GetActiveOrders { response } => {
                let state = self.state.read().await;
                let _ = response.send(state.active_orders.clone());
            }

            PortfolioCommand::GetTradeHistory {
                start_date,
                end_date,
                response,
            } => {
                let trades = self.get_trade_history(start_date, end_date).await;
                let _ = response.send(trades);
            }

            PortfolioCommand::RefreshData { response } => {
                let result = self.refresh_data().await;
                let _ = response.send(result);
            }

            PortfolioCommand::CreateSnapshot { reason, response } => {
                let result = self.create_snapshot(&reason).await;
                let _ = response.send(result);
            }
        }

        Ok(())
    }

    /// Load initial state from storage
    async fn load_initial_state(&mut self) -> Result<()> {
        info!("Loading initial portfolio state...");

        // Try to load latest snapshot
        if let Some(snapshot) = self.storage.load_latest_snapshot().await? {
            let mut state = self.state.write().await;
            state.positions = snapshot.positions;
            state.stats = snapshot.stats;
            state.balances = snapshot.balances;
            state.last_updated = snapshot.timestamp;

            info!("Loaded portfolio state from snapshot: {}", snapshot.timestamp);
        }

        // Load current positions if no snapshot
        if let Ok(positions) = self.storage.load_positions().await {
            let mut state = self.state.write().await;
            if state.positions.is_empty() {
                state.positions = positions;
                info!("Loaded {} positions from storage", state.positions.len());
            }
        }

        Ok(())
    }

    /// Refresh data from API
    async fn refresh_data(&mut self) -> Result<()> {
        debug!("Refreshing portfolio data from API...");

        // Get authenticated client
        let _client = auth::get_authenticated_client(&self.host, &self.data_paths).await?;

        // Fetch balance
        if let Ok(balance_info) = crate::core::portfolio::api::orders::fetch_balance(
            &self.host,
            &self.data_paths,
            &self.address,
        )
        .await
        {
            let mut state = self.state.write().await;
            state.balances = AccountBalances {
                total_value: balance_info.equity_total,
                available_cash: balance_info.cash,
                locked_in_orders: balance_info.bets,
                position_value: balance_info.equity_total - balance_info.cash,
                last_updated: Utc::now(),
            };
            state.last_updated = Utc::now();
            state.is_synced = true;

            debug!(
                "Updated balance: total={}, cash={}, locked={}",
                balance_info.equity_total, balance_info.cash, balance_info.bets
            );
        }

        // TODO: Fetch orders and trades when API endpoints are available

        Ok(())
    }

    /// Create portfolio snapshot
    async fn create_snapshot(&self, reason: &str) -> Result<String> {
        let state = self.state.read().await;

        let snapshot = PortfolioSnapshot {
            timestamp: Utc::now(),
            address: self.address.clone(),
            positions: state.positions.clone(),
            active_orders: Vec::new(), // TODO: Convert ActiveOrder to PolymarketOrder
            stats: state.stats.clone(),
            balances: state.balances.clone(),
            metadata: SnapshotMetadata {
                version: "1.0".to_string(),
                reason: SnapshotReason::Manual,
                previous_snapshot: None,
                previous_hash: None,
            },
        };

        let filename = self.storage.save_snapshot(&snapshot).await?;
        info!("Created snapshot: {} (reason: {})", filename, reason);
        Ok(filename)
    }

    /// Handle buy order
    async fn handle_buy(
        &self,
        market_id: &str,
        token_id: &str,
        price: Decimal,
        size: Decimal,
    ) -> Result<String> {
        info!(
            "Processing buy order: market={}, token={}, price={}, size={}",
            market_id, token_id, price, size
        );

        // TODO: Implement actual buy order via API
        // For now, return a mock order ID
        let order_id = Uuid::new_v4().to_string();

        // Store order in raw storage
        let mock_order = PolymarketOrder {
            id: order_id.clone(),
            owner: self.address.clone(),
            market: market_id.to_string(),
            asset_id: token_id.to_string(),
            side: "BUY".to_string(),
            price,
            size_structured: size,
            size_matched: "0".to_string(),
            status: "PENDING".to_string(),
            created_at: Utc::now().timestamp() as u64,
            maker_address: self.address.clone(),
            outcome: "YES".to_string(), // TODO: Determine actual outcome
            expiration: "0".to_string(),
            order_type: "LIMIT".to_string(),
            associate_trades: Vec::new(),
            fee_rate_bps: Some(100),
            nonce: None,
            condition_id: None,
            token_id: Some(token_id.to_string()),
            question_id: None,
        };

        self.raw_storage.store_order(&mock_order).await?;

        Ok(order_id)
    }

    /// Handle sell order
    async fn handle_sell(
        &self,
        market_id: &str,
        token_id: &str,
        price: Decimal,
        size: Decimal,
    ) -> Result<bool> {
        info!(
            "Processing sell order: market={}, token={}, price={}, size={}",
            market_id, token_id, price, size
        );

        // TODO: Implement actual sell order via API
        // For now, return a mock order ID
        let order_id = Uuid::new_v4().to_string();

        // Store order in raw storage similar to buy
        let mock_order = PolymarketOrder {
            id: order_id.clone(),
            owner: self.address.clone(),
            market: market_id.to_string(),
            asset_id: token_id.to_string(),
            side: "SELL".to_string(),
            price,
            size_structured: size,
            size_matched: "0".to_string(),
            status: "PENDING".to_string(),
            created_at: Utc::now().timestamp() as u64,
            maker_address: self.address.clone(),
            outcome: "NO".to_string(), // TODO: Determine actual outcome
            expiration: "0".to_string(),
            order_type: "LIMIT".to_string(),
            associate_trades: Vec::new(),
            fee_rate_bps: Some(100),
            nonce: None,
            condition_id: None,
            token_id: Some(token_id.to_string()),
            question_id: None,
        };

        self.raw_storage.store_order(&mock_order).await?;

        Ok(true)
    }

    /// Handle cancel order
    async fn handle_cancel(&self, order_id: &str) -> Result<bool> {
        info!("Processing cancel order: {}", order_id);

        // TODO: Implement actual cancel order via API
        // For now, return success
        Ok(true)
    }

    /// Get trade history with optional date filtering
    async fn get_trade_history(
        &self,
        start_date: Option<DateTime<Utc>>,
        end_date: Option<DateTime<Utc>>,
    ) -> Vec<TradeExecution> {
        // Load all trades from raw storage
        let all_trades = match self.raw_storage.load_all_trades().await {
            Ok(trades) => trades,
            Err(e) => {
                warn!("Failed to load trade history: {}", e);
                return Vec::new();
            }
        };

        // Filter by date range if provided
        let filtered_trades = all_trades
            .into_iter()
            .filter(|trade| {
                if let Some(start) = start_date {
                    if trade.timestamp < start {
                        return false;
                    }
                }
                if let Some(end) = end_date {
                    if trade.timestamp > end {
                        return false;
                    }
                }
                true
            })
            .collect();

        filtered_trades
    }
}

/// Start portfolio service and return handle
pub async fn start_portfolio_service(
    data_paths: DataPaths,
    address: String,
    host: String,
) -> Result<PortfolioServiceHandle> {
    let (command_tx, command_rx) = mpsc::channel(100);
    let handle = PortfolioServiceHandle::new(command_tx);

    let service = PortfolioService::new(data_paths, address, host, command_rx);

    // Spawn service in background
    tokio::spawn(async move {
        if let Err(e) = service.run().await {
            error!("Portfolio service error: {}", e);
        }
    });

    info!("Portfolio service started");
    Ok(handle)
}