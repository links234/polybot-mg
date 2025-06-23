//! CLI Commands module
//!
//! This module contains all command implementations for the Polybot CLI.
//! Each command follows a consistent pattern with dedicated Args and Command structs.
//!
//! See README.md for comprehensive documentation of all available commands,
//! their usage patterns, and integration points.

// Command modules
pub mod analyze;
pub mod book;
pub mod buy;
pub mod cancel;
pub mod canvas;
pub mod daemon;
pub mod datasets;
pub mod enrich;
pub mod fetch_all_markets;
pub mod index;
pub mod init;
pub mod install;
pub mod markets;
pub mod orders;
pub mod pipeline;
pub mod portfolio;
pub mod portfolio_tui;
pub mod sell;
pub mod stream;
pub mod version;
pub mod worktree;
pub mod portfolio_status;
pub mod trades;
// pub mod gamma;
