# TUI Dataset Selector Implementation Summary

## Overview
The TUI dataset selector has been enhanced to properly handle all dataset types and extract the actual token IDs from markets.json files for streaming.

## Key Improvements

### 1. Dataset Type Support
- **All dataset types are now selectable** including:
  - Raw market data
  - Analyzed markets
  - Enriched markets
  - Pipeline outputs (if they contain markets.json)
  - Token selections
  - Mixed datasets

### 2. Proper Token Extraction
The TUI now correctly:
- Uses `dataset.token_id` as a dataset identifier (not an actual token ID)
- When user presses 's' to start streaming, it:
  - Loads the markets.json file from each selected dataset
  - Extracts all token IDs from the markets
  - Returns the actual token IDs for streaming

### 3. Terminal Detection
- More lenient terminal detection that works in various environments
- Checks terminal size availability first (works in VSCode integrated terminal)
- Falls back to traditional stdout.is_terminal() check
- Supports `POLYBOT_FORCE_TUI=1` environment variable for forcing TUI mode

### 4. Enhanced Error Handling
- Better error messages with specific guidance
- Proper cleanup on errors (restores terminal state)
- Comprehensive debug logging throughout

### 5. UI Features
- Folder tree view with expand/collapse
- Dataset type icons (📄, 📊, ✨, 🔧, ⭐, 📁)
- Shows selectable vs non-selectable datasets
- Search/filter functionality
- Multi-select with visual checkboxes
- Help screen with '?' or 'h'

## How It Works

1. **Dataset Discovery**: Scans `data/datasets/*` for all dataset types
2. **Tree Building**: Organizes datasets in a folder hierarchy
3. **Selection**: User navigates and selects datasets (not individual tokens)
4. **Token Extraction**: When streaming starts, extracts actual tokens from selected datasets' markets.json files
5. **Streaming**: Returns all extracted token IDs to the stream command

## Usage

```bash
# Run in a proper terminal
cargo run -- stream

# Force TUI mode if terminal detection fails
POLYBOT_FORCE_TUI=1 cargo run -- stream

# With debug logging
RUST_LOG=debug cargo run -- stream
```

## Controls
- `↑/↓` - Navigate
- `Space/Enter` - Toggle selection
- `←/→` - Collapse/Expand folders
- `a` - Select all in folder
- `A` - Select all visible
- `n` - Clear selections
- `s` - Start streaming
- `?/h` - Help
- `q/Esc` - Quit

## Implementation Details

The key fix was recognizing that `DiscoveredDataset.token_id` is actually a dataset name/identifier, not a real token ID. The actual token IDs must be extracted from the markets.json files within each dataset.

The `extract_tokens_from_dataset()` method:
1. Looks for markets.json in the dataset directory
2. If not found, checks subdirectories
3. Parses the JSON and extracts token_id from each market's tokens array
4. Returns all found token IDs for streaming