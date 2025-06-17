# CLI Commands

This directory contains all command implementations for the Polybot CLI. Each command follows a consistent pattern with dedicated Args and Command structs, providing type-safe argument parsing and organized business logic.

## Command Architecture

### Standard Pattern

All commands follow this structure:

```rust
#[derive(Args, Clone)]
pub struct CommandArgs {
    // Command-specific arguments with clap annotations
}

pub struct Command {
    args: CommandArgs,
}

impl Command {
    pub fn new(args: CommandArgs) -> Self {
        Self { args }
    }
    
    pub async fn execute(&self, host: &str, data_paths: DataPaths) -> Result<()> {
        // Command implementation
    }
}
```

## Available Commands

### Authentication & Setup

#### `init` - Initialize Authentication
- **Purpose**: Set up API credentials and authentication
- **Arguments**: 
  - `--pk <private_key>`: Private key in hex format
  - `--nonce <nonce>`: Nonce for key derivation (default: 0)
- **Usage**: `polybot init --pk <private_key>`
- **Integration**: Saves credentials for use by other commands

### Market Discovery

#### `markets` - Browse Markets
- **Purpose**: Search, filter, and discover Polymarket markets
- **Modes**:
  - `list`: Basic market listing (default)
  - `volume`: Markets sorted by trading volume
  - `active`: Markets with active orderbook data
  - `search`: Keyword-based market search
  - `details`: Detailed market information
  - `url`: Extract market from Polymarket URL
- **Key Arguments**:
  - `--mode <mode>`: Operation mode
  - `--limit <n>`: Maximum results (default: 20)
  - `--detailed`: Show comprehensive information
  - `--min-volume <amount>`: Filter by minimum volume
  - `--min-price/--max-price`: Price range filters (0-100)
  - `--min-spread/--max-spread`: Spread filters for active markets
- **Usage**: 
  ```bash
  polybot markets --mode search "election"
  polybot markets --mode volume --limit 10 --detailed
  polybot markets <market_id> --mode details
  ```

#### `fetch_all_markets` - Bulk Market Fetching
- **Purpose**: Download all available markets to JSON file
- **Arguments**: Standard data output to configured directory
- **Usage**: `polybot fetch_all_markets`
- **Integration**: Provides data for analysis and pipeline commands

#### `analyze` - Market Analysis
- **Purpose**: Analyze fetched market data with filters and rankings
- **Arguments**: Analysis criteria and output preferences
- **Usage**: `polybot analyze`
- **Integration**: Works with data from `fetch_all_markets`

#### `enrich` - Market Enrichment
- **Purpose**: Add real-time data to existing market information
- **Arguments**: Target markets and data sources
- **Usage**: `polybot enrich`
- **Integration**: Enhances market data with live pricing and volume

### Trading Operations

#### `book` - Orderbook Display
- **Purpose**: Display current orderbook for a specific token
- **Arguments**:
  - `<token_id>`: Target token identifier
- **Usage**: `polybot book <token_id>`
- **Integration**: Shows live bid/ask data from CLOB API

#### `buy` - Place Buy Orders
- **Purpose**: Place buy orders on the market
- **Arguments**:
  - `<token_id>`: Target token
  - `--price <price>`: Price in USDC (e.g., 0.48)
  - `--size <size>`: Order size in USDC
  - `--yes`: Confirmation flag (required in non-production)
- **Usage**: `polybot buy <token_id> --price 0.65 --size 100 --yes`
- **Safety**: Requires explicit confirmation to prevent accidental orders

#### `sell` - Place Sell Orders
- **Purpose**: Place sell orders on the market
- **Arguments**: Same as buy command with sell-specific logic
- **Usage**: `polybot sell <token_id> --price 0.75 --size 50 --yes`
- **Safety**: Same confirmation requirements as buy orders

#### `cancel` - Cancel Orders
- **Purpose**: Cancel existing open orders
- **Arguments**: Order identification and cancellation criteria
- **Usage**: `polybot cancel <order_id>`
- **Integration**: Works with order management system

#### `orders` - Order Management
- **Purpose**: List and manage open orders
- **Arguments**: Filtering and display options
- **Usage**: `polybot orders`
- **Integration**: Shows orders from authenticated user account

### Real-time Data & Streaming

#### `stream` - WebSocket Streaming
- **Purpose**: Real-time market data streaming with TUI or CLI interface
- **Key Features**:
  - **TUI Mode**: Interactive terminal interface (default)
  - **CLI Mode**: Command-line streaming output
  - **Asset Loading**: From direct args or markets JSON file
  - **Authentication**: Optional user feed authentication
- **Arguments**:
  - `--assets <ids>`: Comma-separated asset IDs to stream
  - `--markets-path <path>`: Load assets from markets JSON file
  - `--markets <ids>`: User markets for authenticated feed
  - `--api-key/--secret/--passphrase`: Authentication credentials
  - `--tui/--no-tui`: Enable/disable TUI interface
  - `--show-book/--show-trades/--show-user`: Event filtering
  - `--summary-interval <seconds>`: Periodic orderbook summaries
- **Usage**:
  ```bash
  polybot stream --assets token1,token2 --tui
  polybot stream --markets-path ./data/markets.json --show-trades
  ```
- **Integration**: Uses WebSocket services and TUI components

#### `daemon` - Streaming Daemon
- **Purpose**: Long-running WebSocket streaming with sample trading strategy
- **Key Features**:
  - **Strategy Integration**: Built-in sample strategy execution
  - **Event Analysis**: Spread analysis and liquidity monitoring
  - **Continuous Operation**: Designed for long-running execution
  - **Graceful Shutdown**: Handles Ctrl+C and cleanup
- **Arguments**:
  - `--assets <ids>`: Required asset IDs for monitoring
  - `--markets <ids>`: Optional user markets
  - `--api-key/--secret/--passphrase`: Authentication for user feed
  - `--heartbeat-interval <seconds>`: WebSocket heartbeat (default: 10)
  - `--summary-interval <seconds>`: Strategy analysis interval (default: 30)
- **Usage**: `polybot daemon --assets token1,token2 --summary-interval 60`
- **Integration**: Combines streaming with strategy execution

### Workflow & Automation

#### `pipeline` - Workflow Execution
- **Purpose**: Execute YAML-defined workflow pipelines
- **Key Features**:
  - **Interactive TUI**: Pipeline selection interface
  - **YAML Configuration**: Workflow definitions in `pipelines/` directory
  - **Parameter Passing**: Custom parameters via command line
  - **Dry Run Mode**: Preview execution without running commands
- **Arguments**:
  - `[name]`: Pipeline name (optional, launches TUI if not provided)
  - `--pipelines-dir <dir>`: Pipeline directory (default: "pipelines")
  - `--param <key=value>`: Custom parameters
  - `--dry-run`: Preview mode
  - `--list`: List available pipelines
- **Usage**:
  ```bash
  polybot pipeline                    # Interactive TUI
  polybot pipeline analysis           # Run specific pipeline
  polybot pipeline --list             # List all pipelines
  polybot pipeline analysis --param market=election --dry-run
  ```
- **Integration**: Executes workflows defined in pipeline module

#### `datasets` - Dataset Management
- **Purpose**: Manage datasets and pipeline outputs
- **Arguments**: Dataset operations and filtering
- **Usage**: `polybot datasets`
- **Integration**: Works with pipeline outputs and data management

### Data Management

#### `index` - Database Indexing
- **Purpose**: Index raw market data into RocksDB for fast queries with parallel processing
- **Key Features**:
  - **Parallel File Processing**: Process multiple files simultaneously using Rayon
  - **Multi-threaded Parsing**: Parse markets within files using thread pool
  - **Batched Database Writes**: Efficient batch operations to minimize I/O
  - **Interactive TUI**: File selection and real-time progress tracking
  - **Thread Control**: Specify thread count or use auto-detection
- **Arguments**:
  - `--rocksdb`: Use RocksDB storage (TypedDbContext with column families)
  - `--chunk-files <files>`: Comma-separated list of specific files to index
  - `--source-dir <dir>`: Directory containing market JSON chunks
  - `--clear`: Clear existing database before indexing
  - `--batch-size <n>`: Batch size for RocksDB writes (default: 1000)
  - `--threads <n>`: Number of parallel threads (0 = auto-detect)
  - `--skip-duplicates`: Skip duplicate markets (default: true)
  - `--detailed`: Show detailed progress information
- **Usage**:
  ```bash
  polybot index --rocksdb                    # Interactive TUI with auto parallelism
  polybot index --rocksdb --threads 8        # Use 8 threads
  polybot index --rocksdb --clear            # Clear and rebuild database
  polybot index --rocksdb --batch-size 2000  # Larger batches for faster writes
  ```
- **Performance**:
  - Saturates CPU with parallel market parsing
  - Maximizes disk I/O with batched writes
  - Typically 3-5x faster than single-threaded
- **Integration**: Provides indexed data for fast market queries

### Development & Testing

#### `tui_test` - TUI Testing
- **Purpose**: Development and testing of TUI components
- **Usage**: `polybot tui_test`
- **Integration**: Standalone TUI testing environment

## Command Integration Patterns

### Authentication Flow
1. User runs `polybot init --pk <private_key>`
2. Credentials stored in data directory
3. Other commands automatically load credentials via `get_authenticated_client()`

### Data Flow
1. `fetch_all_markets` downloads raw market data
2. `analyze` processes and filters the data
3. `enrich` adds real-time information
4. `pipeline` can orchestrate the entire workflow

### Trading Flow
1. `markets` to discover trading opportunities
2. `book` to check current orderbook
3. `buy`/`sell` to place orders
4. `orders` to monitor positions
5. `cancel` to manage orders

### Real-time Monitoring
1. `stream` for interactive market monitoring
2. `daemon` for automated strategy execution
3. Both support authentication for user-specific data

## Error Handling

### Validation Errors
- Argument validation at parse time
- Custom validators for prices, percentages, and identifiers
- Clear error messages with usage hints

### Runtime Errors
- Network connectivity issues with retry logic
- Authentication failures with helpful guidance
- File system errors with path context

### Safety Mechanisms
- Confirmation requirements for trading commands
- Sandbox mode for testing
- Dry-run capabilities for pipelines

## Extension Guidelines

### Adding New Commands
1. Create `command_name.rs` in this directory
2. Define `CommandArgs` struct with `#[derive(Args)]`
3. Implement `Command` struct with `execute` method
4. Add to `Commands` enum in `../mod.rs`
5. Add import and match case in main CLI handler

### Command Best Practices
- Use strong typing for all arguments
- Implement comprehensive error handling
- Provide clear progress feedback
- Support both interactive and scriptable usage
- Include comprehensive documentation

### Integration Points
- Use `DataPaths` for consistent file organization
- Leverage `auth` module for API authentication
- Utilize `logging` for structured output
- Follow established patterns for async execution