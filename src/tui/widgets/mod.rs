//! TUI Widgets module for specialized financial data visualization
//!
//! This module contains reusable terminal UI widgets optimized for displaying
//! real-time financial market data with appropriate formatting, color coding,
//! and interactive features.
//!
//! Current widgets:
//! - Order book widget with price-first layout and cumulative totals
//! - Portfolio widget with positions, orders, and P&L tracking
//! - Interactive scrolling and navigation
//! - Error state visualization for crossed markets
//! - Real-time data synchronization
//!
//! For comprehensive documentation, see [README.md](./README.md)

pub mod order_book;
pub mod portfolio;

pub use order_book::draw_order_book;
// pub use portfolio::{PortfolioWidget, PortfolioTab};