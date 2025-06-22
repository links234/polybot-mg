//! Gamma API integration for historical data
//!
//! This module provides access to Polymarket's Gamma API endpoints for
//! fetching positions, activity, and holder information.

pub mod types;
pub mod client;
pub mod storage;
pub mod tracker;

// Re-export key types
pub use types::{
    GammaPosition, GammaActivity, GammaHolder, GammaMetadata,
    ActivityType, PositionState, GammaError,
};

pub use client::{GammaApiClient, GammaEndpoints};
pub use storage::{GammaStorage, UserDataPaths};
pub use tracker::{GammaTracker, TrackerCommand, start_gamma_tracker, get_gamma_tracker};