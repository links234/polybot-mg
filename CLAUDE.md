# Polybot - Polymarket WebSocket Trading Bot

## STRICT DEVELOPMENT RULES

### Core Principles (MUST FOLLOW)
1. **No Bash Commands**: Always prefer CLI setup and Rust tooling over bash scripts
2. **Strong Typing**: Replace tuples with meaningful structs - no `(Type, Type)` patterns
3. **Type Safety**: Always prefer strongly typed programming, use enums and structs over primitives
4. **Rust Only**: This is a Rust-only project - no mixing with other languages
5. **Documentation**: Always write and update READMEs for each component (every folder should have README.md, referenced in mod.rs if present)
6. **README Content Policy**: Never add Security, Disclaimer, or similar sections to README.md unless explicitly requested by user
7. **File Size Limits**: Keep files below 700-1000 lines (1000 in extreme cases, most <700 lines)
8. **Idiomatic Rust**: Use `impl` methods on `struct`s directly instead of standalone functions
9. **Comprehensive Logging**: Write good debug, info, warn and error logs (written to file) for debugging reference
10. **Never Modify Cargo.toml Versions**: Never change version numbers in Cargo.toml unless explicitly requested by user

### Code Quality Standards
- **ABSOLUTE ZERO WARNINGS POLICY** - ALL code MUST compile with zero warnings. Fix every single warning immediately!
- **No tuples in public APIs** - always use named structs
- **Comprehensive error handling** with custom error types  
- **Method organization** - functionality belongs on the struct it operates on
- **Logging at every important step** for debugging and monitoring
- **Type-driven development** - let the type system prevent bugs

### Anti-Patterns to Avoid
- ❌ `Vec<(Decimal, Decimal)>` → ✅ `Vec<PriceLevel>` with named fields
- ❌ Standalone functions → ✅ `impl` methods on relevant structs
- ❌ Raw string types → ✅ Strong types like `TokenId`, `MarketId`
- ❌ Bash scripts → ✅ Cargo commands and CLI tools
- ❌ Large monolithic files → ✅ Well-organized, focused modules
- ❌ Modifying Cargo.toml versions → ✅ Leave version management to project maintainer

## Project Overview
Polybot is a Rust-based terminal user interface (TUI) application that connects to Polymarket's WebSocket API to stream real-time order book data and provide trading functionality. The application displays live market data in an interactive terminal interface.

## Architecture

### Core Components
- **WebSocket Client** (`src/ws/`): Handles real-time data streaming from Polymarket's CLOB API
- **TUI Interface** (`src/tui/`): Terminal-based user interface using ratatui
- **CLI Commands** (`src/cli/commands/`): Command-line interface and main application logic
- **Market Data** (`src/markets/`): Market fetching, analysis, and storage
- **Services** (`src/services/`): Background services for data streaming and processing

### Key Technologies
- **Rust**: Primary language
- **ratatui**: Terminal UI framework
- **tokio**: Async runtime
- **WebSocket**: Real-time data streaming
- **rust_decimal**: Precise decimal arithmetic for financial data

## Development Workflow

### Testing Commands
```bash
# Run the streaming TUI
cargo run -- stream

# Run with specific tokens
cargo run -- stream --tokens <token_id>

# Test TUI without real data
cargo run -- tui-test
```

### Code Quality
- Run `cargo check` and `cargo clippy` before commits
- **ABSOLUTE ZERO WARNINGS POLICY**: Fix EVERY warning immediately. No exceptions!
- **Remove unused code**: Delete all unused functions, imports, fields, and types immediately
- **Never use #[allow(dead_code)] or #[allow(unused_imports)]** unless explicitly requested by user
- **Fix warnings before proceeding**: STOP everything and fix warnings first
- Test UI responsiveness and data accuracy

## Common Issues and Solutions

### WebSocket Event Parsing
**Problem**: Events may have either "type" or "event_type" fields
**Solution**: Check both field names in parsing logic

### UI Responsiveness
**Problem**: Keyboard input lag due to WebSocket event processing
**Solution**: 
- Prioritize keyboard events in event loop
- Use 1ms polling for input detection
- Avoid blocking operations in UI thread

### Crossed Markets
**Problem**: Invalid orderbooks where bid >= ask
**Solution**: 
- Implement validation in `validate_and_clean()` method
- Display clear error messages in TUI
- Auto-remove invalid price levels

### Real-time Updates
**Problem**: Orderbook not updating with incoming events
**Solution**:
- Ensure event parsing matches Polymarket's format
- Verify orderbook state management in `src/ws/state.rs`
- Check WebSocket connection stability

## System Architecture

### Pipeline System
- **YAML-based workflow definitions** with parameter templating
- **Auto-detection** of Cargo vs binary execution for development/production
- **Parameter substitution** with built-in date/time variables
- **Error recovery** and continuation strategies
- **Step orchestration** with environment variable support

### Event-Driven Architecture
- **WebSocket streaming** with automatic reconnection
- **State synchronization** between real-time data and UI
- **Panic protection** in event processing loops
- **Non-blocking operations** with try_read/try_write patterns
- **Event counting** and metrics collection

### Data Management
- **Strong typing** eliminates tuple usage in public APIs
- **Validation pipelines** for market data integrity
- **Automatic cleanup** of invalid/stale data
- **Hash verification** for order book consistency
- **Thread-safe** concurrent data structures

## Module Organization

### Core Systems
- **Authentication**: L1/L2 API credential management with encryption
- **WebSocket Client**: Real-time market data streaming with reconnection
- **TUI Interface**: Terminal-based user interface with responsive design
- **Pipeline Engine**: Workflow automation and command orchestration
- **Dataset Management**: Data discovery, validation, and health monitoring

### Service Layer
- **Background Services**: Non-blocking data processing
- **Error Resilience**: Automatic recovery and reconnection strategies
- **State Management**: Thread-safe data synchronization
- **Metrics Collection**: Performance and usage tracking

## Data Processing Pipelines

### Validation Systems
- **Market Data Validation**: Crossed market detection and cleanup
- **Price Level Validation**: Automatic removal of invalid entries
- **Hash Verification**: Order book integrity checking
- **Real-time Monitoring**: Connection health and data flow tracking

### Processing Patterns
- **Event-driven Updates**: Real-time data synchronization
- **Batch Operations**: Efficient bulk data processing
- **Error Recovery**: Graceful handling of malformed data
- **State Reconciliation**: Automatic correction of inconsistencies

## Interface Design Principles

### Responsiveness
- **Non-blocking UI**: Prioritize user input processing
- **Real-time Updates**: Immediate data synchronization
- **Progressive Loading**: Chunked data processing
- **Error Feedback**: Clear validation and error messaging

### Data Presentation
- **Financial Data Priority**: Price information displayed first
- **Comprehensive Metrics**: Event counts, timing, and volume data
- **Visual Indicators**: Color coding and status symbols
- **Contextual Information**: Market conditions and spread analysis

## Operational Patterns

### Development Workflow
- **Zero Warnings Policy**: All code must compile without warnings
- **Comprehensive Logging**: Structured logging at debug, info, warn, error levels
- **Testing Strategy**: Unit tests, integration tests, and TUI testing modes
- **Documentation**: README files for every module with architectural context

### Production Considerations
- **Configuration Management**: Environment-based settings and credential encryption
- **Performance Monitoring**: Real-time metrics and resource utilization tracking
- **Error Recovery**: Automatic reconnection and state restoration
- **Data Persistence**: State management and resumable operations

## Integration Architecture

### External APIs
- **Polymarket CLOB**: WebSocket streaming and REST API integration
- **Authentication Flow**: L1/L2 credential management and API key handling
- **Rate Limiting**: Respect API constraints with exponential backoff
- **Data Validation**: Hash verification and consistency checking

### Internal Systems
- **Pipeline Orchestration**: YAML-defined workflows with parameter substitution
- **Dataset Management**: Automatic discovery and health monitoring
- **State Synchronization**: Thread-safe data sharing between components
- **UI Framework**: Terminal-based interface with responsive design patterns

