# Features Documentation

This section contains documentation for all major features and capabilities of Polybot.

## 🌊 Streaming & Real-time Data

### [Stream Improvements](./stream-improvements.md)
Comprehensive documentation of WebSocket streaming enhancements, including:
- Real-time orderbook updates
- Event processing optimizations  
- Connection stability improvements
- Performance metrics and monitoring

### [WebSocket Portfolio Migration](./websocket-portfolio-migration.md)
Migration from HTTP-based portfolio service to WebSocket-only streaming:
- Real-time portfolio updates via WebSocket events
- Elimination of HTTP request overhead
- Consistent with ratatui implementation
- Event-driven architecture benefits

## 🖥️ User Interface

### [TUI Implementation](./tui-implementation.md)
Terminal User Interface features and design:
- Interactive terminal interface
- Real-time data visualization
- Keyboard navigation and controls
- UI responsiveness optimizations

### [GUI Fixes Summary](./gui-fixes-summary.md)
Recent GUI improvements and fixes:
- Border rendering aesthetics
- Focus existing windows behavior
- Smart auto-arrange with clustering
- Layout management enhancements

### [Dataset Selector](./dataset-selector.md)
Unified interface for data source selection:
- Two-panel design (selections vs datasets)
- Interactive navigation
- Smart token extraction
- Terminal environment detection

## 📊 Data Analysis

### [Analyze Command Filters](./analyze-command-filters.md)
Market analysis and filtering capabilities:
- Advanced market filtering
- Data processing pipelines
- Analysis result formatting
- Export and visualization options

## 🤖 Trading Strategies

### [Strategy Enhancement Summary](./strategy-enhancement-summary.md)
Automated trading strategy improvements:
- Token ID auto-detection from prefixes
- Automated order placement with 10-second timer
- User confirmation system for orders
- Real-time order status display
- Progressive discount pricing logic

### [Run Strategy Guide](./run-strategy-guide.md)
Comprehensive guide for running trading strategies:
- Strategy execution commands
- Configuration options
- Error handling and recovery
- Performance monitoring

---

[← Back to Documentation Index](../README.md)