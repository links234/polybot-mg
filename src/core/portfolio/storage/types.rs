//! Storage types and data structures

use anyhow::Result;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use crate::core::portfolio::types::*;
use crate::core::portfolio::api::orders::PolymarketOrder;

/// Portfolio snapshot containing full state at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioSnapshot {
    /// Snapshot timestamp
    pub timestamp: DateTime<Utc>,
    /// Account address
    pub address: String,
    /// Current positions
    pub positions: Vec<Position>,
    /// Active orders
    pub active_orders: Vec<PolymarketOrder>,
    /// Portfolio statistics
    pub stats: PortfolioStats,
    /// Account balances
    pub balances: AccountBalances,
    /// Snapshot metadata
    pub metadata: SnapshotMetadata,
}

/// Account balance information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBalances {
    /// Total account value in USDC
    pub total_value: Decimal,
    /// Available cash balance
    pub available_cash: Decimal,
    /// Balance locked in orders
    pub locked_in_orders: Decimal,
    /// Balance in positions
    pub position_value: Decimal,
    /// Timestamp of balance update
    pub last_updated: DateTime<Utc>,
}

impl Default for AccountBalances {
    fn default() -> Self {
        Self {
            total_value: Decimal::ZERO,
            available_cash: Decimal::ZERO,
            locked_in_orders: Decimal::ZERO,
            position_value: Decimal::ZERO,
            last_updated: Utc::now(),
        }
    }
}

/// Snapshot metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    /// Snapshot version
    pub version: String,
    /// Reason for snapshot (periodic, manual, trade, etc.)
    pub reason: SnapshotReason,
    /// Previous snapshot filename if exists
    pub previous_snapshot: Option<String>,
    /// Hash of previous snapshot for integrity
    pub previous_hash: Option<String>,
}

/// Reason for taking a snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SnapshotReason {
    /// Periodic snapshot (hourly, daily)
    Periodic { interval: String },
    /// Manual snapshot requested by user
    Manual,
    /// Snapshot after trade execution
    TradeExecution { trade_id: String },
    /// Snapshot after significant P&L change
    PnLChange { change_percent: Decimal },
    /// Initial snapshot
    Initial,
    /// Application startup
    Startup,
    /// Before shutdown
    Shutdown,
}

/// Raw data storage for folder-based DB structure
pub struct RawDataStorage {
    base_path: PathBuf,
}

impl RawDataStorage {
    pub fn new<P: AsRef<Path>>(base_path: P) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
        }
    }

    /// Initialize raw storage directories
    pub async fn init_directories(&self) -> Result<()> {
        let dirs = [
            self.base_path.join("raw/trade"),
            self.base_path.join("raw/order"),
            self.base_path.join("raw/trade_query"),
            self.base_path.join("raw/balance"),
            self.base_path.join("cache/trades"),
            self.base_path.join("cache/orders"),
            self.base_path.join("cache/balances"),
        ];

        for dir in &dirs {
            tokio::fs::create_dir_all(dir).await?;
        }

        info!("Initialized raw storage directories at: {:?}", self.base_path);
        Ok(())
    }

    /// Store individual order with unique ID
    pub async fn store_order(&self, order: &PolymarketOrder) -> Result<String> {
        let order_id = &order.id;
        let file_path = self.base_path
            .join("raw/order")
            .join(format!("{}.json", order_id));
        
        let json = serde_json::to_string_pretty(order)?;
        tokio::fs::write(&file_path, json).await?;
        
        debug!("Stored order {} to {:?}", order_id, file_path);
        Ok(order_id.clone())
    }

    /// Store individual trade with unique ID
    pub async fn store_trade(&self, trade: &TradeExecution) -> Result<String> {
        let trade_id = &trade.trade_id;
        let file_path = self.base_path
            .join("raw/trade")
            .join(format!("{}.json", trade_id));
        
        let json = serde_json::to_string_pretty(trade)?;
        tokio::fs::write(&file_path, json).await?;
        
        debug!("Stored trade {} to {:?}", trade_id, file_path);
        Ok(trade_id.clone())
    }

    /// Load all trades (for caching/indexing)
    pub async fn load_all_trades(&self) -> Result<Vec<TradeExecution>> {
        let trade_dir = self.base_path.join("raw/trade");
        let mut trades = Vec::new();

        if !trade_dir.exists() {
            return Ok(trades);
        }

        let mut entries = tokio::fs::read_dir(&trade_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(content) = tokio::fs::read_to_string(&path).await {
                    if let Ok(trade) = serde_json::from_str::<TradeExecution>(&content) {
                        trades.push(trade);
                    } else {
                        warn!("Failed to parse trade file: {:?}", path);
                    }
                }
            }
        }

        // Sort by timestamp
        trades.sort_by_key(|t| t.timestamp);
        Ok(trades)
    }

    /// Load all orders
    pub async fn load_all_orders(&self) -> Result<Vec<PolymarketOrder>> {
        let order_dir = self.base_path.join("raw/order");
        let mut orders = Vec::new();

        if !order_dir.exists() {
            return Ok(orders);
        }

        let mut entries = tokio::fs::read_dir(&order_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(content) = tokio::fs::read_to_string(&path).await {
                    if let Ok(order) = serde_json::from_str::<PolymarketOrder>(&content) {
                        orders.push(order);
                    } else {
                        warn!("Failed to parse order file: {:?}", path);
                    }
                }
            }
        }

        Ok(orders)
    }
}