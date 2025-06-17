# Portfolio Management System

This module implements a comprehensive portfolio management system that tracks positions, orders, and trading history with persistent storage.

## Architecture

### Data Storage Structure

Portfolio data is stored in a hierarchical directory structure:

```
data/trade/account/<ethereum-address>/
├── snapshots/               # Portfolio snapshots by datetime
│   ├── 2024-01-15-14-30-22.json
│   ├── 2024-01-15-16-45-10.json
│   └── 2024-01-15-18-00-00.json
├── trades/                  # Daily trade history
│   ├── 2024-01-15.json
│   ├── 2024-01-16.json
│   └── 2024-01-17.json
├── positions/               # Current positions
│   └── current.json
├── orders/                  # Active orders cache
│   └── active.json
└── stats/                   # Statistics and analytics
    ├── daily/
    │   ├── 2024-01-15.json
    │   └── 2024-01-16.json
    └── monthly/
        └── 2024-01.json
```

### Key Components

#### 1. PortfolioStorage (`storage.rs`)
- **Purpose**: Handles all file I/O operations for portfolio data
- **Features**:
  - Snapshot management with datetime-based filenames
  - Trade history persistence by day
  - Position state tracking
  - Daily/monthly statistics storage
  - Automatic directory creation
  - Snapshot integrity with hash verification

#### 2. PositionReconciler (`reconciler.rs`)
- **Purpose**: Builds positions from order history and calculates P&L
- **Features**:
  - Reconciles filled orders into positions
  - Calculates average entry prices
  - Tracks realized and unrealized P&L
  - Handles buy/sell order sequences
  - Zero trading fees (Polymarket is fee-free)

#### 3. Portfolio Types (`types.rs`)
- **Purpose**: Strongly-typed data structures for portfolio entities
- **Key Types**:
  - `Position`: Individual market position with P&L tracking
  - `PortfolioStats`: Aggregate portfolio statistics
  - `TradeRecord`: Individual trade execution records
  - `PortfolioSnapshot`: Complete portfolio state at a point in time

## Usage

### Basic Portfolio Display
```bash
# View portfolio with interactive TUI
cargo run -- portfolio

# Simple text output
cargo run -- portfolio --text

# Filter by market
cargo run -- portfolio --market "Trump"

# Filter by asset
cargo run -- portfolio --asset "0x123..."
```

### Data Persistence

Every time the portfolio command runs, it:

1. **Fetches** current orders from Polymarket API
2. **Reconciles** positions from filled orders
3. **Calculates** portfolio statistics
4. **Saves** current state to storage:
   - `orders/active.json` - Current orders cache
   - `positions/current.json` - Current positions
   - `snapshots/YYYY-MM-DD-HH-MM-SS.json` - Complete snapshot

### Position Reconciliation

The system automatically builds positions from order history:

```rust
// Example: User bought 100 YES at $0.65, then sold 40 YES at $0.72
// Result: Position of 60 YES with average price $0.65 and realized P&L of $2.80
```

**Position Calculation Logic**:
- **Long positions**: Net positive size from buy orders
- **Average price**: Weighted average of all buy orders
- **Realized P&L**: Calculated on each sell order
- **Unrealized P&L**: Current market price vs. average cost

### Snapshot System

Snapshots provide historical portfolio state:

```json
{
  "timestamp": "2024-01-15T18:00:00Z",
  "address": "0x742d35Cc6C...",
  "positions": [...],
  "active_orders": [...],
  "stats": {
    "total_realized_pnl": "12.50",
    "total_unrealized_pnl": "-2.30",
    "win_rate": "65.2",
    "total_fees_paid": "0.00"
  },
  "balances": {
    "total_value": "1025.67",
    "available_cash": "895.45",
    "position_value": "130.22"
  },
  "metadata": {
    "reason": "Manual",
    "version": "1.0"
  }
}
```

## Technical Features

### Error Resilience
- Graceful handling of API failures
- Automatic directory creation
- Safe JSON parsing with error context
- Non-blocking file operations

### Performance
- Efficient reconciliation algorithms
- Minimal memory footprint
- Concurrent file operations
- Smart caching of market data

### Data Integrity
- Blake3 hash verification for snapshots
- Atomic file writes
- Backup and recovery mechanisms
- Version tracking for format changes

## Integration

### CLI Integration
- Seamless integration with existing CLI commands
- Consistent error handling and logging
- Unified configuration through DataPaths

### TUI Integration
- Real-time position display
- Interactive navigation
- Keyboard shortcuts for common operations
- Responsive layout with proper error states

### API Integration
- Direct Polymarket CLOB API integration
- L2 authentication support
- Robust order fetching with retry logic
- Balance API fallback mechanisms

## Future Enhancements

Planned features for the portfolio system:

1. **Real-time Updates**: WebSocket integration for live position updates
2. **Advanced Analytics**: Sharpe ratio, maximum drawdown, and risk metrics
3. **Export Functionality**: CSV/PDF reports for tax and accounting
4. **Market Data Integration**: Live price feeds for accurate unrealized P&L
5. **Multi-account Support**: Portfolio aggregation across multiple wallets
6. **Performance Benchmarking**: Compare against market indices
7. **Alert System**: Notifications for significant P&L changes
8. **Automated Backup**: Cloud storage integration for data safety

## Development Notes

- All monetary values use `rust_decimal::Decimal` for precision
- Timestamps stored in UTC for consistency
- File I/O is async to prevent blocking
- Extensive logging for debugging and monitoring
- Modular design allows easy extension