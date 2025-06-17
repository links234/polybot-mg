//! Portfolio type definitions with strong typing

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Position side (long/short)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionSide {
    Long,
    Short,
}

/// Position status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionStatus {
    Open,
    Closed,
    Liquidated,
}

/// Position in a specific market/outcome
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub market_id: String,
    pub token_id: String,
    pub outcome: String,
    pub side: PositionSide,
    pub size: Decimal,
    pub average_price: Decimal,
    pub current_price: Option<Decimal>,
    pub realized_pnl: Decimal,
    pub unrealized_pnl: Option<Decimal>,
    pub status: PositionStatus,
    pub opened_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub fees_paid: Decimal,
    pub market_question: Option<String>,
}

impl Position {
    /// Calculate current value of position
    pub fn _current_value(&self) -> Option<Decimal> {
        self.current_price.map(|price| self.size * price)
    }
    
    /// Calculate total P&L (realized + unrealized)
    pub fn total_pnl(&self) -> Decimal {
        self.realized_pnl + self.unrealized_pnl.unwrap_or(Decimal::ZERO)
    }
    
    /// Calculate P&L percentage
    pub fn pnl_percentage(&self) -> Option<Decimal> {
        let cost_basis = self.size * self.average_price;
        if cost_basis.is_zero() {
            None
        } else {
            Some((self.total_pnl() / cost_basis) * Decimal::from(100))
        }
    }
    
    /// Update unrealized P&L based on current price
    pub fn _update_unrealized_pnl(&mut self, current_price: Decimal) {
        self.current_price = Some(current_price);
        let cost_basis = self.size * self.average_price;
        let current_value = self.size * current_price;
        self.unrealized_pnl = Some(match self.side {
            PositionSide::Long => current_value - cost_basis,
            PositionSide::Short => cost_basis - current_value,
        });
        self.updated_at = Utc::now();
    }
}

/// Active order information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveOrder {
    pub order_id: String,
    pub market_id: String,
    pub token_id: String,
    pub outcome: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub price: Decimal,
    pub size: Decimal,
    pub filled_size: Decimal,
    pub remaining_size: Decimal,
    pub status: OrderStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub time_in_force: TimeInForce,
    pub post_only: bool,
    pub reduce_only: bool,
}

/// Order side
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

/// Order type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    Limit,
    Market,
}

/// Order status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    Pending,
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
    Expired,
}

/// Time in force
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeInForce {
    GTC,  // Good Till Cancelled
    IOC,  // Immediate or Cancel
    FOK,  // Fill or Kill
}

/// Portfolio statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioStats {
    pub total_balance: Decimal,
    pub available_balance: Decimal,
    pub locked_balance: Decimal,
    pub total_positions: usize,
    pub open_positions: usize,
    pub total_realized_pnl: Decimal,
    pub total_unrealized_pnl: Decimal,
    pub total_fees_paid: Decimal,
    pub win_rate: Option<Decimal>,
    pub average_win: Option<Decimal>,
    pub average_loss: Option<Decimal>,
    pub sharpe_ratio: Option<Decimal>,
    pub last_updated: DateTime<Utc>,
}

impl PortfolioStats {
    /// Calculate total portfolio value
    pub fn total_portfolio_value(&self) -> Decimal {
        self.total_balance + self.total_unrealized_pnl
    }
    
    /// Calculate total P&L
    pub fn total_pnl(&self) -> Decimal {
        self.total_realized_pnl + self.total_unrealized_pnl
    }
    
    /// Calculate P&L percentage
    pub fn pnl_percentage(&self, initial_balance: Decimal) -> Option<Decimal> {
        if initial_balance.is_zero() {
            None
        } else {
            Some((self.total_pnl() / initial_balance) * Decimal::from(100))
        }
    }
}

/// Order update event from WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderUpdate {
    pub order_id: String,
    pub market_id: String,
    pub token_id: String,
    pub update_type: OrderUpdateType,
    pub timestamp: DateTime<Utc>,
    pub order: ActiveOrder,
}

/// Order update type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderUpdateType {
    Placed,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
    Expired,
}

/// Trade execution event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeExecution {
    pub trade_id: String,
    pub order_id: String,
    pub market_id: String,
    pub token_id: String,
    pub side: OrderSide,
    pub price: Decimal,
    pub size: Decimal,
    pub fee: Decimal,
    pub timestamp: DateTime<Utc>,
    pub is_maker: bool,
}

/// Balance update event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceUpdate {
    pub currency: String,
    pub available_balance: Decimal,
    pub locked_balance: Decimal,
    pub total_balance: Decimal,
    pub timestamp: DateTime<Utc>,
}

/// Portfolio event types for WebSocket streaming
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PortfolioEvent {
    OrderUpdate(OrderUpdate),
    TradeExecution(TradeExecution),
    BalanceUpdate(BalanceUpdate),
    PositionUpdate {
        position: Position,
        update_type: PositionUpdateType,
    },
}

/// Position update type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionUpdateType {
    Opened,
    Updated,
    Closed,
    Liquidated,
}

/// Market position summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketPositionSummary {
    pub market_id: String,
    pub market_question: String,
    pub positions: Vec<Position>,
    pub total_exposure: Decimal,
    pub net_position: Decimal,
    pub total_pnl: Decimal,
    pub has_open_orders: bool,
    pub open_order_count: usize,
}