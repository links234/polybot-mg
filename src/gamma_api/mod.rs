//! Gamma API integration for historical data
//!
//! This module provides access to Polymarket's Gamma API endpoints for
//! fetching positions, activity, and holder information.

pub mod types;
pub mod client;
pub mod storage;
pub mod tracker;

// Note: Types are available via gamma_api::types::* when needed
