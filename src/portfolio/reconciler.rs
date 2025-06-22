//! Position reconciliation from orders and trades
//!
//! Builds positions from order history and tracks P&L

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::collections::HashMap;
use tracing::{debug, info, warn};

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
    /// Create new reconciler
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
            trades: Vec::new(),
        }
    }

    /// Reconcile positions from orders
    pub fn reconcile_from_orders(&mut self, orders: &[PolymarketOrder]) -> Result<Vec<Position>> {
        info!("Reconciling positions from {} orders", orders.len());

        // Group filled orders by token_id
        let mut filled_by_token: HashMap<String, Vec<&PolymarketOrder>> = HashMap::new();

        for order in orders {
            // Only process filled orders
            if order.status == "FILLED" {
                if let Some(token_id) = &order.token_id {
                    filled_by_token
                        .entry(token_id.clone())
                        .or_default()
                        .push(order);
                }
            }
        }

        // Build positions from filled orders
        for (token_id, token_orders) in filled_by_token {
            debug!(
                "Processing {} filled orders for token {}",
                token_orders.len(),
                token_id
            );

            // Sort orders by created_at timestamp
            let mut sorted_orders = token_orders;
            sorted_orders.sort_by_key(|o| o.created_at);

            // Build position from orders
            let position = self.build_position_from_orders(&token_id, &sorted_orders)?;

            if position.size > Decimal::ZERO {
                self.positions.insert(token_id, position);
            }
        }

        // Convert to vector and sort by market_id
        let mut positions: Vec<Position> = self.positions.values().cloned().collect();
        positions.sort_by(|a, b| a.market_id.cmp(&b.market_id));

        info!("Reconciled {} open positions", positions.len());
        Ok(positions)
    }

    /// Build position from a series of orders for a token
    fn build_position_from_orders(
        &self,
        token_id: &str,
        orders: &[&PolymarketOrder],
    ) -> Result<Position> {
        let first_order = orders
            .first()
            .ok_or_else(|| anyhow::anyhow!("No orders provided"))?;

        let mut position = Position {
            market_id: first_order.market.clone(),
            token_id: token_id.to_string(),
            outcome: first_order.outcome.clone(),
            side: PositionSide::Long, // Will be determined by net position
            size: Decimal::ZERO,
            average_price: Decimal::ZERO,
            current_price: None,
            realized_pnl: Decimal::ZERO,
            unrealized_pnl: None,
            status: PositionStatus::Open,
            opened_at: DateTime::from_timestamp(first_order.created_at as i64 / 1000, 0)
                .unwrap_or_else(|| Utc::now()),
            updated_at: Utc::now(),
            closed_at: None,
            fees_paid: Decimal::ZERO,
            market_question: Some(first_order.question_id.clone().unwrap_or_default()),
        };

        let mut total_buy_size = Decimal::ZERO;
        let mut total_buy_value = Decimal::ZERO;
        let mut total_sell_size = Decimal::ZERO;
        let mut total_sell_value = Decimal::ZERO;

        // Process each order
        for order in orders {
            let filled_size = order
                .size_matched
                .parse::<Decimal>()
                .unwrap_or_else(|_| Decimal::ZERO);

            if filled_size.is_zero() {
                continue;
            }

            // Polymarket has no trading fees
            let fee = Decimal::ZERO;
            position.fees_paid += fee;

            match order.side.as_str() {
                "BUY" => {
                    total_buy_size += filled_size;
                    total_buy_value += filled_size * order.price;
                }
                "SELL" => {
                    total_sell_size += filled_size;
                    total_sell_value += filled_size * order.price;

                    // Calculate realized P&L for this sell
                    if total_buy_size > Decimal::ZERO && position.average_price > Decimal::ZERO {
                        let realized = (order.price - position.average_price) * filled_size;
                        position.realized_pnl += realized;
                    }
                }
                _ => warn!("Unknown order side: {}", order.side),
            }

            position.updated_at = Utc::now();
        }

        // Calculate net position
        position.size = total_buy_size - total_sell_size;

        if position.size > Decimal::ZERO {
            // Net long position
            position.side = PositionSide::Long;
            position.average_price = if total_buy_size > Decimal::ZERO {
                total_buy_value / total_buy_size
            } else {
                Decimal::ZERO
            };
        } else if position.size < Decimal::ZERO {
            // Net short position (shouldn't happen with Polymarket)
            position.side = PositionSide::Short;
            position.size = position.size.abs();
            position.average_price = if total_sell_size > Decimal::ZERO {
                total_sell_value / total_sell_size
            } else {
                Decimal::ZERO
            };
        } else {
            // Position closed
            position.status = PositionStatus::Closed;
            position.closed_at = Some(Utc::now());
        }

        Ok(position)
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

    /// Calculate portfolio summary statistics
    pub fn calculate_stats(&self) -> PortfolioStats {
        let positions: Vec<&Position> = self.positions.values().collect();

        let total_positions = positions.len();
        let open_positions = positions
            .iter()
            .filter(|p| p.status == PositionStatus::Open)
            .count();

        let total_realized_pnl: Decimal = positions.iter().map(|p| p.realized_pnl).sum();

        let total_unrealized_pnl: Decimal = positions.iter().filter_map(|p| p.unrealized_pnl).sum();

        let total_fees_paid: Decimal = positions.iter().map(|p| p.fees_paid).sum();

        // Calculate win rate from closed positions
        let closed_positions: Vec<&&Position> = positions
            .iter()
            .filter(|p| p.status == PositionStatus::Closed)
            .collect();

        let (win_rate, average_win, average_loss) = if !closed_positions.is_empty() {
            let wins: Vec<&&Position> = closed_positions
                .iter()
                .filter(|p| p.realized_pnl > Decimal::ZERO)
                .cloned()
                .collect();

            let losses: Vec<&&Position> = closed_positions
                .iter()
                .filter(|p| p.realized_pnl < Decimal::ZERO)
                .cloned()
                .collect();

            let win_rate = if !closed_positions.is_empty() {
                Some(
                    Decimal::from(wins.len()) / Decimal::from(closed_positions.len())
                        * Decimal::from(100),
                )
            } else {
                None
            };

            let average_win = if !wins.is_empty() {
                Some(
                    wins.iter().map(|p| p.realized_pnl).sum::<Decimal>()
                        / Decimal::from(wins.len()),
                )
            } else {
                None
            };

            let average_loss = if !losses.is_empty() {
                Some(
                    losses.iter().map(|p| p.realized_pnl.abs()).sum::<Decimal>()
                        / Decimal::from(losses.len()),
                )
            } else {
                None
            };

            (win_rate, average_win, average_loss)
        } else {
            (None, None, None)
        };

        PortfolioStats {
            total_balance: Decimal::ZERO,     // Will be set externally
            available_balance: Decimal::ZERO, // Will be set externally
            locked_balance: Decimal::ZERO,    // Will be set externally
            total_positions,
            open_positions,
            total_realized_pnl,
            total_unrealized_pnl,
            total_fees_paid,
            win_rate,
            average_win,
            average_loss,
            sharpe_ratio: None, // TODO: Calculate Sharpe ratio
            last_updated: Utc::now(),
        }
    }
}
