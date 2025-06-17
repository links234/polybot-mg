# Improved Dataset Selector

The improved dataset selector provides a more efficient and user-friendly way to browse and select datasets for streaming.

## Key Features

### 1. **Hierarchical Folder View**
- Datasets are organized in a tree structure matching your filesystem
- Expand/collapse folders with arrow keys
- Visual indicators show folder contents: `(5/8)` means 5 streamable out of 8 total datasets

### 2. **Parallel Loading with Rayon**
- Dataset discovery now uses parallel processing for faster loading
- Significantly reduces wait time for large dataset collections
- Progress indicator while loading

### 3. **Enhanced Selection Controls**
- **Space/Enter**: Toggle selection on current item or expand/collapse folder
- **a**: Select all items in the current folder
- **A**: Select all visible items
- **n**: Clear all selections
- **←/→**: Collapse/Expand folders

### 4. **Smart Filtering**
- Type to filter datasets by name or market description
- `/`: Clear filter
- Filtering automatically expands all folders to show matches

### 5. **Dataset Type Awareness**
- Visual icons for different dataset types:
  - 📄 Market Data (streamable)
  - 📊 Analyzed Markets (streamable)
  - ✨ Enriched Markets (streamable)
  - 🔧 Pipeline metadata (not streamable)
  - ⭐ Token Selections (streamable)
- Non-streamable datasets are shown but cannot be selected

### 6. **Keyboard Shortcuts**
- **?/h**: Show help screen with all shortcuts
- **s**: Start streaming selected datasets
- **q/Esc**: Cancel and exit
- **PgUp/PgDn**: Fast scroll (10 items)

## Usage

When you run `cargo run -- stream` without specifying assets, the improved dataset selector will automatically launch:

```bash
# Launch dataset selector
cargo run -- stream

# The selector will show:
# - Folder structure of your datasets
# - Number of streamable datasets in each folder
# - Quick selection options
```

## Performance Improvements

The selector now uses:
- **Rayon** for parallel dataset discovery
- **Efficient tree traversal** with configurable depth limits
- **Lazy loading** - only processes visible items
- **Smart caching** of folder states

## Example Workflow

1. Launch the selector: `cargo run -- stream`
2. Navigate to a folder containing your datasets
3. Press `a` to select all datasets in that folder
4. Or use Space to select individual datasets
5. Press `s` to start streaming
6. The stream will begin with all selected datasets

## Technical Details

The improved selector is implemented in `src/tui/dataset_selector_v2.rs` and includes:
- Parallel dataset discovery using `rayon`
- Hierarchical data structure for efficient folder navigation
- Optimized rendering that only processes visible items
- Type-safe dataset filtering