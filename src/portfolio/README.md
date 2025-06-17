# Portfolio Management Module

This module provides real-time portfolio and position tracking with WebSocket streaming updates from Polymarket's user channel.

## Components

### Types (`types.rs`)
- **Position**: Tracks individual positions with P&L calculations
- **ActiveOrder**: Represents open orders with real-time status
- **PortfolioStats**: Overall portfolio metrics and performance
- **PortfolioEvent**: WebSocket event types for portfolio updates

### Manager (`manager.rs`)
- **PortfolioManager**: Thread-safe portfolio state management
- Handles WebSocket events for orders, trades, and balances
- Calculates real-time P&L and portfolio statistics
- Groups positions by market for summary views

### Orders API (`orders_api.rs`)
- **PolymarketOrder**: Properly typed order structure matching Polymarket API
- **fetch_orders_authenticated**: Direct API implementation for fetching orders
- **build_auth_headers**: Authentication header construction for L2 API calls
- Works around limitations in polymarket-rs-client where OpenOrder type is opaque

## Features

- Real-time position tracking with unrealized P&L
- Active order management with status updates
- Trade execution history
- Portfolio performance metrics (win rate, average win/loss)
- Market-grouped position summaries
- Thread-safe concurrent access
- Direct API integration for order fetching with proper authentication

## Authentication Flow

1. **Address Derivation**: User's Ethereum address is derived from private key using `ethereum_utils`
2. **L2 Headers**: Authentication headers built using stored API credentials (api_key, secret, passphrase)
3. **Signature Generation**: HMAC-SHA256 signatures generated for each request
4. **API Calls**: Direct HTTP requests to Polymarket CLOB API with proper authentication

## Usage

```rust
use crate::portfolio::{PortfolioManager, PortfolioEvent};

// Create portfolio manager
let portfolio = PortfolioManager::new();

// Handle WebSocket events
portfolio.handle_event(event).await?;

// Get current positions
let positions = portfolio.get_positions().await;

// Get portfolio statistics
let stats = portfolio.get_stats().await;

// Fetch orders via API
let orders = fetch_orders_with_client(host, &data_paths, &args).await?;
```

## Known Limitations

- polymarket-rs-client's `OpenOrder` type doesn't expose fields or implement `Serialize`
- Direct API calls used as workaround for order fetching
- Market names need separate lookup from market IDs

## Future Improvements

- Cache market information for better display
- Add WebSocket support for real-time order updates
- Implement position aggregation across markets
- Better integration with polymarket-rs-client when it exposes order fields