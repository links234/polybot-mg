# Terminal User Interface (TUI) Module

This module provides a comprehensive terminal-based user interface for real-time Polymarket order book visualization and interaction. Built with ratatui, it offers a responsive, keyboard-driven experience for monitoring live market data.

## Architecture Overview

The TUI module implements a state-driven architecture with clear separation of concerns:

```
┌─────────────────────────────────────────────────────────────┐
│                     Application State                       │
├─────────────────────────────────────────────────────────────┤
│  • Real-time event processing with panic protection        │
│  • Token activity tracking with concurrent access          │
│  • Order book state synchronization                        │
│  • User interaction state management                       │
└─────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│                      Event Handling                         │
├─────────────────────────────────────────────────────────────┤
│  • Asynchronous keyboard input processing                  │
│  • Real-time WebSocket event integration                   │
│  • Non-blocking UI updates with try_lock patterns          │
│  • Comprehensive error handling and recovery               │
└─────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│                      UI Rendering                           │
├─────────────────────────────────────────────────────────────┤
│  • Multi-view state management (Overview/OrderBook)        │
│  • Responsive layout with dynamic sizing                   │
│  • Real-time data visualization with color coding          │
│  • Interactive navigation and scrolling                    │
└─────────────────────────────────────────────────────────────┘
```

## Components

### 1. Application State (`app.rs`)

**Key Features:**
- **Multi-View State Management**: Seamless transitions between overview and detailed views
- **Real-Time Data Processing**: Concurrent event handling with panic protection
- **Interactive Navigation**: Keyboard-driven selection and scrolling
- **Performance Monitoring**: Comprehensive metrics tracking

**Core Implementation:**
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Overview,
    OrderBook { token_id: String },
}

pub struct App {
    pub state: AppState,
    pub token_activities: Arc<RwLock<HashMap<String, TokenActivity>>>,
    pub streamer: Arc<Streamer>,
    pub current_bids: Vec<(Decimal, Decimal)>,
    pub current_asks: Vec<(Decimal, Decimal)>,
    // ... additional fields
}
```

**CLAUDE.md Compliance:**
- ✅ Strong typing with custom enums for state management
- ✅ Impl methods on App struct for all functionality
- ✅ Comprehensive error handling with panic protection
- ✅ Non-blocking operations with try_read/try_write patterns

**Token Activity Tracking:**
```rust
#[derive(Debug, Clone)]
pub struct TokenActivity {
    pub token_id: String,
    pub event_count: usize,
    pub last_bid: Option<Decimal>,
    pub last_ask: Option<Decimal>,
    pub market_name: Option<String>,
    pub last_update: Option<Instant>,
    pub total_volume: Decimal,
    pub trade_count: usize,
}
```

**Key Methods:**
- `handle_event()`: Process incoming WebSocket events with error protection
- `get_top_active_tokens()`: Retrieve most active tokens sorted by activity
- `select_token()`: Navigate to detailed order book view
- `scroll_orderbook_up/down()`: Handle scrolling in order book view

### 2. Event Handling (`events.rs`)

**Key Features:**
- **Asynchronous Input Processing**: High-frequency polling for responsive input
- **Tick-Based Updates**: Regular UI refresh cycles
- **Error Resilience**: Comprehensive error handling and recovery
- **Resource Management**: Proper cleanup and task lifecycle management

**Core Implementation:**
```rust
#[derive(Debug, Clone)]
pub enum Event {
    Key(KeyEvent),
    Tick,
    Error(String),
}

pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<Event>,
    _task: tokio::task::JoinHandle<()>,
}
```

**CLAUDE.md Compliance:**
- ✅ Strong typing with custom Event enum
- ✅ Comprehensive error handling and logging
- ✅ Async-first design with proper resource management
- ✅ Debug logging for task lifecycle events

**Event Processing Strategy:**
1. **High-Frequency Input Polling**: 1ms polling interval for keyboard responsiveness
2. **Tick-Based Refresh**: Regular UI updates independent of input
3. **Error Isolation**: Failed event reads don't crash the application
4. **Graceful Shutdown**: Proper cleanup when event channel closes

### 3. UI Rendering (`ui.rs`)

**Key Features:**
- **Multi-View Architecture**: Dynamic view switching based on application state
- **Real-Time Data Visualization**: Live updates with appropriate color coding
- **Responsive Layout**: Adaptive sizing based on terminal dimensions
- **Interactive Elements**: Keyboard shortcuts and navigation indicators

**Core Layout Structure:**
```rust
pub fn draw(frame: &mut Frame<'_>, app: &mut App) {
    match app.state.clone() {
        AppState::Overview => draw_overview(frame, app),
        AppState::OrderBook { token_id } => draw_order_book(frame, app, &token_id),
    }
}
```

**Overview View Components:**
- **Title Bar**: Application branding and status
- **Event Log**: Recent WebSocket events with scrolling
- **Token Activity Table**: Top 15 most active tokens with metrics
- **Help Footer**: Keyboard shortcuts and navigation

**CLAUDE.md Compliance:**
- ✅ Structured layout with clear component separation
- ✅ Comprehensive data formatting and validation
- ✅ User-friendly error messages and status indicators
- ✅ Responsive design principles

## User Interface Design

### Overview Screen

```
┌─────────────────────────────────────────────────────────────┐
│                Polymarket WebSocket Stream                  │
├─────────────────────────────────────────────────────────────┤
│ Recent Events (scrolling)                                   │
│ • Asset123... UPDATE BID @ $0.5500 (100)                  │
│ • Asset456... TRADE BUY 50 @ $0.6200                      │
│ • Asset789... BOOK 25 bids, 18 asks                       │
├─────────────────────────────────────────────────────────────┤
│ # │ Token ID      │ Events │ Trades │ Bid    │ Ask    │ ... │
│ 1 │ Asset1...     │ 1,234  │ 56     │ $0.55  │ $0.56  │ ... │
│ 2 │ Asset2...     │ 987    │ 23     │ $0.72  │ $0.73  │ ... │
│ [Selected row highlighted]                                  │
├─────────────────────────────────────────────────────────────┤
│ ↑/↓: Select token | Enter: View order book | q: Quit      │
└─────────────────────────────────────────────────────────────┘
```

### Order Book Screen

```
┌─────────────────────────────────────────────────────────────┐
│                     Order Book View                         │
├─────────────────────────────────────────────────────────────┤
│ Token: Asset123... | Levels: 25 bids, 18 asks | Events: 1.2K│
├─────────────────────────────────────────────────────────────┤
│ Price Level │ Size   │ Total USD                            │
│ $0.5700     │ 150.00 │ $2,340.50    (RED - ASK)           │
│ $0.5650     │ 200.00 │ $1,810.00    (RED - ASK)           │
│ --- MID $0.5575 ---          (YELLOW - MID)                │
│ $0.5500     │ 180.00 │ $1,980.00    (GREEN - BID)         │
│ $0.5450     │ 220.00 │ $3,179.00    (GREEN - BID)         │
├─────────────────────────────────────────────────────────────┤
│ ↑/↓: Scroll | M: Reset to mid | Esc: Back | q: Quit       │
└─────────────────────────────────────────────────────────────┘
```

## State Management Patterns

### Thread-Safe Data Access
```rust
// Non-blocking read operations
if let Ok(activities) = self.token_activities.try_read() {
    // Process data safely
} else {
    // Handle lock contention gracefully
    Vec::new()
}
```

### Event Processing with Protection
```rust
// Panic protection for event formatting
match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    format_event(&event)
})) {
    Ok(log_entry) => self.event_log.push(log_entry),
    Err(_) => self.event_log.push("⚠️ Failed to format event".to_string()),
}
```

### State Synchronization
```rust
// Synchronize order book state with streamer
if let Some(order_book) = self.streamer.get_order_book(asset_id) {
    self.current_bids = order_book.get_bids().to_vec();
    self.current_asks = order_book.get_asks().to_vec();
}
```

## Real-Time Data Flow

```
WebSocket Events → App.handle_event() → State Updates → UI Rendering
      ↓                    ↓                 ↓              ↓
  PolyEvent         Token Activity     View State     Terminal Output
  variants          updates           changes         with colors
```

### Data Processing Pipeline

1. **Event Reception**: WebSocket events received from streamer
2. **State Updates**: Token activities and order book synchronization
3. **UI State Management**: View transitions and interaction handling
4. **Rendering**: Real-time visual updates with appropriate styling

### Performance Optimizations

- **Non-Blocking Operations**: All data access uses try_read/try_write
- **Efficient Sorting**: Token activities sorted by relevance
- **Memory Management**: Event log size limited to prevent memory leaks
- **Batched Updates**: UI updates coordinated with tick events

## Keyboard Navigation

### Global Controls
- `q`: Quit application
- `r`: Refresh/reset state
- `Esc`: Navigate back to previous view

### Overview View
- `↑/↓`: Navigate token selection
- `Enter`: View selected token's order book
- `PageUp/PageDown`: Fast navigation (when available)

### Order Book View
- `↑/↓`: Scroll through price levels
- `M`: Reset scroll to mid-price
- `Home/End`: Jump to top/bottom of order book

## Widget System

### Overview Table Widget
- **Dynamic Columns**: Adjusts to terminal width
- **Selection Highlighting**: Visual feedback for current selection
- **Data Formatting**: Appropriate precision for financial data
- **Status Indicators**: Connection health and data freshness

### Order Book Widget
- **Price-First Layout**: Price levels displayed prominently
- **Color Coding**: Green for bids, red for asks, yellow for mid
- **Cumulative Totals**: Running totals for market depth analysis
- **Scroll Indicators**: Visual feedback for current position

### Event Log Widget
- **Real-Time Updates**: Live event stream with automatic scrolling
- **Formatted Display**: Human-readable event descriptions
- **Size Management**: Automatic cleanup to prevent memory issues
- **Error Highlighting**: Visual indicators for processing errors

## Error Handling and Recovery

### UI Error Patterns
- **Graceful Degradation**: UI continues functioning with reduced data
- **Error Visualization**: Clear error messages for user awareness
- **State Recovery**: Automatic recovery from transient errors
- **Logging Strategy**: Comprehensive error logging for debugging

### Connection Health Monitoring
- **Real-Time Status**: Connection state displayed in UI
- **Reconnection Feedback**: Visual indicators during reconnection
- **Data Freshness**: Timestamps for last received data
- **Error Escalation**: Critical errors reported to user

## Integration Points

### With WebSocket Module
- **Event Stream**: Real-time event processing from WebSocket feeds
- **State Synchronization**: Order book state updates
- **Error Propagation**: Connection errors displayed in UI
- **Configuration**: WebSocket settings accessible through UI

### With Services Module
- **Background Processing**: Coordination with streaming services
- **Data Management**: Efficient data structures for UI consumption
- **Resource Sharing**: Shared state management across components
- **Lifecycle Management**: Proper startup and shutdown coordination

## Development Guidelines

### Adding New Views
1. Extend `AppState` enum with new variant
2. Add view-specific state fields to `App` struct
3. Implement rendering function in `ui.rs`
4. Add navigation logic in event handlers
5. Update keyboard shortcuts and help text

### Implementing New Widgets
1. Create widget module in `widgets/` directory
2. Define widget-specific data structures
3. Implement rendering with proper error handling
4. Add interaction handling if needed
5. Update main UI layout integration

### Performance Considerations
- **Minimize Allocations**: Reuse data structures where possible
- **Efficient Rendering**: Only update changed UI elements
- **Memory Management**: Implement proper cleanup for long-running data
- **Responsiveness**: Prioritize keyboard input over data processing

## Testing Strategy

### Unit Tests
- **State Management**: App state transitions and data integrity
- **Event Processing**: Event handling logic and error recovery
- **Data Formatting**: UI data presentation and validation
- **Navigation Logic**: Keyboard input handling and view transitions

### Integration Tests
- **End-to-End Flow**: Complete user interaction scenarios
- **Error Scenarios**: Recovery from various error conditions
- **Performance**: UI responsiveness under load
- **Data Accuracy**: Correct display of real-time data

### Manual Testing
- **Visual Verification**: Layout and styling across terminal sizes
- **Interaction Testing**: Keyboard navigation and responsiveness
- **Error Handling**: User experience during error conditions
- **Performance**: Smooth operation under high data volume

## Configuration and Customization

### UI Configuration
- **Color Schemes**: Customizable colors for different data types
- **Layout Options**: Adjustable column widths and display preferences
- **Update Intervals**: Configurable refresh rates
- **Display Precision**: Decimal places for financial data

### Keyboard Shortcuts
- **Customizable Bindings**: User-configurable key mappings
- **Context-Sensitive**: Different shortcuts for different views
- **Accessibility**: Alternative input methods for accessibility
- **Documentation**: Built-in help system for keyboard shortcuts

## Future Enhancements

### Advanced Features
1. **Multi-Pane Views**: Split-screen order book comparisons
2. **Historical Data**: Chart overlays and trend analysis
3. **Alert System**: Visual and audio notifications for price changes
4. **Export Functions**: Data export for external analysis
5. **Theming**: Multiple color themes and layout options

### User Experience Improvements
1. **Search Functionality**: Token search and filtering
2. **Bookmark System**: Save frequently viewed tokens
3. **Dashboard Customization**: User-configurable layouts
4. **Performance Metrics**: Real-time performance monitoring
5. **Help System**: Interactive tutorials and documentation

### Technical Enhancements
1. **Async Rendering**: Non-blocking UI updates
2. **Memory Optimization**: Efficient data structures for large datasets
3. **Network Optimization**: Intelligent data caching and prefetching
4. **Error Recovery**: Advanced error handling and recovery mechanisms
5. **Plugin System**: Extensible architecture for custom widgets

## Debugging and Troubleshooting

### Common Issues
1. **UI Lag**: Check event loop priorities and blocking operations
2. **Display Errors**: Verify terminal compatibility and sizing
3. **Data Inconsistencies**: Check WebSocket integration and state sync
4. **Memory Leaks**: Monitor data structure cleanup and lifecycle management

### Debugging Features
- **Debug Mode**: Enhanced logging and diagnostic information
- **Performance Profiling**: Built-in performance monitoring
- **State Inspection**: Runtime state examination tools
- **Error Reporting**: Comprehensive error logging and reporting

### Monitoring Points
- **UI Responsiveness**: Input lag and rendering performance
- **Memory Usage**: Data structure memory consumption
- **Event Processing**: Event handling rates and error frequencies
- **State Consistency**: Data synchronization accuracy