//! CLI Commands module
//!
//! This module contains all command implementations for the Polybot CLI.
//! Each command follows a consistent pattern with dedicated Args and Command structs.
//!
//! See README.md for comprehensive documentation of all available commands,
//! their usage patterns, and integration points.

// Command modules
pub mod init;
pub mod markets;
pub mod book;
pub mod buy;
pub mod sell;
pub mod cancel;
pub mod orders;
pub mod portfolio;
pub mod portfolio_tui;
pub mod fetch_all_markets;
pub mod analyze;
pub mod enrich;
pub mod stream;
pub mod daemon;
pub mod pipeline;
pub mod datasets;
pub mod install;
pub mod version;
pub mod index;
pub mod worktree;
