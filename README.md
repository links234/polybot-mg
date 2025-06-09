# Polybot - Rust CLI Trading Bot for Polymarket CLOB

A production-ready command-line tool for trading on Polymarket's Central Limit Order Book (CLOB) using Rust.

## Features

- 🔐 **Secure Authentication**: L1 (EIP-712) and L2 (API key) authentication flows
- 🔒 **Encrypted Storage**: AES-256-GCM encryption for API credentials
- 📊 **Market Data**: Browse active markets and view real-time orderbooks
- 💰 **Trading**: Place buy/sell limit orders and manage positions
- 🚀 **Fast & Efficient**: Built with async Rust for optimal performance
- 🧪 **Testnet Support**: Mumbai sandbox environment for testing
- 🔄 **Fetch All Markets**: Fetch and save all markets data with resumable state

## Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/polymarket_cli.git
cd polymarket_cli

# Build and install
cargo install --path .
```

## Quick Start

### 1. Initialize Authentication

First, you'll need your Polygon wallet's private key (without the 0x prefix):

```bash
# Initialize with your private key
polybot init --pk YOUR_PRIVATE_KEY_HEX

# You'll be prompted to create a passphrase for credential encryption
```

The tool will:
- Perform L1 authentication using EIP-712 signatures
- Derive L2 API credentials from Polymarket
- Encrypt and store credentials in `~/.config/polybot/creds.json.enc`

### 2. Browse Markets

```bash
# List all active markets
polybot markets

# Filter markets by keyword
polybot markets --filter "bitcoin"

# Limit number of results
polybot markets --limit 10
```

### 3. View Orderbooks

```bash
# Show orderbook for a specific token
polybot book TOKEN_ID

# Show more depth levels
polybot book TOKEN_ID --depth 10
```

### 4. Place Orders

```bash
# Place a buy order
polybot buy TOKEN_ID --price 0.48 --size 100 --yes

# Place a sell order
polybot sell TOKEN_ID --price 0.52 --size 100 --yes

# Note: --yes flag is required unless RUST_ENV=production
```

### 5. Manage Orders

```bash
# List all open orders
polybot orders

# List orders for specific token
polybot orders --token-id TOKEN_ID

# Cancel an order
polybot cancel ORDER_ID --yes
```

### 6. Fetch All Markets

```bash
# Fetch all markets with default settings (100MB chunks)
polybot fetch-all-markets --verbose

# Resume from previous state if interrupted
polybot fetch-all-markets --verbose

# Clear state and start fresh
polybot fetch-all-markets --clear-state --verbose

# Custom output directory and chunk size
polybot fetch-all-markets --output-dir my_markets --chunk-size-mb 50 --verbose

# Use Gamma API instead of CLOB API (different data structure, no auth required)
polybot fetch-all-markets --use-gamma --verbose
```

**Features:**
- **Resumable**: Automatically saves state and resumes from last position if interrupted
- **Chunked Storage**: Splits data into manageable chunks (default 100MB each)
- **No Page Limit**: Fetches ALL available markets (removed artificial limits)
- **Progress Tracking**: Shows detailed progress with `--verbose` flag
- **State Management**: Saves fetch state for resumption
- **Dual API Support**: Can use either CLOB API (authenticated) or Gamma API (public)

**API Differences:**
- **CLOB API**: Requires authentication, provides detailed market data including order book info
- **Gamma API**: Public API, provides market metadata, volumes, and categorization

**Output Structure:**
```
markets_data/
├── fetch_state.json          # CLOB API fetch state
├── gamma_fetch_state.json    # Gamma API fetch state
├── markets_chunk_0001.json   # CLOB API chunks
├── markets_chunk_0002.json   
├── gamma_markets_chunk_0001.json  # Gamma API chunks
└── ...                       
```

## Environment Variables

Create a `.env` file in the project root:

```bash
# Optional: Set passphrase to avoid prompts
POLYBOT_PASSPHRASE=your_secure_passphrase

# Optional: Private key for development
PK=your_private_key_hex

# Production mode (disables confirmation prompts)
RUST_ENV=production
```

## Sandbox/Testnet Mode

To use Mumbai testnet instead of mainnet:

```bash
# Use --sandbox flag with any command
polybot --sandbox init --pk YOUR_PRIVATE_KEY

# Or compile with sandbox feature
cargo build --features sandbox
```

## Security Considerations

- **Private Keys**: Never commit private keys to version control
- **Passphrases**: Use strong passphrases for credential encryption
- **Environment**: Be cautious when setting `RUST_ENV=production`
- **Credentials**: Encrypted credentials are stored in `~/.config/polybot/`

## API Rate Limits

The tool implements exponential backoff for rate limiting and will:
- Retry on 4xx/5xx errors with backoff
- Regenerate auth headers on 401 errors
- Handle network timeouts gracefully

## Development

### Running Tests

```bash
# Run all tests
cargo test

# Run with coverage
cargo tarpaulin --out Html

# Run integration tests
cargo test --test '*' -- --test-threads=1
```

### Building from Source

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run directly without installing
cargo run -- markets
```

## Architecture

```
src/
├── main.rs      # CLI entry point with clap commands
├── auth.rs      # L1/L2 authentication logic
├── config.rs    # Credential encryption/storage
├── markets.rs   # Market listing and orderbook display
└── orders.rs    # Order placement and management
```

## Troubleshooting

### "Failed to load credentials"
Run `polybot init` first to set up authentication.

### "Decryption failed"
Check your passphrase. You can reset by deleting `~/.config/polybot/creds.json.enc` and running init again.

### "Order confirmation required"
Add `--yes` flag or set `RUST_ENV=production`.

### Fetch Interrupted
The fetch-all-markets command automatically saves state. Simply run the command again to resume from where it left off. Use `--clear-state` to start fresh.

## Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Disclaimer

This software is provided as-is. Trading involves risk. Always verify orders before placement and use testnet for development. 