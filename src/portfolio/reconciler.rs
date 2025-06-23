//! Position reconciliation from orders and trades
//!
//! Builds positions from order history and tracks P&L

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::collections::HashMap;

use crate::portfolio::orders_api::PolymarketOrder;
use crate::portfolio::storage::{PositionSummary, TradeRecord};
use crate::portfolio::types::*;

/// Position reconciler that builds positions from orders
pub struct PositionReconciler {
    /// Current positions by token_id
    positions: HashMap<String, Position>,
    /// Trade history
    #[allow(dead_code)]
    trades: Vec<TradeRecord>,
}

impl PositionReconciler {
    /// Create new position reconciler
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
            trades: Vec::new(),
        }
    }

    /// Reconcile positions from orders
    pub fn reconcile_from_orders(&mut self, orders: &[PolymarketOrder]) -> Result<Vec<Position>> {
        self.positions.clear();
        
        for order in orders {
            if let Some(token_id) = &order.token_id {
                // Only process filled orders
                let filled_size = order.size_matched.parse::<Decimal>().unwrap_or_default();
                if filled_size > Decimal::ZERO {
                    let price = order.price;
                    
                    let position = self.positions.entry(token_id.clone()).or_insert_with(|| Position {
                        market_id: order.market.clone(),
                        token_id: token_id.clone(),
                        outcome: order.outcome.clone(),
                        side: PositionSide::Long, // Default to long
                        size: Decimal::ZERO,
                        average_price: Decimal::ZERO,
                        current_price: None,
                        realized_pnl: Decimal::ZERO,
                        unrealized_pnl: None,
                        status: PositionStatus::Open,
                        opened_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                        closed_at: None,
                        fees_paid: Decimal::ZERO,
                        market_question: None,
                    });
                    
                    // Update position size and average price
                    let new_total_cost = position.size * position.average_price + filled_size * price;
                    position.size += filled_size;
                    if position.size > Decimal::ZERO {
                        position.average_price = new_total_cost / position.size;
                    }
                    position.updated_at = chrono::Utc::now();
                }
            }
        }
        
        Ok(self.get_positions())
    }

    /// Calculate portfolio statistics
    pub fn calculate_stats(&self) -> PortfolioStats {
        let total_positions = self.positions.len();
        let open_positions = self.positions.values().filter(|p| p.status == PositionStatus::Open).count();
        let total_realized_pnl = self.positions.values().map(|p| p.realized_pnl).sum();
        let total_unrealized_pnl = self.positions.values().map(|p| p.unrealized_pnl.unwrap_or(Decimal::ZERO)).sum();
        let total_fees_paid = self.positions.values().map(|p| p.fees_paid).sum();
        
        PortfolioStats {
            total_balance: Decimal::ZERO, // Would need to fetch from API
            available_balance: Decimal::ZERO,
            locked_balance: Decimal::ZERO,
            total_positions,
            open_positions,
            total_realized_pnl,
            total_unrealized_pnl,
            total_fees_paid,
            win_rate: None, // Would need trade history to calculate
            average_win: None,
            average_loss: None,
            sharpe_ratio: None,
            last_updated: chrono::Utc::now(),
        }
    }

    /// Create trade record from order execution
    #[allow(dead_code)]
    pub fn create_trade_record(&self, order: &PolymarketOrder) -> Result<TradeRecord> {
        let filled_size = order
            .size_matched
            .parse::<Decimal>()
            .context("Failed to parse filled size")?;

        // Get current position for this token
        let position_after = if let Some(token_id) = &order.token_id {
            self.positions.get(token_id).map(|pos| PositionSummary {
                size: pos.size,
                average_price: pos.average_price,
                realized_pnl: pos.realized_pnl,
                unrealized_pnl: pos.unrealized_pnl,
            })
        } else {
            None
        };

        let trade = TradeRecord {
            trade_id: format!("{}_{}", order.id, order.created_at),
            order_id: order.id.clone(),
            market_id: order.market.clone(),
            asset_id: order.asset_id.clone(),
            market_question: order.question_id.clone().unwrap_or_default(),
            outcome: order.outcome.clone(),
            side: match order.side.as_str() {
                "BUY" => OrderSide::Buy,
                "SELL" => OrderSide::Sell,
                _ => OrderSide::Buy,
            },
            price: order.price,
            size: filled_size,
            fee: Decimal::ZERO, // Polymarket has no trading fees
            timestamp: DateTime::from_timestamp(order.created_at as i64 / 1000, 0)
                .unwrap_or_else(|| Utc::now()),
            pnl_impact: None, // Will be calculated separately
            position_after,
        };

        Ok(trade)
    }

    /// Update positions with current market prices
    #[allow(dead_code)]
    pub fn update_market_prices(&mut self, prices: &HashMap<String, Decimal>) {
        for (token_id, position) in &mut self.positions {
            if let Some(&price) = prices.get(token_id) {
                position.current_price = Some(price);

                // Calculate unrealized P&L
                let current_value = position.size * price;
                let cost_basis = position.size * position.average_price;

                position.unrealized_pnl = Some(match position.side {
                    PositionSide::Long => current_value - cost_basis,
                    PositionSide::Short => cost_basis - current_value,
                });

                position.updated_at = Utc::now();
            }
        }
    }

    /// Get all positions
    #[allow(dead_code)]
    pub fn get_positions(&self) -> Vec<Position> {
        let mut positions: Vec<Position> = self.positions.values().cloned().collect();
        positions.sort_by(|a, b| b.total_pnl().cmp(&a.total_pnl()));
        positions
    }

    /// Get position by token ID
    #[allow(dead_code)]
    pub fn get_position(&self, token_id: &str) -> Option<&Position> {
        self.positions.get(token_id)
    }

}
