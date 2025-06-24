# Gamma Module

This module provides comprehensive functionality for interacting with Polymarket's Gamma API and managing market data.

## Components

### Core Systems

- **Client** (`client.rs`): HTTP client for Gamma API with caching and rate limiting
- **Database** (`database.rs`): SurrealDB storage with RocksDB backend for persistent, deduplicated market data
- **Session Manager** (`session.rs`): Session-based data fetching and storage management
- **Cache** (`cache.rs`): In-memory caching layer with disk persistence
- **TUI** (`tui/`): Terminal user interface for interactive data exploration

### Data Types

- **Types** (`types.rs`): Strongly-typed domain models for all Gamma API entities
- **Analytics** (`analytics.rs`): Data analysis and statistics generation

### Storage

- **Individual Storage** (`individual_storage.rs`): File-based storage for individual market records
- **Session Storage**: Raw API responses stored by session for replay and analysis

## Database Import

The gamma module supports importing session data into SurrealDB for efficient querying and deduplication.

### Import Commands

```bash
# Import a single session
cargo run -- gamma import-session --session-id 1 --yes

# Import all sessions
cargo run -- gamma import-session --session-id all --yes

# Import a range of sessions (NEW)
cargo run -- gamma import-session --from-session-id 1 --to-session-id 10 --yes

# Force reimport (overwrites existing data)
cargo run -- gamma import-session --session-id 1 --force --yes

# Custom batch size for large imports
cargo run -- gamma import-session --session-id all --batch-size 1000 --yes
```

### Range Import

The range import feature allows importing multiple sessions efficiently:
- `--from-session-id`: Starting session ID (inclusive)
- `--to-session-id`: Ending session ID (inclusive)
- Both parameters must be specified together
- Cannot be used with `--session-id`

Example: Import sessions 5 through 15:
```bash
cargo run -- gamma import-session --from-session-id 5 --to-session-id 15 --yes
```

### Database Operations

After importing, use the `gamma markets` command to query the database:
```bash
# Load all markets from database (instead of fetching from API)
cargo run -- gamma markets --from-db

# Non-interactive mode with database
cargo run -- gamma markets --from-db --no-interactive

# Apply filters when loading from database
cargo run -- gamma markets --from-db --active-only --limit 20
cargo run -- gamma markets --from-db --closed-only --min-volume 1000000
cargo run -- gamma markets --from-db --category "Politics" --sort-by liquidity

# Regular API fetch (default behavior)
cargo run -- gamma markets
```

### Key Differences

- **`--from-db`**: Loads all markets from SurrealDB (deduplicated, fast)
- **Without `--from-db`**: Fetches from Gamma API using session-based storage
- All filters and sorting options work with both modes

## Session Management

Sessions track API fetching progress and allow resuming interrupted fetches:
- Each session stores raw API responses in `data/gamma/raw/session-XXX/`
- Sessions are registered in `data/gamma/raw/sessions.json`
- Incomplete sessions can be resumed automatically

## Performance Optimizations

### Parallel Processing

The import system uses rayon for parallel file processing within each session:
- Files within a session are read in parallel (10 files at a time)
- JSON parsing happens concurrently across multiple CPU cores
- Database writes are batched to avoid overwhelming the system

This provides significant speedup when importing large sessions with many files.

### Progress Tracking

The import system displays progress bars when not in verbose mode:
- Main progress bar tracks session completion
- Per-session progress bars track file processing within each session
- Real-time updates show current file being processed
- Final summary shows total markets imported

To see progress bars, run without verbose flag:
```bash
cargo run -- gamma import-session --session-id all --yes
```

To see detailed logs instead, use verbose mode:
```bash
cargo run -- gamma import-session --session-id all --yes -v
```

## Database Commands

The gamma module provides comprehensive database manipulation commands:

### Statistics
```bash
# Show basic database statistics
cargo run -- gamma db stats

# Show detailed statistics with category breakdown
cargo run -- gamma db stats --detailed --by-category

# Show volume distribution
cargo run -- gamma db stats --volume-dist
```

### Search
```bash
# Search markets by keyword (case-insensitive by default)
cargo run -- gamma db search "president"

# Search in specific field
cargo run -- gamma db search "sports" --field category

# Case-sensitive search
cargo run -- gamma db search "Biden" --case-sensitive

# Regex search
cargo run -- gamma db search "trump|biden" --regex

# Limit results and format output
cargo run -- gamma db search "election" --limit 10 --format json
```

### List Markets
```bash
# List markets with pagination
cargo run -- gamma db list --limit 20 --offset 0

# Sort by different fields
cargo run -- gamma db list --sort-by volume --order desc

# Filter active markets only
cargo run -- gamma db list --active-only --min-volume 100000

# Filter by category
cargo run -- gamma db list --category "Politics" --format csv
```

### Export Data
```bash
# Export all markets to JSON
cargo run -- gamma db export markets.json

# Export to CSV with filters
cargo run -- gamma db export active_markets.csv --format csv --active-only

# Export specific fields only
cargo run -- gamma db export minimal.json --fields "id,question,volume,active"

# Export with volume filter
cargo run -- gamma db export high_volume.json --min-volume 1000000
```

### Get Specific Market
```bash
# Get market by ID
cargo run -- gamma db get 123456

# Output in different formats
cargo run -- gamma db get 123456 --format yaml
cargo run -- gamma db get 123456 --format table

# Show all fields including internal ones
cargo run -- gamma db get 123456 --all-fields
```

### Count Markets
```bash
# Simple count
cargo run -- gamma db count

# Count by category
cargo run -- gamma db count --group-by category

# Count active markets
cargo run -- gamma db count --active-only

# Show percentages
cargo run -- gamma db count --group-by active --percentage
```

### Database Cleanup
```bash
# Remove duplicates (dry run)
cargo run -- gamma db cleanup --remove-duplicates --dry-run

# Remove duplicates (actual)
cargo run -- gamma db cleanup --remove-duplicates --yes

# Update statistics
cargo run -- gamma db cleanup --update-stats --yes

# Multiple operations
cargo run -- gamma db cleanup --remove-duplicates --update-stats --yes
```

## Architecture Notes

- **Deduplication**: Markets are deduplicated by market_id during import
- **Performance**: Batch processing with parallel file reading for efficient imports
- **Persistence**: RocksDB provides crash-resistant storage
- **Schema**: SCHEMALESS tables for flexibility with evolving API responses
- **Parallelization**: Uses rayon for concurrent file processing within sessions
- **Query Support**: Full SurrealQL support for complex queries
- **Export Formats**: JSON, CSV, and YAML export capabilities