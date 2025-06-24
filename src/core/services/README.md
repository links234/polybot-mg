# Services Module

The services module provides background service infrastructure for real-time market data streaming, WebSocket connection management, and order book state synchronization. It implements a robust, event-driven architecture for handling live market data feeds.

## Core Purpose and Responsibilities

The services module serves as the backbone for real-time operations:
- **WebSocket Streaming**: Managing persistent connections to market data feeds
- **Order Book Management**: Maintaining real-time order book state across multiple assets
- **Event Broadcasting**: Distributing market events to multiple consumers
- **State Synchronization**: Ensuring order book consistency with hash validation
- **Connection Resilience**: Automatic reconnection and error recovery

## Architecture Overview

```
src/services/
├── mod.rs          # Service layer interface and exports
└── streamer.rs     # WebSocket streaming service implementation
```

## Core Components

### Streaming Service (`streamer.rs`)

The `Streamer` struct provides comprehensive WebSocket management and real-time data processing:

```rust
/// Streaming service that manages WebSocket connections and order book state
pub struct Streamer {
    config: StreamerConfig,
    order_books: Arc<DashMap<String, OrderBook>>,
    event_tx: broadcast::Sender<PolyEvent>,
    event_rx: broadcast::Receiver<PolyEvent>,
    market_client: Option<WsClient>,
    user_client: Option<WsClient>,
    rest_client: Option<Arc<ClobClient>>,
    market_task: Option<JoinHandle<()>>,
    user_task: Option<JoinHandle<()>>,
}
```

### Configuration System

```rust
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
```

## Key Features

### Multi-Feed Management

The service handles both market data and user-specific feeds:

```rust
impl Streamer {
    /// Start the streaming service
    pub async fn start(
        &mut self,
        host: &str,
        data_paths: &DataPaths,
    ) -> Result<(), StreamerError> {
        // Initialize REST client for order book sync
        if self.config.auto_sync_on_hash_mismatch {
            let client = get_authenticated_client(host, data_paths).await?;
            self.rest_client = Some(Arc::new(client));
        }

        // Start market data feed
        if !self.config.market_assets.is_empty() {
            self.start_market_feed().await?;
            
            // Fetch initial orderbooks from REST API
            if let Some(rest_client) = &self.rest_client {
                self.fetch_initial_orderbooks(rest_client.clone()).await;
            }
        }

        // Start user data feed if configured
        if let (Some(markets), Some(auth)) = (
            &self.config.user_markets,
            &self.config.user_auth,
        ) {
            self.start_user_feed(markets.clone(), auth.clone()).await?;
        }

        Ok(())
    }
}
```

### Real-Time Order Book Management

The service maintains accurate order book state through sophisticated event handling:

```rust
/// Handle order book snapshot event
async fn handle_book_event(
    asset_id: &str,
    bids: &[(rust_decimal::Decimal, rust_decimal::Decimal)],
    asks: &[(rust_decimal::Decimal, rust_decimal::Decimal)],
    hash: &str,
    order_books: &DashMap<String, OrderBook>,
    rest_client: &Option<Arc<ClobClient>>,
    auto_sync: bool,
) {
    let mut book = order_books
        .entry(asset_id.to_string())
        .or_insert_with(|| OrderBook::new(asset_id.to_string()));

    match book.replace_with_snapshot(bids.to_vec(), asks.to_vec(), hash.to_string()) {
        Ok(()) => {
            debug!("Order book snapshot applied for {}", asset_id);
        }
        Err(e) => {
            warn!("Failed to apply order book snapshot for {}: {}", asset_id, e);
            // Fallback: apply without hash validation
            book.replace_with_snapshot_no_hash(bids.to_vec(), asks.to_vec());
            
            // Validate and clean the orderbook
            if book.validate_and_clean() {
                warn!("Orderbook for {} was cleaned due to crossed market", asset_id);
            }
        }
    }
}
```

### Event Broadcasting System

The service implements a robust event distribution mechanism:

```rust
/// Get a receiver for streaming events
pub fn events(&self) -> broadcast::Receiver<PolyEvent> {
    self.event_rx.resubscribe()
}

// Event handling in market feed task
while let Ok(ws_message) = messages.recv().await {
    match parse_message(&ws_message) {
        Ok(events) => {
            for event in events {
                match event_tx.send(event) {
                    Ok(receiver_count) => {
                        debug!("Successfully broadcast event to {} receivers", receiver_count);
                    }
                    Err(e) => {
                        warn!("Failed to broadcast event: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            warn!("Failed to parse WebSocket message: {}", e);
        }
    }
}
```

## Integration Patterns

### With WebSocket Module

The services module builds upon the WebSocket infrastructure:

```rust
use crate::ws::{
    client::{WsClient, WsConfig},
    events::{parse_message, AuthPayload, EventError, PolyEvent, WsMessage},
    state::{OrderBook, StateError},
};

// Create market data client
let client = WsClient::new_market(self.config.ws_config.clone()).await?;
client.subscribe_market(self.config.market_assets.clone())?;

// Create user data client
let client = WsClient::new_user(self.config.ws_config.clone()).await?;
client.subscribe_user(markets, auth)?;
```

### With Authentication Module

Integration with the auth module for authenticated operations:

```rust
use crate::auth::get_authenticated_client;

// Initialize REST client for order book sync
let client = get_authenticated_client(host, data_paths).await?;
self.rest_client = Some(Arc::new(client));
```

### With CLI Commands

Services are exposed through daemon and streaming commands:

```rust
// In src/cli/commands/daemon.rs
use crate::services::{Streamer, StreamerConfig};

let config = StreamerConfig {
    market_assets: asset_ids,
    event_buffer_size: 1000,
    auto_sync_on_hash_mismatch: true,
    ..Default::default()
};

let mut streamer = Streamer::new(config);
streamer.start(&host, &data_paths).await?;
```

## Usage Examples

### Basic Streaming Setup

```rust
use crate::services::{Streamer, StreamerConfig};
use crate::ws::WsConfig;

// Configure streaming service
let config = StreamerConfig {
    ws_config: WsConfig::default(),
    market_assets: vec![
        "21742633143463906290569050155826241533067272736897614950488156847949938836455".to_string(),
        "52114319501245915516055106046884519906829948041474353477841779262095928619659".to_string(),
    ],
    event_buffer_size: 1000,
    auto_sync_on_hash_mismatch: true,
    ..Default::default()
};

// Create and start streamer
let mut streamer = Streamer::new(config);
streamer.start("polymarket.com", &data_paths).await?;

// Subscribe to events
let mut events = streamer.events();
while let Ok(event) = events.recv().await {
    match event {
        PolyEvent::Book { asset_id, bids, asks, .. } => {
            println!("Book update for {}: {} bids, {} asks", asset_id, bids.len(), asks.len());
        }
        PolyEvent::Trade { asset_id, price, size, side, .. } => {
            println!("Trade on {}: {:?} {} @ {}", asset_id, side, size, price);
        }
        _ => {}
    }
}
```

### Order Book Access

```rust
// Get current order book for an asset
if let Some(order_book) = streamer.get_order_book(&asset_id) {
    println!("Best bid: {:?}", order_book.best_bid());
    println!("Best ask: {:?}", order_book.best_ask());
    println!("Spread: {:?}", order_book.spread());
}

// Get all order books
let all_books = streamer.get_all_order_books();
for (asset_id, book) in all_books {
    println!("{}: {}", asset_id, book.summary());
}
```

### Event Filtering and Processing

```rust
let mut events = streamer.events();
tokio::spawn(async move {
    while let Ok(event) = events.recv().await {
        match event {
            PolyEvent::PriceChange { asset_id, side, price, size, .. } => {
                // Process price changes
                handle_price_change(&asset_id, side, price, size).await;
            }
            PolyEvent::Trade { asset_id, price, size, .. } => {
                // Process trades
                handle_trade(&asset_id, price, size).await;
            }
            PolyEvent::TickSizeChange { asset_id, tick_size } => {
                // Process tick size changes
                handle_tick_size_change(&asset_id, tick_size).await;
            }
            _ => {}
        }
    }
});
```

## Error Handling

Comprehensive error handling with custom error types:

```rust
#[derive(Error, Debug)]
pub enum StreamerError {
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] crate::ws::client::WsError),
    #[error("Event parsing error: {0}")]
    EventParsing(#[from] EventError),
    #[error("State error: {0}")]
    State(#[from] StateError),
    #[error("Authentication error: {0}")]
    Auth(#[from] anyhow::Error),
}
```

Error recovery strategies:
- **Automatic Reconnection**: WebSocket clients automatically reconnect on disconnection
- **State Validation**: Order books are validated and cleaned when inconsistencies are detected
- **Fallback Mechanisms**: Hash validation can be bypassed when synchronization fails
- **Event Continuity**: Event broadcasting continues even if some consumers fail

## Performance Considerations

### Memory Management

- **Concurrent Data Structures**: Uses `DashMap` for thread-safe order book access
- **Event Buffering**: Configurable buffer sizes to balance memory usage and throughput
- **State Cleanup**: Automatic cleanup of crossed markets and invalid orders

### Concurrency

```rust
// Concurrent order book access
let order_books: Arc<DashMap<String, OrderBook>> = Arc::new(DashMap::new());

// Event broadcasting to multiple consumers
let (event_tx, event_rx) = broadcast::channel(config.event_buffer_size);

// Async task management
let market_task = tokio::spawn(async move {
    // Market data processing
});

let user_task = tokio::spawn(async move {
    // User data processing  
});
```

### Network Optimization

- **Connection Pooling**: Reused WebSocket connections
- **Selective Subscriptions**: Only subscribe to required asset feeds
- **Efficient Parsing**: Optimized message parsing and event creation

## Integration with TUI

The services module integrates seamlessly with the TUI for real-time display:

```rust
// In TUI application
let mut events = streamer.events();
let app_events = app.events();

loop {
    tokio::select! {
        // Handle streaming events
        Ok(stream_event) = events.recv() => {
            app.handle_stream_event(stream_event);
        }
        
        // Handle UI events
        Ok(ui_event) = app_events.recv() => {
            match ui_event {
                InputEvent::Key(key) => app.handle_key(key),
                InputEvent::Tick => app.update_display(),
            }
        }
    }
}
```

## Future Extensibility

The services architecture supports easy extension:

1. **Additional Data Sources**: New WebSocket providers can be integrated
2. **Custom Event Types**: The event system can be extended with new event types
3. **State Persistence**: Order book state can be persisted for crash recovery
4. **Metrics Collection**: Built-in hooks for performance monitoring

The services module provides a solid foundation for real-time trading operations while maintaining high performance and reliability standards required for production trading systems.