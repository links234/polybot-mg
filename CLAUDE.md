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
6. **Documentation Structure**: Follow the established docs/ hierarchy - see Documentation Organization section below
7. **README Content Policy**: Never add Security, Disclaimer, or similar sections to README.md unless explicitly requested by user
8. **File Size Limits**: Keep files below 700-1000 lines (1000 in extreme cases, most <700 lines)
9. **Idiomatic Rust**: Use `impl` methods on `struct`s directly instead of standalone functions
10. **Comprehensive Logging**: Write good debug, info, warn and error logs (written to file) for debugging reference
11. **Never Modify Cargo.toml Versions**: Never change version numbers in Cargo.toml unless explicitly requested by user

### Code Quality Standards

- **ABSOLUTE ZERO WARNINGS POLICY** - ALL code MUST compile with zero warnings. Fix every single warning immediately!
- **No tuples ever** - always use named structs - unless iterator operations or other likewise exceptions
- **Comprehensive error handling** with custom error types
- **Method organization** - functionality belongs on the struct it operates on
- **Logging at every important step** for debugging and monitoring
- **Type-driven development** - let the type system prevent bugs
- **Test Organization**: NEVER put tests in the same file as source code. Tests MUST be in separate `test/` directory with same folder path as file being tested (e.g., `src/markets/fetcher.rs` → `test/markets/fetcher.rs`). This prevents context pollution and improves code retrieval.

### Warning Handling Rules

- **NEVER USE UNDERSCORES FOR**: Structs, Enums, Enum Variants, or Traits - ALWAYS REMOVE/DELETE THEM
- **For Struct Fields**: Prefix with underscore ONLY if genuinely needed for future use
- **For Function/Method Parameters**: Underscore prefix is acceptable
- **For Local Variables**: Underscore prefix is acceptable
- **Zero Warnings Policy**: Code must compile with absolutely zero warnings
- **Unused Code**: REMOVE unused code instead of prefixing with underscore
- **Temporary Files**: DELETE all .sh and test files after task completion

### Helper Workflows

#### Preparing for Main Branch Merge
1. **Check for warnings**: `cargo check` - MUST have zero warnings
2. **Check git status**: `git status` - review all changes
3. **Check git diff**: `git diff` - verify all changes are intentional
4. **Remove temporary files**: Delete all .sh, test scripts, and temporary files
5. **Remove unused code**: Delete (don't prefix) unused enums, structs, traits, and methods
6. **Consolidate documentation**: Move CAPSLOCK .md content into CLAUDE.md
7. **Run tests**: `cargo test` - ensure all tests pass

#### Common Warning Fixes
- **Unused enum variant**: DELETE the variant (don't prefix with underscore)
- **Unused struct**: DELETE the struct definition
- **Unused method**: DELETE the method (unless genuinely needed soon)
- **Unused imports**: Remove with `cargo fix` or manually
- **Dead code**: Remove the code entirely

#### Git Workflow Best Practices
- Always check `git diff` before making large changes
- Consolidate related changes into logical commits
- Use descriptive commit messages
- Never commit with warnings present

### Naming Conventions

- **Structs**: Use PascalCase, NEVER underscores → `OrderBook`, `MarketData`, `PriceLevel`
- **Enums**: Use PascalCase, NEVER underscores → `OrderStatus`, `EventType`, `MarketState`
- **Functions**: Use snake_case but avoid underscores in names when possible → `calculate_spread()`, `get_price()`
- **Methods**: Use snake_case but avoid underscores in names when possible → `update_orderbook()`, `validate_order()`
- **Variables**: Use snake_case → `market_id`, `order_book`, `price_level`
- **Constants**: Use SCREAMING_SNAKE_CASE → `MAX_ORDER_SIZE`, `DEFAULT_TIMEOUT`
- **Modules**: Use snake_case → `order_management`, `market_data`, `websocket_client`
- **Underscores ONLY for**: Function parameters, struct fields, and local variables when following Rust conventions
- **Avoid underscores in**: Type names (structs, enums, traits), function names, method names unless absolutely necessary

### Anti-Patterns to Avoid

- ❌ `Vec<(Decimal, Decimal)>` → ✅ `Vec<PriceLevel>` with named fields
- ❌ Standalone functions → ✅ `impl` methods on relevant structs
- ❌ Raw string types → ✅ Strong types like `TokenId`, `MarketId`
- ❌ Bash scripts → ✅ Cargo commands and CLI tools
- ❌ Large monolithic files → ✅ Well-organized, focused modules
- ❌ Modifying Cargo.toml versions → ✅ Leave version management to project maintainer

## Documentation Organization

### Structured Documentation Hierarchy

All documentation MUST follow the established `docs/` directory structure:

```
docs/
├── README.md                           # Main documentation index
├── architecture/
│   ├── README.md                      # Architecture overview
│   └── data-structure.md              # Data structures and storage
├── development/
│   ├── README.md                      # Development guide
│   ├── testing-stream-tui.md          # TUI testing procedures
│   └── tui-test-instructions.md       # Detailed TUI test instructions
├── features/
│   ├── README.md                      # Features overview
│   ├── analyze-command-filters.md     # Market analysis filters
│   ├── dataset-selector.md            # Dataset selection system
│   ├── stream-improvements.md         # Streaming enhancements
│   └── tui-implementation.md          # TUI implementation details
└── troubleshooting/
    ├── README.md                      # Troubleshooting guide
    └── stream-error-fixes.md          # Stream-specific error fixes
```

### Documentation Creation Rules

1. **NEVER create documentation outside the `docs/` hierarchy**
2. **Always place new docs in the appropriate subdirectory**:
   - `docs/architecture/` - System design, data structures, technical architecture
   - `docs/development/` - Development guides, setup, testing procedures
   - `docs/features/` - Feature specifications, implementation details
   - `docs/troubleshooting/` - Error resolution, debugging guides
3. **Each subdirectory MUST have a `README.md` that serves as an index**
4. **Use descriptive filenames with hyphens**: `feature-name.md`, not `feature_name.md`
5. **Only create documentation when explicitly requested by user**
6. **Update relevant `README.md` indices when adding new docs**

### Documentation Writing Guidelines

- **Concise and actionable** - focus on implementation details
- **Technical depth** - assume reader has development knowledge
- **Code examples** - include relevant Rust code snippets
- **Clear structure** - use consistent markdown formatting
- **Cross-references** - link to related documentation when relevant

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

- **Unit tests in separate `test/` directory** with same folder structure as `src/` (e.g., `src/markets/fetcher.rs` → `test/markets/fetcher.rs`)
- **Integration testing** via CLI commands and dedicated integration test files
- **TUI test mode** for interface validation
- **WebSocket simulation** for offline development
- **Test file organization** prevents context pollution and improves code retrieval

## Git Worktree Management

This section explains how to use the built-in `worktree` command for managing multiple feature branches with automatic data and environment setup.

### What are Git Worktrees?

Git worktrees allow you to have multiple working directories for the same repository, each checked out to different branches. This is perfect for:

- Working on multiple features simultaneously
- Testing different branches without losing work
- Comparing implementations side-by-side
- Running different experiments with separate data

### Quick Start

#### List Current Worktrees
```bash
cargo run -- worktree list
```

This shows all worktrees with data directory sizes, environment files, and credential status.

#### Create a New Worktree
```bash
# Create a new feature branch worktree
cargo run -- worktree create my-feature

# Create from a specific base branch
cargo run -- worktree create my-feature --base develop

# Create with custom path
cargo run -- worktree create my-feature --path /path/to/custom/location

# Create without copying data (faster)
cargo run -- worktree create my-feature --no-data
```

#### Sync Data Between Worktrees
```bash
# Sync everything from main worktree
cargo run -- worktree sync --what all

# Sync only data directory
cargo run -- worktree sync --what data

# Sync only credentials
cargo run -- worktree sync --what creds

# Sync from specific worktree
cargo run -- worktree sync --source ../polybot-main --what all
```

#### Remove a Worktree
```bash
# Remove cleanly (checks for uncommitted changes)
cargo run -- worktree remove ../polybot-my-feature

# Force removal (ignores dirty state)
cargo run -- worktree remove ../polybot-my-feature --force
```

### What Gets Copied

When creating a new worktree, the following are automatically copied:

#### ✅ Data Directory (`data/`)
- **Portfolio snapshots** - Your trading history and positions
- **Market data** - Cached market information and analysis
- **Datasets** - Processed market data files
- **Logs** - Application logs and debugging info

#### ✅ Environment Files
- `.env` - Main environment configuration
- `.env.local` - Local overrides
- `.env.production` - Production settings
- `.env.example` - Template file

#### ✅ Credentials (`data/auth/`)
- **API credentials** - Encrypted Polymarket API keys
- **Private keys** - Encrypted wallet private keys
- **Authentication tokens** - Cached auth data

### Typical Workflows

#### Feature Development
```bash
# 1. Create feature worktree
cargo run -- worktree create new-portfolio-widget

# 2. Switch to new worktree
cd ../polybot-new-portfolio-widget

# 3. Work on your feature
cargo run -- portfolio
# ... make changes ...

# 4. Test your changes
cargo test
cargo run -- portfolio --text

# 5. Commit when ready
git add .
git commit -m "Add new portfolio widget"

# 6. Return to main and clean up when done
cd ../polybot
cargo run -- worktree remove ../polybot-new-portfolio-widget
```

#### Experiment with Different Data
```bash
# Create worktree without copying data
cargo run -- worktree create experiment --no-data

cd ../polybot-experiment

# Start fresh or sync specific data
cargo run -- init  # Fresh credentials
# OR
cargo run -- worktree sync --what creds  # Copy existing creds
```

#### Compare Implementations
```bash
# Keep main worktree running
cd polybot
cargo run -- stream &  # Background process

# Work in feature branch
cd ../polybot-new-feature
cargo run -- stream  # Different implementation

# Compare side-by-side
```

### Directory Structure

After creating worktrees, your directory structure might look like:

```
work/
├── tp/
│   ├── polybot/                    # Main worktree (main branch)
│   │   ├── data/                   # 564 MB of data
│   │   ├── .env
│   │   └── src/
│   ├── polybot-portfolio-upgrade/  # Feature worktree
│   │   ├── data/                   # Copied from main
│   │   ├── .env                    # Copied from main
│   │   ├── WORKTREE.md            # Auto-generated guide
│   │   └── src/
│   └── polybot-experiment/         # Experiment worktree
│       ├── data/                   # Independent data
│       └── src/
```

### Advanced Usage

#### Custom Sync Strategies

```bash
# Sync only recent portfolio data
cargo run -- worktree sync --what data
# Then manually clean old data if needed

# Sync everything except credentials (use fresh auth)
cargo run -- worktree sync --what data
cargo run -- worktree sync --what env
# Skip creds sync, run: cargo run -- init

# Sync from non-main worktree
cargo run -- worktree sync --source ../polybot-experiment --what data
```

#### Data Management

```bash
# Check data sizes across worktrees
cargo run -- worktree list

# Clean up large datasets in specific worktrees
cd ../polybot-experiment
rm -rf data/datasets/*  # Remove large market data files

# Sync fresh data when needed
cargo run -- worktree sync --what data
```

#### Development Tips

1. **Use meaningful branch names** - They become directory names
2. **Commit regularly** - Worktrees make it easy to switch contexts
3. **Sync data periodically** - Keep portfolio data up to date
4. **Clean up unused worktrees** - They consume disk space
5. **Use `--no-data` for quick experiments** - Skip copying large datasets

### Troubleshooting

#### "Worktree path already exists"
```bash
# Remove the directory manually if it's stale
rm -rf ../polybot-my-feature
git worktree prune  # Clean up git's worktree list
```

#### "Source worktree not found"
```bash
# List worktrees to see available sources
cargo run -- worktree list

# Specify source explicitly
cargo run -- worktree sync --source ../polybot --what data
```

#### "Permission denied" during copy
```bash
# Check file permissions
ls -la data/auth/

# Re-run with proper permissions
chmod -R 755 data/
```

#### Large data directory sync is slow
```bash
# Sync only what you need
cargo run -- worktree sync --what creds  # Just credentials
cargo run -- worktree sync --what env    # Just environment

# Or create without data and sync later
cargo run -- worktree create my-branch --no-data
cd ../polybot-my-branch
cargo run -- worktree sync --what creds
```

### Integration with Git

The `worktree` command is built on top of `git worktree` and provides:

- ✅ **Automatic branch creation** from base branch
- ✅ **Data directory management** with size tracking
- ✅ **Environment file copying** with validation
- ✅ **Credential management** with encryption preservation
- ✅ **Cleanup utilities** with safety checks
- ✅ **Status overview** with visual indicators

All standard git operations work normally in each worktree:
- `git status`, `git commit`, `git push`
- `git merge`, `git rebase`, `git cherry-pick`
- `git branch`, `git checkout`, `git log`

The worktrees share the same git history but have independent working directories and can be on different branches simultaneously

## Portfolio Management CLI Examples

This section shows examples of using the enhanced portfolio management system through the CLI.

### Basic Commands

#### 1. View Portfolio Status
```bash
# Basic portfolio summary
polybot portfolio-status

# Show detailed dashboard
polybot portfolio-status --dashboard

# View cache statistics
polybot portfolio-status --cache-stats

# Create manual snapshot
polybot portfolio-status --snapshot --reason "before-trading"

# Clear cache
polybot portfolio-status --clear-cache
```

#### 2. View Trade History
```bash
# Show recent trades (default: last 20)
polybot trades

# Show trades from last 7 days
polybot trades --days 7

# Show specific date range
polybot trades --from 2025-06-01 --to 2025-06-20

# Export to CSV
polybot trades --export --output my-trades.csv

# Show more trades
polybot trades --limit 50
```

#### 3. Enhanced Portfolio View
```bash
# View portfolio with enhanced display
polybot portfolio

# Filter by market
polybot portfolio --market "presidential"

# Filter by asset
polybot portfolio --asset "YES"

# Simple text mode (no TUI)
polybot portfolio --text
```

#### 4. Trading Commands with Portfolio Integration

##### Buy Order
```bash
# Place buy order (with portfolio tracking)
polybot buy <token_id> --price 0.45 --size 100 --yes

# With market ID for better tracking
polybot buy <token_id> --price 0.45 --size 100 --market-id <market_id> --yes
```

##### Sell Order
```bash
# Place sell order (with portfolio tracking)
polybot sell <token_id> --price 0.55 --size 100 --yes

# With market ID
polybot sell <token_id> --price 0.55 --size 100 --market-id <market_id> --yes
```

##### Cancel Order
```bash
# Cancel order (updates portfolio state)
polybot cancel <order_id>
```

#### 5. View Orders with Enhanced Display
```bash
# List all orders with dashboard view
polybot orders --dashboard

# Filter by token
polybot orders --token <token_id>
```

### Portfolio Service Features

#### Automatic Features
- **Real-time Sync**: Portfolio data refreshes automatically every 30 seconds
- **Snapshots**: Automatic snapshots after each trade for audit trail
- **Caching**: Performance optimization with intelligent cache management
- **Persistence**: All data stored in JSON files at `data/raw/`

#### Data Storage Structure
```
data/
├── raw/
│   ├── trade/          # Individual trade records
│   │   └── <trade_id>.json
│   ├── order/          # Individual order records
│   │   └── <order_id>.json
│   └── trade_query/    # Query results
│       └── <query_name>_<timestamp>.json
├── cache/              # Performance cache
│   ├── positions/
│   ├── orders/
│   ├── trades/
│   └── balances/
└── trade/              # Legacy portfolio storage
    └── account/
        └── <address>/
            ├── snapshots/
            ├── positions/
            ├── orders/
            └── stats/
```

### Example Workflow

```bash
# 1. Check current portfolio status
polybot portfolio-status --dashboard

# 2. View recent trades
polybot trades --days 7

# 3. Place a buy order
polybot buy 0x123...abc --price 0.45 --size 100 --yes

# 4. Check updated portfolio
polybot portfolio

# 5. Export trade history
polybot trades --export --output trades-$(date +%Y%m%d).csv

# 6. Create manual snapshot
polybot portfolio-status --snapshot --reason "end-of-day"
```

### Tips

1. **Performance**: Use `--cache-stats` to monitor cache performance
2. **Debugging**: Check logs in `data/logs/` for detailed operation logs
3. **Snapshots**: Created automatically after trades, or manually with `--snapshot`
4. **Data Export**: Use `trades --export` to create CSV reports
5. **Real-time Monitoring**: Use `polybot stream` for live updates with portfolio tracking

## Address Book CLI Examples

This section shows examples of using the address book management system through the CLI.

### Automatic User Address

Your primary wallet address (from your private key) is **automatically added** to the address book when you:
- Run `polybot init` to set up authentication
- Use any address book command for the first time

The auto-added address will:
- Be labeled as "my-wallet"
- Be set as your current/default address
- Have the type "Own"
- Be tagged with "primary" and "auto-added"

### ID System

All address book commands that display addresses now show **IDs** for easy reference:
- `address list` - Shows ID in the first column
- `address quick` - Shows ID in brackets [0], [1], etc.
- `address query` - Shows ID in results

These IDs can be used with the `address edit` command for quick editing.

### Basic Commands

#### 1. Add New Address
```bash
# Add a watched address with label
polybot address add 0x123...abc --label "alice-wallet" --description "Alice's trading wallet" -t watched

# Add your own address
polybot address add 0x456...def --label "my-trading" --description "My main trading account" -t own

# Add with tags
polybot address add 0x789...ghi --label "market-maker-1" -t market-maker --tags "defi,arbitrage"
```

#### 2. List Addresses
```bash
# List all addresses
polybot address list

# List with details
polybot address list --detailed

# List only own addresses
polybot address list --own

# List only watched addresses
polybot address list --watched

# Limit results
polybot address list --limit 10
```

#### 3. Quick View
```bash
# Quick overview of all addresses
polybot address quick

# Show only own addresses with values
polybot address quick --own --with-value

# Show only watched addresses
polybot address quick --watched
```

#### 4. Update Address (by address)
```bash
# Update label
polybot address update 0x123...abc --label "alice-main"

# Add description
polybot address update 0x123...abc --description "Alice's main trading wallet"

# Add tags
polybot address update 0x123...abc --tags "vip,whale"

# Add notes
polybot address update 0x123...abc --notes "High volume trader, track carefully"

# Deactivate address
polybot address update 0x123...abc --active false
```

#### 5. Edit Address (by ID)
```bash
# First, list addresses to see IDs
polybot address list

# Edit by ID - add tags
polybot address edit 0 --add-tags "whale,vip"

# Remove tags
polybot address edit 0 --remove-tags "test,temp"

# Replace all tags
polybot address edit 0 --tags "premium,tracked"

# Update multiple fields
polybot address edit 0 --label "alice-primary" --description "Primary wallet" --active true

# Clear fields
polybot address edit 0 --clear label  # Remove label
polybot address edit 0 --clear description  # Remove description
polybot address edit 0 --clear notes  # Remove notes

# Add and remove tags in one command
polybot address edit 0 --add-tags "new,tags" --remove-tags "old,unwanted"
```

#### 6. Toggle Active Status
```bash
# Toggle address active/inactive
polybot address toggle alice-wallet

# Toggle by address
polybot address toggle 0x123...abc
```

#### 7. Query Addresses
```bash
# Search by text
polybot address query "alice"

# Filter by type
polybot address query -t own

# Filter by tags
polybot address query --tags "defi,whale"

# Combined search
polybot address query "trading" -t watched --tags "vip"

# Sort by value
polybot address query --sort value

# Sort by last queried
polybot address query --sort queries --ascending
```

#### 8. Set Current Address
```bash
# Set by label
polybot address set-current my-trading

# Set by address
polybot address set-current 0x456...def

# View current address
polybot address current
```

#### 9. Sync Portfolio Data
```bash
# Sync current address
polybot address sync

# Sync specific address
polybot address sync 0x123...abc

# Sync all addresses
polybot address sync-all

# Sync only active addresses
polybot address sync-all --active-only

# Limit sync count
polybot address sync-all --limit 5
```

#### 10. Tag Management
```bash
# Add tags to addresses by type
polybot address tag "whale,important" --address-type own

# Add tags to addresses matching pattern
polybot address tag "defi,arbitrage" --pattern "market"

# Add multiple tags
polybot address tag "tracked,analysis,priority"
```

#### 11. Import/Export
```bash
# Export to CSV
polybot address export addresses.csv

# Import from CSV
polybot address import addresses.csv
```

#### 12. Statistics
```bash
# Show address book stats
polybot address stats
```

#### 13. Remove Address
```bash
# Remove with confirmation
polybot address remove 0x123...abc

# Remove without confirmation
polybot address remove 0x123...abc --yes
```

### Example Workflows

#### Setting Up Multiple Trading Accounts
```bash
# Add your main account
polybot address add 0x111...aaa --label "main" -t own --description "Main trading account"

# Add secondary account
polybot address add 0x222...bbb --label "secondary" -t own --description "Arbitrage account" --tags "arbitrage"

# Add watched competitors
polybot address add 0x333...ccc --label "competitor-1" -t watched --tags "competitor,whale"
polybot address add 0x444...ddd --label "competitor-2" -t watched --tags "competitor"

# Set main as current
polybot address set-current main

# Sync all accounts
polybot address sync-all
```

#### Tracking Market Makers
```bash
# Add market makers with tags
polybot address add 0xaaa...111 --label "mm-alpha" -t market-maker --tags "high-volume,tracked"
polybot address add 0xbbb...222 --label "mm-beta" -t market-maker --tags "medium-volume,tracked"

# Query all market makers
polybot address query -t market-maker

# Sync their data
polybot address sync-all --active-only

# Tag all market makers
polybot address tag "important,analysis" --address-type market-maker
```

#### Quick Status Check
```bash
# Quick view with values
polybot address quick --with-value

# Check current address
polybot address current

# Sync and check stats
polybot address sync
polybot address stats
```

#### Bulk Operations
```bash
# Export current state
polybot address export backup-$(date +%Y%m%d).csv

# Deactivate all watched addresses
polybot address query -t watched | while read addr; do
  polybot address toggle "$addr"
done

# Sync all active addresses
polybot address sync-all --active-only
```

### CSV Format

When importing/exporting, the CSV format is:
```csv
Address,Label,Type,Description,Tags,Added At,Last Synced,Total Value,Active
0x123...abc,alice-wallet,Watched,Alice's wallet,vip;whale,2025-06-21 10:00:00,2025-06-21 15:00:00,50000.00,true
```

### Tips

1. **Labels**: Use descriptive labels for easy reference
2. **Tags**: Use tags to group and filter addresses
3. **Active Status**: Deactivate addresses you're not actively tracking
4. **Sync**: Regular syncing keeps portfolio data up to date
5. **Current Address**: Set your main trading address as current for quick access
6. **Quick View**: Use `address quick` for fast overview
7. **Backups**: Export regularly to CSV for backups
