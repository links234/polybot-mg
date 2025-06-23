//! Portfolio management and position tracking
//!
//! This module provides comprehensive portfolio management with an actor-based service,
//! real-time data tracking, API integration, and persistent storage.

pub mod manager;
pub mod orders_api;
pub mod reconciler;
pub mod service;
pub mod command_handlers;
pub mod display;
pub mod storage;
pub mod types;

// Core types
pub use types::{
    Position, PositionStatus, PortfolioStats, ActiveOrder,
    OrderSide, OrderStatus, MarketPositionSummary, TradeExecution,
    PositionSide,
};

// Storage
pub use storage::PortfolioStorage;

// Service
pub use service::{
    PortfolioServiceHandle, PortfolioState,
    start_portfolio_service,
};

// Reconciler
pub use reconciler::PositionReconciler;

// Command handlers
pub use command_handlers::{
    get_portfolio_service_handle,
};

// Display utilities
pub use display::{
    TradesFormatter,
    DashboardFormatter,
};