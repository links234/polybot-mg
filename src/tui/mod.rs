//! Terminal User Interface (TUI) module for Polymarket data visualization
//!
//! This module provides a comprehensive terminal-based interface for real-time
//! order book visualization and market data monitoring using ratatui.
//!
//! Key components:
//! - Application state management with real-time event processing
//! - Interactive terminal UI with keyboard navigation
//! - Specialized widgets for financial data display
//! - Responsive layout with dynamic sizing
//!
//! For comprehensive documentation, see [README.md](./README.md)

pub mod app;
pub mod ui;
pub mod events;
pub mod widgets;
pub mod portfolio_view;
pub mod portfolio_simple;

pub use app::{App, AppState};
pub use events::EventHandler;
// pub use portfolio_view::{PortfolioViewState, run_portfolio_tui};
// pub use portfolio_simple::display_portfolio_simple;