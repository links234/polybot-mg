//! Streaming service that manages WebSocket connections and order book state

use crate::auth::get_authenticated_client;
use crate::data_paths::DataPaths;
use crate::core::execution::orderbook::AssetOrderBook;
use crate::core::types::market::PriceLevel;
use crate::core::ws::{
    WsClient, WsConfig,
    parse_message, AuthPayload, EventError, PolyEvent, WsMessage,
    OrderBook, StateError,
};
use dashmap::DashMap;
use polymarket_rs_client::ClobClient;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, trace, warn};

#[derive(Error, Debug)]
pub enum StreamerError {
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] crate::core::ws::WsError),
    #[error("Event parsing error: {0}")]
    EventParsing(#[from] EventError),
    #[error("State error: {0}")]
    State(#[from] StateError),
    #[error("Authentication error: {0}")]
    Auth(#[from] anyhow::Error),
}

/// Configuration for the streaming service
#[derive(Debug, Clone)]
pub struct StreamerConfig {
    /// WebSocket client configuration
    pub ws_config: WsConfig,
    /// Asset IDs to subscribe to for market data
    pub market_assets: Vec<String>,
    /// Markets to subscribe to for user data (optional)
    pub user_markets: Option<Vec<String>>,
    /// Authentication for user feed (optional)
    pub user_auth: Option<AuthPayload>,
    /// Buffer size for event broadcast channel
    pub event_buffer_size: usize,
    /// Whether to automatically sync order books on hash mismatch
    pub auto_sync_on_hash_mismatch: bool,
}

impl Default for StreamerConfig {
    fn default() -> Self {
        Self {
            ws_config: WsConfig::default(),
            market_assets: Vec::new(),
            user_markets: None,
            user_auth: None,
            event_buffer_size: 1000,
            auto_sync_on_hash_mismatch: true,
        }
    }
}

/// Streaming service that manages WebSocket connections and order book state
pub struct Streamer {
    config: StreamerConfig,
    order_books: Arc<DashMap<String, OrderBook>>,
    last_trade_prices: Arc<DashMap<String, (rust_decimal::Decimal, u64)>>, // (price, timestamp)
    event_tx: broadcast::Sender<PolyEvent>,
    event_rx: broadcast::Receiver<PolyEvent>,
    market_client: Option<WsClient>,
    user_client: Option<WsClient>,
    rest_client: Option<Arc<ClobClient>>,
    market_task: Option<JoinHandle<()>>,
    user_task: Option<JoinHandle<()>>,
}

impl Streamer {
    /// Create a new streaming service
    pub fn new(config: StreamerConfig) -> Self {
        let (event_tx, event_rx) = broadcast::channel(config.event_buffer_size);

        Self {
            config,
            order_books: Arc::new(DashMap::new()),
            last_trade_prices: Arc::new(DashMap::new()),
            event_tx,
            event_rx,
            market_client: None,
            user_client: None,
            rest_client: None,
            market_task: None,
            user_task: None,
        }
    }




    /// Start the streaming service
    pub async fn start(&mut self, host: &str, data_paths: &DataPaths) -> Result<(), StreamerError> {
        info!("Starting Polymarket streaming service");

        // Initialize REST client for order book sync
        if self.config.auto_sync_on_hash_mismatch {
            let client = get_authenticated_client(host, data_paths).await?;
            self.rest_client = Some(Arc::new(client));
        }

        // Start market data feed
        if !self.config.market_assets.is_empty() {
            self.start_market_feed().await?;

            // Fetch initial orderbooks from REST API if available
            if let Some(rest_client) = &self.rest_client {
                self.fetch_initial_orderbooks(rest_client.clone()).await;
            }
        }

        // Start user data feed if configured
        if let (Some(markets), Some(auth)) = (&self.config.user_markets, &self.config.user_auth) {
            self.start_user_feed(markets.clone(), auth.clone()).await?;
        }

        info!(
            "Streaming service started for {} assets",
            self.config.market_assets.len()
        );

        Ok(())
    }

    /// Stop the streaming service
    pub async fn stop(&mut self) {
        info!("Stopping streaming service");

        // Disconnect WebSocket clients
        if let Some(client) = &self.market_client {
            let _ = client.disconnect();
        }
        if let Some(client) = &self.user_client {
            let _ = client.disconnect();
        }

        // Wait for tasks to complete
        if let Some(task) = self.market_task.take() {
            let _ = task.await;
        }
        if let Some(task) = self.user_task.take() {
            let _ = task.await;
        }

        self.market_client = None;
        self.user_client = None;

        info!("Streaming service stopped");
    }

    /// Get a receiver for streaming events
    pub fn events(&self) -> broadcast::Receiver<PolyEvent> {
        self.event_rx.resubscribe()
    }

    /// Get current order book for an asset
    pub fn get_order_book(&self, asset_id: &str) -> Option<OrderBook> {
        self.order_books.get(asset_id).map(|entry| entry.clone())
    }

    /// Get all current order books
    pub fn get_all_order_books(&self) -> Vec<AssetOrderBook> {
        self.order_books
            .iter()
            .map(|entry| AssetOrderBook::new(entry.key().clone(), entry.value().clone()))
            .collect()
    }

    /// Start market data feed
    async fn start_market_feed(&mut self) -> Result<(), StreamerError> {
        info!(
            "Starting market data feed for {} assets",
            self.config.market_assets.len()
        );

        let client = WsClient::new_market(self.config.ws_config.clone()).await?;
        client.subscribe_market(self.config.market_assets.clone())?;

        let mut messages = client.messages();
        let order_books = Arc::clone(&self.order_books);
        let last_trade_prices = Arc::clone(&self.last_trade_prices);
        let event_tx = self.event_tx.clone();
        let rest_client = self.rest_client.clone();
        let auto_sync = self.config.auto_sync_on_hash_mismatch;
        let skip_hash_verification = self.config.ws_config.skip_hash_verification;

        let task = tokio::spawn(async move {
            info!("Market data feed task started, waiting for messages...");
            if skip_hash_verification {
                info!("📋 Hash verification is DISABLED for orderbook updates");
            }
            while let Ok(ws_message) = messages.recv().await {
                debug!("Received WebSocket message: {:?}", ws_message);
                Self::handle_market_message(
                    ws_message,
                    &order_books,
                    &last_trade_prices,
                    &event_tx,
                    &rest_client,
                    auto_sync,
                    skip_hash_verification,
                )
                .await;
            }
            warn!("Market data feed task ended");
        });

        self.market_client = Some(client);
        self.market_task = Some(task);

        Ok(())
    }

    /// Start user data feed
    async fn start_user_feed(
        &mut self,
        markets: Vec<String>,
        auth: AuthPayload,
    ) -> Result<(), StreamerError> {
        info!("Starting user data feed for {} markets", markets.len());

        let client = WsClient::new_user(self.config.ws_config.clone()).await?;
        client.subscribe_user(markets, auth)?;

        let mut messages = client.messages();
        let event_tx = self.event_tx.clone();

        let task = tokio::spawn(async move {
            while let Ok(ws_message) = messages.recv().await {
                Self::handle_user_message(ws_message, &event_tx).await;
            }
        });

        self.user_client = Some(client);
        self.user_task = Some(task);

        Ok(())
    }

    /// Handle market WebSocket message
    async fn handle_market_message(
        ws_message: WsMessage,
        order_books: &DashMap<String, OrderBook>,
        last_trade_prices: &DashMap<String, (rust_decimal::Decimal, u64)>,
        event_tx: &broadcast::Sender<PolyEvent>,
        rest_client: &Option<Arc<ClobClient>>,
        auto_sync: bool,
        skip_hash_verification: bool,
    ) {
        match parse_message(&ws_message) {
            Ok(events) => {
                for event in events {
                    match &event {
                        PolyEvent::Book {
                            asset_id,
                            bids,
                            asks,
                            hash,
                        } => {
                            trace!(
                                "Received book update for asset {}: {} bids, {} asks",
                                asset_id,
                                bids.len(),
                                asks.len()
                            );
                            Self::handle_book_event(
                                asset_id,
                                bids,
                                asks,
                                hash,
                                order_books,
                                rest_client,
                                auto_sync,
                                skip_hash_verification,
                            )
                            .await;
                        }
                        PolyEvent::PriceChange {
                            asset_id,
                            side,
                            price,
                            size,
                            hash,
                        } => {
                            trace!(
                                "Received price change for asset {}: {:?} {} @ {}",
                                asset_id,
                                side,
                                size,
                                price
                            );
                            Self::handle_price_change_event(
                                asset_id,
                                *side,
                                *price,
                                *size,
                                hash,
                                order_books,
                                rest_client,
                                auto_sync,
                                skip_hash_verification,
                            )
                            .await;
                        }
                        PolyEvent::Trade {
                            asset_id,
                            price,
                            size,
                            side,
                            ..
                        } => {
                            trace!(
                                "Received trade for asset {}: {:?} {} @ {}",
                                asset_id,
                                side,
                                size,
                                price
                            );
                        }
                        PolyEvent::TickSizeChange {
                            asset_id,
                            tick_size,
                        } => {
                            trace!(
                                "Received tick size change for asset {}: {}",
                                asset_id,
                                tick_size
                            );
                            if let Some(mut book) = order_books.get_mut(asset_id) {
                                book.set_tick_size(*tick_size);
                            }
                        }
                        PolyEvent::LastTradePrice {
                            asset_id,
                            price,
                            timestamp,
                        } => {
                            info!(
                                "Received last trade price for asset {}: ${} at {}",
                                asset_id, price, timestamp
                            );
                            // Store the last trade price
                            last_trade_prices.insert(asset_id.clone(), (*price, *timestamp));
                            // TODO: Store in trades flatbuffer file for persistence
                        }
                        _ => {
                            debug!("Received other event: {:?}", event);
                        }
                    }

                    match event_tx.send(event) {
                        Ok(receiver_count) => {
                            debug!(
                                "Successfully broadcast event to {} receivers",
                                receiver_count
                            );
                        }
                        Err(e) => {
                            warn!("Failed to broadcast event: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to parse WebSocket message: {}", e);

                // Count parsing errors and potentially notify user in extreme cases
                static PARSING_ERROR_COUNT: std::sync::atomic::AtomicUsize =
                    std::sync::atomic::AtomicUsize::new(0);
                let error_count =
                    PARSING_ERROR_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;

                if error_count <= 5 {
                    // Log detailed information for the first few errors
                    error!("Parsing error #{}: {}", error_count, e);
                    error!("This indicates a message format issue that may affect data quality");
                } else if error_count == 10 {
                    error!("Suppressing further parsing error details after 10 errors");
                    error!(
                        "Continuing to process other messages, but data quality may be compromised"
                    );
                } else if error_count % 100 == 0 {
                    // Log every 100th error to track ongoing issues
                    error!(
                        "Parsing error count reached {}, data stream may be corrupted",
                        error_count
                    );
                }
            }
        }
    }

    /// Handle user WebSocket message
    async fn handle_user_message(ws_message: WsMessage, event_tx: &broadcast::Sender<PolyEvent>) {
        match parse_message(&ws_message) {
            Ok(events) => {
                for event in events {
                    if let Err(e) = event_tx.send(event) {
                        warn!("Failed to broadcast user event: {}", e);
                    }
                }
            }
            Err(e) => {
                error!("Failed to parse user WebSocket message: {}", e);

                // Count user message parsing errors separately
                static USER_PARSING_ERROR_COUNT: std::sync::atomic::AtomicUsize =
                    std::sync::atomic::AtomicUsize::new(0);
                let error_count =
                    USER_PARSING_ERROR_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;

                if error_count <= 5 {
                    error!("User message parsing error #{}: {}", error_count, e);
                    error!("This may affect user order/trade visibility");
                } else if error_count == 10 {
                    error!("Suppressing further user parsing error details after 10 errors");
                }
            }
        }
    }

    /// Handle order book snapshot event
    async fn handle_book_event(
        asset_id: &str,
        bids: &[PriceLevel],
        asks: &[PriceLevel],
        hash: &str,
        order_books: &DashMap<String, OrderBook>,
        _rest_client: &Option<Arc<ClobClient>>,
        _auto_sync: bool,
        skip_hash_verification: bool,
    ) {
        let mut book = order_books
            .entry(asset_id.to_string())
            .or_insert_with(|| OrderBook::new(asset_id.to_string()));

        if skip_hash_verification {
            // Skip hash verification - apply directly
            book.replace_with_snapshot_no_hash(bids.to_vec(), asks.to_vec());
            debug!(
                "Order book snapshot applied (no hash verification) for {}",
                asset_id
            );
        } else {
            match book.replace_with_snapshot(bids.to_vec(), asks.to_vec(), hash.to_string()) {
                Ok(()) => {
                    debug!("Order book snapshot applied for {}", asset_id);
                }
                Err(e) => {
                    warn!(
                        "Failed to apply order book snapshot for {}: {}",
                        asset_id, e
                    );
                    // Fallback to no hash validation
                    warn!("Applying snapshot without hash validation for {}", asset_id);
                    book.replace_with_snapshot_no_hash(bids.to_vec(), asks.to_vec());
                }
            }
        }

        // Always validate and clean the orderbook after snapshot
        if book.validate_and_clean() {
            warn!(
                "Orderbook for {} was cleaned due to crossed market",
                asset_id
            );
        }
    }

    /// Handle price change event
    async fn handle_price_change_event(
        asset_id: &str,
        side: crate::core::types::common::Side,
        price: rust_decimal::Decimal,
        size: rust_decimal::Decimal,
        hash: &str,
        order_books: &DashMap<String, OrderBook>,
        _rest_client: &Option<Arc<ClobClient>>,
        _auto_sync: bool,
        skip_hash_verification: bool,
    ) {
        if let Some(mut book) = order_books.get_mut(asset_id) {
            if skip_hash_verification {
                // Skip hash verification - apply directly
                book.apply_price_change_no_hash(side, price, size);
                debug!(
                    "Price change applied (no hash verification) for {}: {:?} {} @ {}",
                    asset_id, side, size, price
                );
            } else {
                match book.apply_price_change(side, price, size, hash.to_string()) {
                    Ok(()) => {
                        debug!(
                            "Price change applied for {}: {:?} {} @ {}",
                            asset_id, side, size, price
                        );
                    }
                    Err(e) => {
                        warn!(
                            "Failed to apply price change for {}: {} - {:?} {} @ {}",
                            asset_id, e, side, size, price
                        );
                        // Fallback to no hash validation
                        warn!(
                            "Applying price change without hash validation for {}",
                            asset_id
                        );
                        book.apply_price_change_no_hash(side, price, size);
                    }
                }
            }

            // Always validate and clean the orderbook after changes
            if book.validate_and_clean() {
                warn!(
                    "Orderbook for {} was cleaned due to crossed market after price change",
                    asset_id
                );
            }
        } else {
            warn!("Received price change for unknown asset: {}", asset_id);
        }
    }

    /// Get summary of all order books
    pub fn summary(&self) -> Vec<String> {
        self.order_books
            .iter()
            .map(|entry| entry.value().summary())
            .collect()
    }

    /// Fetch initial orderbooks from REST API
    async fn fetch_initial_orderbooks(&self, rest_client: Arc<ClobClient>) {
        info!(
            "Fetching initial orderbooks for {} assets",
            self.config.market_assets.len()
        );

        let mut success_count = 0;
        let mut failure_count = 0;

        for (index, asset_id) in self.config.market_assets.iter().enumerate() {
            // Add rate limiting to avoid overwhelming the API
            if index > 0 && index % 100 == 0 {
                // Add small delay every 100 requests to avoid API throttling
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }

            if index % 5000 == 0 && index > 0 {
                info!("Processing asset {} of {} ({:.1}% complete) - {} successes, {} failures so far",
                      index + 1, self.config.market_assets.len(),
                      (index as f64 / self.config.market_assets.len() as f64 * 100.0),
                      success_count, failure_count);
            }

            // Add small delay between each request to avoid overwhelming the API
            if index > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }

            match rest_client.get_order_book(asset_id).await {
                Ok(orderbook_response) => {
                    debug!(
                        "Successfully fetched orderbook for asset {}: {} bids, {} asks",
                        asset_id,
                        orderbook_response.bids.len(),
                        orderbook_response.asks.len()
                    );
                    info!("Fetched initial orderbook for asset {}", asset_id);

                    // Convert REST API format to our internal format
                    let bids: Vec<PriceLevel> = orderbook_response
                        .bids
                        .iter()
                        .filter_map(|bid| {
                            // The price and size are already Decimals from the REST API
                            Some(PriceLevel::new(bid.price, bid.size))
                        })
                        .collect();

                    let asks: Vec<PriceLevel> = orderbook_response
                        .asks
                        .iter()
                        .filter_map(|ask| {
                            // The price and size are already Decimals from the REST API
                            Some(PriceLevel::new(ask.price, ask.size))
                        })
                        .collect();

                    // Store the orderbook without hash validation (REST API doesn't provide hash)
                    let mut book = self
                        .order_books
                        .entry(asset_id.clone())
                        .or_insert_with(|| OrderBook::new(asset_id.clone()));

                    book.replace_with_snapshot_no_hash(bids, asks);

                    // Validate and clean the orderbook
                    if book.validate_and_clean() {
                        warn!(
                            "Initial orderbook for {} was cleaned due to crossed market",
                            asset_id
                        );
                    }

                    info!(
                        "Stored initial orderbook for asset {} with {} bids and {} asks",
                        asset_id,
                        book.bids.len(),
                        book.asks.len()
                    );
                    success_count += 1;
                }
                Err(e) => {
                    failure_count += 1;

                    // Check if this is the common "no orderbook exists" error
                    let error_str = e.to_string();
                    if error_str.contains("No orderbook exists")
                        || error_str.contains("error decoding response body")
                    {
                        // This is expected for many assets - just debug log it
                        debug!("No orderbook available for asset {}: {}", asset_id, e);
                    } else {
                        // This is an unexpected error - log as error with details
                        error!(
                            "Unexpected error fetching orderbook for asset {}: {}",
                            asset_id, e
                        );

                        // Get raw response for debugging unexpected errors
                        if let Ok(response) = reqwest::Client::new()
                            .get(&format!(
                                "https://clob.polymarket.com/book?token_id={}",
                                asset_id
                            ))
                            .send()
                            .await
                        {
                            if let Ok(response_text) = response.text().await {
                                error!("Raw response for unexpected error: {}", response_text);
                            }
                        }
                    }
                }
            }
        }

        info!(
            "Finished fetching initial orderbooks: {} successes, {} failures ({:.1}% success rate)",
            success_count,
            failure_count,
            if success_count + failure_count > 0 {
                success_count as f64 / (success_count + failure_count) as f64 * 100.0
            } else {
                0.0
            }
        );
    }
}

impl Drop for Streamer {
    fn drop(&mut self) {
        if self.market_task.is_some() || self.user_task.is_some() {
            warn!("Streamer dropped without calling stop()");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streamer_config_default() {
        let config = StreamerConfig::default();
        assert_eq!(config.event_buffer_size, 1000);
        assert!(config.auto_sync_on_hash_mismatch);
        assert!(config.market_assets.is_empty());
    }

    #[test]
    fn test_streamer_creation() {
        let config = StreamerConfig::default();
        let streamer = Streamer::new(config);
        assert_eq!(streamer.order_books.len(), 0);
    }
}
