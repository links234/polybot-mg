# P&L Calculation Analysis

## Current Implementation

The P&L calculation in the address book service (`src/address_book/service.rs`) uses a **cash flow based approach** in the `calculate_trading_metrics` method.

### How P&L is Currently Calculated

The system tracks all cash flows (money in/out) through different activity types:

1. **BUY trades**: 
   - Treated as cash OUT (negative cash flow)
   - `total_realized_pnl -= usdc_size`

2. **SELL trades**:
   - Treated as cash IN (positive cash flow)
   - `total_realized_pnl += usdc_size`

3. **REDEEM activities**:
   - Treated as cash IN from winning positions
   - `total_realized_pnl += usdc_size`

4. **CONVERSION activities**:
   - Treated as cash IN from market resolution
   - `total_realized_pnl += usdc_size`

5. **MERGE activities**:
   - Treated as cash IN from position merge/settlement
   - `total_realized_pnl += usdc_size`

6. **REWARD activities**:
   - Treated as cash IN from platform rewards
   - `total_realized_pnl += usdc_size`

### The Problem

This approach calculates **net cash flow**, not true P&L:
- It sums all money spent (BUYs) and all money received (SELLs, REDEEMs, etc.)
- Result: `Total Cash In - Total Cash Out`

## How Polymarket Likely Calculates P&L

Polymarket probably uses a **position-based P&L calculation**:

### For Each Position:
1. Track the cost basis (total spent on BUYs)
2. Track proceeds (total received from SELLs/REDEEMs)
3. Calculate: `P&L = Proceeds - Cost Basis`

### Key Differences:
1. **Position Matching**: Links BUYs and SELLs for the same market/outcome
2. **True Profit/Loss**: Shows actual gains/losses per position
3. **Excludes Transfers**: Only counts trading activities

## Discrepancy Examples

### Example 1: Simple Trade
- BUY 100 shares at $0.50 = -$50 (cost)
- SELL 100 shares at $0.70 = +$70 (proceeds)

**Our calculation**: $70 - $50 = $20 (correct by coincidence)
**Polymarket**: $70 - $50 = $20 profit

### Example 2: Multiple Positions
- BUY Position A: -$1000
- BUY Position B: -$500
- SELL Position A: +$1200
- Position B expires worthless

**Our calculation**: $1200 - $1500 = -$300
**Polymarket**: 
- Position A: $1200 - $1000 = +$200 profit
- Position B: $0 - $500 = -$500 loss
- Total: -$300 (same result, but per-position breakdown)

### Example 3: Including Rewards
- Trading P&L: -$100
- REWARD received: +$50

**Our calculation**: -$100 + $50 = -$50
**Polymarket**: Might show trading P&L (-$100) separately from rewards

## Recommended Solution

### 1. Position-Based P&L Tracking

Create a proper position tracking system:

```rust
struct PositionTracker {
    positions: HashMap<(String, String), PositionPnL>, // (market_id, outcome) -> PnL
}

struct PositionPnL {
    total_bought: Decimal,     // Total cost basis
    total_sold: Decimal,       // Total proceeds
    shares_bought: Decimal,    // Total shares purchased
    shares_sold: Decimal,      // Total shares sold
    current_shares: Decimal,   // Remaining shares
    realized_pnl: Decimal,     // Actual P&L from closed portion
    avg_buy_price: Decimal,    // Average purchase price
    avg_sell_price: Decimal,   // Average sell price
}
```

### 2. Separate Activity Types

Track different types of income separately:
- Trading P&L (BUY/SELL only)
- Market resolutions (REDEEM/CONVERSION)
- Platform rewards (REWARD)
- Other activities (MERGE, etc.)

### 3. Match Trades to Positions

When processing activities:
1. Group by `conditionId` and `outcome`
2. Calculate cost basis from BUYs
3. Calculate proceeds from SELLs/REDEEMs
4. Compute P&L per position

### 4. Handle Edge Cases

- Partial sells (FIFO/LIFO/Average cost)
- Market resolutions (winning vs losing)
- Fee handling
- Transfer activities (should not affect P&L)

## Implementation Steps

1. **Analyze Polymarket API Response**:
   - Check if activities include `conditionId` field
   - Verify `outcome` field presence
   - Look for any P&L fields Polymarket provides

2. **Create Position Aggregator**:
   - Group activities by market/outcome
   - Build position history
   - Calculate per-position P&L

3. **Update AddressStats**:
   - Add `trading_pnl` field (BUY/SELL only)
   - Add `resolution_pnl` field (REDEEM/CONVERSION)
   - Add `rewards_earned` field
   - Keep `total_realized_pnl` as sum

4. **Test with Real Data**:
   - Compare results with Polymarket UI
   - Validate edge cases
   - Ensure consistency

## Debugging Current Discrepancies

To debug why our P&L doesn't match Polymarket:

1. **Log all activities** with their contributions to P&L
2. **Compare activity counts** - are we missing some?
3. **Check activity types** - are we handling all types correctly?
4. **Verify amounts** - is `usdcSize` the right field?
5. **Look for fees** - are fees affecting the calculation?

## Next Steps

1. Examine actual API responses to understand data structure
2. Implement position-based tracking
3. Add detailed logging for P&L calculation
4. Create comparison tool to validate against Polymarket
5. Update the calculation logic based on findings