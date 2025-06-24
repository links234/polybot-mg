# Execution Module

The execution module provides a unified framework for streaming and processing market data in real-time, replay, and simulation modes. It orchestrates data sources, event processing strategies, and maintains orderbook state across different execution environments.

## Core Components

### `ExecutionEngine` (`engine.rs`)
Main orchestrator that coordinates all execution activities:
- Manages data source lifecycle (start/stop)
- Coordinates strategy execution and event processing
- Maintains orderbook state and metrics collection
- Provides unified event broadcasting to external consumers

### `ExecutionConfig` (`config.rs`)
Strongly-typed configuration for execution modes:
- **RealTime**: Live WebSocket streaming with asset filtering
- **Replay**: Historical data playback with speed control
- **Simulation**: Synthetic data generation for testing

### Event System (`events.rs`)
Unified event model supporting all execution modes:
- `ExecutionEvent`: Wrapper for all event types with metadata
- `MarketEvent`: Orderbook snapshots, price changes, trades
- `UserEvent`: Order updates, user trades, balance changes
- `SystemEvent`: Execution lifecycle, connection status, errors
- `MetricsEvent`: Performance and health statistics

### Data Sources (`sources.rs`)
Abstraction layer for different data sources:
- `WebSocketDataSource`: Real-time streaming from Polymarket CLOB
- `ReplayDataSource`: Historical data playback from files
- `SimulationDataSource`: Synthetic event generation

### Strategy Framework (`strategies.rs`)
Extensible strategy system for event processing:
- `MarketAnalysisStrategy`: Spread and liquidity analysis with alerts
- `LoggingStrategy`: Comprehensive event logging
- Configurable thresholds, filters, and output options

## Architecture

```
ExecutionEngine
├── DataSource (WebSocket/Replay/Simulation)
│   └── EventStream → ExecutionEvent
├── Strategies []
│   ├── MarketAnalysisStrategy
│   └── LoggingStrategy
├── OrderBook State (HashMap<AssetId, OrderBook>)
└── Event Broadcasting (broadcast::Sender)
```

## Usage Examples

### Real-time Market Streaming
```rust
use polybot::execution::*;

let config = ExecutionConfig::real_time(
    vec![AssetId::from("0x123")],
    true // enable user feed
);

let mut engine = ExecutionEngine::new(config);
engine.add_default_strategies();

engine.start().await?;
let mut event_stream = engine.event_stream();

while let Ok(event) = event_stream.recv().await {
    match event.data {
        EventData::Market(market_event) => {
            // Process market data
        }
        _ => {}
    }
}
```

### Historical Data Replay
```rust
let config = ExecutionConfig::replay(
    PathBuf::from("./data/2024-01-15"),
    Some(vec![AssetId::from("0x123")]), // filter assets
    2.0 // 2x speed
);

let mut engine = ExecutionEngine::new(config);
engine.add_strategy(Box::new(MarketAnalysisStrategy::new(
    StrategyConfig::market_analysis()
)));

engine.start().await?;
```

### Simulation Mode
```rust
let config = ExecutionConfig::simulation(
    5,   // 5 assets
    Duration::from_millis(100) // event every 100ms
);

let mut engine = ExecutionEngine::new(config);
engine.start().await?;
```

## Key Features

- **Unified Interface**: Same API for real-time, replay, and simulation modes
- **Strong Typing**: No tuples in public APIs, comprehensive error handling
- **Event-Driven**: Async/await with tokio, non-blocking event processing
- **Extensible**: Strategy pattern for custom event processing logic
- **Observable**: Comprehensive metrics and health monitoring
- **Thread-Safe**: Arc<RwLock<T>> for concurrent orderbook access

## Integration Points

- **CLI Commands**: Stream, replay, and analysis commands use execution engine
- **TUI**: Terminal interface subscribes to execution event stream
- **WebSocket**: Existing WsClient integrates via WebSocketDataSource
- **OrderBook**: Maintains compatibility with existing orderbook structures
- **Logging**: Structured logging with tracing crate throughout

## Configuration

All execution modes support:
- Asset filtering (specific tokens vs all available)
- Output configuration (TUI, console, file logging)
- Strategy configuration (thresholds, parameters)
- Performance tuning (buffer sizes, timeouts)

## Error Handling

- `ExecutionError`: Engine lifecycle and configuration errors
- `DataSourceError`: Connection and data source specific errors
- `StrategyError`: Strategy initialization and processing errors
- `StreamError`: Event stream and parsing errors

All errors implement `thiserror::Error` for comprehensive error context and chaining.