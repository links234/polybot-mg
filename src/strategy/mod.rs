//! Strategy system for automated trading
//!
//! This module provides a flexible framework for implementing trading strategies
//! that react to orderbook updates and trade events.

use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;
use std::sync::Arc;
use polymarket_rs_client::ClobClient;

use crate::core::types::common::Side;
use crate::core::ws::{OrderBook, PolyEvent};

pub mod simple_strategy;

// Re-export for convenience
pub use simple_strategy::SimpleStrategy;

/// Trade event for strategy consumption
#[derive(Debug, Clone)]
pub struct TradeEvent {
    pub asset_id: String,
    pub price: Decimal,
    pub size: Decimal,
    pub side: Side,
    pub timestamp: u64,
}

impl From<&PolyEvent> for Option<TradeEvent> {
    fn from(event: &PolyEvent) -> Self {
        match event {
            PolyEvent::Trade { asset_id, price, size, side } => Some(TradeEvent {
                asset_id: asset_id.clone(),
                price: *price,
                size: *size,
                side: *side,
                timestamp: chrono::Utc::now().timestamp() as u64,
            }),
            PolyEvent::LastTradePrice { asset_id, price, timestamp } => Some(TradeEvent {
                asset_id: asset_id.clone(),
                price: *price,
                size: Decimal::ZERO, // Last trade price doesn't include size
                side: Side::Buy, // Default side for price updates
                timestamp: *timestamp,
            }),
            _ => None,
        }
    }
}

/// Strategy trait for single token operations
/// Simple interface focused on orderbook updates and trade events
#[async_trait]
pub trait SingleTokenStrategy: Send + Sync {
    /// Called when orderbook is updated
    async fn orderbook_update(&self, orderbook: &OrderBook) -> Result<()>;
    
    /// Called when a trade event occurs
    async fn trade_event(&self, trade: &TradeEvent) -> Result<()>;
    
    /// Set the ClobClient for order placement
    fn set_clob_client(&mut self, client: Arc<tokio::sync::Mutex<ClobClient>>);
    
    /// Process any pending orders that need to be placed
    async fn process_pending_orders(&self) -> Result<()>;
    
    /// Get strategy name for logging
    fn name(&self) -> &str;
    
    /// Token ID this strategy is responsible for
    fn token_id(&self) -> &str;
    
    /// Shutdown the strategy gracefully
    async fn shutdown(&self) -> Result<()>;
}