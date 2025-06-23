//! # Gamma API Integration Module
//! 
//! Comprehensive integration with Polymarket's Gamma API providing market data,
//! events, trades, and position management functionality.
//! 
//! ## Architecture
//! 
//! - **Client**: HTTP client with built-in caching and rate limiting
//! - **Types**: Strongly-typed domain models for all API entities
//! - **Database**: SurrealDB storage with RocksDB backend for persistence
//! - **Session Manager**: Session-based data fetching and storage
//! - **TUI**: Interactive terminal interface for data exploration
//! - **Analytics**: Real-time market analytics and statistics
//! 
//! ## Usage
//! 
//! ```rust
//! use polybot::gamma::{GammaClient, MarketQuery};
//! 
//! let client = GammaClient::new(None)?;
//! let query = MarketQuery::default().with_limit(100);
//! let markets = client.fetch_markets(&query).await?;
//! ```
//! 
//! ## Database Import
//! 
//! Import session data into SurrealDB:
//! ```bash
//! # Import single session
//! cargo run -- gamma import-session --session-id 1
//! 
//! # Import range of sessions
//! cargo run -- gamma import-session --from-session-id 1 --to-session-id 10
//! 
//! # Import all sessions
//! cargo run -- gamma import-session --session-id all
//! ```

pub mod types;
pub mod client;
pub mod storage;
pub mod database;
pub mod tui;
pub mod search;
pub mod cache;
pub mod individual_storage;
pub mod session;
pub mod search_index;
pub mod fast_search;
pub mod fast_search_service;
pub mod index_service;

pub use types::*;
pub use client::GammaClient;
pub use storage::GammaStorage;
pub use database::GammaDatabase;
pub use search::{GammaSearchEngine, MarketAnalytics};
pub use tui::GammaTui;
#[allow(unused_imports)]
pub use cache::{GammaCache, Cursor, CacheStats, MarketFilter};
pub use individual_storage::IndividualMarketStorage;
pub use session::SessionManager;
pub use search_index::get_index_path;
pub use fast_search::{FastSearchEngine, SearchParams, build_fast_search_index};
pub use fast_search_service::{ServiceStatus, init_search_service};
pub use index_service::{IndexService, IndexStatus, IndexProgress, init_index_service};