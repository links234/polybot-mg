//! Portfolio management and position tracking
//!
//! This module provides real-time portfolio and position tracking
//! with WebSocket streaming updates from the user channel.

pub mod manager;
pub mod types;
pub mod orders_api;
pub mod storage;
pub mod reconciler;

// pub use manager::PortfolioManager;
pub use types::{
    Position, PositionSide, PositionStatus, PortfolioStats, ActiveOrder,
    // _PortfolioEvent,
    OrderSide, OrderStatus, MarketPositionSummary
};
pub use storage::{PortfolioStorage, PortfolioSnapshot, AccountBalances};
pub use reconciler::PositionReconciler;
// pub use orders_api::{PolymarketOrder, fetch_orders_via_client};