//! WebSocket streaming module for Polymarket real-time data
//! 
//! This module provides:
//! - WebSocket client for market and user feeds
//! - Order book state management with hash verification
//! - Event models for market and user data
//! - Auto-reconnection and heartbeat functionality
//!
//! For comprehensive documentation, see [README.md](./README.md)

pub mod client;
pub mod events;
pub mod state;
pub mod types;

pub use client::*;
pub use events::*;
pub use state::*; 