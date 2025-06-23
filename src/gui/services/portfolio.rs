//! Portfolio service for managing portfolio data in GUI with local storage and HTTP refresh

use anyhow::{anyhow, Result};
use rust_decimal::Decimal;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::config;
use crate::data_paths::DataPaths;
use crate::ethereum_utils;
use crate::execution::orders::{EnhancedOrder, OrderManager};
use crate::portfolio::{PortfolioStats, PortfolioStorage, Position};

/// Placeholder for balance information - would need proper implementation
#[derive(Debug, Clone)]
pub struct BalanceInfo {
    pub cash: Decimal,
    pub bets: Decimal,
    pub equity_total: Decimal,
}

/// Service for managing portfolio data in the GUI
#[derive(Clone)]
pub struct PortfolioService {
    data_paths: DataPaths,
    host: String,
    user_address: Arc<RwLock<Option<String>>>,
    orders: Arc<RwLock<Vec<EnhancedOrder>>>,
    positions: Arc<RwLock<Vec<Position>>>,
    stats: Arc<RwLock<Option<PortfolioStats>>>,
    balance: Arc<RwLock<Option<BalanceInfo>>>,
    is_initialized: Arc<RwLock<bool>>,
    last_refresh: Arc<RwLock<Option<Instant>>>,
    is_refreshing: Arc<RwLock<bool>>,
    order_manager: Arc<RwLock<Option<OrderManager>>>,
    portfolio_storage: Arc<RwLock<Option<PortfolioStorage>>>,
}

impl PortfolioService {
    /// Create new portfolio service
    pub fn new(host: String, data_paths: DataPaths) -> Self {
        Self {
            data_paths,
            host,
            user_address: Arc::new(RwLock::new(None)),
            orders: Arc::new(RwLock::new(Vec::new())),
            positions: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(None)),
            balance: Arc::new(RwLock::new(None)),
            is_initialized: Arc::new(RwLock::new(false)),
            last_refresh: Arc::new(RwLock::new(None)),
            is_refreshing: Arc::new(RwLock::new(false)),
            order_manager: Arc::new(RwLock::new(None)),
            portfolio_storage: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialize the service and load existing local data
    pub async fn init(&self) -> Result<()> {
        info!("Initializing portfolio service");

        // Load private key to derive user address
        let private_key = config::load_private_key(&self.data_paths)
            .await
            .map_err(|e| anyhow!("No private key found. Run 'cargo run -- init' first: {}", e))?;

        // Derive user's Ethereum address
        let address = ethereum_utils::derive_address_from_private_key(&private_key)?;

        {
            let mut user_address = self.user_address.write().await;
            *user_address = Some(address.clone());
        }

        // Initialize order manager and portfolio storage
        let order_manager = OrderManager::new();
        let portfolio_storage = PortfolioStorage::new(self.data_paths.root(), &address);

        {
            let mut om = self.order_manager.write().await;
            *om = Some(order_manager);
        }

        {
            let mut ps = self.portfolio_storage.write().await;
            *ps = Some(portfolio_storage);
        }

        // Load existing data from local storage
        self.load_existing_data().await?;

        {
            let mut is_initialized = self.is_initialized.write().await;
            *is_initialized = true;
        }

        info!("Portfolio service initialized for user: {}", address);
        Ok(())
    }

    /// Load existing data from local storage
    async fn load_existing_data(&self) -> Result<()> {
        info!("Loading existing portfolio data from local storage");

        // Load existing data
        if let Some(portfolio_storage) = self.portfolio_storage.read().await.as_ref() {
            // Load existing positions
            match portfolio_storage.load_positions().await {
                Ok(existing_positions) => {
                    {
                        let mut positions = self.positions.write().await;
                        *positions = existing_positions.clone();
                    }
                    info!(
                        "Loaded {} existing positions from local storage",
                        existing_positions.len()
                    );
                }
                Err(e) => {
                    warn!("No existing positions found in local storage: {}", e);
                }
            }

            // TODO: Load existing stats (method doesn't exist yet)
            info!("Stats loading not yet implemented");
        }

        Ok(())
    }

    /// Refresh portfolio data via HTTP API
    pub async fn refresh_data(&self) -> Result<()> {
        {
            let mut is_refreshing = self.is_refreshing.write().await;
            *is_refreshing = true;
        }

        info!("Refreshing portfolio data via HTTP API");

        // Get user address
        let user_address = {
            let addr = self.user_address.read().await;
            addr.clone()
                .ok_or_else(|| anyhow!("Portfolio service not initialized"))?
        };

        // Get order manager
        let order_manager = {
            let om = self.order_manager.read().await;
            om.clone()
                .ok_or_else(|| anyhow!("Order manager not initialized"))?
        };

        // Get portfolio storage
        let portfolio_storage = {
            let ps = self.portfolio_storage.read().await;
            ps.as_ref()
                .ok_or_else(|| anyhow!("Portfolio storage not initialized"))?
                .clone()
        };

        // Fetch orders from API
        match order_manager
            .fetch_orders(&self.host, &self.data_paths, &user_address)
            .await
        {
            Ok(fetched_orders) => {
                info!(
                    "Successfully fetched {} orders from API",
                    fetched_orders.len()
                );

                // Update orders in memory
                {
                    let mut orders = self.orders.write().await;
                    *orders = fetched_orders.clone();
                }

                // Convert EnhancedOrder to PolymarketOrder for storage compatibility
                let poly_orders: Vec<crate::portfolio::orders_api::PolymarketOrder> =
                    fetched_orders
                        .iter()
                        .map(|enhanced_order| {
                            self.convert_enhanced_to_polymarket_order(enhanced_order)
                        })
                        .collect();

                // Save orders to local storage
                if let Err(e) = portfolio_storage._save_active_orders(&poly_orders).await {
                    warn!("Failed to save orders to local storage: {}", e);
                } else {
                    info!("Saved {} orders to local storage", poly_orders.len());
                }

                // Update positions from orders using the portfolio reconciler
                match self
                    .update_positions_from_orders(&poly_orders, &portfolio_storage)
                    .await
                {
                    Ok(positions_count) => {
                        info!("Updated {} positions from orders", positions_count);
                    }
                    Err(e) => {
                        warn!("Failed to update positions from orders: {}", e);
                    }
                }

                // Try to fetch and update balance information
                match crate::portfolio::orders_api::fetch_balance(
                    &self.host,
                    &self.data_paths,
                    &user_address,
                )
                .await
                {
                    Ok(api_balance) => {
                        info!("Successfully fetched balance from API");
                        let balance_info = BalanceInfo {
                            cash: api_balance.cash,
                            bets: api_balance.bets,
                            equity_total: api_balance.equity_total,
                        };

                        {
                            let mut balance = self.balance.write().await;
                            *balance = Some(balance_info);
                        }
                    }
                    Err(e) => {
                        info!("Balance API not available: {}", e);
                        // Balance API may not be reliable, this is expected
                    }
                }
            }
            Err(e) => {
                error!("Failed to fetch orders from API: {}", e);
                // Still complete the refresh cycle even if it failed
            }
        }

        // Update refresh time
        {
            let mut last_refresh = self.last_refresh.write().await;
            *last_refresh = Some(Instant::now());
        }

        {
            let mut is_refreshing = self.is_refreshing.write().await;
            *is_refreshing = false;
        }

        info!("Portfolio data refresh completed");
        Ok(())
    }

    /// Update positions from orders using the portfolio reconciler
    async fn update_positions_from_orders(
        &self,
        orders: &[crate::portfolio::orders_api::PolymarketOrder],
        portfolio_storage: &crate::portfolio::PortfolioStorage,
    ) -> Result<usize> {
        use crate::portfolio::PositionReconciler;

        // Use position reconciler to generate positions from orders
        let mut reconciler = PositionReconciler::_new();
        let positions = reconciler._reconcile_from_orders(orders)?;

        // Update positions in memory
        {
            let mut pos = self.positions.write().await;
            *pos = positions.clone();
        }

        // Save positions to storage
        portfolio_storage._save_positions(&positions).await?;

        // Calculate and update stats
        let stats = reconciler._calculate_stats();
        {
            let mut stats_lock = self.stats.write().await;
            *stats_lock = Some(stats);
        }

        Ok(positions.len())
    }

    /// Convert EnhancedOrder to PolymarketOrder for storage compatibility
    fn convert_enhanced_to_polymarket_order(
        &self,
        enhanced: &crate::execution::orders::EnhancedOrder,
    ) -> crate::portfolio::orders_api::PolymarketOrder {
        use rust_decimal::prelude::FromPrimitive;
        use rust_decimal::Decimal;

        // Extract additional fields for conversion
        let market = enhanced
            .additional_fields
            .get("market")
            .and_then(|v| v.as_str())
            .unwrap_or(&enhanced.asset_id)
            .to_string();

        let owner = enhanced
            .additional_fields
            .get("owner")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let outcome = enhanced
            .additional_fields
            .get("outcome")
            .and_then(|v| v.as_str())
            .unwrap_or("YES")
            .to_string();

        let order_type = enhanced
            .additional_fields
            .get("order_type")
            .and_then(|v| v.as_str())
            .unwrap_or("LIMIT")
            .to_string();

        let expiration = enhanced
            .additional_fields
            .get("expiration")
            .and_then(|v| v.as_str())
            .unwrap_or("0")
            .to_string();

        let maker_address = enhanced
            .additional_fields
            .get("maker_address")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let fee_rate_bps = enhanced
            .additional_fields
            .get("fee_rate_bps")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32);

        let condition_id = enhanced
            .additional_fields
            .get("condition_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let question_id = enhanced
            .additional_fields
            .get("question_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Convert side
        let side = match enhanced.side {
            crate::execution::orders::OrderSide::Buy => "BUY".to_string(),
            crate::execution::orders::OrderSide::Sell => "SELL".to_string(),
        };

        // Convert status
        let status = match enhanced.status {
            crate::execution::orders::OrderStatus::Open => "OPEN".to_string(),
            crate::execution::orders::OrderStatus::Filled => "FILLED".to_string(),
            crate::execution::orders::OrderStatus::Cancelled => "CANCELLED".to_string(),
            crate::execution::orders::OrderStatus::PartiallyFilled => {
                "PARTIALLY_FILLED".to_string()
            }
            crate::execution::orders::OrderStatus::Rejected => "REJECTED".to_string(),
            crate::execution::orders::OrderStatus::Pending => "PENDING".to_string(),
        };

        crate::portfolio::orders_api::PolymarketOrder {
            id: enhanced.id.clone(),
            owner,
            market,
            asset_id: enhanced.asset_id.clone(),
            side,
            price: Decimal::from_f64(enhanced.price).unwrap_or_default(),
            size_structured: Decimal::from_f64(enhanced.original_size).unwrap_or_default(),
            size_matched: enhanced.filled_size.to_string(),
            status,
            created_at: enhanced.created_at.timestamp() as u64,
            maker_address,
            outcome,
            expiration,
            order_type,
            associate_trades: Vec::new(),
            fee_rate_bps,
            nonce: None,
            condition_id,
            token_id: Some(enhanced.asset_id.clone()),
            question_id,
        }
    }

    /// Get user address (non-blocking for UI)
    pub fn get_user_address_sync(&self) -> Option<String> {
        if let Ok(address) = self.user_address.try_read() {
            address.clone()
        } else {
            None
        }
    }

    /// Get orders (non-blocking for UI)
    pub fn get_orders_sync(&self) -> Vec<EnhancedOrder> {
        if let Ok(orders) = self.orders.try_read() {
            orders.clone()
        } else {
            Vec::new()
        }
    }

    /// Get positions (non-blocking for UI)
    pub fn get_positions_sync(&self) -> Vec<Position> {
        if let Ok(positions) = self.positions.try_read() {
            positions.clone()
        } else {
            Vec::new()
        }
    }

    /// Get stats (non-blocking for UI)
    pub fn get_stats_sync(&self) -> Option<PortfolioStats> {
        if let Ok(stats) = self.stats.try_read() {
            stats.clone()
        } else {
            None
        }
    }

    /// Get balance (non-blocking for UI)
    pub fn get_balance_sync(&self) -> Option<BalanceInfo> {
        if let Ok(balance) = self.balance.try_read() {
            balance.clone()
        } else {
            None
        }
    }

    /// Check if currently refreshing (non-blocking for UI)
    pub fn is_refreshing_sync(&self) -> bool {
        if let Ok(refreshing) = self.is_refreshing.try_read() {
            *refreshing
        } else {
            false
        }
    }

    /// Get time since last refresh (non-blocking for UI)
    pub fn time_since_last_refresh_sync(&self) -> Option<std::time::Duration> {
        if let Ok(last_refresh) = self.last_refresh.try_read() {
            last_refresh.map(|t| t.elapsed())
        } else {
            None
        }
    }

    /// Refresh data asynchronously (spawn task)
    pub fn refresh_data_async(&self) {
        let service = self.clone();
        tokio::spawn(async move {
            if let Err(e) = service.refresh_data().await {
                error!("Failed to refresh portfolio data: {}", e);
            }
        });
    }

    /// Check if initialized
    pub fn is_initialized_sync(&self) -> bool {
        if let Ok(initialized) = self.is_initialized.try_read() {
            *initialized
        } else {
            false
        }
    }
}
