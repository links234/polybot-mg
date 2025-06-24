//! Portfolio command handlers that integrate with the portfolio service
//!
//! This module provides enhanced command handlers that use the portfolio service
//! for all trading operations, providing better tracking, storage, and error handling.

use anyhow::{anyhow, Result};
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::OnceCell;
use tracing::{info, warn};

use crate::config;
use crate::core::portfolio::api::{PortfolioServiceHandle, PortfolioState, start_portfolio_service};
use crate::core::portfolio::types::{ActiveOrder, OrderSide, TradeExecution};
use crate::data_paths::DataPaths;
use crate::ethereum_utils;

/// Global portfolio service handle
static PORTFOLIO_SERVICE: OnceCell<Arc<PortfolioServiceHandle>> = OnceCell::const_new();

/// Portfolio command handlers with service integration
pub struct PortfolioCommandHandlers {
    service_handle: Arc<PortfolioServiceHandle>,
    address: String,
    host: String,
}

impl PortfolioCommandHandlers {
    /// Create new command handlers
    pub async fn new(host: String, data_paths: DataPaths) -> Result<Self> {
        // Load private key to derive user address
        let private_key = config::load_private_key(&data_paths)
            .await
            .map_err(|e| anyhow!("No private key found. Run 'polybot init' first: {}", e))?;

        // Derive user's Ethereum address
        let address = ethereum_utils::derive_address_from_private_key(&private_key)?;

        // Get or start portfolio service
        let service_handle =
            get_or_start_portfolio_service(data_paths, address.clone(), host.clone()).await?;

        Ok(Self {
            service_handle,
            address,
            host,
        })
    }

    /// Execute buy order with enhanced tracking
    pub async fn execute_buy(
        &self,
        token_id: &str,
        price: Decimal,
        size: Decimal,
        market_id: Option<String>,
        confirm: bool,
    ) -> Result<String> {
        // Check confirmation in non-production environments
        if !confirm && std::env::var("RUST_ENV").unwrap_or_default() != "production" {
            warn!("⚠️  Order confirmation required. Use --yes to confirm.");
            return Err(anyhow!("Order confirmation required"));
        }

        info!(
            "Placing buy order: token={}, price={}, size={}",
            token_id, price, size
        );

        // Use market_id if provided, otherwise use token_id as fallback
        let market_id = market_id.unwrap_or_else(|| token_id.to_string());

        // Execute buy order through portfolio service
        let order_id = self
            .service_handle
            .buy(market_id, token_id.to_string(), price, size)
            .await?;

        info!("✅ Buy order placed successfully: {}", order_id);

        // Refresh portfolio data after order placement
        if let Err(e) = self.service_handle.refresh().await {
            warn!("Failed to refresh portfolio data after buy order: {}", e);
        }

        // Create snapshot after trade
        if let Err(e) = self
            .service_handle
            .create_snapshot(format!("buy_order_{}", order_id))
            .await
        {
            warn!("Failed to create snapshot after buy order: {}", e);
        }

        Ok(order_id)
    }

    /// Execute sell order with enhanced tracking
    pub async fn execute_sell(
        &self,
        token_id: &str,
        price: Decimal,
        size: Decimal,
        market_id: Option<String>,
        confirm: bool,
    ) -> Result<bool> {
        // Check confirmation in non-production environments
        if !confirm && std::env::var("RUST_ENV").unwrap_or_default() != "production" {
            warn!("⚠️  Order confirmation required. Use --yes to confirm.");
            return Err(anyhow!("Order confirmation required"));
        }

        info!(
            "Placing sell order: token={}, price={}, size={}",
            token_id, price, size
        );

        // Use market_id if provided, otherwise use token_id as fallback
        let market_id = market_id.unwrap_or_else(|| token_id.to_string());

        // Execute sell order through portfolio service
        let result = self
            .service_handle
            .sell(market_id, token_id.to_string(), price, size)
            .await?;

        info!("✅ Sell order placed successfully");

        // Refresh portfolio data after order placement
        if let Err(e) = self.service_handle.refresh().await {
            warn!("Failed to refresh portfolio data after sell order: {}", e);
        }

        // Create snapshot after trade
        if let Err(e) = self
            .service_handle
            .create_snapshot("sell_order".to_string())
            .await
        {
            warn!("Failed to create snapshot after sell order: {}", e);
        }

        Ok(result)
    }

    /// Cancel order with enhanced tracking
    pub async fn execute_cancel(&self, order_id: &str) -> Result<bool> {
        info!("Cancelling order: {}", order_id);

        // Execute cancel order through portfolio service
        let success = self.service_handle.cancel(order_id.to_string()).await?;

        if success {
            info!("✅ Order cancelled successfully: {}", order_id);

            // Refresh portfolio data after cancellation
            if let Err(e) = self.service_handle.refresh().await {
                warn!("Failed to refresh portfolio data after cancel: {}", e);
            }

            // Create snapshot after cancellation
            if let Err(e) = self
                .service_handle
                .create_snapshot(format!("cancel_order_{}", order_id))
                .await
            {
                warn!("Failed to create snapshot after cancel: {}", e);
            }
        } else {
            warn!("⚠️ Order cancellation failed: {}", order_id);
        }

        Ok(success)
    }

    /// Get portfolio state with all data
    pub async fn get_portfolio_state(&self) -> Result<PortfolioState> {
        self.service_handle.get_state().await
    }

    /// Get active orders
    pub async fn get_active_orders(&self) -> Result<Vec<ActiveOrder>> {
        self.service_handle.get_active_orders().await
    }

    /// Get trade history with optional date filtering
    pub async fn get_trade_history(
        &self,
        start_date: Option<chrono::DateTime<chrono::Utc>>,
        end_date: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Vec<TradeExecution>> {
        self.service_handle
            .get_trade_history(start_date, end_date)
            .await
    }

    /// Refresh portfolio data
    pub async fn refresh_data(&self) -> Result<()> {
        self.service_handle.refresh().await
    }

    /// Get user address
    pub fn get_address(&self) -> &str {
        &self.address
    }

    /// Get host
    pub fn get_host(&self) -> &str {
        &self.host
    }
}

/// Get or start the global portfolio service
async fn get_or_start_portfolio_service(
    data_paths: DataPaths,
    address: String,
    host: String,
) -> Result<Arc<PortfolioServiceHandle>> {
    PORTFOLIO_SERVICE
        .get_or_try_init(|| async {
            info!("Starting portfolio service for address: {}", address);
            let handle = start_portfolio_service(data_paths, address, host).await?;
            Ok(Arc::new(handle))
        })
        .await
        .map(|handle| handle.clone())
}

/// Enhanced buy command implementation
pub async fn enhanced_buy_command(
    token_id: &str,
    price: Decimal,
    size: Decimal,
    market_id: Option<String>,
    confirm: bool,
    host: &str,
    data_paths: DataPaths,
) -> Result<()> {
    let handlers = PortfolioCommandHandlers::new(host.to_string(), data_paths).await?;
    let order_id = handlers
        .execute_buy(token_id, price, size, market_id, confirm)
        .await?;

    println!("🚀 Buy order placed!");
    println!("📋 Order ID: {}", order_id);
    println!("🎯 Token: {}", token_id);
    println!("💰 Price: ${}", price);
    println!("📊 Size: ${}", size);
    println!("👤 Account: {}", handlers.get_address());

    Ok(())
}

/// Enhanced sell command implementation
pub async fn enhanced_sell_command(
    token_id: &str,
    price: Decimal,
    size: Decimal,
    market_id: Option<String>,
    confirm: bool,
    host: &str,
    data_paths: DataPaths,
) -> Result<()> {
    let handlers = PortfolioCommandHandlers::new(host.to_string(), data_paths).await?;
    let _result = handlers
        .execute_sell(token_id, price, size, market_id, confirm)
        .await?;

    println!("🚀 Sell order placed!");
    println!("🎯 Token: {}", token_id);
    println!("💰 Price: ${}", price);
    println!("📊 Size: ${}", size);
    println!("👤 Account: {}", handlers.get_address());

    Ok(())
}

/// Enhanced cancel command implementation
pub async fn enhanced_cancel_command(order_id: &str, host: &str, data_paths: DataPaths) -> Result<()> {
    let handlers = PortfolioCommandHandlers::new(host.to_string(), data_paths).await?;
    let success = handlers.execute_cancel(order_id).await?;

    if success {
        println!("✅ Order cancelled successfully!");
        println!("📋 Order ID: {}", order_id);
        println!("👤 Account: {}", handlers.get_address());
    } else {
        println!("❌ Order cancellation failed!");
        println!("📋 Order ID: {}", order_id);
        println!("💡 The order may have already been filled or cancelled.");
    }

    Ok(())
}

/// Enhanced portfolio display command
pub async fn enhanced_portfolio_command(
    market_filter: Option<String>,
    asset_filter: Option<String>,
    _text_mode: bool,
    host: &str,
    data_paths: DataPaths,
) -> Result<()> {
    let handlers = PortfolioCommandHandlers::new(host.to_string(), data_paths).await?;

    // Refresh data first
    if let Err(e) = handlers.refresh_data().await {
        warn!("Failed to refresh portfolio data: {}", e);
    }

    // Get portfolio state
    let portfolio_state = handlers.get_portfolio_state().await?;

    // Display portfolio information
    println!("\n📊 Enhanced Portfolio Overview\n");
    println!("👤 User: {}", handlers.get_address());
    println!(
        "🔗 Profile: https://polymarket.com/profile/{}",
        handlers.get_address()
    );
    println!("🌐 API Host: {}", handlers.get_host());
    println!(
        "🔄 Last Updated: {}",
        portfolio_state
            .last_updated
            .format("%Y-%m-%d %H:%M:%S UTC")
    );
    println!(
        "✅ Synced: {}",
        if portfolio_state.is_synced {
            "Yes"
        } else {
            "No"
        }
    );
    println!();

    // Display balances
    println!("💰 Account Balances:");
    println!(
        "  💵 Total Value: ${:.2}",
        portfolio_state.balances.total_value
    );
    println!(
        "  💴 Available Cash: ${:.2}",
        portfolio_state.balances.available_cash
    );
    println!(
        "  🔒 Locked in Orders: ${:.2}",
        portfolio_state.balances.locked_in_orders
    );
    println!(
        "  📈 Position Value: ${:.2}",
        portfolio_state.balances.position_value
    );
    println!();

    // Display statistics
    println!("📊 Portfolio Statistics:");
    println!(
        "  🎯 Total Positions: {}",
        portfolio_state.stats.total_positions
    );
    println!(
        "  📈 Open Positions: {}",
        portfolio_state.stats.open_positions
    );
    println!("  💹 Total P&L: ${:.2}", portfolio_state.stats.total_pnl());
    println!(
        "  ✅ Realized P&L: ${:.2}",
        portfolio_state.stats.total_realized_pnl
    );
    println!(
        "  📊 Unrealized P&L: ${:.2}",
        portfolio_state.stats.total_unrealized_pnl
    );
    println!(
        "  💸 Total Fees: ${:.2}",
        portfolio_state.stats.total_fees_paid
    );

    if let Some(win_rate) = portfolio_state.stats.win_rate {
        println!("  🏆 Win Rate: {:.1}%", win_rate);
    }

    println!();

    // Display positions
    let mut positions = portfolio_state.positions;
    if let Some(market_filter) = &market_filter {
        positions.retain(|p| p.market_id.contains(market_filter));
    }
    if let Some(asset_filter) = &asset_filter {
        positions.retain(|p| p.token_id.contains(asset_filter) || p.outcome.contains(asset_filter));
    }

    if !positions.is_empty() {
        println!("📍 Positions ({}):", positions.len());
        for (i, position) in positions.iter().enumerate() {
            println!(
                "  {}. {} {} - Size: {}, Avg Price: ${:.3}, P&L: ${:.2}",
                i + 1,
                position.outcome,
                &position.market_id[..8.min(position.market_id.len())],
                position.size,
                position.average_price,
                position.total_pnl()
            );
        }
        println!();
    }

    // Display active orders
    let mut orders = portfolio_state.active_orders;
    if let Some(market_filter) = &market_filter {
        orders.retain(|o| o.market_id.contains(market_filter));
    }
    if let Some(asset_filter) = &asset_filter {
        orders.retain(|o| o.token_id.contains(asset_filter) || o.outcome.contains(asset_filter));
    }

    if !orders.is_empty() {
        println!("📋 Active Orders ({}):", orders.len());
        for (i, order) in orders.iter().enumerate() {
            println!(
                "  {}. {} {} @ ${:.3} (Size: ${:.2}) - {}",
                i + 1,
                match order.side {
                    OrderSide::Buy => "BUY",
                    OrderSide::Sell => "SELL",
                },
                &order.order_id[..8.min(order.order_id.len())],
                order.price,
                order.size,
                format!("{:?}", order.status)
            );
        }
        println!();
    } else {
        println!("📋 No active orders found");
        println!();
    }

    // Show recent trade history
    match handlers.get_trade_history(None, None).await {
        Ok(trades) => {
            if !trades.is_empty() {
                let recent_trades: Vec<_> = trades.iter().rev().take(5).collect();
                println!("📈 Recent Trades ({} total, showing last 5):", trades.len());
                for (i, trade) in recent_trades.iter().enumerate() {
                    println!(
                        "  {}. {} {} @ ${:.3} (Size: {}) - {}",
                        i + 1,
                        match trade.side {
                            OrderSide::Buy => "BUY",
                            OrderSide::Sell => "SELL",
                        },
                        &trade.trade_id[..8.min(trade.trade_id.len())],
                        trade.price,
                        trade.size,
                        trade.timestamp.format("%Y-%m-%d %H:%M")
                    );
                }
            } else {
                println!("📈 No trade history found");
            }
        }
        Err(e) => {
            warn!("Failed to load trade history: {}", e);
        }
    }

    println!();
    println!("💡 Use 'polybot portfolio --help' for more options");
    println!("💡 Use 'polybot stream' for real-time portfolio monitoring");

    Ok(())
}

/// Get portfolio service handle for other components
pub async fn get_portfolio_service_handle(
    host: &str,
    data_paths: &DataPaths,
) -> Result<Arc<PortfolioServiceHandle>> {
    // Load private key to derive user address
    let private_key = config::load_private_key(data_paths)
        .await
        .map_err(|e| anyhow!("No private key found. Run 'polybot init' first: {}", e))?;

    // Derive user's Ethereum address
    let address = ethereum_utils::derive_address_from_private_key(&private_key)?;

    get_or_start_portfolio_service(data_paths.clone(), address, host.to_string()).await
}