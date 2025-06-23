# GUI Sidebar and Screenshot Fixes

## Issues Fixed

### 1. Sidebar Black Bar When Market Streams Populate

**Problem**: The sidebar showed black bars when market streams started getting populated with data. This was caused by dynamic sizing issues when stream statistics changed from loading to populated state.

**Root Cause**: The Connection Management section's layout would shift when content changed from "Loading..." to actual statistics, causing visual artifacts.

**Solution**:
- Set minimum height for horizontal layouts: `ui.set_min_height(20.0)`
- Use `ui.allocate_space(egui::vec2(0.0, 0.0))` to prevent layout shifts
- Apply fixed-width formatting for numeric values:
  - `{:>3}` for stream counts
  - `{:>5.1}` for events per second
- Ensure loading state text matches populated state width

### 2. Screenshot Functionality Implementation

**Problem**: The screenshot button was not properly implemented and didn't save screenshots.

**Solution**:
1. Added state fields to TradingApp:
   - `pending_screenshot: Option<(PathBuf, String)>` - tracks pending screenshot
   - `screenshot_message: Option<(String, Instant)>` - shows toast notification

2. Implemented `save_screenshot` method:
   - Converts egui::ColorImage to image::ImageBuffer
   - Saves as PNG using the image crate
   - Handles errors gracefully

3. Updated `update()` method:
   - Shows toast notification for 3 seconds
   - Processes pending screenshots using `frame.screenshot()`
   - Creates screenshots/ directory if needed

## Code Changes

### app.rs Modifications

1. **Imports**:
   ```rust
   use image;
   ```

2. **Struct Fields**:
   ```rust
   /// Screenshot state
   pending_screenshot: Option<(std::path::PathBuf, String)>,
   screenshot_message: Option<(String, std::time::Instant)>,
   ```

3. **Connection Management Section**:
   ```rust
   ui.horizontal(|ui| {
       ui.set_min_height(20.0); // Prevent collapse
       // ... rest of layout
   });
   ```

4. **Screenshot Handler in update()**:
   ```rust
   if let Some((filepath, filename)) = self.pending_screenshot.take() {
       if let Some(screenshot) = frame.screenshot() {
           match Self::save_screenshot(&screenshot, &filepath) {
               Ok(_) => { /* success */ },
               Err(e) => { /* error */ }
           }
       }
   }
   ```

### Cargo.toml Addition

```toml
image = "0.25"
```

## Testing

1. **Sidebar Stability**:
   - Start the GUI with `cargo run -- canvas`
   - Connect to WebSocket streams
   - Observe that sidebar remains stable as streams populate
   - No black bars should appear

2. **Screenshot Feature**:
   - Click the "📷 Screenshot" button in sidebar
   - Check screenshots/ directory for saved PNG files
   - Verify toast notification appears
   - Filename format: `screenshot_YYYY-MM-DD_HH-MM-SS.png`