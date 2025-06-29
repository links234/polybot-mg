//! Portfolio API layer
//!
//! Provides service-based API for all portfolio operations including
//! trading, order management, and position queries.

pub mod orders;
pub mod service;
pub mod types;

// Note: orders::* re-export removed to eliminate unused import warnings
pub use service::start_portfolio_service;
pub use types::{PortfolioServiceHandle, PortfolioState};