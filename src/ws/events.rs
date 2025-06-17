//! WebSocket event models for Polymarket streaming data

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize, Deserializer};
use thiserror::Error;
use tracing::{debug, info, warn, error};
use crate::execution::orderbook::PriceLevel;

#[derive(Error, Debug)]
pub enum EventError {
    #[error("Invalid message format: {0}")]
    InvalidFormat(String),
    #[error("Unknown event type: {0}")]
    UnknownEventType(String),
}

/// High-level events published by the streamer
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum PolyEvent {
    /// Order book snapshot or update
    Book {
        asset_id: String,
        bids: Vec<PriceLevel>,
        asks: Vec<PriceLevel>,
        hash: String,
    },
    /// Price level change (add/remove/update)
    PriceChange {
        asset_id: String,
        side: Side,
        price: Decimal,
        size: Decimal, // 0 means removal
        hash: String,
    },
    /// Tick size change
    TickSizeChange {
        asset_id: String,
        tick_size: Decimal,
    },
    /// Trade executed
    Trade {
        asset_id: String,
        price: Decimal,
        size: Decimal,
        side: Side,
    },
    /// User's order update
    MyOrder {
        asset_id: String,
        side: Side,
        price: Decimal,
        size: Decimal,
        status: OrderStatus,
    },
    /// User's trade
    MyTrade {
        asset_id: String,
        side: Side,
        price: Decimal,
        size: Decimal,
    },
}

/// Raw WebSocket message envelope
#[derive(Debug, Clone, Deserialize)]
pub struct WsMessage {
    #[serde(alias = "event_type", rename = "type")]
    pub event_type: String,
    #[serde(flatten)]
    pub data: serde_json::Value,
}

/// Market feed subscription message
#[derive(Debug, Serialize)]
pub struct MarketSubscription {
    #[serde(rename = "type")]
    pub event_type: String, // "market"
    pub assets_ids: Vec<String>,
}

impl MarketSubscription {
    pub fn new(asset_ids: Vec<String>) -> Self {
        Self {
            event_type: "market".to_string(),
            assets_ids: asset_ids,
        }
    }
}

/// User feed subscription message
#[derive(Debug, Serialize)]
pub struct UserSubscription {
    #[serde(rename = "type")]
    pub event_type: String, // "user"
    pub markets: Vec<String>,
    pub auth: AuthPayload,
}

impl UserSubscription {
    pub fn new(markets: Vec<String>, auth: AuthPayload) -> Self {
        Self {
            event_type: "user".to_string(),
            markets,
            auth,
        }
    }
}

/// Authentication payload for user feed
#[derive(Debug, Serialize, Clone)]
pub struct AuthPayload {
    #[serde(rename = "apiKey")]
    pub api_key: String,
    pub secret: String,
    pub passphrase: String,
}

/// Trading side
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Side {
    Buy,
    Sell,
}

impl<'de> serde::Deserialize<'de> for Side {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "buy" => Ok(Side::Buy),
            "sell" => Ok(Side::Sell),
            _ => Err(serde::de::Error::unknown_variant(&s, &["buy", "sell"])),
        }
    }
}

/// Order status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum OrderStatus {
    Open,
    Filled,
    Cancelled,
    PartiallyFilled,
}

/// Order book snapshot event (Polymarket format)
#[derive(Debug, Deserialize)]
pub struct BookEvent {
    pub asset_id: String,
    #[serde(alias = "buys", alias = "bids", deserialize_with = "deserialize_order_levels")]
    pub bids: Vec<PriceLevel>,
    #[serde(alias = "sells", alias = "asks", deserialize_with = "deserialize_order_levels")]
    pub asks: Vec<PriceLevel>,
    #[serde(default)]
    pub hash: String,
}

/// Order level for book events  
#[derive(Debug, Deserialize)]
struct OrderLevel {
    #[serde(deserialize_with = "deserialize_decimal_flexible")]
    price: Decimal,
    #[serde(deserialize_with = "deserialize_decimal_flexible")]
    size: Decimal,
}

/// Price change event (order add/cancel/update) - Polymarket format
#[derive(Debug, Deserialize)]
pub struct PriceChangeEvent {
    pub asset_id: String,
    pub changes: Vec<PriceChange>,
    pub hash: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PriceChange {
    #[serde(deserialize_with = "deserialize_decimal_flexible")]
    pub price: Decimal,
    pub side: Side,
    #[serde(deserialize_with = "deserialize_decimal_flexible")]
    pub size: Decimal,
}

/// Tick size change event
#[derive(Debug, Deserialize)]
pub struct TickSizeChangeEvent {
    pub asset_id: String,
    #[serde(deserialize_with = "deserialize_decimal_flexible")]
    pub tick_size: Decimal,
}

/// Trade event
#[derive(Debug, Deserialize)]
pub struct TradeEvent {
    pub asset_id: String,
    #[serde(deserialize_with = "deserialize_decimal_flexible")]
    pub price: Decimal,
    #[serde(deserialize_with = "deserialize_decimal_flexible")]
    pub size: Decimal,
    pub side: Side,
    pub timestamp: u64,
}

/// User order event
#[derive(Debug, Deserialize)]
pub struct UserOrderEvent {
    pub order_id: String,
    pub asset_id: String,
    pub side: Side,
    #[serde(deserialize_with = "deserialize_decimal_flexible")]
    pub price: Decimal,
    #[serde(deserialize_with = "deserialize_decimal_flexible")]
    pub size: Decimal,
    pub status: OrderStatus,
}

/// User trade event
#[derive(Debug, Deserialize)]
pub struct UserTradeEvent {
    pub trade_id: String,
    pub order_id: String,
    pub asset_id: String,
    pub side: Side,
    #[serde(deserialize_with = "deserialize_decimal_flexible")]
    pub price: Decimal,
    #[serde(deserialize_with = "deserialize_decimal_flexible")]
    pub size: Decimal,
    pub timestamp: u64,
}

/// Helper function to deserialize order levels from Polymarket book events
fn deserialize_order_levels<'de, D>(deserializer: D) -> Result<Vec<PriceLevel>, D::Error>
where
    D: Deserializer<'de>,
{
    let levels: Vec<OrderLevel> = Deserialize::deserialize(deserializer)?;
    Ok(levels.into_iter().map(|level| PriceLevel::new(level.price, level.size)).collect())
}


/// Helper function to deserialize decimal from string
fn _deserialize_decimal_string<'de, D>(deserializer: D) -> Result<Decimal, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    s.parse::<Decimal>()
        .map_err(|_| serde::de::Error::custom("Invalid decimal format"))
}

/// Helper function to deserialize decimal from either string or number
fn deserialize_decimal_flexible<'de, D>(deserializer: D) -> Result<Decimal, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::{Visitor, Error};
    use std::fmt;

    struct DecimalVisitor;

    impl<'de> Visitor<'de> for DecimalVisitor {
        type Value = Decimal;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a decimal number as string or number")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            value.parse::<Decimal>()
                .map_err(|_| E::custom(format!("Invalid decimal string: {}", value)))
        }

        fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Decimal::try_from(value)
                .map_err(|_| E::custom(format!("Invalid decimal number: {}", value)))
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(Decimal::from(value))
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(Decimal::from(value))
        }
    }

    deserializer.deserialize_any(DecimalVisitor)
}

/// Default hash value for book events when hash field is missing
fn _default_hash() -> String {
    "unknown".to_string()
}

/// Parse a raw WebSocket message into typed events
pub fn parse_message(msg: &WsMessage) -> Result<Vec<PolyEvent>, EventError> {
    debug!(event_type = %msg.event_type, "Parsing WebSocket message");
    
    match msg.event_type.as_str() {
        "book" => {
            let event: BookEvent = serde_json::from_value(msg.data.clone())
                .map_err(|e| {
                    error!(error = %e, event_type = "book", raw_data = ?msg.data, "Failed to parse book event");
                    // Log the keys available in the data to help debug
                    if let Some(obj) = msg.data.as_object() {
                        let keys: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
                        error!(available_keys = ?keys, "Available keys in book event");
                    }
                    EventError::InvalidFormat(e.to_string())
                })?;
            
            info!(
                asset_id = %event.asset_id,
                bid_levels = event.bids.len(),
                ask_levels = event.asks.len(),
                hash = %event.hash,
                "Parsed order book snapshot"
            );
            
            Ok(vec![PolyEvent::Book {
                asset_id: event.asset_id,
                bids: event.bids,
                asks: event.asks,
                hash: event.hash,
            }])
        }
        "price_change" => {
            let event: PriceChangeEvent = serde_json::from_value(msg.data.clone())
                .map_err(|e| {
                    error!(error = %e, event_type = "price_change", "Failed to parse price change event");
                    EventError::InvalidFormat(e.to_string())
                })?;
            
            debug!(
                asset_id = %event.asset_id,
                changes_count = event.changes.len(),
                hash = %event.hash,
                "Parsed price change event"
            );
            
            // Return all changes as individual events
            let mut events = Vec::new();
            for change in &event.changes {
                debug!(
                    asset_id = %event.asset_id,
                    side = ?change.side,
                    price = %change.price,
                    size = %change.size,
                    "Processing price change"
                );
                
                events.push(PolyEvent::PriceChange {
                    asset_id: event.asset_id.clone(),
                    side: change.side,
                    price: change.price,
                    size: change.size,
                    hash: event.hash.clone(),
                });
            }
            
            if events.is_empty() {
                warn!(asset_id = %event.asset_id, "Price change event has no changes");
                Err(EventError::InvalidFormat("Price change event has no changes".to_string()))
            } else {
                info!(asset_id = %event.asset_id, events_count = events.len(), "Generated price change events");
                Ok(events)
            }
        }
        "tick_size_change" => {
            let event: TickSizeChangeEvent = serde_json::from_value(msg.data.clone())
                .map_err(|e| {
                    error!(error = %e, event_type = "tick_size_change", "Failed to parse tick size change event");
                    EventError::InvalidFormat(e.to_string())
                })?;
            
            info!(
                asset_id = %event.asset_id,
                tick_size = %event.tick_size,
                "Parsed tick size change event"
            );
            
            Ok(vec![PolyEvent::TickSizeChange {
                asset_id: event.asset_id,
                tick_size: event.tick_size,
            }])
        }
        "trade" => {
            let event: TradeEvent = serde_json::from_value(msg.data.clone())
                .map_err(|e| {
                    error!(error = %e, event_type = "trade", "Failed to parse trade event");
                    EventError::InvalidFormat(e.to_string())
                })?;
            
            info!(
                asset_id = %event.asset_id,
                price = %event.price,
                size = %event.size,
                side = ?event.side,
                timestamp = event.timestamp,
                "Parsed trade event"
            );
            
            Ok(vec![PolyEvent::Trade {
                asset_id: event.asset_id,
                price: event.price,
                size: event.size,
                side: event.side,
            }])
        }
        "order" => {
            let event: UserOrderEvent = serde_json::from_value(msg.data.clone())
                .map_err(|e| {
                    error!(error = %e, event_type = "order", "Failed to parse user order event");
                    EventError::InvalidFormat(e.to_string())
                })?;
            
            info!(
                order_id = %event.order_id,
                asset_id = %event.asset_id,
                side = ?event.side,
                price = %event.price,
                size = %event.size,
                status = ?event.status,
                "Parsed user order event"
            );
            
            Ok(vec![PolyEvent::MyOrder {
                asset_id: event.asset_id,
                side: event.side,
                price: event.price,
                size: event.size,
                status: event.status,
            }])
        }
        "user_trade" => {
            let event: UserTradeEvent = serde_json::from_value(msg.data.clone())
                .map_err(|e| {
                    error!(error = %e, event_type = "user_trade", "Failed to parse user trade event");
                    EventError::InvalidFormat(e.to_string())
                })?;
            
            info!(
                trade_id = %event.trade_id,
                order_id = %event.order_id,
                asset_id = %event.asset_id,
                side = ?event.side,
                price = %event.price,
                size = %event.size,
                timestamp = event.timestamp,
                "Parsed user trade event"
            );
            
            Ok(vec![PolyEvent::MyTrade {
                asset_id: event.asset_id,
                side: event.side,
                price: event.price,
                size: event.size,
            }])
        }
        _ => {
            warn!(event_type = %msg.event_type, "Unknown event type");
            Err(EventError::UnknownEventType(msg.event_type.clone()))
        }
    }
} 