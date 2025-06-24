//! Markets module containing all market-related functionality
//!
//! This module provides a unified interface for all market operations including:
//! - **CLOB**: Central limit order book operations and market data
//! - **Datasets**: Market data management and storage
//! - **File Store**: File-based storage utilities for market data
//! - **Gamma**: Gamma API client and related functionality  
//! - **Gamma API**: Enhanced gamma API operations and storage
//! - **Search**: Indexed search capabilities for market data

pub mod clob;
pub mod datasets;
pub mod file_store;
pub mod gamma;
pub mod gamma_api;
pub mod search;

// Re-export commonly used functions from CLOB
pub use clob::{
    analyze_markets, enrich_markets, fetch_all_markets, fetch_all_markets_gamma,
    list_active_markets, list_filtered_markets, list_markets, search_markets,
    show_orderbook, get_market_details, get_market_from_url
};

// Note: Re-exports for individual modules removed to eliminate unused import warnings.
// Use explicit module paths when needed:
// - markets::datasets::DatasetManager
// - markets::file_store::*
// - markets::gamma::GammaClient, etc.

// Note: For gamma_api and search types, use the full module path:
// - gamma_api::client::GammaApiClient
// - gamma_api::tracker::GammaTracker  
// - search::milli_service::MilliSearchService
// - search::search_types::SearchFilters