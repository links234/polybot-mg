# WebSocket Streaming Module

This module provides real-time streaming connectivity to Polymarket's CLOB (Central Limit Order Book) WebSocket API, enabling live order book updates, price changes, and trade data.

## Architecture Overview

The WebSocket module follows a layered architecture with strong typing and comprehensive error handling:

```
┌─────────────────────────────────────────────────────────────┐
│                      WS Client Layer                        │
├─────────────────────────────────────────────────────────────┤
│  • Auto-reconnection with exponential backoff             │
│  • Heartbeat monitoring and connection health              │
│  • Command processing (subscribe/unsubscribe)              │
│  • Concurrent message handling with broadcast channels     │
└─────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│                      Event Processing                       │
├─────────────────────────────────────────────────────────────┤
│  • Typed event parsing with comprehensive error handling   │
│  • Multi-format message support (arrays and single events) │
│  • Strong typing with custom deserializers                 │
│  • Event transformation to unified PolyEvent format        │
└─────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│                     State Management                        │
├─────────────────────────────────────────────────────────────┤
│  • Order book state with Blake3 hash verification          │
│  • Atomic updates with validation                          │
│  • Crossed market detection and cleanup                    │
│  • Comprehensive market data calculations                  │
└─────────────────────────────────────────────────────────────┘
```

## Components

### 1. WebSocket Client (`client.rs`)

**Key Features:**
- **Dual Connection Support**: Separate clients for market feed and user feed
- **Auto-Reconnection**: Exponential backoff with configurable retry limits
- **Heartbeat Management**: Ping/pong monitoring with timeout detection
- **Concurrent Processing**: Non-blocking message handling with tokio channels

**Core Implementation:**
```rust
pub struct WsClient {
    command_tx: mpsc::UnboundedSender<WsCommand>,
    message_rx: broadcast::Receiver<WsMessage>,
}

pub enum WsCommand {
    SubscribeMarket(Vec<String>),
    SubscribeUser(Vec<String>, AuthPayload),
    Disconnect,
}
```

**CLAUDE.md Compliance:**
- ✅ Strong typing with custom enums instead of primitives
- ✅ Comprehensive error handling with custom `WsError` type
- ✅ Extensive logging at debug, info, warn, and error levels
- ✅ Impl methods on structs for functionality organization

**Usage Example:**
```rust
use crate::ws::{WsClient, WsConfig};

// Create market feed client
let config = WsConfig::default();
let client = WsClient::new_market(config).await?;

// Subscribe to specific assets
client.subscribe_market(vec!["asset_id_1".to_string()])?;

// Receive messages
let mut messages = client.messages();
while let Ok(msg) = messages.recv().await {
    // Process message
}
```

### 2. Event Models (`events.rs`)

**Key Features:**
- **Unified Event System**: All WebSocket events transformed to `PolyEvent` enum
- **Type-Safe Deserialization**: Custom deserializers for decimal strings and complex structures
- **Multiple Format Support**: Handles both array-based and single event messages
- **Trading Primitives**: Strong types for `Side`, `OrderStatus`, etc.

**Core Types:**
```rust
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum PolyEvent {
    Book { asset_id: String, bids: Vec<(Decimal, Decimal)>, asks: Vec<(Decimal, Decimal)>, hash: String },
    PriceChange { asset_id: String, side: Side, price: Decimal, size: Decimal, hash: String },
    Trade { asset_id: String, price: Decimal, size: Decimal, side: Side, timestamp: u64 },
    // ... more event types
}
```

**CLAUDE.md Compliance:**
- ✅ No tuples in public APIs - considering migration to named structs
- ✅ Strong typing with custom enums (`Side`, `OrderStatus`)
- ✅ Comprehensive error handling with `EventError` type
- ✅ Type-driven development with extensive validation

**Event Processing Flow:**
1. Raw JSON message received from WebSocket
2. Parsed into `WsMessage` envelope
3. Event type determined and specific parsing applied
4. Validated and transformed to `PolyEvent`
5. Forwarded to application layer

### 3. State Management (`state.rs`)

**Key Features:**
- **Hash-Verified Order Books**: Blake3 hashing for state integrity
- **Atomic Updates**: All-or-nothing state changes with validation
- **Market Analysis**: Comprehensive price calculations and metrics
- **Data Validation**: Crossed market detection and cleanup

**Core Implementation:**
```rust
#[derive(Debug, Clone)]
pub struct OrderBook {
    pub asset_id: String,
    pub bids: BTreeMap<Decimal, Decimal>,
    pub asks: BTreeMap<Decimal, Decimal>,
    pub last_hash: Option<String>,
    pub tick_size: Option<Decimal>,
}

impl OrderBook {
    pub fn replace_with_snapshot(&mut self, bids: Vec<(Decimal, Decimal)>, asks: Vec<(Decimal, Decimal)>, hash: String) -> Result<(), StateError>
    pub fn apply_price_change(&mut self, side: Side, price: Decimal, size: Decimal, expected_hash: String) -> Result<(), StateError>
    pub fn validate_and_clean(&mut self) -> bool
    // ... more methods
}
```

**CLAUDE.md Compliance:**
- ✅ Impl methods on structs for all functionality
- ✅ Strong error handling with custom `StateError` type
- ✅ Comprehensive logging for debugging
- ✅ Type safety with validated operations

**State Management Features:**
- **Integrity Verification**: Blake3 hash validation for all updates
- **Atomic Operations**: Rollback on validation failure
- **Market Analytics**: Spread, mid-price, depth calculations
- **Error Recovery**: Graceful handling of corrupted state

## Real-Time Data Flow

```
WebSocket Stream → Message Parsing → Event Validation → State Updates → UI Updates
     ↓                    ↓                ↓                ↓             ↓
Raw JSON         WsMessage      PolyEvent       OrderBook     TUI Render
                 envelope       variants        updates       updates
```

### Data Integrity Guarantees

1. **Hash Verification**: Every order book update verified with Blake3
2. **Atomic Updates**: Failed validations result in no state change
3. **Crossed Market Detection**: Invalid spreads automatically cleaned
4. **Connection Resilience**: Auto-reconnection with exponential backoff

### Performance Characteristics

- **Non-Blocking**: All operations use async/await patterns
- **Memory Efficient**: BTreeMap storage for O(log n) operations
- **Concurrent**: Separate tasks for connection management and message processing
- **Scalable**: Broadcast channels for multiple consumers

## Error Handling Strategy

### Error Types
- `WsError`: Connection and communication errors
- `EventError`: Message parsing and validation errors
- `StateError`: Order book state inconsistencies

### Recovery Mechanisms
1. **Connection Errors**: Automatic reconnection with backoff
2. **Parse Errors**: Log and continue with next message
3. **State Errors**: Rollback to previous valid state
4. **Hash Mismatches**: Warning logs with state preservation

## Configuration

### WebSocket Configuration
```rust
#[derive(Debug, Clone)]
pub struct WsConfig {
    pub market_url: String,           // Market feed WebSocket URL
    pub user_url: String,             // User feed WebSocket URL
    pub heartbeat_interval: u64,      // Ping interval in seconds
    pub max_reconnection_attempts: u32, // 0 = infinite
    pub initial_reconnection_delay: u64, // Initial delay in ms
    pub max_reconnection_delay: u64,    // Max delay in ms
}
```

### Default Configuration
- Market URL: `wss://ws-subscriptions-clob.polymarket.com/ws/market`
- User URL: `wss://ws-subscriptions-clob.polymarket.com/ws/user`
- Heartbeat: 10 seconds
- Reconnection: Infinite attempts with 1s to 30s exponential backoff

## Integration Points

### With TUI Module
- Event forwarding via broadcast channels
- State synchronization for order book display
- Real-time metrics for UI updates

### With Services Module
- Background streaming coordination
- Connection health monitoring
- Event aggregation and processing

## Testing

### Unit Tests
- Order book state validation
- Event parsing accuracy
- Hash verification logic
- Connection configuration

### Integration Tests
- WebSocket connection establishment
- Message flow end-to-end
- Error recovery scenarios
- Performance under load

## Development Guidelines

### Adding New Event Types
1. Define event struct in `events.rs`
2. Add variant to `PolyEvent` enum
3. Implement parsing in `parse_message` function
4. Add validation logic if needed
5. Update state management if applicable

### Extending Order Book Functionality
1. Add methods to `OrderBook` impl block
2. Include comprehensive error handling
3. Add logging for debugging
4. Validate state consistency
5. Include unit tests

### Connection Management
- Always use auto-reconnection patterns
- Implement proper cleanup on disconnect
- Log connection state changes
- Handle authentication for user feeds

## Future Enhancements

1. **Multi-Asset Streaming**: Optimize for multiple concurrent subscriptions
2. **Advanced Analytics**: Real-time market depth and liquidity metrics
3. **Event Filtering**: Client-side filtering for specific event types
4. **Compression**: Message compression for high-frequency updates
5. **Circuit Breakers**: Advanced error recovery mechanisms

## Performance Tuning

### Memory Management
- Use `BTreeMap` for sorted order book storage
- Implement proper cleanup for stale data
- Monitor memory usage under high load

### CPU Optimization
- Minimize allocations in hot paths
- Use efficient serialization/deserialization
- Batch operations where possible

### Network Optimization
- Implement message batching
- Use compression when available
- Monitor connection quality metrics

## Debugging

### Common Issues
1. **Hash Mismatches**: Check event ordering and data integrity
2. **Connection Drops**: Verify network stability and heartbeat timing
3. **Parse Errors**: Examine raw message format changes
4. **State Inconsistencies**: Review order book update logic

### Logging Strategy
- `DEBUG`: Detailed message processing
- `INFO`: Connection state changes
- `WARN`: Recoverable errors and inconsistencies
- `ERROR`: Critical failures requiring attention

### Monitoring Points
- Connection uptime and reconnection frequency
- Message processing rate and latency
- Hash verification success rate
- Error rates by type and severity