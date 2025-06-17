//! Portfolio manager for tracking positions and orders

use anyhow::Result;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::portfolio::types::*;

/// Portfolio manager with thread-safe state management
#[allow(dead_code)]
pub struct PortfolioManager {
    /// Active positions by token ID
    positions: Arc<RwLock<HashMap<String, Position>>>,
    
    /// Active orders by order ID
    active_orders: Arc<RwLock<HashMap<String, ActiveOrder>>>,
    
    /// Portfolio statistics
    stats: Arc<RwLock<PortfolioStats>>,
    
    /// Historical trades
    trade_history: Arc<RwLock<Vec<TradeExecution>>>,
    
    /// Market info cache
    market_info: Arc<RwLock<HashMap<String, MarketInfo>>>,
}

/// Market information cache
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct MarketInfo {
    _market_id: String,
    market_question: String,
    _token_outcomes: HashMap<String, String>,
    _last_updated: DateTime<Utc>,
}

#[allow(dead_code)]
impl PortfolioManager {
    /// Create new portfolio manager
    pub fn new() -> Self {
        Self {
            positions: Arc::new(RwLock::new(HashMap::new())),
            active_orders: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(PortfolioStats {
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
            })),
            trade_history: Arc::new(RwLock::new(Vec::new())),
            market_info: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Handle portfolio event from WebSocket
    pub async fn handle_event(&self, event: PortfolioEvent) -> Result<()> {
        match event {
            PortfolioEvent::OrderUpdate(update) => {
                self.handle_order_update(update).await?;
            }
            PortfolioEvent::TradeExecution(trade) => {
                self.handle_trade_execution(trade).await?;
            }
            PortfolioEvent::BalanceUpdate(balance) => {
                self.handle_balance_update(balance).await?;
            }
            PortfolioEvent::PositionUpdate { position, update_type } => {
                self.handle_position_update(position, update_type).await?;
            }
        }
        Ok(())
    }
    
    /// Handle order update
    async fn handle_order_update(&self, update: OrderUpdate) -> Result<()> {
        info!(
            order_id = %update.order_id,
            update_type = ?update.update_type,
            "Processing order update"
        );
        
        let mut orders = self.active_orders.write().await;
        
        match update.update_type {
            OrderUpdateType::Placed => {
                orders.insert(update.order_id.clone(), update.order);
            }
            OrderUpdateType::Filled | OrderUpdateType::Cancelled | 
            OrderUpdateType::Rejected | OrderUpdateType::Expired => {
                orders.remove(&update.order_id);
            }
            OrderUpdateType::PartiallyFilled => {
                orders.insert(update.order_id.clone(), update.order);
            }
        }
        
        // Update stats
        self.update_stats().await?;
        
        Ok(())
    }
    
    /// Handle trade execution
    async fn handle_trade_execution(&self, trade: TradeExecution) -> Result<()> {
        info!(
            trade_id = %trade.trade_id,
            size = %trade.size,
            price = %trade.price,
            "Processing trade execution"
        );
        
        // Add to trade history
        {
            let mut history = self.trade_history.write().await;
            history.push(trade.clone());
            
            // Keep only last 1000 trades
            if history.len() > 1000 {
                history.drain(0..100);
            }
        }
        
        // Update position
        let mut positions = self.positions.write().await;
        let position = positions.entry(trade.token_id.clone()).or_insert_with(|| {
            Position {
                market_id: trade.market_id.clone(),
                token_id: trade.token_id.clone(),
                outcome: String::new(), // Will be filled from market info
                side: match trade.side {
                    OrderSide::Buy => PositionSide::Long,
                    OrderSide::Sell => PositionSide::Short,
                },
                size: Decimal::ZERO,
                average_price: Decimal::ZERO,
                current_price: None,
                realized_pnl: Decimal::ZERO,
                unrealized_pnl: None,
                status: PositionStatus::Open,
                opened_at: Utc::now(),
                updated_at: Utc::now(),
                closed_at: None,
                fees_paid: Decimal::ZERO,
                market_question: None,
            }
        });
        
        // Update position based on trade
        match trade.side {
            OrderSide::Buy => {
                let new_size = position.size + trade.size;
                position.average_price = if position.size.is_zero() {
                    trade.price
                } else {
                    (position.average_price * position.size + trade.price * trade.size) / new_size
                };
                position.size = new_size;
            }
            OrderSide::Sell => {
                if position.size >= trade.size {
                    // Calculate realized P&L
                    let realized = (trade.price - position.average_price) * trade.size;
                    position.realized_pnl += realized;
                    position.size -= trade.size;
                    
                    if position.size.is_zero() {
                        position.status = PositionStatus::Closed;
                        position.closed_at = Some(Utc::now());
                    }
                } else {
                    warn!("Sell size exceeds position size");
                }
            }
        }
        
        position.fees_paid += trade.fee;
        position.updated_at = Utc::now();
        
        drop(positions);
        
        // Update stats
        self.update_stats().await?;
        
        Ok(())
    }
    
    /// Handle balance update
    async fn handle_balance_update(&self, balance: BalanceUpdate) -> Result<()> {
        info!(
            available = %balance.available_balance,
            locked = %balance.locked_balance,
            "Processing balance update"
        );
        
        let mut stats = self.stats.write().await;
        stats.available_balance = balance.available_balance;
        stats.locked_balance = balance.locked_balance;
        stats.total_balance = balance.total_balance;
        stats.last_updated = Utc::now();
        
        Ok(())
    }
    
    /// Handle position update
    async fn handle_position_update(&self, position: Position, update_type: PositionUpdateType) -> Result<()> {
        info!(
            token_id = %position.token_id,
            update_type = ?update_type,
            "Processing position update"
        );
        
        let mut positions = self.positions.write().await;
        
        match update_type {
            PositionUpdateType::Opened | PositionUpdateType::Updated => {
                positions.insert(position.token_id.clone(), position);
            }
            PositionUpdateType::Closed | PositionUpdateType::Liquidated => {
                positions.remove(&position.token_id);
            }
        }
        
        drop(positions);
        
        // Update stats
        self.update_stats().await?;
        
        Ok(())
    }
    
    /// Update portfolio statistics
    pub async fn update_stats(&self) -> Result<()> {
        let positions = self.positions.read().await;
        let mut stats = self.stats.write().await;
        
        // Count positions
        stats.total_positions = positions.len();
        stats.open_positions = positions.values()
            .filter(|p| p.status == PositionStatus::Open)
            .count();
        
        // Calculate P&L
        stats.total_realized_pnl = positions.values()
            .map(|p| p.realized_pnl)
            .sum();
        
        stats.total_unrealized_pnl = positions.values()
            .filter_map(|p| p.unrealized_pnl)
            .sum();
        
        stats.total_fees_paid = positions.values()
            .map(|p| p.fees_paid)
            .sum();
        
        // Calculate win rate
        let closed_positions: Vec<_> = positions.values()
            .filter(|p| p.status == PositionStatus::Closed)
            .collect();
        
        if !closed_positions.is_empty() {
            let wins = closed_positions.iter()
                .filter(|p| p.realized_pnl > Decimal::ZERO)
                .count();
            
            stats.win_rate = Some(
                Decimal::from(wins) / Decimal::from(closed_positions.len()) * Decimal::from(100)
            );
            
            let winning_trades: Vec<_> = closed_positions.iter()
                .filter(|p| p.realized_pnl > Decimal::ZERO)
                .collect();
            
            let losing_trades: Vec<_> = closed_positions.iter()
                .filter(|p| p.realized_pnl < Decimal::ZERO)
                .collect();
            
            if !winning_trades.is_empty() {
                stats.average_win = Some(
                    winning_trades.iter()
                        .map(|p| p.realized_pnl)
                        .sum::<Decimal>() / Decimal::from(winning_trades.len())
                );
            }
            
            if !losing_trades.is_empty() {
                stats.average_loss = Some(
                    losing_trades.iter()
                        .map(|p| p.realized_pnl.abs())
                        .sum::<Decimal>() / Decimal::from(losing_trades.len())
                );
            }
        }
        
        stats.last_updated = Utc::now();
        
        Ok(())
    }
    
    /// Get all positions
    pub async fn get_positions(&self) -> Vec<Position> {
        let positions = self.positions.read().await;
        positions.values().cloned().collect()
    }
    
    /// Get all active orders
    pub async fn get_active_orders(&self) -> Vec<ActiveOrder> {
        let orders = self.active_orders.read().await;
        orders.values().cloned().collect()
    }
    
    /// Get portfolio statistics
    pub async fn get_stats(&self) -> PortfolioStats {
        let stats = self.stats.read().await;
        stats.clone()
    }
    
    /// Get positions grouped by market
    pub async fn get_market_positions(&self) -> Vec<MarketPositionSummary> {
        let positions = self.positions.read().await;
        let orders = self.active_orders.read().await;
        let market_info = self.market_info.read().await;
        
        // Group positions by market
        let mut market_positions: HashMap<String, Vec<Position>> = HashMap::new();
        for position in positions.values() {
            market_positions.entry(position.market_id.clone())
                .or_insert_with(Vec::new)
                .push(position.clone());
        }
        
        // Create summaries
        let mut summaries = Vec::new();
        for (market_id, positions) in market_positions {
            let market_orders: Vec<_> = orders.values()
                .filter(|o| o.market_id == market_id)
                .collect();
            
            let total_exposure = positions.iter()
                .map(|p| p.size * p.average_price)
                .sum();
            
            let net_position = positions.iter()
                .map(|p| match p.side {
                    PositionSide::Long => p.size,
                    PositionSide::Short => -p.size,
                })
                .sum();
            
            let total_pnl = positions.iter()
                .map(|p| p.total_pnl())
                .sum();
            
            let market_question = market_info.get(&market_id)
                .map(|info| info.market_question.clone())
                .unwrap_or_else(|| format!("Market {}", &market_id[..8]));
            
            summaries.push(MarketPositionSummary {
                market_id: market_id.clone(),
                market_question,
                positions,
                total_exposure,
                net_position,
                total_pnl,
                has_open_orders: !market_orders.is_empty(),
                open_order_count: market_orders.len(),
            });
        }
        
        // Sort by total exposure
        summaries.sort_by(|a, b| b.total_exposure.cmp(&a.total_exposure));
        
        summaries
    }
    
    /// Update market info
    pub async fn _update_market_info(&self, market_id: String, question: String, token_outcomes: HashMap<String, String>) {
        let mut market_info = self.market_info.write().await;
        market_info.insert(market_id.clone(), MarketInfo {
            _market_id: market_id,
            market_question: question,
            _token_outcomes: token_outcomes,
            _last_updated: Utc::now(),
        });
    }
}