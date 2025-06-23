//! GUI module for the egui-based trading interface

pub mod app;
pub mod components;
pub mod panes;
pub mod services;

pub use app::TradingApp;

use crate::data_paths::DataPaths;
use anyhow::Result;
use tracing::{error, info};

/// Launch the trading canvas GUI application
pub async fn launch_trading_canvas(
    _width: u32,
    _height: u32,
    dark_mode: bool,
    title: &str,
    host: &str,
    data_paths: DataPaths,
) -> Result<()> {
    info!("🎨 Launching Trading Canvas GUI in fullscreen mode");

    // Get screen resolution for fullscreen
    let (screen_width, screen_height) = get_screen_resolution();
    info!(
        "🖥️ Detected screen resolution: {}x{}",
        screen_width, screen_height
    );

    // Configure native options for fullscreen mode
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([screen_width, screen_height])
            .with_title(title)
            .with_min_inner_size([800.0, 600.0])
            .with_fullscreen(true) // Start in fullscreen mode
            .with_maximized(true) // Also maximize as fallback
            .with_decorations(false) // No window decorations in fullscreen
            .with_active(true) // Request window focus
            .with_visible(true), // Ensure window is visible
        ..Default::default()
    };

    info!("🚀 About to create and run the trading app...");

    // Create and run the trading app
    let app_result = eframe::run_native(
        title,
        native_options,
        Box::new({
            let host = host.to_string();
            let _title = title.to_string();
            move |cc| {
                info!("📱 GUI context created, setting up styling...");

                // Configure egui styling
                setup_custom_style(&cc.egui_ctx, dark_mode);

                info!("🎯 Creating TradingApp instance...");

                // Create the trading app
                let app = TradingApp::new(cc, host, data_paths);

                info!("✅ TradingApp created successfully");

                Ok(Box::new(app))
            }
        }),
    );

    info!("📊 eframe::run_native call completed");

    match app_result {
        Ok(()) => {
            info!("Trading canvas closed successfully");
            Ok(())
        }
        Err(e) => {
            error!("Trading canvas error: {}", e);
            Err(anyhow::anyhow!("GUI error: {}", e))
        }
    }
}

fn setup_custom_style(ctx: &egui::Context, dark_mode: bool) {
    let mut style = (*ctx.style()).clone();

    // Configure spacing
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(12.0, 8.0);
    style.spacing.indent = 20.0;

    // Configure colors for trading theme
    if dark_mode {
        ctx.set_visuals(egui::Visuals::dark());

        // Customize colors for trading
        style.visuals.widgets.noninteractive.bg_fill = egui::Color32::from_gray(25);
        style.visuals.widgets.inactive.bg_fill = egui::Color32::from_gray(35);
        style.visuals.widgets.hovered.bg_fill = egui::Color32::from_gray(45);
        style.visuals.widgets.active.bg_fill = egui::Color32::from_gray(55);

        // Trading-specific colors
        style.visuals.error_fg_color = egui::Color32::from_rgb(220, 80, 80); // Red for sells/losses
        style.visuals.warn_fg_color = egui::Color32::from_rgb(255, 200, 100); // Yellow for warnings
    } else {
        ctx.set_visuals(egui::Visuals::light());
    }

    ctx.set_style(style);
}

/// Get the primary monitor's screen resolution
fn get_screen_resolution() -> (f32, f32) {
    // Try to get screen resolution using platform-specific methods
    #[cfg(target_os = "macos")]
    {
        get_macos_screen_resolution()
    }
    #[cfg(target_os = "windows")]
    {
        get_windows_screen_resolution()
    }
    #[cfg(target_os = "linux")]
    {
        get_linux_screen_resolution()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        // Fallback for unsupported platforms
        info!("⚠️ Screen resolution detection not supported on this platform, using default");
        (1920.0, 1080.0)
    }
}

#[cfg(target_os = "macos")]
fn get_macos_screen_resolution() -> (f32, f32) {
    use std::process::Command;

    // Use system_profiler to get display information
    if let Ok(output) = Command::new("system_profiler")
        .args(&["SPDisplaysDataType", "-json"])
        .output()
    {
        if let Ok(json_str) = String::from_utf8(output.stdout) {
            // Parse the JSON to extract resolution
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&json_str) {
                if let Some(displays) = json["SPDisplaysDataType"].as_array() {
                    for display in displays {
                        if let Some(resolution) = display["_spdisplays_resolution"].as_str() {
                            if let Some((width_str, height_str)) = resolution.split_once(" x ") {
                                if let (Ok(width), Ok(height)) =
                                    (width_str.parse::<f32>(), height_str.parse::<f32>())
                                {
                                    info!(
                                        "📱 Detected macOS display resolution: {}x{}",
                                        width, height
                                    );
                                    return (width, height);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Alternative approach using displayplacer command if available
    if let Ok(output) = Command::new("displayplacer").args(&["list"]).output() {
        if let Ok(output_str) = String::from_utf8(output.stdout) {
            // Parse displayplacer output for resolution
            for line in output_str.lines() {
                if line.contains("Resolution:") {
                    if let Some(resolution_part) = line.split("Resolution: ").nth(1) {
                        if let Some(resolution) = resolution_part.split_whitespace().next() {
                            if let Some((width_str, height_str)) = resolution.split_once("x") {
                                if let (Ok(width), Ok(height)) =
                                    (width_str.parse::<f32>(), height_str.parse::<f32>())
                                {
                                    info!("📱 Detected macOS display resolution via displayplacer: {}x{}", width, height);
                                    return (width, height);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Fallback for macOS
    info!("⚠️ Could not detect macOS screen resolution, using default");
    (1920.0, 1080.0)
}

#[cfg(target_os = "windows")]
fn get_windows_screen_resolution() -> (f32, f32) {
    use std::process::Command;

    // Use wmic to get screen resolution
    if let Ok(output) = Command::new("wmic")
        .args(&[
            "path",
            "Win32_VideoController",
            "get",
            "CurrentHorizontalResolution,CurrentVerticalResolution",
            "/format:csv",
        ])
        .output()
    {
        if let Ok(output_str) = String::from_utf8(output.stdout) {
            for line in output_str.lines() {
                if line.contains(",") && !line.contains("Node") {
                    let parts: Vec<&str> = line.split(',').collect();
                    if parts.len() >= 3 {
                        if let (Ok(width), Ok(height)) =
                            (parts[1].parse::<f32>(), parts[2].parse::<f32>())
                        {
                            if width > 0.0 && height > 0.0 {
                                info!(
                                    "📱 Detected Windows display resolution: {}x{}",
                                    width, height
                                );
                                return (width, height);
                            }
                        }
                    }
                }
            }
        }
    }

    // Fallback for Windows
    info!("⚠️ Could not detect Windows screen resolution, using default");
    (1920.0, 1080.0)
}

#[cfg(target_os = "linux")]
fn get_linux_screen_resolution() -> (f32, f32) {
    use std::process::Command;

    // Try xrandr first (most common)
    if let Ok(output) = Command::new("xrandr").output() {
        if let Ok(output_str) = String::from_utf8(output.stdout) {
            for line in output_str.lines() {
                if line.contains(" connected ") && line.contains("x") {
                    // Parse xrandr output: "1920x1080+0+0"
                    for part in line.split_whitespace() {
                        if part.contains("x") && part.contains("+") {
                            if let Some(resolution) = part.split('+').next() {
                                if let Some((width_str, height_str)) = resolution.split_once('x') {
                                    if let (Ok(width), Ok(height)) =
                                        (width_str.parse::<f32>(), height_str.parse::<f32>())
                                    {
                                        info!("📱 Detected Linux display resolution via xrandr: {}x{}", width, height);
                                        return (width, height);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Try xdpyinfo as fallback
    if let Ok(output) = Command::new("xdpyinfo").output() {
        if let Ok(output_str) = String::from_utf8(output.stdout) {
            for line in output_str.lines() {
                if line.trim().starts_with("dimensions:") {
                    if let Some(dimensions) = line.split("dimensions:").nth(1) {
                        if let Some(resolution) = dimensions.trim().split_whitespace().next() {
                            if let Some((width_str, height_str)) = resolution.split_once('x') {
                                if let (Ok(width), Ok(height)) =
                                    (width_str.parse::<f32>(), height_str.parse::<f32>())
                                {
                                    info!(
                                        "📱 Detected Linux display resolution via xdpyinfo: {}x{}",
                                        width, height
                                    );
                                    return (width, height);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Fallback for Linux
    info!("⚠️ Could not detect Linux screen resolution, using default");
    (1920.0, 1080.0)
}
