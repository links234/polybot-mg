# TUI Widgets Module

This module contains specialized user interface widgets for displaying complex financial data in the terminal. Each widget is designed for optimal performance and user experience when visualizing real-time market data.

## Architecture Overview

The widgets module implements a component-based architecture with reusable, specialized widgets:

```
┌─────────────────────────────────────────────────────────────┐
│                    Widget Components                        │
├─────────────────────────────────────────────────────────────┤
│  • Order Book Widget: Real-time price level visualization  │
│  • Price Display: Precision financial data formatting      │
│  • Market Depth: Cumulative volume analysis               │
│  • Status Indicators: Connection and data health          │
└─────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│                    Data Processing                          │
├─────────────────────────────────────────────────────────────┤
│  • Real-time data transformation for display               │
│  • Efficient sorting and filtering algorithms              │
│  • Price level aggregation and calculation                 │
│  • Color coding and visual styling logic                   │
└─────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│                   Rendering Pipeline                        │
├─────────────────────────────────────────────────────────────┤
│  • Layout calculation and responsive design                │
│  • Scroll management and viewport optimization             │
│  • Interactive element highlighting                        │
│  • Error state visualization                               │
└─────────────────────────────────────────────────────────────┘
```

## Components

### 1. Order Book Widget (`order_book.rs`)

**Key Features:**
- **Unified Price Level Display**: Combined bid/ask visualization with mid-price indication
- **Cumulative Volume Analysis**: Running totals for market depth analysis
- **Interactive Scrolling**: Smooth navigation with mid-price anchoring
- **Error State Handling**: Clear visualization of crossed markets and data issues

**Core Data Structure:**
```rust
#[derive(Debug, Clone)]
struct OrderBookLevel {
    price: Decimal,
    bid_size: Option<Decimal>,
    ask_size: Option<Decimal>,
    is_mid: bool,
    cumulative_bid_total: Option<Decimal>,
    cumulative_ask_total: Option<Decimal>,
}
```

**CLAUDE.md Compliance:**
- ✅ Strong typing with custom structs instead of tuples
- ✅ Comprehensive error handling and validation
- ✅ Extensive logging for debugging
- ✅ Impl pattern functions organized logically

### Layout Design

The order book widget uses a sophisticated layout that combines bids and asks in a single, coherent view:

```
Price Level    │ Size     │ Total USD
$0.5700       │ 150.00   │ $2,340.50    (ASK - RED)
$0.5650       │ 200.00   │ $1,810.00    (ASK - RED)
$0.5625       │ 180.00   │ $1,350.00    (ASK - RED)
--- MID $0.5575 ---                     (MID - YELLOW)
$0.5500       │ 180.00   │ $1,980.00    (BID - GREEN)
$0.5450       │ 220.00   │ $3,179.00    (BID - GREEN)
$0.5400       │ 165.00   │ $4,069.00    (BID - GREEN)
```

### Key Features

#### 1. Price-First Design
- **Primary Column**: Price levels displayed prominently as first column
- **Precision Formatting**: Appropriate decimal places for price display
- **Sort Order**: Prices sorted from highest to lowest (natural market order)
- **Mid-Price Indication**: Clear visual separator between bids and asks

#### 2. Cumulative Volume Analysis
- **Running Totals**: USD amounts to clear all levels to that point
- **Market Depth**: Understanding of liquidity at each level
- **Impact Analysis**: Cost estimation for large orders
- **Visual Hierarchy**: Clear distinction between size and total columns

#### 3. Interactive Navigation
- **Scroll Management**: Smooth scrolling through price levels
- **Mid-Price Anchoring**: Automatic centering on mid-price with 'M' key
- **Viewport Optimization**: Efficient rendering of visible levels only
- **Status Indicators**: Scroll position and total level count display

#### 4. Error State Visualization
- **Crossed Market Detection**: Clear warning when bid >= ask
- **Data Validation**: Visual indicators for invalid or stale data
- **Connection Status**: Display of data freshness and connection health
- **Error Recovery**: Graceful handling of temporary data issues

## Implementation Details

### Data Preparation Pipeline

```rust
fn prepare_order_book_levels(app: &App) -> Vec<OrderBookLevel> {
    // 1. Process ask levels (above mid)
    // 2. Insert mid-price indicator
    // 3. Process bid levels (below mid)
    // 4. Calculate cumulative totals
    // 5. Sort by price (highest to lowest)
}
```

### Cumulative Calculation Strategy

```rust
fn calculate_cumulative_totals(levels: &mut [OrderBookLevel]) {
    // Ask totals: calculated from lowest ask upward
    // Bid totals: calculated from highest bid downward
    // Running totals represent USD needed to clear to that level
}
```

### Error Handling Patterns

#### Crossed Market Handling
```rust
if bid >= ask {
    // Display prominent error message
    Row::new(vec![
        "⚠️ CROSSED MARKET ERROR ⚠️".to_string(),
        "BID >= ASK".to_string(), 
        "INVALID SPREAD".to_string(),
    ]).style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
}
```

#### Data Validation
- **Empty Order Books**: Graceful handling with informative messages
- **Missing Data**: Fallback displays for incomplete information
- **Precision Handling**: Consistent decimal formatting across all values
- **Size Validation**: Proper handling of zero-size levels

### Performance Optimizations

#### Efficient Rendering
- **Viewport Calculation**: Only render visible price levels
- **Scroll Optimization**: Intelligent scroll bounds to prevent excessive navigation
- **Memory Management**: Efficient data structure usage for large order books
- **Update Batching**: Coordinate updates with UI refresh cycles

#### Data Processing
- **Lazy Evaluation**: Calculate cumulative totals only when needed
- **Efficient Sorting**: Optimized sort algorithms for price level ordering
- **Memory Pooling**: Reuse data structures where possible
- **Cache-Friendly Access**: Organize data for optimal memory access patterns

## Color Coding and Visual Design

### Color Scheme
- **Green (Bids)**: Buy orders indicating demand
- **Red (Asks)**: Sell orders indicating supply  
- **Yellow (Mid)**: Mid-price and summary information
- **Gray**: Secondary information and status indicators
- **White**: Primary text and important data

### Visual Hierarchy
1. **Price Levels**: Most prominent, first column
2. **Size Information**: Secondary but important
3. **Cumulative Totals**: Analytical information
4. **Status/Meta Information**: Least prominent

### Accessibility Considerations
- **High Contrast**: Clear distinction between different data types
- **Consistent Styling**: Predictable visual patterns
- **Text-Based**: Compatible with screen readers and text-only terminals
- **Size Flexibility**: Adapts to different terminal dimensions

## Integration with Application State

### Data Synchronization
```rust
// Real-time synchronization with WebSocket data
if let Some(order_book) = self.streamer.get_order_book(&token_id) {
    self.current_bids = order_book.get_bids().to_vec();
    self.current_asks = order_book.get_asks().to_vec();
}
```

### State Management
- **View State**: Scroll position and display preferences
- **Data State**: Current order book snapshot
- **UI State**: Selection and interaction state
- **Error State**: Validation and error information

## Keyboard Interaction

### Navigation Controls
- `↑/↓`: Scroll through price levels
- `M`: Reset scroll position to mid-price
- `Home/End`: Jump to top/bottom of order book
- `PageUp/PageDown`: Fast scrolling through levels

### Interactive Features
- **Smooth Scrolling**: Responsive to user input
- **Mid-Price Centering**: Automatic positioning for optimal view
- **Bounds Checking**: Prevents scrolling beyond available data
- **Visual Feedback**: Clear indication of current scroll position

## Error Handling and Recovery

### Data Validation
- **Price Consistency**: Ensure proper bid/ask ordering
- **Size Validation**: Handle zero and negative sizes appropriately
- **Hash Verification**: Display hash mismatch warnings
- **Timestamp Checks**: Indicate stale or outdated data

### User Feedback
- **Error Messages**: Clear, actionable error information
- **Status Indicators**: Real-time connection and data health
- **Recovery Instructions**: Guidance for resolving issues
- **Graceful Degradation**: Continued functionality with limited data

## Development Guidelines

### Adding New Widget Features
1. **Data Structure Design**: Define clear, typed data structures
2. **Rendering Logic**: Implement efficient, responsive rendering
3. **Interaction Handling**: Add appropriate keyboard/mouse support
4. **Error Handling**: Include comprehensive error states
5. **Testing**: Unit and integration tests for new features

### Performance Considerations
- **Rendering Efficiency**: Minimize unnecessary redraws
- **Memory Usage**: Efficient data structure utilization
- **Scroll Performance**: Smooth navigation even with large datasets
- **Update Coordination**: Batch updates for optimal performance

### Code Organization
- **Separation of Concerns**: Clear division between data processing and rendering
- **Reusable Components**: Modular design for component reuse
- **Type Safety**: Strong typing throughout the widget hierarchy
- **Documentation**: Comprehensive inline documentation

## Testing Strategy

### Unit Tests
- **Data Processing**: Order book level preparation and calculation
- **Rendering Logic**: Layout calculation and formatting
- **Error Handling**: Various error condition responses
- **Scroll Management**: Navigation and bounds checking

### Integration Tests
- **Real Data**: Testing with actual market data streams
- **User Interaction**: Complete user workflow scenarios
- **Error Recovery**: Response to various error conditions
- **Performance**: Rendering performance under load

### Visual Testing
- **Layout Verification**: Correct display across terminal sizes
- **Color Schemes**: Appropriate visual styling
- **Error States**: Clear error message display
- **Responsiveness**: Smooth interaction and navigation

## Future Enhancements

### Advanced Features
1. **Price Level Highlighting**: Highlight significant price levels
2. **Volume Profiling**: Visual indicators for high-volume levels
3. **Historical Overlays**: Previous price level comparisons
4. **Advanced Filtering**: User-configurable display filters
5. **Export Functions**: Order book data export capabilities

### User Experience
1. **Customizable Layouts**: User-configurable column ordering
2. **Precision Settings**: Adjustable decimal place display
3. **Color Themes**: Multiple color scheme options
4. **Accessibility**: Enhanced accessibility features
5. **Keyboard Shortcuts**: Extended keyboard navigation

### Technical Improvements
1. **Performance Optimization**: Further rendering performance improvements
2. **Memory Efficiency**: Optimized data structure usage
3. **Animation Support**: Smooth transitions and visual effects
4. **Plugin Architecture**: Extensible widget system
5. **Configuration System**: Persistent user preferences

## Debugging and Troubleshooting

### Common Issues
1. **Layout Problems**: Incorrect column widths or spacing
2. **Data Display**: Inconsistent number formatting or precision
3. **Scroll Issues**: Navigation problems or bounds errors
4. **Performance**: Slow rendering or high memory usage

### Debugging Tools
- **Debug Rendering**: Enhanced logging for rendering pipeline
- **Data Inspection**: Runtime data structure examination
- **Performance Profiling**: Built-in performance monitoring
- **Visual Debugging**: Layout and styling diagnostic tools

### Monitoring Points
- **Rendering Performance**: Frame rates and update latency
- **Memory Usage**: Widget memory consumption patterns  
- **User Interaction**: Input responsiveness and feedback
- **Data Accuracy**: Correct display of financial information