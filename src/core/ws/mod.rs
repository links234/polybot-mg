//! WebSocket client and state management
//! 
//! This module provides the core WebSocket functionality for connecting to
//! Polymarket's CLOB API and managing real-time market data streams.

pub mod client;
pub mod events;
pub mod state;
pub mod types;

// Re-export commonly used items
pub use client::{WsClient, WsConfig, WsError};
pub use events::{PolyEvent, WsMessage, parse_message, EventError};
pub use state::{OrderBook, StateError};

// Re-export authentication types
pub use events::AuthPayload;