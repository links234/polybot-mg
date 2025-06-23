//! Multi-connection WebSocket streaming service
//!
//! This module provides a scalable streaming service that manages multiple
//! WebSocket connections, with each connection handling a subset of tokens.

pub mod config;
pub mod event_aggregator;
pub mod service;
pub mod token_distributor;
pub mod traits;
pub mod worker;

pub use config::StreamingServiceConfig;
pub use service::StreamingService;
pub use traits::StreamingServiceTrait;
