# GUI Module

This module provides the graphical user interface for Polybot using egui/eframe.

## Latest Updates

### UI Improvements (Fixed Issues)

1. **Fixed Black Vertical Strip in Sidebar**
   - Removed left margin from SidePanel frame
   - Set left margin to 0.0 to prevent black strip
   - Added `stroke(egui::Stroke::NONE)` to remove border

2. **Added Auto-Arrange Functionality**
   - New checkbox: "Auto-arrange on add" in Layout Management
   - When enabled, automatically arranges all panes optimally when adding new ones
   - Stored in `auto_arrange_on_add` field in TradingApp struct

3. **Layout Save/Load Feature**
   - Added "💾 Save Layout" button - saves current layout to timestamped JSON file
   - Added "📂 Load Layout" button - loads most recent saved layout
   - Layouts saved to: `./data/config/layouts/`
   - Format: `layout_YYYYMMDD_HHMMSS.json`

## Dataset Loading

The dataset loading functionality has been implemented in the main app:

### Key Components

1. **DatasetInfo struct**: Local representation of dataset information
   - `name`: Dataset identifier
   - `path`: Full path to dataset directory
   - `asset_count`: Number of market/asset files
   - `size_mb`: Total size in megabytes

2. **load_available_datasets()**: Scans the filesystem for available datasets
   - Uses `DatasetManager` from the datasets module
   - Scans the configured datasets directory (default: `./data/datasets`)
   - Converts dataset information to GUI-friendly format

3. **Dataset Selection Dialog**: Interactive UI for selecting datasets
   - Shows all available datasets with metadata
   - Allows multi-selection with checkboxes
   - Refresh button to rescan datasets
   - Displays helpful instructions if no datasets found

### Usage

When the user clicks "Start Streaming" in the sidebar or streams pane:
1. The dataset selector dialog opens
2. Available datasets are loaded from the filesystem
3. User can select one or more datasets
4. Clicking "Start Streaming" initiates streaming with selected datasets

### Current Status

- Dataset discovery and listing: ✅ Implemented
- Dataset selection UI: ✅ Implemented
- Loading actual token IDs from datasets: ⚠️ TODO
- Streaming initialization: ⚠️ TODO

### Next Steps

To complete the streaming functionality:
1. Implement reading of market data files (JSON chunks)
2. Extract token IDs from market data
3. Initialize the Streamer service with collected tokens
4. Connect streaming data to the UI components