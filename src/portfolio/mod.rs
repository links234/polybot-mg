//! Portfolio management and position tracking
//!
//! This module provides comprehensive portfolio management with an actor-based service,
//! real-time data tracking, API integration, and persistent storage.

pub mod manager;
pub mod types;
pub mod orders_api;
pub mod storage;
pub mod reconciler;
pub mod service;
// pub mod api;
// pub mod cache;
pub mod command_handlers;
pub mod display;

// Core types
pub use types::{
    Position, PositionStatus, PortfolioStats, ActiveOrder,
    OrderSide, OrderStatus, MarketPositionSummary, TradeExecution,
};

// Storage
// pub use storage::{PortfolioStorage, PortfolioSnapshot, AccountBalances};

// Service
pub use service::{
    PortfolioServiceHandle, PortfolioState,
    start_portfolio_service,
};

// API integration
// pub use api::{PortfolioApiClient, ApiTrade, ApiPosition};
// pub use orders_api::{PolymarketOrder, BalanceInfo};

// Cache
// pub use cache::PortfolioCache;

// Command handlers
pub use command_handlers::{
    get_portfolio_service_handle,
};

// Display utilities
pub use display::{
    TradesFormatter,
    DashboardFormatter,
};

// Other components
// pub use reconciler::PositionReconciler;