# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

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
- **No tuples ever** - always use named structs - unless iterator operations or other likewise exceptions
- **Comprehensive error handling** with custom error types
- **Method organization** - functionality belongs on the struct it operates on
- **Logging at every important step** for debugging and monitoring
- **Type-driven development** - let the type system prevent bugs

### Warning Handling Rules

- **For Traits**: NEVER use underscores. Either remove/delete them or comment them out if valuable for future use
- **For Error Enums**: NEVER use underscores. Remove unused variants to keep match clauses clean and meaningful
- **For Struct Fields**: Prefix with underscore if unused
- **For Methods/Functions**: Prefix with underscore if unused
- **Zero Warnings Policy**: Code must compile with absolutely zero warnings

### Anti-Patterns to Avoid

- ❌ `Vec<(Decimal, Decimal)>` → ✅ `Vec<PriceLevel>` with named fields
- ❌ Standalone functions → ✅ `impl` methods on relevant structs
- ❌ Raw string types → ✅ Strong types like `TokenId`, `MarketId`
- ❌ Bash scripts → ✅ Cargo commands and CLI tools
- ❌ Large monolithic files → ✅ Well-organized, focused modules
- ❌ Modifying Cargo.toml versions → ✅ Leave version management to project maintainer

## Project Overview

Polybot is a Rust-based terminal trading bot for Polymarket's Central Limit Order Book (CLOB). It provides real-time WebSocket streaming, order management, market analysis, and workflow automation through a comprehensive command-line interface.

## Architecture

### Core Components

- **WebSocket Client** (`src/ws/`): Handles real-time data streaming from Polymarket's CLOB API
- **TUI Interface** (`src/tui/`): Terminal-based user interface using ratatui
- **CLI Commands** (`src/cli/commands/`): Command-line interface with 15+ commands
- **Execution Engine** (`src/execution/`): Unified framework for real-time, replay, and simulation
- **Pipeline System** (`src/pipeline/`): YAML-based workflow automation
- **Market Data** (`src/markets/`): Market fetching, analysis, and storage
- **Services** (`src/services/`): Background services for data streaming and processing

### Key Technologies

- **Rust**: Primary language with async/await throughout
- **ratatui**: Terminal UI framework
- **tokio**: Async runtime with full features
- **WebSocket**: Real-time data streaming via tokio-tungstenite
- **rust_decimal**: Precise decimal arithmetic for financial data
- **blake3**: Fast cryptographic hashing for orderbook verification

## Development Commands

### Building and Running

```bash
# Development - run any command
cargo run -- [command]

# Common commands
cargo run -- stream                     # Run streaming TUI
cargo run -- stream --tokens <token_id> # Stream specific tokens
cargo run -- tui-test                   # Test TUI without real data
cargo run -- markets list               # List markets
cargo run -- pipeline run <name>        # Run a pipeline

# Build for release
cargo build --release

# Install globally
cargo install --path .
```

### Code Quality

```bash
# Check code (run before every commit)
cargo check
cargo clippy  # MUST have zero warnings
cargo test

# Format code
cargo fmt
```

### Testing

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run with output
cargo test -- --nocapture

# Test TUI interface
cargo run -- tui-test
```

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
- **Execution Framework**: Unified real-time/replay/simulation engine

### Service Layer

- **Background Services**: Non-blocking data processing
- **Error Resilience**: Automatic recovery and reconnection strategies
- **State Management**: Thread-safe data synchronization
- **Metrics Collection**: Performance and usage tracking

## Configuration and Data

### Directory Structure

```
./data/                  # Default data directory
./data/auth/            # Encrypted credentials
./data/datasets/        # Market data storage
./data/pipelines/       # Pipeline configurations
./data/runs/           # Pipeline execution outputs
./data/logs/           # Application logs
```

### Environment Variables

- `POLYBOT_PASSPHRASE` - Avoid credential prompts
- `PK` - Private key for development
- `RUST_ENV` - Production mode flag

### WebSocket Endpoints

- Market feed: `wss://ws-subscriptions-clob.polymarket.com/ws/market`
- User feed: `wss://ws-subscriptions-clob.polymarket.com/ws/user`
- Heartbeat monitoring with 10s intervals
- Automatic reconnection with exponential backoff

## Development Practices

### CLI Commands Overview

- `init` - Initialize authentication
- `markets` - Market data operations
- `stream` - Real-time TUI streaming
- `buy`/`sell` - Order placement
- `orders` - Order management
- `cancel` - Cancel orders
- `balance` - Check balances
- `pipeline` - Workflow automation
- `datasets` - Data management
- `fetch-all-markets` - Bulk data fetching
- `tui-test` - UI testing mode

### Error Handling Patterns

- Custom error types using `thiserror`
- Comprehensive error context with suggestions
- Recovery mechanisms (auto-reconnection, state rollback)
- User-friendly error messages with actionable feedback

### Performance Considerations

- Non-blocking UI updates with 1ms polling
- Efficient data structures (BTreeMap for orderbooks)
- Memory-conscious event log management
- Blake3 hashing for fast orderbook verification

### Testing Strategy

- Unit tests in module files (using `#[cfg(test)]`)
- Integration testing via CLI commands
- TUI test mode for interface validation
- WebSocket simulation for offline development
