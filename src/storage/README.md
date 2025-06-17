# Storage Module

This module provides efficient binary storage for WebSocket streaming data using the bincode serialization format.

## Overview

The storage system captures real-time WebSocket data feeds and stores them in a structured directory format for later analysis and replay.

## Storage Structure

```
data/stream/market/<token-id>/<session-id-date-hour>/
├── snapshot.bin      # Initial orderbook state
├── metadata.bin      # Session information
└── updates/
    ├── 000000001.bin # Sequential update files
    ├── 000000002.bin
    └── ...
```

## Components

### BinaryStorage
Core storage implementation using bincode serialization:
- Efficient binary format with minimal overhead
- Preserves decimal precision using string representation
- Supports versioning for future compatibility

### SessionManager
Manages streaming sessions with automatic session ID generation:
- Session ID format: `YYYYMMDD-HH-MM-SS`
- Tracks session state and update counts
- Thread-safe with async operations

### StorageWriter
High-level async writer with buffered operations:
- Non-blocking writes using background task
- Command queue for write operations
- Graceful shutdown support
- Error recovery mechanisms

## Usage

### CLI Integration

Enable storage when streaming:
```bash
polybot stream --assets <token-ids> --enable-storage --storage-path ./data --storage-max-file-mb 100
```

### Programmatic Usage

```rust
use polybot::storage::{StorageWriter, StorageConfig};

// Configure storage
let config = StorageConfig {
    enabled: true,
    base_path: PathBuf::from("./data"),
    max_file_size_mb: 100,
    buffer_size: 1000,
};

// Create writer
let writer = StorageWriter::new(config)?;

// Write snapshot when orderbook is first received
writer.write_snapshot(token_id, orderbook, market, outcome).await?;

// Write updates as they arrive
writer.write_update(token_id, event, orderbook_hash).await?;

// End session when done
writer.end_session(token_id).await?;

// Shutdown writer
writer.shutdown().await?;
```

## Data Format

### Snapshot (snapshot.bin)
```rust
BinarySnapshot {
    version: u32,
    token_id: String,
    timestamp_utc: i64,
    market: String,
    outcome: String,
    bids: Vec<(String, String)>, // (price, size)
    asks: Vec<(String, String)>,
    tick_size: Option<String>,
    hash: Option<String>,
}
```

### Update (updates/*.bin)
```rust
BinaryUpdate {
    version: u32,
    token_id: String,
    timestamp_utc: i64,
    sequence_number: u64,
    update_type: UpdateType,
    data: UpdateData,
}
```

### Metadata (metadata.bin)
```rust
SessionMetadata {
    version: u32,
    session_id: String,
    token_id: String,
    start_timestamp: i64,
    end_timestamp: Option<i64>,
    update_count: u64,
    market: String,
    outcome: String,
}
```

## Performance Characteristics

- **Async I/O**: Non-blocking writes don't impact streaming performance
- **Buffered Writes**: Commands are queued and processed in background
- **Binary Format**: Efficient serialization with bincode
- **Sequential Files**: Updates written to separate files for parallel access

## Future Enhancements

1. **Compression**: Add optional compression for storage efficiency
2. **File Rotation**: Implement size-based file rotation for updates
3. **Reader API**: Add utilities for reading and replaying stored data
4. **FlatBuffers**: Consider FlatBuffers for zero-copy deserialization
5. **Metrics**: Add storage metrics and monitoring