//! Execution Engine - Unified streaming and data processing system
//!
//! This module provides a comprehensive execution framework that supports:
//! - Real-time WebSocket streaming from Polymarket
//! - Replay of historical data from files
//! - Event processing and state management
//! - Multiple execution modes and strategies
//! - Order management and execution
//! - Orderbook representation and manipulation
//!
//! See README.md for detailed architecture documentation.

pub mod config;
pub mod engine;
pub mod events;
pub mod orderbook;
pub mod orders;
pub mod sources;
pub mod strategies;

// Note: AssetOrderBook is imported directly from orderbook module where needed
