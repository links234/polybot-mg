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
// Temporarily commented out due to missing storage module dependencies
// pub mod dataset_selector_v2;
// pub mod selection_builder;
// pub mod selections_menu;

pub use app::{App, AppState};
pub use events::EventHandler;
// pub use selection_builder::{SelectionBuilder, SelectionBuilderResult};
// pub use selections_menu::{SelectionsMenu, SelectionsMenuResult};