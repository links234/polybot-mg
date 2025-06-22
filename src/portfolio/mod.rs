//! Portfolio management and position tracking
//!
//! This module provides real-time portfolio and position tracking
//! with WebSocket streaming updates from the user channel.

pub mod manager;
pub mod orders_api;
pub mod reconciler;
pub mod storage;
pub mod types;

// pub use manager::PortfolioManager;
pub use reconciler::PositionReconciler;
pub use storage::{AccountBalances, PortfolioSnapshot, PortfolioStorage};
pub use types::{
    ActiveOrder,
    MarketPositionSummary,
    // _PortfolioEvent,
    OrderSide,
    OrderStatus,
    PortfolioStats,
    Position,
    PositionSide,
    PositionStatus,
};
// pub use orders_api::{PolymarketOrder, fetch_orders_via_client};
