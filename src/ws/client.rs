//! WebSocket client for Polymarket streaming with auto-reconnection

use crate::ws::events::{WsMessage, MarketSubscription, UserSubscription};
use backoff::{ExponentialBackoff, backoff::Backoff};
use futures::{SinkExt, StreamExt};
use serde_json;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{interval, Instant, MissedTickBehavior};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

#[derive(Error, Debug)]
pub enum WsError {
    #[error("Connection error: {0}")]
    Connection(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Channel send error")]
    ChannelSend,
}

/// Commands that can be sent to the WebSocket client
#[derive(Debug)]
pub enum WsCommand {
    /// Subscribe to market feed for given asset IDs
    SubscribeMarket(Vec<String>),
    /// Subscribe to user feed for given markets and auth
    SubscribeUser(Vec<String>, crate::ws::events::AuthPayload),
    /// Disconnect
    Disconnect,
}

/// WebSocket client configuration
#[derive(Debug, Clone)]
pub struct WsConfig {
    /// Market feed URL
    pub market_url: String,
    /// User feed URL  
    pub user_url: String,
    /// Heartbeat interval in seconds
    pub heartbeat_interval: u64,
    /// Maximum reconnection attempts (0 = infinite)
    pub max_reconnection_attempts: u32,
    /// Initial reconnection delay in milliseconds
    pub initial_reconnection_delay: u64,
    /// Maximum reconnection delay in milliseconds
    pub max_reconnection_delay: u64,
}

impl Default for WsConfig {
    fn default() -> Self {
        Self {
            market_url: "wss://ws-subscriptions-clob.polymarket.com/ws/market".to_string(),
            user_url: "wss://ws-subscriptions-clob.polymarket.com/ws/user".to_string(),
            heartbeat_interval: 10,
            max_reconnection_attempts: 0, // Infinite retries
            initial_reconnection_delay: 1000,
            max_reconnection_delay: 30000,
        }
    }
}

/// WebSocket client for a single connection
pub struct WsClient {
    command_tx: mpsc::UnboundedSender<WsCommand>,
    message_rx: broadcast::Receiver<WsMessage>,
}

impl WsClient {
    /// Create a new WebSocket client for market feed
    pub async fn new_market(config: WsConfig) -> Result<Self, WsError> {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (message_tx, message_rx) = broadcast::channel(1000);

        let url = config.market_url.clone();
        let client_config = config.clone();

        // Spawn the connection task
        tokio::spawn(async move {
            Self::connection_task(url, client_config, command_rx, message_tx).await;
        });

        Ok(Self {
            command_tx,
            message_rx,
        })
    }

    /// Create a new WebSocket client for user feed
    pub async fn new_user(config: WsConfig) -> Result<Self, WsError> {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (message_tx, message_rx) = broadcast::channel(1000);

        let url = config.user_url.clone();
        let client_config = config.clone();

        // Spawn the connection task
        tokio::spawn(async move {
            Self::connection_task(url, client_config, command_rx, message_tx).await;
        });

        Ok(Self {
            command_tx,
            message_rx,
        })
    }

    /// Subscribe to market feed for given asset IDs
    pub fn subscribe_market(&self, asset_ids: Vec<String>) -> Result<(), WsError> {
        self.command_tx
            .send(WsCommand::SubscribeMarket(asset_ids))
            .map_err(|_| WsError::ChannelSend)
    }

    /// Subscribe to user feed for given markets and auth
    pub fn subscribe_user(
        &self,
        markets: Vec<String>,
        auth: crate::ws::events::AuthPayload,
    ) -> Result<(), WsError> {
        self.command_tx
            .send(WsCommand::SubscribeUser(markets, auth))
            .map_err(|_| WsError::ChannelSend)
    }

    /// Get a receiver for incoming messages
    pub fn messages(&self) -> broadcast::Receiver<WsMessage> {
        self.message_rx.resubscribe()
    }



    /// Disconnect
    pub fn disconnect(&self) -> Result<(), WsError> {
        self.command_tx
            .send(WsCommand::Disconnect)
            .map_err(|_| WsError::ChannelSend)
    }

    /// Main connection task with auto-reconnection
    async fn connection_task(
        url: String,
        config: WsConfig,
        mut command_rx: mpsc::UnboundedReceiver<WsCommand>,
        message_tx: broadcast::Sender<WsMessage>,
    ) {
        let mut reconnection_attempts = 0;
        
        loop {
            match Self::connect_and_run(&url, &config, &mut command_rx, &message_tx).await {
                Ok(()) => {
                    info!("WebSocket connection closed normally");
                    break;
                }
                Err(e) => {
                    error!("WebSocket connection error: {}", e);
                    
                    // Check if we should attempt reconnection
                    if config.max_reconnection_attempts > 0 
                        && reconnection_attempts >= config.max_reconnection_attempts {
                        error!("Maximum reconnection attempts reached");
                        break;
                    }
                    
                    reconnection_attempts += 1;
                    
                    // Calculate backoff delay
                    let mut backoff = ExponentialBackoff {
                        initial_interval: Duration::from_millis(config.initial_reconnection_delay),
                        max_interval: Duration::from_millis(config.max_reconnection_delay),
                        max_elapsed_time: None,
                        ..Default::default()
                    };
                    
                    if let Some(delay) = backoff.next_backoff() {
                        warn!("Reconnecting in {:?} (attempt {})", delay, reconnection_attempts);
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }
    }

    /// Connect and run the WebSocket session
    async fn connect_and_run(
        url: &str,
        config: &WsConfig,
        command_rx: &mut mpsc::UnboundedReceiver<WsCommand>,
        message_tx: &broadcast::Sender<WsMessage>,
    ) -> Result<(), WsError> {
        info!("Connecting to WebSocket: {}", url);
        
        let (ws_stream, _response) = connect_async(url).await?;
        let (mut write, mut read) = ws_stream.split();
        
        info!("WebSocket connected successfully. Status: {:?}", _response.status());
        debug!("Response headers: {:?}", _response.headers());
        
        // Set up heartbeat timer
        let mut heartbeat = interval(Duration::from_secs(config.heartbeat_interval));
        heartbeat.set_missed_tick_behavior(MissedTickBehavior::Skip);
        
        let mut last_pong = Instant::now();
        let pong_timeout = Duration::from_secs(config.heartbeat_interval * 2);
        
        loop {
            tokio::select! {
                // Handle incoming messages
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            debug!("Raw WebSocket message received: {}", text);
                            
                            // Try to parse as different formats to understand the structure
                            if text.trim() == "[]" {
                                debug!("Received empty array - likely subscription confirmation or no data");
                                continue;
                            }
                            
                            // Try to parse as array of events (Polymarket format)
                            match serde_json::from_str::<Vec<serde_json::Value>>(&text) {
                                Ok(events) => {
                                    debug!("Parsed {} events from websocket", events.len());
                                    
                                    // Send each event individually
                                    for event in events {
                                        // Polymarket uses either "type" or "event_type" field
                                        let event_type = event.get("type")
                                            .or_else(|| event.get("event_type"))
                                            .and_then(|v| v.as_str());
                                            
                                        if let Some(event_type) = event_type {
                                            let ws_msg = WsMessage {
                                                event_type: event_type.to_string(),
                                                data: event,
                                            };
                                            
                                            if let Err(e) = message_tx.send(ws_msg) {
                                                warn!("Failed to send event to channel: {}", e);
                                            }
                                        } else {
                                            warn!("Event missing type/event_type field: {:?}", event);
                                        }
                                    }
                                }
                                Err(e) => {
                                    // Fallback: try to parse as single WsMessage
                                    match serde_json::from_str::<WsMessage>(&text) {
                                        Ok(ws_msg) => {
                                            debug!("Parsed single WebSocket message: {:?}", ws_msg);
                                            if let Err(e) = message_tx.send(ws_msg) {
                                                warn!("Failed to send message to channel: {}", e);
                                            }
                                        }
                                        Err(e2) => {
                                            error!("Failed to parse message: Array parse: {}, Single parse: {} - Raw: {}", e, e2, text);
                                        }
                                    }
                                }
                            }
                        }
                        Some(Ok(Message::Pong(_))) => {
                            debug!("Received pong");
                            last_pong = Instant::now();
                        }
                        Some(Ok(Message::Close(_))) => {
                            info!("WebSocket closed by server");
                            break;
                        }
                        Some(Err(e)) => {
                            error!("WebSocket error: {}", e);
                            return Err(WsError::Connection(e));
                        }
                        None => {
                            warn!("WebSocket stream ended");
                            return Err(WsError::Connection(
                                tokio_tungstenite::tungstenite::Error::ConnectionClosed
                            ));
                        }
                        _ => {
                            // Ignore other message types
                        }
                    }
                }
                
                // Handle commands
                cmd = command_rx.recv() => {
                    match cmd {
                        Some(WsCommand::SubscribeMarket(asset_ids)) => {
                            let subscription = MarketSubscription::new(asset_ids.clone());
                            let msg = serde_json::to_string(&subscription)?;
                            info!("Sending market subscription for {} assets", asset_ids.len());
                            debug!("Market subscription message: {}", msg);
                            debug!("Asset IDs: {:?}", asset_ids);
                            write.send(Message::Text(msg.into())).await?;
                            info!("Market subscription sent successfully");
                        }
                        Some(WsCommand::SubscribeUser(markets, auth)) => {
                            let subscription = UserSubscription::new(markets, auth);
                            let msg = serde_json::to_string(&subscription)?;
                            debug!("Sending user subscription");
                            write.send(Message::Text(msg.into())).await?;
                        }
                        Some(WsCommand::Disconnect) => {
                            info!("Disconnect requested");
                            write.send(Message::Close(None)).await?;
                            break;
                        }
                        None => {
                            warn!("Command channel closed");
                            break;
                        }
                    }
                }
                
                // Heartbeat
                _ = heartbeat.tick() => {
                    // Check if we received a recent pong
                    if last_pong.elapsed() > pong_timeout {
                        warn!("Heartbeat timeout - no pong received");
                        return Err(WsError::Connection(
                            tokio_tungstenite::tungstenite::Error::ConnectionClosed
                        ));
                    }
                    
                    debug!("Sending heartbeat ping");
                    if let Err(e) = write.send(Message::Ping(vec![].into())).await {
                        error!("Failed to send heartbeat: {}", e);
                        return Err(WsError::Connection(e));
                    }
                }
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_config_default() {
        let config = WsConfig::default();
        assert_eq!(config.heartbeat_interval, 10);
        assert_eq!(config.max_reconnection_attempts, 0);
        assert!(config.market_url.contains("ws-subscriptions-clob.polymarket.com"));
    }

    #[tokio::test]
    async fn test_client_creation() {
        let config = WsConfig::default();
        let client = WsClient::new_market(config).await;
        assert!(client.is_ok());
    }
} 