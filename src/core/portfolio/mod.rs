//! Portfolio management and position tracking
//!
//! This module provides comprehensive portfolio management with:
//! - **API Layer**: Service-based API for portfolio operations
//! - **Controller**: Business logic and state management
//! - **Storage**: Persistent storage with caching
//! - **CLI**: Command-line interface handlers
//! - **Display**: Formatting and display utilities

pub mod api;
pub mod cli;
pub mod controller;
pub mod display;
pub mod storage;
pub mod types;

// Re-export core types
pub use types::{
    ActiveOrder, MarketPositionSummary, OrderSide, OrderStatus,
    PortfolioStats, Position, PositionSide, PositionStatus,
    TradeExecution,
};

// Re-export API components
pub use api::{PortfolioServiceHandle};

// Re-export storage
pub use storage::PortfolioStorage;