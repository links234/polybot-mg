//! Portfolio display utilities and formatters
//!
//! This module provides formatting and display utilities for portfolio data
//! including positions, orders, trades, and statistics.

use rust_decimal::Decimal;

use crate::portfolio::types::*;

/// Format portfolio statistics for display
pub struct PortfolioStatsFormatter<'a> {
    pub stats: &'a PortfolioStats,
}

impl<'a> PortfolioStatsFormatter<'a> {
    pub fn new(stats: &'a PortfolioStats) -> Self {
        Self { stats }
    }

    /// Format as a table
    pub fn format_table(&self) -> String {
        let mut output = String::new();
        
        output.push_str("┌─────────────────────────┬─────────────────┐\n");
        output.push_str("│ Portfolio Statistics    │ Value           │\n");
        output.push_str("├─────────────────────────┼─────────────────┤\n");
        
        output.push_str(&format!("│ Total Balance           │ ${:>14.2} │\n", self.stats.total_balance));
        output.push_str(&format!("│ Available Balance       │ ${:>14.2} │\n", self.stats.available_balance));
        output.push_str(&format!("│ Locked Balance          │ ${:>14.2} │\n", self.stats.locked_balance));
        output.push_str(&format!("│ Total Positions         │ {:>15} │\n", self.stats.total_positions));
        output.push_str(&format!("│ Open Positions          │ {:>15} │\n", self.stats.open_positions));
        output.push_str(&format!("│ Realized P&L            │ ${:>14.2} │\n", self.stats.total_realized_pnl));
        output.push_str(&format!("│ Unrealized P&L          │ ${:>14.2} │\n", self.stats.total_unrealized_pnl));
        output.push_str(&format!("│ Total P&L               │ ${:>14.2} │\n", self.stats.total_pnl()));
        output.push_str(&format!("│ Total Fees Paid         │ ${:>14.2} │\n", self.stats.total_fees_paid));
        
        if let Some(win_rate) = self.stats.win_rate {
            output.push_str(&format!("│ Win Rate                │ {:>13.1}% │\n", win_rate));
        }
        
        if let Some(avg_win) = self.stats.average_win {
            output.push_str(&format!("│ Average Win             │ ${:>14.2} │\n", avg_win));
        }
        
        if let Some(avg_loss) = self.stats.average_loss {
            output.push_str(&format!("│ Average Loss            │ ${:>14.2} │\n", avg_loss));
        }
        
        output.push_str("└─────────────────────────┴─────────────────┘\n");
        
        output
    }

}

/// Format positions for display
pub struct PositionsFormatter<'a> {
    pub positions: &'a [Position],
}

impl<'a> PositionsFormatter<'a> {
    pub fn new(positions: &'a [Position]) -> Self {
        Self { positions }
    }

    /// Format as a table
    pub fn format_table(&self) -> String {
        if self.positions.is_empty() {
            return "No positions found.\n".to_string();
        }

        let mut output = String::new();
        
        output.push_str("┌──────────┬─────────────┬──────────┬───────────┬──────────┬─────────────┐\n");
        output.push_str("│ Market   │ Outcome     │ Side     │ Size      │ Avg Price│ P&L         │\n");
        output.push_str("├──────────┼─────────────┼──────────┼───────────┼──────────┼─────────────┤\n");
        
        for position in self.positions {
            let market_short = if position.market_id.len() > 8 {
                format!("{}...", &position.market_id[..8])
            } else {
                position.market_id.clone()
            };
            
            let outcome_short = if position.outcome.len() > 11 {
                format!("{}...", &position.outcome[..8])
            } else {
                position.outcome.clone()
            };
            
            let side_str = match position.side {
                PositionSide::Long => "LONG",
                PositionSide::Short => "SHORT",
            };
            
            let pnl = position.total_pnl();
            let pnl_color = if pnl >= Decimal::ZERO { "+" } else { "" };
            
            output.push_str(&format!(
                "│ {:<8} │ {:<11} │ {:<8} │ {:>9.2} │ {:>8.3} │ {}{:>10.2} │\n",
                market_short,
                outcome_short,
                side_str,
                position.size,
                position.average_price,
                pnl_color,
                pnl
            ));
        }
        
        output.push_str("└──────────┴─────────────┴──────────┴───────────┴──────────┴─────────────┘\n");
        
        output
    }

}

/// Format active orders for display
pub struct OrdersFormatter<'a> {
    pub orders: &'a [ActiveOrder],
}

impl<'a> OrdersFormatter<'a> {
    pub fn new(orders: &'a [ActiveOrder]) -> Self {
        Self { orders }
    }

    /// Format as a table
    pub fn format_table(&self) -> String {
        if self.orders.is_empty() {
            return "No active orders found.\n".to_string();
        }

        let mut output = String::new();
        
        output.push_str("┌─────────────┬─────────────┬──────────┬──────────┬───────────┬─────────────┐\n");
        output.push_str("│ Order ID    │ Outcome     │ Side     │ Price    │ Size      │ Status      │\n");
        output.push_str("├─────────────┼─────────────┼──────────┼──────────┼───────────┼─────────────┤\n");
        
        for order in self.orders {
            let order_id_short = if order.order_id.len() > 11 {
                format!("{}...", &order.order_id[..8])
            } else {
                order.order_id.clone()
            };
            
            let outcome_short = if order.outcome.len() > 11 {
                format!("{}...", &order.outcome[..8])
            } else {
                order.outcome.clone()
            };
            
            let side_str = match order.side {
                OrderSide::Buy => "BUY",
                OrderSide::Sell => "SELL",
            };
            
            let status_str = format!("{:?}", order.status);
            let status_short = if status_str.len() > 11 {
                format!("{}...", &status_str[..8])
            } else {
                status_str
            };
            
            output.push_str(&format!(
                "│ {:<11} │ {:<11} │ {:<8} │ {:>8.3} │ {:>9.2} │ {:<11} │\n",
                order_id_short,
                outcome_short,
                side_str,
                order.price,
                order.size,
                status_short
            ));
        }
        
        output.push_str("└─────────────┴─────────────┴──────────┴──────────┴───────────┴─────────────┘\n");
        
        output
    }

}

/// Format trade history for display
pub struct TradesFormatter<'a> {
    pub trades: &'a [TradeExecution],
}

impl<'a> TradesFormatter<'a> {
    pub fn new(trades: &'a [TradeExecution]) -> Self {
        Self { trades }
    }

    /// Format as a table
    pub fn format_table(&self, limit: Option<usize>) -> String {
        if self.trades.is_empty() {
            return "No trades found.\n".to_string();
        }

        let trades_to_show = if let Some(limit) = limit {
            self.trades.iter().rev().take(limit).collect::<Vec<_>>()
        } else {
            self.trades.iter().collect::<Vec<_>>()
        };

        let mut output = String::new();
        
        output.push_str("┌─────────────┬──────────┬──────────┬───────────┬──────────┬─────────────────┐\n");
        output.push_str("│ Trade ID    │ Side     │ Price    │ Size      │ Fee      │ Timestamp       │\n");
        output.push_str("├─────────────┼──────────┼──────────┼───────────┼──────────┼─────────────────┤\n");
        
        for trade in trades_to_show {
            let trade_id_short = if trade.trade_id.len() > 11 {
                format!("{}...", &trade.trade_id[..8])
            } else {
                trade.trade_id.clone()
            };
            
            let side_str = match trade.side {
                OrderSide::Buy => "BUY",
                OrderSide::Sell => "SELL",
            };
            
            output.push_str(&format!(
                "│ {:<11} │ {:<8} │ {:>8.3} │ {:>9.2} │ {:>8.3} │ {:<15} │\n",
                trade_id_short,
                side_str,
                trade.price,
                trade.size,
                trade.fee,
                trade.timestamp.format("%m-%d %H:%M")
            ));
        }
        
        output.push_str("└─────────────┴──────────┴──────────┴───────────┴──────────┴─────────────────┘\n");
        
        if let Some(limit) = limit {
            if self.trades.len() > limit {
                output.push_str(&format!("Showing {} of {} trades\n", limit, self.trades.len()));
            }
        }
        
        output
    }

}

/// Portfolio dashboard formatter
pub struct DashboardFormatter<'a> {
    pub portfolio_state: &'a crate::portfolio::PortfolioState,
    pub address: &'a str,
    pub host: &'a str,
}

impl<'a> DashboardFormatter<'a> {
    pub fn new(
        portfolio_state: &'a crate::portfolio::PortfolioState,
        address: &'a str,
        host: &'a str,
    ) -> Self {
        Self {
            portfolio_state,
            address,
            host,
        }
    }

    /// Format complete portfolio dashboard
    pub fn format_dashboard(&self) -> String {
        let mut output = String::new();
        
        // Header
        output.push_str("╔══════════════════════════════════════════════════════════════════════════════╗\n");
        output.push_str("║                              PORTFOLIO DASHBOARD                             ║\n");
        output.push_str("╚══════════════════════════════════════════════════════════════════════════════╝\n\n");
        
        // Account info
        output.push_str(&format!("👤 Account: {}\n", self.address));
        output.push_str(&format!("🔗 Profile: https://polymarket.com/profile/{}\n", self.address));
        output.push_str(&format!("🌐 Host: {}\n", self.host));
        output.push_str(&format!("🔄 Last Updated: {}\n", self.portfolio_state.last_updated.format("%Y-%m-%d %H:%M:%S UTC")));
        output.push_str(&format!("✅ Synced: {}\n\n", if self.portfolio_state.is_synced { "Yes" } else { "No" }));
        
        // Account balances
        output.push_str("💰 ACCOUNT BALANCES\n");
        output.push_str(&format!("   Total Value: ${:.2}\n", self.portfolio_state.balances.total_value));
        output.push_str(&format!("   Available Cash: ${:.2}\n", self.portfolio_state.balances.available_cash));
        output.push_str(&format!("   Locked in Orders: ${:.2}\n", self.portfolio_state.balances.locked_in_orders));
        output.push_str(&format!("   Position Value: ${:.2}\n\n", self.portfolio_state.balances.position_value));
        
        // Statistics
        output.push_str("📊 PORTFOLIO STATISTICS\n");
        let stats_formatter = PortfolioStatsFormatter::new(&self.portfolio_state.stats);
        output.push_str(&stats_formatter.format_table());
        output.push('\n');
        
        // Positions
        if !self.portfolio_state.positions.is_empty() {
            output.push_str("📍 POSITIONS\n");
            let positions_formatter = PositionsFormatter::new(&self.portfolio_state.positions);
            output.push_str(&positions_formatter.format_table());
            output.push('\n');
        }
        
        // Active orders
        if !self.portfolio_state.active_orders.is_empty() {
            output.push_str("📋 ACTIVE ORDERS\n");
            let orders_formatter = OrdersFormatter::new(&self.portfolio_state.active_orders);
            output.push_str(&orders_formatter.format_table());
            output.push('\n');
        }
        
        output.push_str("💡 Use 'polybot stream' for real-time monitoring\n");
        output.push_str("💡 Use 'polybot buy/sell --help' for trading commands\n");
        
        output
    }

}

