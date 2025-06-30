# Strategy Runner Guide

## Overview

The `run-strategy` command allows you to run automated trading strategies on Polymarket assets using real-time WebSocket data.

## Quick Start

```bash
# Run strategy with a valid asset ID
cargo run -- run-strategy --token-id 108468416668663017133298741485453125150952822149773262784582671647441799250111

# Show example asset IDs
cargo run -- run-strategy --show-examples

# Run with custom parameters
cargo run -- run-strategy \
  --token-id 108468416668663017133298741485453125150952822149773262784582671647441799250111 \
  --min-spread 0.002 \
  --max-spread 0.02 \
  --log-frequency 5
```

## Important: Asset ID Format

Polymarket uses **long decimal numbers** (70+ digits) as asset IDs, NOT short token IDs. 

❌ **Incorrect**: `6441420301603917` (too short)
✅ **Correct**: `108468416668663017133298741485453125150952822149773262784582671647441799250111`

## Finding Active Asset IDs

### Method 1: Use Example IDs
```bash
./target/debug/polybot run-strategy --show-examples
```

### Method 2: From Your Trading History
Look in your data directory for previously traded assets:
```bash
grep -h "asset_id" data/trade/account/*/snapshots/*.json | sort -u
```

### Method 3: Use Markets Command
```bash
# List active markets (if available)
cargo run -- markets list

# Use stream command to find active assets
cargo run -- stream
```

## Strategy Parameters

- `--token-id`: Asset ID(s) to monitor (comma-separated for multiple)
- `--min-spread`: Minimum spread threshold in price units (default: 0.001)
- `--max-spread`: Maximum spread threshold in price units (default: 0.01)
- `--volume-window`: Time window for volume analysis in seconds (default: 300)
- `--log-frequency`: How often to log orderbook updates (default: every 10 updates)
- `--verbose`: Enable detailed logging

## What the Strategy Does

The simple strategy:
1. **Monitors orderbook spreads** - Tracks bid/ask spread and categorizes as TIGHT/NORMAL/WIDE
2. **Calculates market metrics**:
   - Mid price
   - Spread percentage
   - Order book imbalance
   - Rolling volume by side
3. **Tracks significant changes** - Alerts on large spread movements
4. **Logs market analysis** - Periodic updates on market conditions

## Example Output

```
[SimpleStrategy-10846841] Market Analysis - Spread: $0.0100 (6.06%) [NORMAL] | Mid: $0.1650 | Imbalance: 67.6% | Buy Vol: 0 | Sell Vol: 0
```

## Troubleshooting

### No Events Received
If you see "Finished fetching initial orderbooks: 0 successes, 1 failures":
- Your asset ID is incorrect or the asset is not actively traded
- Use `--show-examples` to get valid asset IDs
- Check the warning messages about token ID format

### WebSocket Connection Issues
- Ensure you have valid credentials set up (`polybot init`)
- Check your internet connection
- Try the `stream` command first to verify WebSocket connectivity

## Advanced Usage

### Multiple Assets
```bash
cargo run -- run-strategy --token-id ID1,ID2,ID3
```
Note: Currently, the strategy monitors the first asset in the list.

### All Markets Mode (Testing)
```bash
cargo run -- run-strategy --token-id dummy --all-markets
```
This subscribes to all markets but may be overwhelming for a single strategy.

## Integration with PolyBot

The strategy integrates with PolyBot's order management system:
- Access to `OrderManager` for placing/tracking orders
- Access to `Portfolio` for position management
- Thread-safe architecture for concurrent operations

Future strategies can extend `SingleTokenStrategy` trait to implement custom logic.