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
pub mod index;
pub mod markets;

pub use app::{App, AppState};
pub use events::EventHandler;
pub use index::IndexTui;
pub use markets::MarketsTui;
// pub use portfolio_view::{PortfolioViewState, run_portfolio_tui};
// pub use portfolio_simple::display_portfolio_simple;

// Re-export progress types from indexing module
#[derive(Debug, Clone)]
pub struct IndexingProgress {
    pub current_file: usize,
    pub total_files: usize,
    pub current_file_name: String,
    pub markets_in_file: usize,
    pub markets_processed: usize,
    pub total_markets_indexed: usize,
    pub total_conditions: usize,
    pub total_tokens: usize,
    pub duplicates_skipped: usize,
    pub phase: IndexingPhase,
    pub events: Vec<String>,
    pub start_time: std::time::Instant,
}

impl Default for IndexingProgress {
    fn default() -> Self {
        Self {
            current_file: 0,
            total_files: 0,
            current_file_name: String::new(),
            markets_in_file: 0,
            markets_processed: 0,
            total_markets_indexed: 0,
            total_conditions: 0,
            total_tokens: 0,
            duplicates_skipped: 0,
            phase: IndexingPhase::Starting,
            events: Vec::new(),
            start_time: std::time::Instant::now(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum IndexingPhase {
    Starting,
    ProcessingFiles,
    IndexingConditions,
    IndexingTokens,
    Finalizing,
    Completed,
    Failed(String),
}

#[derive(Debug, Clone)]
pub enum ProgressUpdate {
    FileStart { file_index: usize, total_files: usize, file_name: String, market_count: usize },
    MarketProcessed { markets_in_batch: usize },
    FileComplete { duplicates: usize },
    PhaseChange(IndexingPhase),
    Event(String),
    ConditionCount(usize),
    TokenCount(usize),
    Complete,
    Error(String),
}

pub fn create_progress_channel() -> (tokio::sync::mpsc::UnboundedSender<ProgressUpdate>, tokio::sync::mpsc::UnboundedReceiver<ProgressUpdate>) {
    tokio::sync::mpsc::unbounded_channel()
}
