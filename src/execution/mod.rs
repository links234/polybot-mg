//! Execution Engine - Unified streaming and data processing system
//! 
//! This module provides a comprehensive execution framework that supports:
//! - Real-time WebSocket streaming from Polymarket
//! - Replay of historical data from files
//! - Event processing and state management
//! - Multiple execution modes and strategies
//!
//! See README.md for detailed architecture documentation.

pub mod engine;
pub mod sources;
pub mod events;
pub mod strategies;
pub mod config;

pub use engine::ExecutionEngine;
pub use config::ExecutionMode;
pub use sources::{DataSource, EventStream};
pub use events::{ExecutionEvent, EventHandler};
pub use strategies::{Strategy, StrategyConfig};
pub use config::ExecutionConfig;