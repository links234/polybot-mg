//! Portfolio management and position tracking
//!
//! This module provides real-time portfolio and position tracking
//! with WebSocket streaming updates from the user channel.

pub mod manager;
pub mod types;
pub mod orders_api;

// pub use manager::PortfolioManager;
pub use types::{
    Position, PositionStatus, PortfolioStats, ActiveOrder,
    // _PortfolioEvent,
    OrderSide, OrderStatus, MarketPositionSummary
};
// pub use orders_api::{PolymarketOrder, fetch_orders_via_client};