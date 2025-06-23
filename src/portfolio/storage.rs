//! Portfolio storage and persistence layer
//!
//! Stores portfolio state in: data/trade/account/<address>/
//! - snapshots/YYYY-MM-DD-HH-MM-SS.json - Full portfolio snapshots
//! - trades/YYYY-MM-DD.json - Daily trade history
//! - positions/current.json - Current positions
//! - orders/active.json - Active orders cache
//! - stats/daily/YYYY-MM-DD.json - Daily statistics

use anyhow::{Context, Result};
use chrono::{DateTime, Local, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{info, warn};

use crate::portfolio::orders_api::PolymarketOrder;
use crate::portfolio::types::*;

/// Portfolio storage manager
#[derive(Clone)]
pub struct PortfolioStorage {
    /// Base directory for account data
    account_dir: PathBuf,
    /// Account address
    #[allow(dead_code)]
    address: String,
}

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
}

/// Trade record for history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    /// Trade ID
    pub trade_id: String,
    /// Order ID
    pub order_id: String,
    /// Market ID
    pub market_id: String,
    /// Asset/Token ID
    pub asset_id: String,
    /// Market question
    pub market_question: String,
    /// Outcome (YES/NO)
    pub outcome: String,
    /// Trade side
    pub side: OrderSide,
    /// Execution price
    pub price: Decimal,
    /// Trade size
    pub size: Decimal,
    /// Fee paid
    pub fee: Decimal,
    /// Trade timestamp
    pub timestamp: DateTime<Utc>,
    /// P&L impact
    pub pnl_impact: Option<Decimal>,
    /// Position after trade
    pub position_after: Option<PositionSummary>,
}

/// Position summary for trade records
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionSummary {
    pub size: Decimal,
    pub average_price: Decimal,
    pub realized_pnl: Decimal,
    pub unrealized_pnl: Option<Decimal>,
}

/// Daily statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyStats {
    /// Date
    pub date: String,
    /// Starting portfolio value
    pub start_value: Decimal,
    /// Ending portfolio value
    pub end_value: Decimal,
    /// Daily P&L
    pub daily_pnl: Decimal,
    /// Number of trades
    pub trade_count: usize,
    /// Volume traded
    pub volume_traded: Decimal,
    /// Fees paid
    pub fees_paid: Decimal,
    /// Win rate for closed positions
    pub win_rate: Option<Decimal>,
    /// Best trade
    pub best_trade: Option<TradeSummary>,
    /// Worst trade
    pub worst_trade: Option<TradeSummary>,
    /// Markets traded
    pub markets_traded: Vec<String>,
}

/// Trade summary for daily stats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeSummary {
    pub trade_id: String,
    pub market_question: String,
    pub pnl: Decimal,
    pub return_percent: Decimal,
}

impl PortfolioStorage {
    /// Create new portfolio storage manager
    pub fn new(data_dir: &Path, address: &str) -> Self {
        let account_dir = data_dir.join("trade").join("account").join(address);
        Self {
            account_dir,
            address: address.to_string(),
        }
    }

    /// Initialize storage directories
    pub async fn init_directories(&self) -> Result<()> {
        // Create directory structure
        let dirs = [
            self.account_dir.join("snapshots"),
            self.account_dir.join("trades"),
            self.account_dir.join("positions"),
            self.account_dir.join("orders"),
            self.account_dir.join("stats").join("daily"),
            self.account_dir.join("stats").join("monthly"),
        ];

        for dir in &dirs {
            fs::create_dir_all(dir)
                .await
                .context(format!("Failed to create directory: {:?}", dir))?;
        }

        info!(
            "Initialized portfolio storage directories at: {:?}",
            self.account_dir
        );
        Ok(())
    }

    /// Save portfolio snapshot
    pub async fn save_snapshot(&self, snapshot: &PortfolioSnapshot) -> Result<String> {
        self.init_directories().await?;

        // Generate filename with timestamp
        let filename = format!("{}.json", snapshot.timestamp.format("%Y-%m-%d-%H-%M-%S"));
        let filepath = self.account_dir.join("snapshots").join(&filename);

        // Serialize snapshot
        let json = serde_json::to_string_pretty(&snapshot)?;

        // Write to file
        fs::write(&filepath, json)
            .await
            .context("Failed to write snapshot")?;

        info!("Saved portfolio snapshot: {}", filename);
        Ok(filename)
    }

    /// Load latest snapshot
    #[allow(dead_code)]
    pub async fn load_latest_snapshot(&self) -> Result<Option<PortfolioSnapshot>> {
        let snapshots_dir = self.account_dir.join("snapshots");

        if !snapshots_dir.exists() {
            return Ok(None);
        }

        // Find latest snapshot file
        let mut entries = fs::read_dir(&snapshots_dir).await?;
        let mut latest_file: Option<PathBuf> = None;
        let mut latest_time: Option<DateTime<Utc>> = None;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(metadata) = entry.metadata().await {
                    if let Ok(modified) = metadata.modified() {
                        let modified_time: DateTime<Utc> = modified.into();
                        if latest_time.is_none() || modified_time > latest_time.unwrap() {
                            latest_time = Some(modified_time);
                            latest_file = Some(path);
                        }
                    }
                }
            }
        }

        if let Some(filepath) = latest_file {
            let content = fs::read_to_string(&filepath).await?;
            let snapshot: PortfolioSnapshot = serde_json::from_str(&content)?;
            info!("Loaded latest snapshot from: {:?}", filepath);
            Ok(Some(snapshot))
        } else {
            Ok(None)
        }
    }


    /// Save active orders to cache
    pub async fn save_active_orders(&self, orders: &[PolymarketOrder]) -> Result<()> {
        self.init_directories().await?;
        
        let filepath = self.account_dir.join("orders").join("active.json");
        let json = serde_json::to_string_pretty(orders)?;
        
        fs::write(&filepath, json)
            .await
            .context("Failed to save active orders")?;
            
        info!("Saved {} active orders to cache", orders.len());
        Ok(())
    }

    /// Save current positions
    pub async fn save_positions(&self, positions: &[Position]) -> Result<()> {
        self.init_directories().await?;
        
        let filepath = self.account_dir.join("positions").join("current.json");
        let data = serde_json::json!({
            "timestamp": Utc::now(),
            "positions": positions
        });
        let json = serde_json::to_string_pretty(&data)?;
        
        fs::write(&filepath, json)
            .await
            .context("Failed to save positions")?;
            
        info!("Saved {} positions to cache", positions.len());
        Ok(())
    }

    /// Load current positions
    #[allow(dead_code)]
    pub async fn load_positions(&self) -> Result<Vec<Position>> {
        let filepath = self.account_dir.join("positions").join("current.json");

        if !filepath.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&filepath).await?;
        let data: serde_json::Value = serde_json::from_str(&content)?;

        if let Some(positions) = data.get("positions") {
            let positions: Vec<Position> = serde_json::from_value(positions.clone())?;
            Ok(positions)
        } else {
            Ok(Vec::new())
        }
    }


    /// Record a trade
    #[allow(dead_code)]
    pub async fn record_trade(&self, trade: &TradeRecord) -> Result<()> {
        self.init_directories().await?;

        // Get today's trade file
        let date = Local::now().format("%Y-%m-%d").to_string();
        let filepath = self
            .account_dir
            .join("trades")
            .join(format!("{}.json", date));

        // Load existing trades or create new list
        let mut trades: Vec<TradeRecord> = if filepath.exists() {
            let content = fs::read_to_string(&filepath).await?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Vec::new()
        };

        // Add new trade
        trades.push(trade.clone());

        // Save back
        let json = serde_json::to_string_pretty(&trades)?;
        fs::write(&filepath, json).await?;

        info!(
            "Recorded trade: {} in market {}",
            trade.trade_id,
            &trade.market_id[..8]
        );
        Ok(())
    }

    /// Load trade history for date range
    #[allow(dead_code)]
    pub async fn load_trade_history(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Result<Vec<TradeRecord>> {
        let trades_dir = self.account_dir.join("trades");
        let mut all_trades = Vec::new();

        if !trades_dir.exists() {
            return Ok(all_trades);
        }

        let mut current_date = start_date.date_naive();
        let end_date_naive = end_date.date_naive();

        while current_date <= end_date_naive {
            let filename = format!("{}.json", current_date.format("%Y-%m-%d"));
            let filepath = trades_dir.join(&filename);

            if filepath.exists() {
                match fs::read_to_string(&filepath).await {
                    Ok(content) => {
                        if let Ok(trades) = serde_json::from_str::<Vec<TradeRecord>>(&content) {
                            all_trades.extend(trades);
                        }
                    }
                    Err(e) => warn!("Failed to load trades from {}: {}", filename, e),
                }
            }

            current_date = current_date.succ_opt().unwrap_or(current_date);
        }

        // Sort by timestamp
        all_trades.sort_by_key(|t| t.timestamp);

        info!(
            "Loaded {} trades from {} to {}",
            all_trades.len(),
            start_date.format("%Y-%m-%d"),
            end_date.format("%Y-%m-%d")
        );

        Ok(all_trades)
    }

    /// Save daily statistics
    #[allow(dead_code)]
    pub async fn save_daily_stats(&self, stats: &DailyStats) -> Result<()> {
        self.init_directories().await?;

        let filepath = self
            .account_dir
            .join("stats")
            .join("daily")
            .join(format!("{}.json", stats.date));

        let json = serde_json::to_string_pretty(&stats)?;
        fs::write(&filepath, json).await?;

        info!("Saved daily stats for {}", stats.date);
        Ok(())
    }

    /// Create periodic snapshot
    #[allow(dead_code)]
    pub async fn create_periodic_snapshot(
        &self,
        positions: Vec<Position>,
        orders: Vec<PolymarketOrder>,
        stats: PortfolioStats,
        balances: AccountBalances,
        interval: &str,
    ) -> Result<String> {
        // Load previous snapshot for reference
        let previous = self.load_latest_snapshot().await?;
        let (prev_filename, prev_hash) = if let Some(prev) = previous {
            let filename = format!("{}.json", prev.timestamp.format("%Y-%m-%d-%H-%M-%S"));
            let hash = self.calculate_snapshot_hash(&prev)?;
            (Some(filename), Some(hash))
        } else {
            (None, None)
        };

        let snapshot = PortfolioSnapshot {
            timestamp: Utc::now(),
            address: self.address.clone(),
            positions,
            active_orders: orders,
            stats,
            balances,
            metadata: SnapshotMetadata {
                version: "1.0".to_string(),
                reason: SnapshotReason::Periodic {
                    interval: interval.to_string(),
                },
                previous_snapshot: prev_filename,
                previous_hash: prev_hash,
            },
        };

        self.save_snapshot(&snapshot).await
    }

    /// Calculate snapshot hash for integrity
    #[allow(dead_code)]
    fn calculate_snapshot_hash(&self, snapshot: &PortfolioSnapshot) -> Result<String> {
        let json = serde_json::to_string(&snapshot)?;
        let hash = blake3::hash(json.as_bytes());
        Ok(hash.to_hex().to_string())
    }

    /// Clean old snapshots (keep last N)
    #[allow(dead_code)]
    pub async fn clean_old_snapshots(&self, keep_count: usize) -> Result<usize> {
        let snapshots_dir = self.account_dir.join("snapshots");

        if !snapshots_dir.exists() {
            return Ok(0);
        }

        // Collect all snapshot files with timestamps
        let mut snapshots: Vec<(PathBuf, DateTime<Utc>)> = Vec::new();
        let mut entries = fs::read_dir(&snapshots_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(metadata) = entry.metadata().await {
                    if let Ok(modified) = metadata.modified() {
                        snapshots.push((path, modified.into()));
                    }
                }
            }
        }

        // Sort by timestamp, newest first
        snapshots.sort_by_key(|(_, time)| std::cmp::Reverse(*time));

        // Remove old snapshots
        let mut removed = 0;
        for (path, _) in snapshots.into_iter().skip(keep_count) {
            match fs::remove_file(&path).await {
                Ok(_) => {
                    removed += 1;
                    info!("Removed old snapshot: {:?}", path);
                }
                Err(e) => warn!("Failed to remove snapshot {:?}: {}", path, e),
            }
        }

        Ok(removed)
    }
}
