# Testing Stream TUI

## Issue
When running `cargo run -- stream` without any arguments, the TUI dataset selector should launch. However, in some environments (like when running through automation tools or certain terminal emulators), the terminal detection may fail.

## Testing the TUI

### 1. Direct Terminal Test
Run this command directly in your terminal (not through any automation):
```bash
cargo run -- stream
```

This should launch the interactive dataset selector TUI.

### 2. Force TUI Mode
If the terminal detection is too strict, you can force TUI mode:
```bash
POLYBOT_FORCE_TUI=1 cargo run -- stream
```

### 3. Alternative: Use Direct Assets
If the TUI doesn't work in your environment, you can specify assets directly:
```bash
# Example with a Bitcoin price bet token
cargo run -- stream --assets 85949163243245471221790979452091560100141884930227668573477517865165344048388
```

### 4. Alternative: Use Markets File
Load assets from a markets.json file:
```bash
cargo run -- stream --markets-path data/datasets/bitcoin_price_bets/2025-06-15/2025-06-15_19-48-01/markets.json
```

## Common Issues

### "Device not configured" Error
This error occurs when the terminal doesn't support raw mode. This typically happens when:
- Running through SSH without proper TTY allocation
- Running in CI/CD environments
- Running through certain automation tools

### Solutions
1. Ensure you're running in a proper terminal emulator
2. If using SSH, use `ssh -t` to allocate a TTY
3. Use the `POLYBOT_FORCE_TUI=1` environment variable
4. Use one of the alternative methods above

## TUI Controls
Once the dataset selector launches:
- `↑/↓` - Navigate through datasets
- `Space` - Expand/collapse folders or toggle dataset selection
- `Enter` - Confirm selection and start streaming
- `q` - Quit without selecting
- `/` - Search/filter datasets
- `h` - Show help

## Debugging
To see detailed logs:
```bash
RUST_LOG=debug cargo run -- stream
```

Then check the log file in `data/logs/` for detailed information about what's happening.