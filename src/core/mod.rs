//! Core module containing fundamental components of the polybot system
//! 
//! This module provides the core building blocks including:
//! - **Execution engine**: Unified streaming and orderbook management
//! - **Services**: WebSocket streaming and market data management  
//! - **WebSocket client**: Real-time event handling and state management
//! - **Common types**: Shared data structures and events
//! - **Trait definitions**: Shared interfaces and abstractions

pub mod execution;
pub mod portfolio;
pub mod services;
pub mod traits;
pub mod types;
pub mod ws;

// Note: Re-exports removed to eliminate unused import warnings
// Use explicit module paths for clarity: core::execution::, core::services::, etc.


