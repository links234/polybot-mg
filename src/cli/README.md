# CLI Module

The CLI module provides the command-line interface for Polybot, a Rust trading bot for the Polymarket CLOB. It uses `clap` for argument parsing and provides a structured command pattern for all trading operations.

## Architecture

The CLI module follows a clear separation of concerns:

- **Main CLI struct** (`Cli`): Root CLI parser with global options
- **Commands enum**: Enumeration of all available subcommands
- **Command pattern**: Each command has its own Args struct and Command struct
- **Args parsing**: Utility functions for parsing and validating arguments

## Key Components

### `Cli` Struct

The main CLI struct defines global options available to all commands:

```rust
pub struct Cli {
    pub command: Commands,
    pub sandbox: bool,           // Use Mumbai testnet
    pub data_dir: PathBuf,       // Data directory (default: ./data)
    pub verbose: u8,             // Verbosity level
}
```

### `Commands` Enum

All available subcommands are defined in the `Commands` enum:

- `Init` - Initialize authentication credentials
- `Markets` - Browse and search markets
- `FetchAllMarkets` - Fetch all markets to JSON
- `Analyze` - Analyze fetched market data
- `Enrich` - Add real-time data to markets
- `Book` - Display orderbook for a token
- `Buy/Sell` - Place trading orders
- `Cancel` - Cancel existing orders
- `Orders` - List open orders
- `Stream` - Real-time WebSocket streaming
- `Daemon` - Long-running daemon with strategy
- `Pipeline` - Execute workflow scripts
- `Datasets` - Manage data and pipeline outputs

### Command Pattern

Each command follows a consistent pattern:

```rust
// Args struct with clap derive
#[derive(Args)]
pub struct CommandArgs {
    // Command-specific arguments
}

// Command struct with business logic
pub struct Command {
    args: CommandArgs,
}

impl Command {
    pub fn new(args: CommandArgs) -> Self { ... }
    pub async fn execute(&self, host: &str, data_paths: DataPaths) -> Result<()> { ... }
}
```

## Global Options

### Environment Selection
- `--sandbox`: Switch to Mumbai testnet (default: mainnet)
- Host URLs are automatically selected based on environment

### Data Management
- `--data-dir <PATH>`: Custom data directory (default: `./data`)
- All commands use the same data directory structure via `DataPaths`

### Logging & Verbosity
- `-v, --verbose`: Increase verbosity (can be repeated: `-vv`, `-vvv`)
- Logging is configured per command based on requirements

## Command Execution Flow

1. **Parse CLI**: `clap` parses command line arguments into `Cli` struct
2. **Setup Environment**: Determine host URL and create data paths
3. **Execute Command**: Match on command type and execute appropriate handler
4. **Error Handling**: Consistent error propagation using `anyhow::Result`

```rust
impl Cli {
    pub async fn execute(self) -> Result<()> {
        let host = self.get_host();
        let data_paths = DataPaths::new(&self.data_dir);
        data_paths.ensure_directories()?;
        
        match self.command {
            Commands::Init(args) => InitCommand::new(args).execute(host, data_paths).await,
            Commands::Markets(args) => MarketsCommand::new(args).execute(host, data_paths).await,
            // ... other commands
        }
    }
}
```

## Integration Points

### Authentication
- Commands requiring API access use `crate::auth::get_authenticated_client()`
- Credentials are managed through the `Init` command and stored securely

### Data Storage
- All commands use the same `DataPaths` structure for consistent file organization
- Markets data, logs, and pipeline outputs are stored in organized directories

### Error Handling
- All commands return `anyhow::Result<()>` for consistent error handling
- Rich error messages with context for better user experience

### Logging
- Comprehensive logging using `tracing` crate
- Command-specific logging configurations (console, file, or both)
- Different log levels for user feedback vs debugging

## Usage Patterns

### Basic Commands
```bash
polybot init --pk <private_key>
polybot markets --mode search "election"
polybot buy <token_id> --price 0.65 --size 100 --yes
```

### Environment Selection
```bash
polybot --sandbox markets         # Use testnet
polybot --data-dir ./custom markets  # Custom data directory
```

### Advanced Features
```bash
polybot stream --assets token1,token2 --tui          # Real-time TUI
polybot daemon --assets token1,token2               # Long-running daemon
polybot pipeline analysis --param market=election   # Workflow execution
```

## Strong Typing

The CLI module adheres to strong typing principles:

- No tuples in public APIs - all data structures use named fields
- Custom types for domain concepts (prices, sizes, token IDs)
- Comprehensive validation at argument parsing level
- Type-safe enum variants for command modes and options

## Error Recovery

Commands implement graceful error handling:

- Network errors: Automatic retries with exponential backoff
- Authentication errors: Clear messages directing to `init` command
- Validation errors: Specific field-level error messages
- File system errors: Path-specific error context

## Extension Points

The CLI module is designed for easy extension:

- New commands: Add to `Commands` enum and implement the command pattern
- New arguments: Extend existing Args structs or add global options
- New validation: Add custom `value_parser` functions like `parse_percentage`
- New output formats: Commands can support multiple output modes