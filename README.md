# Polybot - Rust CLI Trading Bot for Polymarket CLOB

A production-ready command-line tool for trading on Polymarket's Central Limit Order Book (CLOB) using Rust.

## Quick Start

```bash
# Initialize authentication
polybot init --pk YOUR_PRIVATE_KEY_HEX

# Browse markets
polybot markets

# View orderbook
polybot book TOKEN_ID

# Stream real-time data
polybot stream

# Run interactive pipeline system
polybot pipeline
```

## Core Features

- 🔐 **Secure Authentication**: L1 (EIP-712) and L2 (API key) authentication flows
- 📊 **Real-time Streaming**: WebSocket-based live market data with TUI and GUI interfaces
- 🖥️ **Enhanced UI**: Both terminal (TUI) and graphical (GUI) interfaces with egui
- 💰 **Trading Operations**: Place buy/sell orders and manage positions
- 🔄 **Market Analysis**: Comprehensive data fetching and enrichment pipelines
- 📈 **WebSocket Portfolio**: Real-time portfolio updates via WebSocket events
- 🧪 **Testnet Support**: Mumbai sandbox environment for testing
- 🚀 **High Performance**: Async Rust with optimal performance

## Architecture

See module-specific READMEs for detailed documentation:

- [`src/cli/`](src/cli/README.md) - Command-line interface and argument parsing
- [`src/ws/`](src/ws/README.md) - WebSocket client and real-time data streaming
- [`src/tui/`](src/tui/README.md) - Terminal user interface components
- [`src/gui/`](src/gui/README.md) - Graphical user interface with egui
- [`src/markets/`](src/markets/README.md) - Market data fetching and management
- [`src/services/`](src/services/README.md) - Background services and streaming
- [`src/pipeline/`](src/pipeline/README.md) - Workflow automation system
- [`src/datasets/`](src/datasets/README.md) - Data management and analysis
- [`src/portfolio/`](src/portfolio/README.md) - Portfolio management and tracking
- [`src/execution/`](src/execution/README.md) - Order execution engine
- [`src/auth.rs`](src/) - Authentication and credential management
- [`src/config.rs`](src/) - Configuration and encryption utilities

## Installation

```bash
# Clone and build
git clone https://github.com/yourusername/polymarket_cli.git
cd polymarket_cli
cargo install --path .
```

## Environment Variables

```bash
# Optional: Set passphrase to avoid prompts
POLYBOT_PASSPHRASE=your_secure_passphrase

# Optional: Private key for development
PK=your_private_key_hex

# Production mode (disables confirmation prompts)
RUST_ENV=production
```

## Documentation

📚 **[Complete Documentation](./docs/README.md)** - Comprehensive guides and references

- **[Features](./docs/features/)** - Core functionality and feature documentation
- **[Architecture](./docs/architecture/)** - System design and data structures  
- **[Development](./docs/development/)** - Development guides and testing
- **[Troubleshooting](./docs/troubleshooting/)** - Common issues and solutions

## Development

```bash
# Run tests
cargo test

# Build release
cargo build --release

# Run with specific command
cargo run -- stream --tokens TOKEN_ID
```

