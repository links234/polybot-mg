//! Portfolio API types and command definitions

use anyhow::Result;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::core::portfolio::types::*;
use crate::core::portfolio::storage::AccountBalances;

/// Portfolio service commands
#[derive(Debug)]
pub enum PortfolioCommand {
    // Trading operations
    Buy {
        market_id: String,
        token_id: String,
        price: Decimal,
        size: Decimal,
        response: oneshot::Sender<Result<String>>,
    },
    Sell {
        market_id: String,
        token_id: String,
        price: Decimal,
        size: Decimal,
        response: oneshot::Sender<Result<bool>>,
    },
    Cancel {
        order_id: String,
        response: oneshot::Sender<Result<bool>>,
    },
    
    // Query operations
    GetPortfolioState {
        response: oneshot::Sender<PortfolioState>,
    },
    GetActiveOrders {
        response: oneshot::Sender<Vec<ActiveOrder>>,
    },
    GetTradeHistory {
        start_date: Option<DateTime<Utc>>,
        end_date: Option<DateTime<Utc>>,
        response: oneshot::Sender<Vec<TradeExecution>>,
    },
    
    // Data operations
    RefreshData {
        response: oneshot::Sender<Result<()>>,
    },
    CreateSnapshot {
        reason: String,
        response: oneshot::Sender<Result<String>>,
    },
}

/// Complete portfolio state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioState {
    pub positions: Vec<Position>,
    pub active_orders: Vec<ActiveOrder>,
    pub stats: PortfolioStats,
    pub balances: AccountBalances,
    pub last_updated: DateTime<Utc>,
    pub is_synced: bool,
}

/// Portfolio service handle for external communication
#[derive(Clone)]
pub struct PortfolioServiceHandle {
    pub(crate) sender: tokio::sync::mpsc::Sender<PortfolioCommand>,
}

impl PortfolioServiceHandle {
    /// Create a new handle
    pub fn new(sender: tokio::sync::mpsc::Sender<PortfolioCommand>) -> Self {
        Self { sender }
    }

    /// Buy tokens
    pub async fn buy(
        &self,
        market_id: String,
        token_id: String,
        price: Decimal,
        size: Decimal,
    ) -> Result<String> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(PortfolioCommand::Buy {
                market_id,
                token_id,
                price,
                size,
                response: tx,
            })
            .await?;
        rx.await?
    }

    /// Sell tokens
    pub async fn sell(
        &self,
        market_id: String,
        token_id: String,
        price: Decimal,
        size: Decimal,
    ) -> Result<bool> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(PortfolioCommand::Sell {
                market_id,
                token_id,
                price,
                size,
                response: tx,
            })
            .await?;
        rx.await?
    }

    /// Cancel order
    pub async fn cancel(&self, order_id: String) -> Result<bool> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(PortfolioCommand::Cancel {
                order_id,
                response: tx,
            })
            .await?;
        rx.await?
    }

    /// Get portfolio state
    pub async fn get_state(&self) -> Result<PortfolioState> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(PortfolioCommand::GetPortfolioState { response: tx })
            .await?;
        Ok(rx.await?)
    }

    /// Get active orders
    pub async fn get_active_orders(&self) -> Result<Vec<ActiveOrder>> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(PortfolioCommand::GetActiveOrders { response: tx })
            .await?;
        Ok(rx.await?)
    }

    /// Get trade history
    pub async fn get_trade_history(
        &self,
        start_date: Option<DateTime<Utc>>,
        end_date: Option<DateTime<Utc>>,
    ) -> Result<Vec<TradeExecution>> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(PortfolioCommand::GetTradeHistory {
                start_date,
                end_date,
                response: tx,
            })
            .await?;
        Ok(rx.await?)
    }

    /// Refresh portfolio data
    pub async fn refresh(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(PortfolioCommand::RefreshData { response: tx })
            .await?;
        rx.await?
    }

    /// Create snapshot
    pub async fn create_snapshot(&self, reason: String) -> Result<String> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(PortfolioCommand::CreateSnapshot { reason, response: tx })
            .await?;
        rx.await?
    }
}