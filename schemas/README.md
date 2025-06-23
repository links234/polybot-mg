# FlatBuffer Schemas

This directory contains FlatBuffer schema definitions for efficient storage of WebSocket streaming data.

## Schema Files

### orderbook.fbs
Defines the schema for storing orderbook snapshots and updates:

- **OrderBookSnapshot**: Initial orderbook state with full bid/ask levels
- **OrderBookUpdate**: Incremental changes to the orderbook
- **SessionMetadata**: Metadata about a streaming session

## Storage Structure

Data is stored in the following directory structure:
```
data/stream/market/<token-id>/<session-id-date-hour>/
  ├── snapshot.fbs      # Initial orderbook snapshot
  ├── metadata.fbs      # Session metadata
  └── updates/
      ├── 000000001.fbs # Update files (sequential)
      ├── 000000002.fbs
      └── ...
```

## Building Schemas

To generate Rust code from schemas:
```bash
flatc --rust -o src/generated/ schemas/orderbook.fbs
```

## Design Decisions

1. **String for Decimals**: Using strings to preserve exact decimal precision
2. **Separate Update Files**: Each update in its own file for parallel writes and reads
3. **Blake3 Hashes**: For orderbook state verification
4. **Session-based Organization**: Easy to archive and query specific time periods