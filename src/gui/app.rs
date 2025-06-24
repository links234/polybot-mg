//! Main trading application with egui interface

use egui_tiles::{TileId, Tiles, Tree};
use rust_decimal::Decimal;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::data_paths::DataPaths;
use crate::markets::datasets::{DatasetManager, DatasetManagerConfig};
use crate::core::execution::orders::{EnhancedOrder, OrderManager};
use crate::gui::panes::Pane;
use crate::gui::services::PortfolioService;
use crate::core::portfolio::controller::PortfolioManager;
use crate::core::services::streaming::{StreamingService, StreamingServiceConfig, StreamingServiceTrait};
use crate::core::ws::{PolyEvent, WsConfig};
use crate::core::types::common::Side;

// Additional imports for screenshot functionality
use std::fs;
use chrono::{DateTime, Local};
use image;
use std::path::PathBuf;
use serde::{Serialize, Deserialize};

/// Saved layout data including pinned tiles
#[derive(Serialize, Deserialize)]
struct SavedLayout {
    tree: Tree<Pane>,
    pinned_tiles: Vec<TileId>,  // Using Vec since HashSet doesn't serialize nicely
}

#[derive(Clone, Debug)]
struct DatasetInfo {
    name: String,
    path: std::path::PathBuf,
    asset_count: usize,
    size_mb: f64,
}

#[derive(Clone, Debug, PartialEq)]
enum StreamingState {
    Disconnected,
    Initializing { progress: f32, message: String },
    Connected,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct TokenActivity {
    pub token_id: String,
    pub event_count: usize,
    pub last_bid: Option<Decimal>,
    pub last_ask: Option<Decimal>,
    pub last_update: Option<Instant>,
    pub total_volume: Decimal,
    pub trade_count: usize,
    pub last_messages_sent: usize,
    pub messages_sorted: usize,
    pub last_trade_price: Option<Decimal>,
    pub last_trade_timestamp: Option<u64>,
}

#[derive(Debug, Clone)]
struct OrderBookChange {
    token_id: String,
    price: Decimal,
    size: Decimal,
    changed_at: Instant,
    is_bid: bool,
}

pub struct TradingApp {
    /// Tile management
    tree: Tree<Pane>,
    focused_tile_id: Option<egui_tiles::TileId>,
    tiles_to_close: Vec<egui_tiles::TileId>,

    /// Application state
    host: String,
    data_paths: DataPaths,

    /// Trading infrastructure
    portfolio_manager: Arc<PortfolioManager>,
    portfolio_service: PortfolioService,
    _order_manager: OrderManager,

    /// Data state
    orders_cache: Arc<RwLock<Vec<EnhancedOrder>>>,

    /// UI state
    show_new_order_dialog: bool,
    show_dataset_selector: bool,
    show_streams_overview: bool,
    sidebar_width: f32,
    auto_arrange_on_add: bool,

    /// Menu state
    show_about: bool,
    show_settings: bool,

    /// Streaming state
    streaming_service: Option<Arc<StreamingService>>,
    streaming_assets: Vec<String>,
    streaming_state: StreamingState,
    streaming_task: Option<tokio::task::JoinHandle<()>>,
    streaming_progress_rx: Option<tokio::sync::mpsc::Receiver<(f32, String)>>,
    streaming_result_rx:
        Option<tokio::sync::oneshot::Receiver<Result<Arc<StreamingService>, anyhow::Error>>>,

    /// Position refresh state (unused but kept for compatibility)
    _last_position_fetch: Option<std::time::Instant>,
    _is_fetching_positions: bool,

    /// Orderbook state
    current_token_id: Option<String>,
    current_bids: Vec<crate::core::types::market::PriceLevel>,
    current_asks: Vec<crate::core::types::market::PriceLevel>,

    /// Track orderbook changes for flash animation
    orderbook_changes: Vec<OrderBookChange>,

    /// Dataset selection state
    available_datasets: Vec<DatasetInfo>,
    selected_datasets: std::collections::HashSet<String>,

    /// Token activity tracking for streams overview
    token_activities: Arc<RwLock<HashMap<String, TokenActivity>>>,
    event_receiver: Option<tokio::sync::broadcast::Receiver<PolyEvent>>,

    /// Pending new orderbook to open
    pending_new_orderbook: Option<String>,
    /// Pending new worker details to open
    pending_new_worker_details: Option<usize>,

    /// Cached streaming data to avoid blocking calls in GUI

    /// WebSocket Manager state
    selected_worker_id: Option<usize>,
    worker_stream_events: Vec<PolyEvent>,
    worker_stream_max_events: usize,
    cached_streaming_tokens: Vec<String>,
    cached_orderbook: Option<crate::core::ws::OrderBook>,
    cached_last_trade_price: Option<(Decimal, u64)>,
    cached_streaming_stats: Option<crate::core::services::streaming::traits::StreamingStats>,
    cached_worker_statuses: Vec<crate::core::services::streaming::traits::WorkerStatus>,

    /// Screenshot state
    pending_screenshot: Option<(std::path::PathBuf, String)>,
    pending_tile_screenshot: Option<(TileId, std::path::PathBuf, String)>,
    screenshot_message: Option<(String, std::time::Instant)>,
    
    /// Pinned tiles that should not be moved or affected by auto-arrange
    pinned_tiles: HashSet<TileId>,
    tile_bounds: std::collections::HashMap<TileId, egui::Rect>,
    
    /// Track whether the layout has unsaved changes
    has_unsaved_layout_changes: bool,
    
    /// Track previous tree state to detect drag/drop changes
    previous_tree_hash: Option<u64>,

    /// Background task handle for data updates
    _data_update_task: Option<tokio::task::JoinHandle<()>>,

    /// Channels for receiving cached data from background task
    cached_data_receivers: Option<(
        std::sync::mpsc::Receiver<Vec<String>>,
        std::sync::mpsc::Receiver<crate::core::ws::OrderBook>,
        std::sync::mpsc::Receiver<(Decimal, u64)>,
        std::sync::mpsc::Receiver<crate::core::services::streaming::traits::StreamingStats>,
        std::sync::mpsc::Receiver<Vec<crate::core::services::streaming::traits::WorkerStatus>>,
    )>,

    /// Channel for sending current token updates to background task
    current_token_sender: Option<std::sync::mpsc::Sender<Option<String>>>,

    /// Track if this is the first update call
    first_update: bool,

    /// Track fullscreen state for proper F11 toggling
    is_fullscreen: bool,
}

impl TradingApp {
    pub fn new(cc: &eframe::CreationContext<'_>, host: String, data_paths: DataPaths) -> Self {
        info!("🏗️ TradingApp::new() called");

        // Enable persistence
        if let Some(storage) = cc.storage {
            info!("💾 Storage available, checking for saved workspace");
            if let Some(saved_data) = storage.get_string("trading_workspace") {
                // Try to load new format first
                if let Ok(saved_layout) = serde_json::from_str::<SavedLayout>(&saved_data) {
                    info!("📂 Found saved workspace with {} pinned tiles, restoring...", saved_layout.pinned_tiles.len());
                    let mut app = Self::with_tree(saved_layout.tree, host, data_paths);
                    app.pinned_tiles = saved_layout.pinned_tiles.into_iter().collect();
                    app.has_unsaved_layout_changes = false;
                    return app;
                } else if let Ok(tree) = serde_json::from_str::<Tree<Pane>>(&saved_data) {
                    // Fall back to old format
                    info!("📂 Found saved workspace (legacy format), restoring...");
                    return Self::with_tree(tree, host, data_paths);
                }
            }
        }

        info!("🆕 No saved workspace found, checking for recent layouts...");
        
        // Try to load the most recent layout from files
        let mut app = Self::with_tree(Self::create_default_layout(), host, data_paths);
        
        // Check for saved layout files
        let layouts_dir = app.data_paths.data().join("config").join("layouts");
        if layouts_dir.exists() {
            match fs::read_dir(&layouts_dir) {
                Ok(entries) => {
                    let mut layout_files: Vec<PathBuf> = entries
                        .filter_map(|entry| entry.ok())
                        .map(|entry| entry.path())
                        .filter(|path| {
                            path.extension()
                                .and_then(|ext| ext.to_str())
                                .map(|ext| ext == "json")
                                .unwrap_or(false)
                        })
                        .collect();

                    if !layout_files.is_empty() {
                        // Sort by modification time (newest first)
                        layout_files.sort_by_key(|path| {
                            fs::metadata(path)
                                .and_then(|meta| meta.modified())
                                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                        });
                        layout_files.reverse();

                        // Load the most recent layout
                        if let Some(latest_file) = layout_files.first() {
                            match fs::read_to_string(latest_file) {
                                Ok(content) => {
                                    // Try to load new format first (with pinned tiles)
                                    match serde_json::from_str::<SavedLayout>(&content) {
                                        Ok(saved_layout) => {
                                            info!("📂 Auto-loaded most recent layout from: {} (with {} pinned tiles)", 
                                                  latest_file.display(), saved_layout.pinned_tiles.len());
                                            app.tree = saved_layout.tree;
                                            app.pinned_tiles = saved_layout.pinned_tiles.into_iter().collect();
                                            app.has_unsaved_layout_changes = false;
                                            return app;
                                        }
                                        Err(_) => {
                                            // Fall back to old format (just tree)
                                            match serde_json::from_str::<Tree<Pane>>(&content) {
                                                Ok(tree) => {
                                                    info!("📂 Auto-loaded most recent layout from: {} (legacy format)", 
                                                          latest_file.display());
                                                    app.tree = tree;
                                                    return app;
                                                }
                                                Err(e) => {
                                                    warn!("Failed to load layout from {}: {}", latest_file.display(), e);
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to read layout file {}: {}", latest_file.display(), e);
                                }
                            }
                        }
                    } else {
                        info!("📁 No saved layouts found in directory");
                    }
                }
                Err(e) => {
                    warn!("Failed to read layouts directory: {}", e);
                }
            }
        } else {
            info!("📁 Layouts directory does not exist yet");
        }
        
        info!("🆕 Using default layout");
        app
    }

    fn with_tree(tree: Tree<Pane>, host: String, data_paths: DataPaths) -> Self {
        info!("🔧 with_tree() called, loading datasets...");

        // Load datasets on startup
        let available_datasets = Self::load_available_datasets(&data_paths);
        info!("📊 Loaded {} datasets", available_datasets.len());

        info!("💼 Creating portfolio service...");
        // Create portfolio service
        let portfolio_service = PortfolioService::new(host.clone(), data_paths.clone());
        info!("✅ Portfolio service created");

        let app = Self {
            tree,
            focused_tile_id: None,
            tiles_to_close: Vec::new(),
            host,
            data_paths,
            portfolio_manager: Arc::new(PortfolioManager::new()),
            portfolio_service,
            _order_manager: OrderManager::new(),
            orders_cache: Arc::new(RwLock::new(Vec::new())),
            show_new_order_dialog: false,
            show_dataset_selector: false,
            show_streams_overview: false,
            sidebar_width: 180.0,
            auto_arrange_on_add: false,
            show_about: false,
            show_settings: false,
            streaming_service: None,
            streaming_assets: Vec::new(),
            streaming_state: StreamingState::Disconnected,
            streaming_task: None,
            streaming_progress_rx: None,
            streaming_result_rx: None,
            _last_position_fetch: None,
            _is_fetching_positions: false,
            current_token_id: None,
            current_bids: Vec::new(),
            current_asks: Vec::new(),
            orderbook_changes: Vec::new(),
            available_datasets,
            selected_datasets: std::collections::HashSet::new(),
            token_activities: Arc::new(RwLock::new(HashMap::new())),
            event_receiver: None,
            pending_new_orderbook: None,
            pending_new_worker_details: None,
            cached_streaming_tokens: Vec::new(),
            cached_orderbook: None,
            cached_last_trade_price: None,
            cached_streaming_stats: None,
            cached_worker_statuses: Vec::new(),
            selected_worker_id: None,
            worker_stream_events: Vec::new(),
            worker_stream_max_events: 100,
            pending_screenshot: None,
            pending_tile_screenshot: None,
            screenshot_message: None,
            pinned_tiles: HashSet::new(),
            has_unsaved_layout_changes: false,
            previous_tree_hash: None,
            tile_bounds: std::collections::HashMap::new(),
            _data_update_task: None,
            cached_data_receivers: None,
            current_token_sender: None,
            first_update: true,
            is_fullscreen: false,
        };

        info!("🎉 TradingApp fully constructed successfully");
        app
    }

    fn create_default_layout() -> Tree<Pane> {
        let mut tiles = Tiles::default();

        // Create main panes
        let orders = tiles.insert_pane(Pane::Orders);
        let portfolio = tiles.insert_pane(Pane::Portfolio);
        let streams = tiles.insert_pane(Pane::Streams);
        let market_depth = tiles.insert_pane(Pane::MarketDepth(None));

        // Create layout using only linear splits - no tabs:
        // Left side: Portfolio and Orders in vertical split
        // Middle: Market Streams
        // Right side: Market Depth (orderbook)
        let left_split = tiles.insert_vertical_tile(vec![portfolio, orders]);

        // Create three-column layout with grid splits only
        let root = tiles.insert_horizontal_tile(vec![left_split, streams, market_depth]);

        Tree::new("trading_workspace", root, tiles)
    }

    fn show_menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                // File menu
                ui.menu_button("File", |ui| {
                    if ui.button("📄 New Workspace").clicked() {
                        self.tree = Self::create_default_layout();
                        self.has_unsaved_layout_changes = true;
                        ui.close_menu();
                    }

                    if ui.button("💾 Save Layout").clicked() {
                        info!("Layout saved");
                        ui.close_menu();
                    }

                    ui.separator();

                    if ui.button("⚙️ Settings").clicked() {
                        self.show_settings = true;
                        ui.close_menu();
                    }

                    ui.separator();

                    if ui.button("🚪 Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                // View menu
                ui.menu_button("View", |ui| {
                    if ui.button("📋 Add Orders Pane").clicked() {
                        self.add_pane(Pane::Orders);
                        ui.close_menu();
                    }

                    if ui.button("💼 Add Portfolio Pane").clicked() {
                        self.add_pane(Pane::Portfolio);
                        ui.close_menu();
                    }

                    if ui.button("📡 Add Streams Pane").clicked() {
                        self.add_pane(Pane::Streams);
                        ui.close_menu();
                    }

                    if ui.button("📊 Add Market Depth Pane").clicked() {
                        self.add_pane(Pane::MarketDepth(None));
                        ui.close_menu();
                    }

                    if ui.button("📈 Add Charts Pane").clicked() {
                        self.add_pane(Pane::Charts);
                        ui.close_menu();
                    }

                    ui.separator();

                    if ui.button("🔍 Streams Overview").clicked() {
                        self.show_streams_overview = true;
                        ui.close_menu();
                    }
                });

                // Trading menu
                ui.menu_button("Trading", |ui| {
                    if ui.button("🛒 New Order").clicked() {
                        self.show_new_order_dialog = true;
                        ui.close_menu();
                    }

                    if ui.button("🔄 Refresh Orders").clicked() {
                        self.refresh_orders();
                        ui.close_menu();
                    }

                    ui.separator();

                    if ui.button("💰 Check Balances").clicked() {
                        self.add_pane(Pane::Balances);
                        ui.close_menu();
                    }

                    if ui.button("🔌 WebSocket Manager").clicked() {
                        self.add_pane(Pane::WebSocketManager);
                        ui.close_menu();
                    }
                });

                // Help menu
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.menu_button("Help", |ui| {
                        if ui.button("📖 About").clicked() {
                            self.show_about = true;
                            ui.close_menu();
                        }

                        if ui.button("⌨️ Keyboard Shortcuts").clicked() {
                            self.show_about = true; // Reuse about dialog to show shortcuts
                            ui.close_menu();
                        }

                        ui.separator();

                        if ui.button("🔗 Documentation").clicked() {
                            // Open documentation URL
                            ui.close_menu();
                        }
                    });
                });
            });
        });
    }

    fn show_sidebar(&mut self, ctx: &egui::Context) {
        // Get the window fill color to use consistently
        let window_fill = ctx.style().visuals.window_fill();
        
        // Configure sidebar panel with proper resizing and clean appearance
        let sidebar_response = egui::SidePanel::left("sidebar")
            .default_width(self.sidebar_width)  // Use default_width to allow proper layout
            .min_width(150.0)                   // Set minimum width
            .max_width(400.0)                   // Set maximum width
            .resizable(true)                    // Keep resizable but control the handle
            .show_separator_line(false)         // Disable the separator line to eliminate visual gap
            .frame(egui::Frame::default()
                .fill(window_fill)              // Use window fill color directly
                .inner_margin(egui::Margin {
                    left: 6,
                    right: 6,                 // Reduce content spacing
                    top: 6,
                    bottom: 6,
                })
                .outer_margin(egui::Margin::ZERO)       // No outer margin
                .stroke(egui::Stroke::NONE)             // No border stroke
                .shadow(egui::epaint::Shadow::NONE)     // No shadow
                .corner_radius(egui::CornerRadius::ZERO)         // No rounding
            )
            .show(ctx, |ui| {
                // Make sure the entire UI uses consistent background
                ui.visuals_mut().panel_fill = window_fill;
                ui.visuals_mut().widgets.noninteractive.bg_fill = window_fill;
                ui.visuals_mut().extreme_bg_color = window_fill;

                // Use a vertical layout with proper spacing
                egui::ScrollArea::vertical()
                    .id_salt("sidebar_scroll")
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.set_width(ui.available_width());
                            // Trading Actions Section
                            // Use a custom frame instead of group for better control
                            egui::Frame::default()
                                .fill(ui.visuals().faint_bg_color)
                                .inner_margin(egui::Margin::same(6))
                                .corner_radius(egui::CornerRadius::same(4))
                                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.vertical(|ui| {
                        ui.heading("🎯 Trading Actions");
                        ui.add_space(2.0);

                        // Full width buttons
                        let button_height = 28.0;
                        if ui
                            .add_sized(
                                [ui.available_width(), button_height],
                                egui::Button::new("🛒 New Order"),
                            )
                            .clicked()
                        {
                            self.show_new_order_dialog = true;
                        }

                        if ui
                            .add_sized(
                                [ui.available_width(), button_height],
                                egui::Button::new("📋 View Orders"),
                            )
                            .clicked()
                        {
                            self.add_pane(Pane::Orders);
                        }

                        if ui
                            .add_sized(
                                [ui.available_width(), button_height],
                                egui::Button::new("💼 Portfolio"),
                            )
                            .clicked()
                        {
                            self.add_pane(Pane::Portfolio);
                        }
                    });
                });

                ui.add_space(8.0);  // Spacing between sections

                // Market Data Section
                egui::Frame::default()
                    .fill(ui.visuals().faint_bg_color)
                    .inner_margin(egui::Margin::same(8))
                    .corner_radius(egui::CornerRadius::same(4))
                    .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.vertical(|ui| {
                        ui.heading("📊 Market Data");
                        ui.add_space(2.0);

                        let button_height = 28.0;
                        if ui
                            .add_sized(
                                [ui.available_width(), button_height],
                                egui::Button::new("📈 Add Chart"),
                            )
                            .clicked()
                        {
                            self.add_pane(Pane::Charts);
                        }

                        if ui
                            .add_sized(
                                [ui.available_width(), button_height],
                                egui::Button::new("📊 Order Book"),
                            )
                            .clicked()
                        {
                            self.add_pane(Pane::MarketDepth(None));
                        }

                        if ui
                            .add_sized(
                                [ui.available_width(), button_height],
                                egui::Button::new("📜 Trade History"),
                            )
                            .clicked()
                        {
                            self.add_pane(Pane::TradeHistory);
                        }
                    });
                });

                ui.add_space(8.0);  // Spacing between sections

                // Connection Management Section
                egui::Frame::default()
                    .fill(ui.visuals().faint_bg_color)
                    .inner_margin(egui::Margin::same(8))
                    .corner_radius(egui::CornerRadius::same(4))
                    .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.vertical(|ui| {
                        ui.heading("🔌 Connection Management");
                        ui.add_space(2.0);

                        // Quick connection stats from cached data
                        if let Some(_streaming_service) = &self.streaming_service {
                            // Use a fixed-height horizontal layout to prevent resizing issues
                            ui.horizontal(|ui| {
                                ui.set_min_height(20.0); // Set minimum height to prevent collapse
                                if let Some(stats) = &self.cached_streaming_stats {
                                    // Calculate active streams from token activities with time-based statistics
                                    let token_activities =
                                        if let Ok(activities) = self.token_activities.try_read() {
                                            activities.clone()
                                        } else {
                                            HashMap::new()
                                        };

                                    // Active streams in last 5 minutes (300 seconds)
                                    let active_streams_5m = token_activities
                                        .values()
                                        .filter(|activity| {
                                            if let Some(last_update) = activity.last_update {
                                                last_update.elapsed().as_secs() <= 300
                                            } else {
                                                false
                                            }
                                        })
                                        .count();

                                    // All-time active streams (tokens that have ever received events)
                                    let active_streams_all = token_activities
                                        .values()
                                        .filter(|activity| activity.event_count > 0)
                                        .count();

                                    let total_streams = token_activities.len();

                                    // Use allocate_space to ensure consistent layout
                                    ui.label(format!(
                                        "🔗 Connections: {}",
                                        stats.active_connections
                                    ));
                                    ui.allocate_space(egui::vec2(0.0, 0.0)); // Prevent layout shift
                                    ui.separator();
                                    
                                    // Use monospace font for stream stats to ensure consistent width
                                    ui.label(format!(
                                        "📊 Streams: {:>3} active (5m) / {:>3} active (all) / {:>3} total",
                                        active_streams_5m, active_streams_all, total_streams
                                    ));
                                    ui.allocate_space(egui::vec2(0.0, 0.0)); // Prevent layout shift
                                    ui.separator();
                                    
                                    ui.label(format!("📈 {:>5.1}/s", stats.events_per_second));
                                } else {
                                    // Reserve space for the loading state to match the loaded state
                                    ui.colored_label(egui::Color32::GRAY, "⏳ Loading...                                                    ");
                                }
                            });
                        } else {
                            ui.horizontal(|ui| {
                                ui.set_min_height(20.0); // Set minimum height to prevent collapse
                                ui.colored_label(egui::Color32::GRAY, "🔗 No connections");
                                ui.separator();
                                ui.colored_label(egui::Color32::GRAY, "📊 Streams: 0 active (5m) / 0 active (all) / 0 total");
                                ui.separator();
                                ui.colored_label(egui::Color32::GRAY, "📈  0.0/s");
                            });
                        }

                        ui.add_space(2.0);

                        let button_height = 28.0;
                        if ui
                            .add_sized(
                                [ui.available_width(), button_height],
                                egui::Button::new("🔌 WebSocket Manager"),
                            )
                            .clicked()
                        {
                            self.add_pane(Pane::WebSocketManager);
                        }

                        if ui
                            .add_sized(
                                [ui.available_width(), button_height],
                                egui::Button::new("📡 All Streams"),
                            )
                            .clicked()
                        {
                            self.add_pane(Pane::Streams);
                        }
                    });
                });

                ui.add_space(8.0);  // Spacing between sections

                // Streaming Status Section
                egui::Frame::default()
                    .fill(ui.visuals().faint_bg_color)
                    .inner_margin(egui::Margin::same(8))
                    .corner_radius(egui::CornerRadius::same(4))
                    .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.vertical(|ui| {
                        ui.heading("📡 Streaming Status");
                        ui.add_space(2.0);

                        // Status indicator with frame
                        ui.horizontal(|ui| match &self.streaming_state {
                            StreamingState::Connected => {
                                ui.colored_label(
                                    egui::Color32::from_rgb(100, 200, 100),
                                    "● Connected",
                                );
                            }
                            StreamingState::Initializing { .. } => {
                                ui.colored_label(
                                    egui::Color32::from_rgb(255, 200, 100),
                                    "● Initializing",
                                );
                            }
                            StreamingState::Disconnected => {
                                ui.colored_label(
                                    egui::Color32::from_rgb(200, 100, 100),
                                    "● Disconnected",
                                );
                            }
                            StreamingState::Error(_) => {
                                ui.colored_label(egui::Color32::from_rgb(255, 50, 50), "● Error");
                            }
                        });

                        // Streaming info
                        match &self.streaming_state {
                            StreamingState::Connected => {
                                ui.label(format!("Assets: {}", self.streaming_assets.len()));

                                let button_height = 26.0;
                                if ui
                                    .add_sized(
                                        [ui.available_width(), button_height],
                                        egui::Button::new("⏹️ Stop Streaming"),
                                    )
                                    .clicked()
                                {
                                    self.stop_streaming();
                                }
                            }
                            StreamingState::Initializing { progress, message } => {
                                ui.label(message);
                                ui.add(egui::ProgressBar::new(*progress).show_percentage());
                            }
                            StreamingState::Disconnected => {
                                let button_height = 26.0;
                                if ui
                                    .add_sized(
                                        [ui.available_width(), button_height],
                                        egui::Button::new("▶️ Start Streaming"),
                                    )
                                    .clicked()
                                {
                                    self.show_dataset_selector = true;
                                }

                                if ui
                                    .add_sized(
                                        [ui.available_width(), button_height],
                                        egui::Button::new("🚀 Quick Start"),
                                    )
                                    .clicked()
                                {
                                    self.quick_start_streaming();
                                }
                            }
                            StreamingState::Error(error) => {
                                ui.label(format!("Error: {}", error));

                                let button_height = 26.0;
                                if ui
                                    .add_sized(
                                        [ui.available_width(), button_height],
                                        egui::Button::new("🔄 Retry"),
                                    )
                                    .clicked()
                                {
                                    self.show_dataset_selector = true;
                                }
                            }
                        }
                    });
                });

                ui.add_space(8.0);  // Spacing between sections

                // Statistics Section
                egui::Frame::default()
                    .fill(ui.visuals().faint_bg_color)
                    .inner_margin(egui::Margin::same(8))
                    .corner_radius(egui::CornerRadius::same(4))
                    .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.vertical(|ui| {
                        ui.heading("📊 Statistics");
                        ui.add_space(2.0);

                        // Order count
                        let order_count = if let Ok(orders) = self.orders_cache.try_read() {
                            orders.len()
                        } else {
                            0
                        };

                        ui.horizontal(|ui| {
                            ui.label("Active Orders:");
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.strong(format!("{}", order_count));
                                },
                            );
                        });

                        // Active panes count
                        let pane_count = self.get_active_panes().len();
                        ui.horizontal(|ui| {
                            ui.label("Open Panes:");
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.strong(format!("{}", pane_count));
                                },
                            );
                        });

                        if let Some(_streaming_service) = &self.streaming_service {
                            // Use cached token count instead of blocking async call
                            let orderbook_count = self.cached_streaming_tokens.len();
                            ui.horizontal(|ui| {
                                ui.label("Order Books:");
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.strong(format!("{}", orderbook_count));
                                    },
                                );
                            });
                        }
                    });
                });

                ui.add_space(8.0);  // Spacing between sections

                // Layout Management Section
                egui::Frame::default()
                    .fill(ui.visuals().faint_bg_color)
                    .inner_margin(egui::Margin::same(8))
                    .corner_radius(egui::CornerRadius::same(4))
                    .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.vertical(|ui| {
                        ui.heading("🔧 Layout Management");
                        ui.add_space(2.0);

                        let button_height = 28.0;
                        
                        // Auto-arrange checkbox
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut self.auto_arrange_on_add, "Auto-arrange on add")
                                .on_hover_text("Automatically arrange panes when adding new ones");
                        });
                        ui.add_space(2.0);
                        
                        // Auto-arrange button
                        if ui
                            .add_sized(
                                [ui.available_width(), button_height],
                                egui::Button::new("🔧 Auto Arrange")
                                    .fill(egui::Color32::from_rgb(40, 80, 120)), // Steel blue color
                            )
                            .on_hover_text("Automatically arrange all panes in an optimal grid layout")
                            .clicked()
                        {
                            self.auto_arrange_tiles();
                        }

                        // Reset layout button
                        if ui
                            .add_sized(
                                [ui.available_width(), button_height],
                                egui::Button::new("🔄 Reset Layout"),
                            )
                            .on_hover_text("Reset to default layout")
                            .clicked()
                        {
                            self.tree = Self::create_default_layout();
                            self.focused_tile_id = None;
                            self.has_unsaved_layout_changes = true;
                            info!("Layout reset to default");
                        }

                        ui.add_space(2.0);

                        // Save layout button
                        let save_button_text = if self.has_unsaved_layout_changes {
                            "💾 Save Layout*"
                        } else {
                            "💾 Save Layout"
                        };
                        
                        let save_button_color = if self.has_unsaved_layout_changes {
                            egui::Color32::from_rgb(150, 100, 30) // Orange/amber color for unsaved changes
                        } else {
                            egui::Color32::from_rgb(30, 85, 35) // Green color when saved
                        };
                        
                        let hover_text = if self.has_unsaved_layout_changes {
                            "Save current layout to file (unsaved changes)"
                        } else {
                            "Save current layout to file"
                        };
                        
                        if ui
                            .add_sized(
                                [ui.available_width(), button_height],
                                egui::Button::new(save_button_text)
                                    .fill(save_button_color),
                            )
                            .on_hover_text(hover_text)
                            .clicked()
                        {
                            self.save_layout_to_file();
                        }

                        // Load layout button
                        if ui
                            .add_sized(
                                [ui.available_width(), button_height],
                                egui::Button::new("📂 Load Layout")
                                    .fill(egui::Color32::from_rgb(30, 60, 100)), // Darker blue for better contrast
                            )
                            .on_hover_text("Load layout from file")
                            .clicked()
                        {
                            self.load_layout_from_file();
                        }

                        // Screenshot button
                        if ui
                            .add_sized(
                                [ui.available_width(), button_height],
                                egui::Button::new("📷 Screenshot")
                                    .fill(egui::Color32::from_rgb(80, 30, 140)), // Blue violet color
                            )
                            .on_hover_text("Save screenshot to screenshots/ directory")
                            .clicked()
                        {
                            self.take_screenshot(ctx);
                        }
                    });
                });
                        }); // Close ui.vertical
                    }); // Close scroll area

                // Fill remaining space and add exit button at bottom
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.add_space(8.0);  // Spacing between sections

                    // Exit button at the very bottom
                    let button_height = 40.0;
                    if ui
                        .add_sized(
                            [ui.available_width() * 0.9, button_height],
                            egui::Button::new("🚪 Exit Application")
                                .fill(egui::Color32::from_rgb(120, 40, 40)),
                        )
                        .on_hover_text("Exit Polybot Trading Application")
                        .clicked()
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        info!("🚪 User clicked exit button");
                    }

                    ui.add_space(8.0);  // Spacing between sections
                    ui.separator();
                    ui.hyperlink_to("📖 Help", "https://docs.polymarket.com");
                    ui.add_space(4.0);
                });
            });
        
        // Update the sidebar width if it was resized
        let rect = sidebar_response.response.rect;
        let new_width = rect.width();
        if (new_width - self.sidebar_width).abs() > 1.0 {  // Only update if significantly different
            self.sidebar_width = new_width;
        }
    }

    fn get_active_panes(&self) -> Vec<(TileId, &Pane)> {
        self.tree
            .tiles
            .iter()
            .filter_map(|(id, tile)| {
                if tile.is_pane() {
                    // SAFETY: We know this is a pane from is_pane() check
                    if let egui_tiles::Tile::Pane(pane) = tile {
                        Some((*id, pane))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    }

    fn add_pane(&mut self, pane_type: Pane) {
        // First check if this pane type already exists and focus it instead
        if let Some(existing_tile_id) = self.find_existing_pane(&pane_type) {
            self.focus_pane(existing_tile_id);
            info!("Focused existing {} pane instead of creating new one", pane_type.title());
            return;
        }

        let pane_id = self.tree.tiles.insert_pane(pane_type.clone());

        // Use smart placement based on focused tile
        if let Some(focused_id) = self.focused_tile_id {
            // Try to split intelligently from the focused tile
            if self.split_tile_intelligently(focused_id, pane_id) {
                info!(
                    "Added {} pane by intelligently splitting from focused tile",
                    pane_type.title()
                );
                return;
            }
        }

        // Fallback: find a suitable tile to split or create at root
        if let Some(root) = self.tree.root {
            if self.split_tile_intelligently(root, pane_id) {
                info!("Added {} pane by splitting from root", pane_type.title());
                return;
            }

            // Last resort: create a new linear split at root level
            let new_root = self.tree.tiles.insert_horizontal_tile(vec![root, pane_id]);
            self.tree.root = Some(new_root);
        } else {
            // No root, set this as root
            self.tree.root = Some(pane_id);
        }

        info!("Added {} pane to workspace", pane_type.title());
        
        // Mark layout as having unsaved changes
        self.has_unsaved_layout_changes = true;

        // Auto-arrange if enabled
        if self.auto_arrange_on_add {
            self.auto_arrange_tiles();
            info!("Auto-arranged tiles after adding new pane");
        }
    }
    

    /// Split a tile intelligently based on aspect ratio - never creates tabs
    fn split_tile_intelligently(
        &mut self,
        target_tile_id: egui_tiles::TileId,
        new_pane_id: egui_tiles::TileId,
    ) -> bool {
        // Check if the target tile exists
        if self.tree.tiles.get(target_tile_id).is_none() {
            return false;
        }

        // Get the target tile's current rectangular bounds to determine aspect ratio
        let should_split_vertically = self.should_split_vertically(target_tile_id);

        // If the target tile is the root, create a new split as root
        if self.tree.root == Some(target_tile_id) {
            let new_split = if should_split_vertically {
                self.tree
                    .tiles
                    .insert_vertical_tile(vec![target_tile_id, new_pane_id])
            } else {
                self.tree
                    .tiles
                    .insert_horizontal_tile(vec![target_tile_id, new_pane_id])
            };
            self.tree.root = Some(new_split);
            return true;
        }

        // Find the parent container and replace the target tile with a split
        if let Some(parent_id) = self.find_parent_container(target_tile_id) {
            // Create the new split first
            let new_split = if should_split_vertically {
                self.tree
                    .tiles
                    .insert_vertical_tile(vec![target_tile_id, new_pane_id])
            } else {
                self.tree
                    .tiles
                    .insert_horizontal_tile(vec![target_tile_id, new_pane_id])
            };

            // Replace the target tile with the new split in the parent container
            if let Some(egui_tiles::Tile::Container(parent)) = self.tree.tiles.get_mut(parent_id) {
                // Collect children first to avoid borrow checker issues
                let children: Vec<egui_tiles::TileId> = parent.children().copied().collect();
                for child_id in children {
                    if child_id == target_tile_id {
                        // Remove the old child and add the new split
                        parent.remove_child(child_id);
                        parent.add_child(new_split);
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Determine if a tile should be split vertically based on aspect ratio
    /// Returns true for vertical split (side-by-side), false for horizontal split (top-bottom)
    fn should_split_vertically(&self, _tile_id: egui_tiles::TileId) -> bool {
        // For now, we'll use a simple heuristic:
        // Default to vertical splitting (side-by-side) as it's often more useful for trading UIs
        // In a more sophisticated implementation, we could track actual tile dimensions
        true
    }

    /// Find the parent container of a given tile
    fn find_parent_container(
        &self,
        target_tile_id: egui_tiles::TileId,
    ) -> Option<egui_tiles::TileId> {
        // Search through all containers to find which one contains the target tile
        for (&container_id, tile) in self.tree.tiles.iter() {
            if let egui_tiles::Tile::Container(container) = tile {
                // Check if this container contains the target tile
                for &child_id in container.children() {
                    if child_id == target_tile_id {
                        return Some(container_id);
                    }
                }
            }
        }
        None
    }

    /// Find existing pane of the same type to focus instead of creating duplicates
    fn find_existing_pane(&self, pane_type: &Pane) -> Option<egui_tiles::TileId> {
        self.tree.tiles
            .iter()
            .find_map(|(tile_id, tile)| {
                if let egui_tiles::Tile::Pane(existing_pane) = tile {
                    // Check if panes are the same type
                    if self.panes_are_same_type(existing_pane, pane_type) {
                        Some(tile_id)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .copied()
    }

    /// Check if two panes are of the same type (for focusing existing panes)
    fn panes_are_same_type(&self, pane1: &Pane, pane2: &Pane) -> bool {
        match (pane1, pane2) {
            (Pane::Orders, Pane::Orders) => true,
            (Pane::Streams, Pane::Streams) => true,
            (Pane::Portfolio, Pane::Portfolio) => true,
            (Pane::Tokens, Pane::Tokens) => true,
            (Pane::Charts, Pane::Charts) => true,
            (Pane::TradeHistory, Pane::TradeHistory) => true,
            (Pane::Balances, Pane::Balances) => true,
            (Pane::WebSocketManager, Pane::WebSocketManager) => true,
            // MarketDepth and WorkerDetails can have multiple instances with different parameters
            (Pane::MarketDepth(_), Pane::MarketDepth(_)) => false,
            (Pane::WorkerDetails(_), Pane::WorkerDetails(_)) => false,
            _ => false,
        }
    }

    /// Focus an existing pane by setting it as the focused tile
    fn focus_pane(&mut self, tile_id: egui_tiles::TileId) {
        self.focused_tile_id = Some(tile_id);
        info!("Focused existing pane with tile_id: {:?}", tile_id);
    }

    /// Auto-arrange all tiles in an optimal grid layout
    fn auto_arrange_tiles(&mut self) {
        info!("Auto-arranging tiles for optimal layout");
        
        // First, check if we have any pinned tiles
        if !self.pinned_tiles.is_empty() {
            info!("Found {} pinned tiles that will preserve their position and size", self.pinned_tiles.len());
        }
        
        // Collect all existing unpinned panes (tiles that are marked as Pane and not pinned)
        let unpinned_panes: Vec<(TileId, Pane)> = self.tree.tiles
            .iter()
            .filter_map(|(tile_id, tile)| {
                if let egui_tiles::Tile::Pane(pane) = tile {
                    // Skip pinned tiles completely - we won't touch them
                    if !self.pinned_tiles.contains(tile_id) {
                        Some((*tile_id, pane.clone()))
                    } else {
                        info!("Preserving pinned tile {:?} during auto-arrange", tile_id);
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        
        if unpinned_panes.is_empty() {
            warn!("No unpinned panes to arrange");
            return;
        }
        
        info!("Found {} unpinned panes to arrange", unpinned_panes.len());
        
        // Instead of clearing the entire tree, we'll selectively remove only unpinned tiles
        // First, collect the IDs of tiles to remove (unpinned panes and their parent containers if empty)
        let mut tiles_to_remove: HashSet<TileId> = HashSet::new();
        
        // Mark unpinned panes for removal
        for (tile_id, _) in &unpinned_panes {
            tiles_to_remove.insert(*tile_id);
        }
        
        // Find and mark containers that will become empty after removing unpinned panes
        let mut containers_to_check: Vec<TileId> = Vec::new();
        for (tile_id, tile) in self.tree.tiles.iter() {
            if let egui_tiles::Tile::Container(_container) = tile {
                containers_to_check.push(*tile_id);
            }
        }
        
        // Check containers and mark empty ones for removal (but NOT if they contain pinned tiles)
        let mut changed = true;
        while changed {
            changed = false;
            let mut new_containers_to_remove = Vec::new();
            
            for container_id in &containers_to_check {
                if let Some(egui_tiles::Tile::Container(container)) = self.tree.tiles.get(*container_id) {
                    let mut has_pinned = false;
                    let mut has_non_removed = false;
                    
                    for child_id in container.children() {
                        if self.pinned_tiles.contains(child_id) {
                            has_pinned = true;
                            break;
                        }
                        if !tiles_to_remove.contains(child_id) {
                            has_non_removed = true;
                        }
                    }
                    
                    // Only mark for removal if it has no pinned tiles and all children will be removed
                    if !has_pinned && !has_non_removed && !tiles_to_remove.contains(container_id) {
                        new_containers_to_remove.push(*container_id);
                        changed = true;
                    }
                }
            }
            
            for id in new_containers_to_remove {
                tiles_to_remove.insert(id);
            }
        }
        
        // Remove the tiles marked for removal
        for tile_id in &tiles_to_remove {
            self.tree.tiles.remove(*tile_id);
        }
        
        // Now create new tiles for the unpinned panes with optimal arrangement
        let panes_to_arrange: Vec<Pane> = unpinned_panes.into_iter().map(|(_, pane)| pane).collect();
        
        // Always ensure Streams pane is included if not present
        let mut arranged_panes = panes_to_arrange;
        if !arranged_panes.iter().any(|p| matches!(p, Pane::Streams)) {
            arranged_panes.insert(0, Pane::Streams);
        }
        
        // Sort panes by category and priority for intelligent clustering
        arranged_panes.sort_by(|a, b| {
            let category_a = self.get_pane_category(a);
            let category_b = self.get_pane_category(b);
            let priority_a = self.get_pane_priority(a);
            let priority_b = self.get_pane_priority(b);
            
            // First sort by category, then by priority within category
            category_a.cmp(&category_b).then(priority_a.cmp(&priority_b))
        });
        
        // Create new tiles for the arranged panes
        let mut new_tile_ids: Vec<TileId> = Vec::new();
        for pane in arranged_panes {
            let tile_id = self.tree.tiles.insert_pane(pane);
            new_tile_ids.push(tile_id);
        }
        
        // Now we need to intelligently insert these new tiles into the existing structure
        // If we have a root and it's not removed, try to add to it
        if let Some(root_id) = self.tree.root {
            if self.tree.tiles.get(root_id).is_some() {
                // Try to add the new arrangement to the existing root
                self.add_arranged_tiles_to_existing_structure(root_id, new_tile_ids);
            } else {
                // Root was removed, create a new layout structure
                self.create_new_layout_structure(new_tile_ids);
            }
        } else {
            // No root, create a new layout structure
            self.create_new_layout_structure(new_tile_ids);
        }
        
        // Clear focused tile since layout changed
        self.focused_tile_id = None;
        
        // Mark layout as having unsaved changes
        self.has_unsaved_layout_changes = true;
        
        info!("Auto-arrangement complete");
    }

    /// Get the category of a pane for intelligent clustering
    fn get_pane_category(&self, pane: &Pane) -> u8 {
        match pane {
            // Trading group - highest priority
            Pane::Orders | Pane::Portfolio | Pane::Balances => 1,
            // Market data group - medium priority  
            Pane::Streams | Pane::MarketDepth(_) | Pane::Charts | Pane::TradeHistory => 2,
            // Token and utility group
            Pane::Tokens => 3,
            // Management group - lowest priority
            Pane::WebSocketManager | Pane::WorkerDetails(_) => 4,
        }
    }

    /// Get the priority of a pane within its category (lower number = higher priority)
    fn get_pane_priority(&self, pane: &Pane) -> u8 {
        match pane {
            // Trading group priorities
            Pane::Orders => 1,
            Pane::Portfolio => 2,
            Pane::Balances => 3,
            
            // Market data group priorities
            Pane::Streams => 1,
            Pane::MarketDepth(_) => 2,
            Pane::Charts => 3,
            Pane::TradeHistory => 4,
            
            // Token group
            Pane::Tokens => 1,
            
            // Management group priorities
            Pane::WebSocketManager => 1,
            Pane::WorkerDetails(_) => 2,
        }
    }
    
    /// Calculate optimal grid dimensions for a given number of panes
    fn calculate_optimal_grid(&self, pane_count: usize) -> (usize, usize) {
        if pane_count == 0 {
            return (0, 0);
        }
        
        if pane_count == 1 {
            return (1, 1);
        }
        
        if pane_count == 2 {
            return (1, 2); // Side by side
        }
        
        if pane_count == 3 {
            return (1, 3); // Three columns
        }
        
        if pane_count == 4 {
            return (2, 2); // 2x2 grid
        }
        
        if pane_count <= 6 {
            return (2, 3); // 2x3 grid
        }
        
        if pane_count <= 8 {
            return (2, 4); // 2x4 grid
        }
        
        if pane_count <= 9 {
            return (3, 3); // 3x3 grid
        }
        
        if pane_count <= 12 {
            return (3, 4); // 3x4 grid
        }
        
        // For larger numbers, try to get close to square
        let sqrt_count = (pane_count as f64).sqrt();
        let rows = sqrt_count.ceil() as usize;
        let cols = (pane_count as f64 / rows as f64).ceil() as usize;
        
        (rows, cols)
    }

    /// Add arranged tiles to existing tree structure, working around pinned tiles
    fn add_arranged_tiles_to_existing_structure(&mut self, root_id: TileId, new_tile_ids: Vec<TileId>) {
        if new_tile_ids.is_empty() {
            return;
        }
        
        // Calculate optimal grid for the new tiles
        let (rows, cols) = self.calculate_optimal_grid(new_tile_ids.len());
        
        // Create the arrangement structure for new tiles
        let arranged_root = if rows == 1 && cols == 1 {
            // Single tile
            new_tile_ids[0]
        } else if rows == 1 {
            // Single row - create horizontal container
            self.tree.tiles.insert_horizontal_tile(new_tile_ids)
        } else if cols == 1 {
            // Single column - create vertical container
            self.tree.tiles.insert_vertical_tile(new_tile_ids)
        } else {
            // Multi-row grid - create rows of columns
            let mut row_containers = Vec::new();
            
            for row in 0..rows {
                let start_idx = row * cols;
                let end_idx = std::cmp::min(start_idx + cols, new_tile_ids.len());
                
                if start_idx < new_tile_ids.len() {
                    let row_tiles = new_tile_ids[start_idx..end_idx].to_vec();
                    let row_container = self.tree.tiles.insert_horizontal_tile(row_tiles);
                    row_containers.push(row_container);
                }
            }
            
            self.tree.tiles.insert_vertical_tile(row_containers)
        };
        
        // Now we need to add this arranged structure to the existing tree
        // Try to find a suitable place in the existing structure
        if let Some(egui_tiles::Tile::Container(root_container)) = self.tree.tiles.get(root_id).cloned() {
            // If root is a container, we can try to add to it
            let children: Vec<&TileId> = root_container.children().collect();
            if children.len() == 1 && !self.pinned_tiles.contains(children[0]) {
                // Replace single unpinned child with new arrangement
                // Since we can't modify children directly, we'll replace the whole tile
                let kind = root_container.kind();
                let new_container_id = match kind {
                    egui_tiles::ContainerKind::Horizontal => {
                        self.tree.tiles.insert_horizontal_tile(vec![arranged_root])
                    }
                    egui_tiles::ContainerKind::Vertical => {
                        self.tree.tiles.insert_vertical_tile(vec![arranged_root])
                    }
                    _ => {
                        self.tree.tiles.insert_horizontal_tile(vec![arranged_root])
                    }
                };
                // Replace the root with the new container
                self.tree.tiles.remove(root_id);
                self.tree.root = Some(new_container_id);
            } else {
                // Create a new container that includes existing root and new arrangement
                let new_root = match root_container.kind() {
                    egui_tiles::ContainerKind::Horizontal => {
                        self.tree.tiles.insert_horizontal_tile(vec![root_id, arranged_root])
                    }
                    egui_tiles::ContainerKind::Vertical => {
                        self.tree.tiles.insert_vertical_tile(vec![root_id, arranged_root])
                    }
                    _ => {
                        // For tabs or grids, default to horizontal
                        self.tree.tiles.insert_horizontal_tile(vec![root_id, arranged_root])
                    }
                };
                self.tree.root = Some(new_root);
            }
        } else {
            // Root is a pane, create a new container
            let new_root = self.tree.tiles.insert_horizontal_tile(vec![root_id, arranged_root]);
            self.tree.root = Some(new_root);
        }
    }
    
    /// Create a completely new layout structure for the given tiles
    fn create_new_layout_structure(&mut self, tile_ids: Vec<TileId>) {
        if tile_ids.is_empty() {
            return;
        }
        
        let (rows, cols) = self.calculate_optimal_grid(tile_ids.len());
        
        if rows == 1 && cols == 1 {
            // Single tile becomes root
            self.tree.root = Some(tile_ids[0]);
        } else if rows == 1 {
            // Single row - create horizontal container
            let root_id = self.tree.tiles.insert_horizontal_tile(tile_ids);
            self.tree.root = Some(root_id);
        } else if cols == 1 {
            // Single column - create vertical container
            let root_id = self.tree.tiles.insert_vertical_tile(tile_ids);
            self.tree.root = Some(root_id);
        } else {
            // Multi-row grid - create rows of columns
            let mut row_containers = Vec::new();
            
            for row in 0..rows {
                let start_idx = row * cols;
                let end_idx = std::cmp::min(start_idx + cols, tile_ids.len());
                
                if start_idx < tile_ids.len() {
                    let row_tiles = tile_ids[start_idx..end_idx].to_vec();
                    let row_container = self.tree.tiles.insert_horizontal_tile(row_tiles);
                    row_containers.push(row_container);
                }
            }
            
            if !row_containers.is_empty() {
                let root_id = self.tree.tiles.insert_vertical_tile(row_containers);
                self.tree.root = Some(root_id);
            }
        }
    }

    fn refresh_orders(&mut self) {
        info!("Orders are updated automatically via WebSocket stream");
        // With WebSocket-based portfolio service, orders update automatically
        // No manual refresh needed - data comes from streaming events
    }

    fn stop_streaming(&mut self) {
        info!("Stopping streaming...");

        // Cancel the streaming task if it exists
        if let Some(task) = self.streaming_task.take() {
            task.abort();
        }

        // Reset state
        self.streaming_state = StreamingState::Disconnected;
        self.streaming_assets.clear();
        self.streaming_service = None;
    }


    fn load_available_datasets(data_paths: &DataPaths) -> Vec<DatasetInfo> {
        let mut datasets = Vec::new();

        // Create a dataset manager with default config
        let config = DatasetManagerConfig {
            base_dir: data_paths.datasets(),
            ..Default::default()
        };

        let mut manager = DatasetManager::new(config);

        // Scan for datasets
        if let Err(e) = manager.scan_datasets() {
            warn!("Failed to scan datasets: {}", e);
            return datasets;
        }

        // Convert dataset info to our local struct
        for dataset in manager.get_datasets() {
            // Count assets by looking for market chunks or markets.json files
            let asset_count = dataset
                .files
                .iter()
                .filter(|f| f.name.contains("markets") || f.name.contains("chunk"))
                .count();

            let size_mb = dataset.size_bytes as f64 / (1024.0 * 1024.0);

            datasets.push(DatasetInfo {
                name: dataset.name.clone(),
                path: dataset.path.clone(),
                asset_count,
                size_mb,
            });
        }

        info!("Found {} datasets", datasets.len());
        datasets
    }

    fn start_streaming_with_datasets(&mut self) {
        if self.selected_datasets.is_empty() {
            warn!("No datasets selected for streaming");
            return;
        }

        // Set initial state
        self.streaming_state = StreamingState::Initializing {
            progress: 0.0,
            message: "Loading dataset information...".to_string(),
        };

        // Clone necessary data for the async task
        let selected_datasets = self.selected_datasets.clone();
        let available_datasets = self.available_datasets.clone();
        let host = self.host.clone();
        let data_paths = self.data_paths.clone();

        // Spawn async task to initialize streaming
        let (progress_tx, progress_rx) = tokio::sync::mpsc::channel(10);

        // Create a channel to receive the initialized streamer
        let (streamer_tx, streamer_rx) = tokio::sync::oneshot::channel();

        let task = tokio::spawn(async move {
            match Self::initialize_streaming_async(
                selected_datasets,
                available_datasets,
                host,
                data_paths,
                progress_tx,
            )
            .await
            {
                Ok(streaming_service) => {
                    let _ = streamer_tx.send(Ok(streaming_service));
                }
                Err(e) => {
                    let _ = streamer_tx.send(Err(e));
                }
            }
        });

        self.streaming_task = Some(task);
        self.streaming_progress_rx = Some(progress_rx);
        self.streaming_result_rx = Some(streamer_rx);

        info!("Started streaming initialization task");
    }

    fn quick_start_streaming(&mut self) {
        info!("🚀 Quick Start streaming initiated");

        // Find the first available dataset with data
        if let Some(dataset) = self.available_datasets.first() {
            info!("Auto-selecting dataset: {}", dataset.name);

            // Clear any existing selections and select this dataset
            self.selected_datasets.clear();
            self.selected_datasets.insert(dataset.name.clone());

            // Start streaming with this dataset
            self.start_streaming_with_datasets();
        } else {
            warn!("No datasets available for quick start");
            self.streaming_state = StreamingState::Error(
                "No datasets available. Use CLI to fetch market data first.".to_string(),
            );
        }
    }

    async fn initialize_streaming_async(
        selected_datasets: std::collections::HashSet<String>,
        available_datasets: Vec<DatasetInfo>,
        host: String,
        data_paths: DataPaths,
        progress_tx: tokio::sync::mpsc::Sender<(f32, String)>,
    ) -> Result<Arc<StreamingService>, anyhow::Error> {
        let _ = progress_tx
            .send((0.1, "Collecting token IDs from datasets...".to_string()))
            .await;

        // Collect all token IDs from selected datasets
        let mut all_tokens = Vec::new();
        let total_datasets = selected_datasets.len();
        let mut processed = 0;

        for dataset_name in &selected_datasets {
            if let Some(dataset) = available_datasets.iter().find(|d| d.name == *dataset_name) {
                info!("Loading tokens from dataset: {}", dataset.name);

                // Load actual token IDs from the dataset files
                match Self::load_tokens_from_dataset(&dataset.path).await {
                    Ok(tokens) => {
                        info!(
                            "✅ Loaded {} tokens from dataset: {}",
                            tokens.len(),
                            dataset.name
                        );
                        all_tokens.extend(tokens);
                    }
                    Err(e) => {
                        error!(
                            "❌ Failed to load tokens from dataset {}: {}",
                            dataset.name, e
                        );
                        // Continue with other datasets even if one fails
                    }
                }

                processed += 1;
                let progress = 0.1 + (0.4 * processed as f32 / total_datasets as f32);
                let _ = progress_tx
                    .send((progress, format!("Loaded {} datasets...", processed)))
                    .await;
            }
        }

        if all_tokens.is_empty() {
            return Err(anyhow::anyhow!("No tokens found in selected datasets"));
        }

        // Remove duplicates
        all_tokens.sort();
        all_tokens.dedup();

        let _ = progress_tx
            .send((
                0.5,
                format!(
                    "Initializing streaming service with {} unique tokens...",
                    all_tokens.len()
                ),
            ))
            .await;

        // Create streaming service configuration
        let ws_config = WsConfig::default();

        let config = StreamingServiceConfig {
            ws_config,
            _host: host,
            _data_paths: data_paths,
            tokens_per_worker: 25, // Increased to reduce worker count
            event_buffer_size: 1000,
            worker_event_buffer_size: 500,
            auto_reconnect: true,
            reconnect_delay_ms: 2000,      // Longer initial delay
            max_reconnect_delay_ms: 60000, // Longer max delay
            max_reconnect_attempts: 5,     // Fewer attempts initially
            health_check_interval_secs: 30,
            stats_interval_secs: 5,
            worker_connection_delay_ms: 500, // 500ms delay between connections
            max_concurrent_connections: 2,   // Only 2 concurrent connections
        };

        let _ = progress_tx
            .send((0.7, "Starting streaming service...".to_string()))
            .await;

        info!(
            "🔌 Creating streaming service with {} tokens...",
            all_tokens.len()
        );

        // Create and start the streaming service
        let streaming_service = StreamingService::new(config);

        info!("🌐 Starting streaming service");
        match streaming_service.start().await {
            Ok(_) => {
                info!("✅ Streaming service started successfully");
            }
            Err(e) => {
                error!("❌ Streaming service failed to start: {}", e);
                return Err(anyhow::anyhow!("Streaming service failed to start: {}", e));
            }
        }

        let _ = progress_tx
            .send((0.8, "Adding tokens to streaming service...".to_string()))
            .await;

        // Add tokens to the streaming service
        match streaming_service.add_tokens(all_tokens).await {
            Ok(_) => {
                info!("✅ Tokens added to streaming service successfully");
            }
            Err(e) => {
                error!("❌ Failed to add tokens to streaming service: {}", e);
                return Err(anyhow::anyhow!("Failed to add tokens: {}", e));
            }
        }

        let _ = progress_tx
            .send((0.9, "Waiting for initial data...".to_string()))
            .await;

        // Wait a bit for initial data
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        let _ = progress_tx.send((1.0, "Connected!".to_string())).await;

        Ok(streaming_service)
    }

    fn poll_streaming_events(&mut self, ctx: &egui::Context) {
        // Only poll if we're connected and have a receiver
        if !matches!(self.streaming_state, StreamingState::Connected) {
            return;
        }

        // Collect events first to avoid borrow checker issues
        let mut events_to_process = Vec::new();
        let mut should_close_receiver = false;

        if let Some(receiver) = &mut self.event_receiver {
            let max_events_per_frame = 100;

            for _ in 0..max_events_per_frame {
                match receiver.try_recv() {
                    Ok(event) => {
                        events_to_process.push(event);
                    }
                    Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {
                        break;
                    }
                    Err(tokio::sync::broadcast::error::TryRecvError::Closed) => {
                        warn!("Event receiver closed");
                        should_close_receiver = true;
                        break;
                    }
                    Err(tokio::sync::broadcast::error::TryRecvError::Lagged(n)) => {
                        warn!("Event receiver lagged by {} messages", n);
                        continue;
                    }
                }
            }
        }

        // Process collected events
        let events_count = events_to_process.len();
        for event in events_to_process {
            self.handle_streaming_event(event);
        }

        // Close receiver if needed
        if should_close_receiver {
            self.event_receiver = None;
        }

        // Request repaint if we processed events
        if events_count > 0 {
            ctx.request_repaint();
        }

        // Update orderbook for selected token using cached data
        if let (Some(token_id), Some(_streaming_service)) =
            (&self.current_token_id, &self.streaming_service)
        {
            if let Some(order_book) = &self.cached_orderbook {
                let new_bids = order_book.get_bids().to_vec();
                let new_asks = order_book.get_asks().to_vec();

                // Track changes in bids
                for new_bid in &new_bids {
                    let changed = if let Some(old_bid) =
                        self.current_bids.iter().find(|b| b.price == new_bid.price)
                    {
                        old_bid.size != new_bid.size
                    } else {
                        true // New price level
                    };

                    if changed {
                        self.orderbook_changes.push(OrderBookChange {
                            token_id: token_id.clone(),
                            price: new_bid.price,
                            size: new_bid.size,
                            changed_at: Instant::now(),
                            is_bid: true,
                        });
                    }
                }

                // Track changes in asks
                for new_ask in &new_asks {
                    let changed = if let Some(old_ask) =
                        self.current_asks.iter().find(|a| a.price == new_ask.price)
                    {
                        old_ask.size != new_ask.size
                    } else {
                        true // New price level
                    };

                    if changed {
                        self.orderbook_changes.push(OrderBookChange {
                            token_id: token_id.clone(),
                            price: new_ask.price,
                            size: new_ask.size,
                            changed_at: Instant::now(),
                            is_bid: false,
                        });
                    }
                }

                // Update current state
                self.current_bids = new_bids;
                self.current_asks = new_asks;

                // Clean up old changes (older than 2 seconds)
                let now = Instant::now();
                self.orderbook_changes
                    .retain(|change| now.duration_since(change.changed_at).as_secs() < 2);
            }
        }

        // Handle pending new orderbook
        if let Some(token_id) = self.pending_new_orderbook.take() {
            // Create the new MarketDepth pane with the token ID
            let new_pane = Pane::MarketDepth(Some(token_id.clone()));
            self.add_pane(new_pane);

            // Set this as the current token for orderbook data fetching
            self.current_token_id = Some(token_id.clone());

            // Notify background task of current token change
            if let Some(sender) = &self.current_token_sender {
                let _ = sender.send(Some(token_id.clone()));
                info!("Updated current token to: {}", token_id);
            }

            info!("Created new orderbook pane for token: {}", token_id);

            // Request UI repaint to show the new pane immediately
            ctx.request_repaint();
        }

        // Handle pending new worker details
        if let Some(worker_id) = self.pending_new_worker_details.take() {
            // Create the new WorkerDetails pane with the worker ID
            let new_pane = Pane::WorkerDetails(worker_id);
            self.add_pane(new_pane);

            info!("Created new worker details pane for worker: {}", worker_id);

            // Request UI repaint to show the new pane immediately
            ctx.request_repaint();
        }

        // Request continuous updates while streaming
        if matches!(self.streaming_state, StreamingState::Connected) {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    /// Start background task to update cached streaming data
    fn start_data_update_task(&mut self) {
        if let Some(streaming_service) = &self.streaming_service {
            let streaming_service = Arc::clone(streaming_service);

            // Create channels for sending cached data back to UI thread
            let (tokens_tx, tokens_rx) = std::sync::mpsc::channel();
            let (orderbook_tx, orderbook_rx) = std::sync::mpsc::channel();
            let (trade_price_tx, trade_price_rx) = std::sync::mpsc::channel();
            let (stats_tx, stats_rx) = std::sync::mpsc::channel();
            let (workers_tx, workers_rx) = std::sync::mpsc::channel();

            // Create a channel for sending current token updates to background task
            let (current_token_tx, current_token_rx) = std::sync::mpsc::channel();

            // Store receivers for polling in update()
            self.cached_data_receivers = Some((
                tokens_rx,
                orderbook_rx,
                trade_price_rx,
                stats_rx,
                workers_rx,
            ));

            // Store the current token sender for updating the background task
            self.current_token_sender = Some(current_token_tx);

            // Send initial current token if any
            if let (Some(token_id), Some(sender)) =
                (&self.current_token_id, &self.current_token_sender)
            {
                let _ = sender.send(Some(token_id.clone()));
            }

            // Spawn background task
            let task = tokio::task::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
                let mut current_token_id: Option<String> = None;

                loop {
                    interval.tick().await;

                    // Check for current token updates
                    while let Ok(new_token) = current_token_rx.try_recv() {
                        current_token_id = new_token;
                    }

                    // Update streaming tokens
                    let tokens = streaming_service.get_streaming_tokens().await;
                    let _ = tokens_tx.send(tokens.clone());

                    // Update orderbook for current token if available
                    if let Some(token_id) = &current_token_id {
                        if let Some(orderbook) = streaming_service.get_order_book(token_id).await {
                            let _ = orderbook_tx.send(orderbook);
                        }

                        // Update last trade price
                        if let Some(trade_data) =
                            streaming_service.get_last_trade_price(token_id).await
                        {
                            let _ = trade_price_tx.send(trade_data);
                        }
                    }

                    // Update streaming stats
                    let stats = streaming_service.get_stats().await;
                    let _ = stats_tx.send(stats);

                    // Update worker statuses
                    let workers = streaming_service.get_worker_statuses().await;
                    let _ = workers_tx.send(workers);
                }
            });

            self._data_update_task = Some(task);
        }
    }

    /// Poll for cached data updates from background task
    fn poll_cached_data_updates(&mut self) {
        if let Some((tokens_rx, orderbook_rx, trade_price_rx, stats_rx, workers_rx)) =
            &self.cached_data_receivers
        {
            // Update cached streaming tokens
            if let Ok(tokens) = tokens_rx.try_recv() {
                self.cached_streaming_tokens = tokens;
                self.streaming_assets = self.cached_streaming_tokens.clone();
            }

            // Update cached orderbook
            if let Ok(orderbook) = orderbook_rx.try_recv() {
                self.cached_orderbook = Some(orderbook);
            }

            // Update cached last trade price
            if let Ok(trade_price) = trade_price_rx.try_recv() {
                self.cached_last_trade_price = Some(trade_price);
            }

            // Update cached streaming stats
            if let Ok(stats) = stats_rx.try_recv() {
                self.cached_streaming_stats = Some(stats);
            }

            // Update cached worker statuses
            if let Ok(workers) = workers_rx.try_recv() {
                self.cached_worker_statuses = workers;
            }
        }
    }

    fn handle_streaming_event(&mut self, event: PolyEvent) {
        // Note: Portfolio data is managed via HTTP refresh, not WebSocket events

        match &event {
            PolyEvent::PriceChange {
                asset_id,
                side,
                price,
                size,
                ..
            } => {
                if let Ok(mut activities) = self.token_activities.try_write() {
                    let activity =
                        activities
                            .entry(asset_id.clone())
                            .or_insert_with(|| TokenActivity {
                                token_id: asset_id.clone(),
                                event_count: 0,
                                last_bid: None,
                                last_ask: None,
                                last_update: None,
                                total_volume: Decimal::ZERO,
                                trade_count: 0,
                                last_messages_sent: 0,
                                messages_sorted: 0,
                                last_trade_price: None,
                                last_trade_timestamp: None,
                            });

                    activity.event_count += 1;
                    activity.last_update = Some(Instant::now());
                    activity.last_messages_sent += 1;

                    // Update bid/ask if size > 0
                    if *size > Decimal::ZERO {
                        match side {
                            Side::Buy => activity.last_bid = Some(*price),
                            Side::Sell => activity.last_ask = Some(*price),
                        }
                    }
                }
            }
            PolyEvent::Book {
                asset_id,
                bids,
                asks,
                ..
            } => {
                if let Ok(mut activities) = self.token_activities.try_write() {
                    let activity =
                        activities
                            .entry(asset_id.clone())
                            .or_insert_with(|| TokenActivity {
                                token_id: asset_id.clone(),
                                event_count: 0,
                                last_bid: None,
                                last_ask: None,
                                last_update: None,
                                total_volume: Decimal::ZERO,
                                trade_count: 0,
                                last_messages_sent: 0,
                                messages_sorted: 0,
                                last_trade_price: None,
                                last_trade_timestamp: None,
                            });

                    activity.event_count += 1;
                    activity.last_update = Some(Instant::now());
                    activity.messages_sorted += 1;

                    if let Some(level) = bids.first() {
                        activity.last_bid = Some(level.price);
                    }
                    if let Some(level) = asks.first() {
                        activity.last_ask = Some(level.price);
                    }
                }
            }
            PolyEvent::Trade {
                asset_id,
                price,
                size,
                ..
            } => {
                if let Ok(mut activities) = self.token_activities.try_write() {
                    let activity =
                        activities
                            .entry(asset_id.clone())
                            .or_insert_with(|| TokenActivity {
                                token_id: asset_id.clone(),
                                event_count: 0,
                                last_bid: None,
                                last_ask: None,
                                last_update: None,
                                total_volume: Decimal::ZERO,
                                trade_count: 0,
                                last_messages_sent: 0,
                                messages_sorted: 0,
                                last_trade_price: None,
                                last_trade_timestamp: None,
                            });

                    activity.event_count += 1;
                    activity.last_update = Some(Instant::now());
                    activity.trade_count += 1;
                    activity.total_volume += price * size;
                }
            }
            PolyEvent::LastTradePrice {
                asset_id,
                price,
                timestamp,
            } => {
                if let Ok(mut activities) = self.token_activities.try_write() {
                    let activity =
                        activities
                            .entry(asset_id.clone())
                            .or_insert_with(|| TokenActivity {
                                token_id: asset_id.clone(),
                                event_count: 0,
                                last_bid: None,
                                last_ask: None,
                                last_update: None,
                                total_volume: Decimal::ZERO,
                                trade_count: 0,
                                last_messages_sent: 0,
                                messages_sorted: 0,
                                last_trade_price: None,
                                last_trade_timestamp: None,
                            });

                    activity.event_count += 1;
                    activity.last_update = Some(Instant::now());
                    activity.last_trade_price = Some(*price);
                    activity.last_trade_timestamp = Some(*timestamp);
                }
            }
            _ => {}
        }
    }

    async fn load_tokens_from_dataset(
        dataset_path: &std::path::Path,
    ) -> Result<Vec<String>, anyhow::Error> {
        let mut tokens = Vec::new();

        // Look for markets.json files
        let markets_file = dataset_path.join("markets.json");
        if markets_file.exists() {
            let contents = tokio::fs::read_to_string(&markets_file).await?;
            let markets: Vec<serde_json::Value> = serde_json::from_str(&contents)?;

            for market in markets {
                if let Some(market_tokens) = market.get("tokens").and_then(|v| v.as_array()) {
                    for token in market_tokens {
                        if let Some(token_id) = token.get("token_id").and_then(|v| v.as_str()) {
                            tokens.push(token_id.to_string());
                        }
                    }
                }
            }
        }

        // Also look for market chunk files
        if let Ok(entries) = std::fs::read_dir(dataset_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.starts_with("markets_chunk_") && n.ends_with(".json"))
                        .unwrap_or(false)
                {
                    match tokio::fs::read_to_string(&path).await {
                        Ok(contents) => {
                            if let Ok(markets) =
                                serde_json::from_str::<Vec<serde_json::Value>>(&contents)
                            {
                                for market in markets {
                                    if let Some(market_tokens) =
                                        market.get("tokens").and_then(|v| v.as_array())
                                    {
                                        for token in market_tokens {
                                            if let Some(token_id) =
                                                token.get("token_id").and_then(|v| v.as_str())
                                            {
                                                tokens.push(token_id.to_string());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to read chunk file {:?}: {}", path, e);
                        }
                    }
                }
            }
        }

        Ok(tokens)
    }

    /// Initialize portfolio service if not already done
    fn init_portfolio_service_if_needed(&mut self) {
        // Check if we have a user address already
        if !self.portfolio_service.is_initialized_sync() {
            // Spawn async task to initialize
            let service = self.portfolio_service.clone();
            tokio::spawn(async move {
                if let Err(e) = service.init().await {
                    error!("Failed to initialize portfolio service: {}", e);
                } else {
                    info!("Portfolio service initialized successfully");
                    // Portfolio data will be updated automatically via WebSocket events
                }
            });
        }
    }

    fn show_dialogs(&mut self, ctx: &egui::Context) {
        // New Order Dialog
        if self.show_new_order_dialog {
            egui::Window::new("🛒 New Order")
                .default_width(400.0)
                .show(ctx, |ui| {
                    ui.label("Order placement functionality coming soon!");

                    ui.separator();

                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.show_new_order_dialog = false;
                        }

                        if ui.button("Place Order").clicked() {
                            // TODO: Implement order placement
                            self.show_new_order_dialog = false;
                        }
                    });
                });
        }

        // About Dialog
        if self.show_about {
            egui::Window::new("📖 About Polybot & Keyboard Shortcuts")
                .default_width(500.0)
                .show(ctx, |ui| {
                    ui.label("Polybot Trading Canvas v0.1.0");
                    ui.label("Advanced trading interface for Polymarket");

                    ui.separator();

                    ui.heading("⌨️ Keyboard Shortcuts");
                    ui.label("• ESC - Exit fullscreen mode");
                    ui.label("• F11 - Toggle fullscreen mode");
                    ui.label("• Cmd/Ctrl+Q - Quit application");

                    ui.separator();

                    ui.label("Built with:");
                    ui.label("• egui for the interface");
                    ui.label("• egui_tiles for layout management");
                    ui.label("• Rust for performance and safety");

                    ui.separator();

                    ui.label("💡 Tips:");
                    ui.label("• The application starts in fullscreen mode - use ESC to exit");
                    ui.label("• Drag panes to reorder them or create new splits");
                    ui.label("• Drag a pane out to create a new window/panel");
                    ui.label("• Close panes with the ✖ button (except Streams)");

                    ui.separator();

                    if ui.button("Close").clicked() {
                        self.show_about = false;
                    }
                });
        }

        // Settings Dialog
        if self.show_settings {
            egui::Window::new("⚙️ Settings")
                .default_width(500.0)
                .show(ctx, |ui| {
                    ui.label("Settings panel coming soon!");

                    ui.separator();

                    if ui.button("Close").clicked() {
                        self.show_settings = false;
                    }
                });
        }

        // Streams Overview Dialog
        if self.show_streams_overview {
            egui::Window::new("📡 Market Streams Overview")
                .default_width(1000.0)
                .default_height(600.0)
                .show(ctx, |ui| {
                    ui.label("Real-time overview of all streaming market data:");

                    ui.separator();

                    // Show streaming status
                    use crate::gui::components::market_data::streaming_status;
                    let is_streaming = matches!(self.streaming_state, StreamingState::Connected);
                    streaming_status(ui, is_streaming, self.streaming_assets.len());

                    ui.separator();

                    if is_streaming {
                        if let Ok(activities) = self.token_activities.try_read() {
                            if activities.is_empty() {
                                ui.label("Waiting for streaming data...");
                            } else {
                                let mut sorted_activities: Vec<_> =
                                    activities.values().cloned().collect();
                                sorted_activities.sort_by(|a, b| b.event_count.cmp(&a.event_count));

                                ui.label(format!(
                                    "Total active streams: {}",
                                    sorted_activities.len()
                                ));

                                // Summary stats
                                let total_events: usize =
                                    sorted_activities.iter().map(|a| a.event_count).sum();
                                let total_trades: usize =
                                    sorted_activities.iter().map(|a| a.trade_count).sum();
                                let total_volume: Decimal =
                                    sorted_activities.iter().map(|a| a.total_volume).sum();

                                ui.horizontal(|ui| {
                                    ui.group(|ui| {
                                        ui.vertical_centered(|ui| {
                                            ui.label("Total Events");
                                            ui.heading(format!("{}", total_events));
                                        });
                                    });

                                    ui.group(|ui| {
                                        ui.vertical_centered(|ui| {
                                            ui.label("Total Trades");
                                            ui.heading(format!("{}", total_trades));
                                        });
                                    });

                                    ui.group(|ui| {
                                        ui.vertical_centered(|ui| {
                                            ui.label("Total Volume");
                                            ui.heading(format!("${:.2}", total_volume));
                                        });
                                    });
                                });

                                ui.separator();

                                // Full table with all tokens
                                egui::ScrollArea::vertical()
                                    .id_salt("streams_overview_scroll")
                                    .max_height(400.0)
                                    .show(ui, |ui| {
                                        egui::Grid::new("full_streams_grid")
                                            .num_columns(8)
                                            .spacing([8.0, 4.0])
                                            .striped(true)
                                            .show(ui, |ui| {
                                                // Header
                                                ui.heading("Token ID");
                                                ui.heading("Events");
                                                ui.heading("Msg Sent");
                                                ui.heading("Msg Sorted");
                                                ui.heading("Trades");
                                                ui.heading("Volume");
                                                ui.heading("Last Price");
                                                ui.heading("Last Update");
                                                ui.end_row();

                                                for activity in &sorted_activities {
                                                    // Full token ID
                                                    ui.monospace(&activity.token_id);

                                                    // Event count
                                                    ui.label(format!("{}", activity.event_count));

                                                    // Messages sent
                                                    ui.label(format!(
                                                        "{}",
                                                        activity.last_messages_sent
                                                    ));

                                                    // Messages sorted
                                                    ui.label(format!(
                                                        "{}",
                                                        activity.messages_sorted
                                                    ));

                                                    // Trade count
                                                    ui.label(format!("{}", activity.trade_count));

                                                    // Volume
                                                    ui.label(format!(
                                                        "${:.2}",
                                                        activity.total_volume
                                                    ));

                                                    // Last price (bid or ask)
                                                    if let Some(bid) = activity.last_bid {
                                                        ui.colored_label(
                                                            egui::Color32::from_rgb(100, 200, 100),
                                                            format!("${:.4}", bid),
                                                        );
                                                    } else if let Some(ask) = activity.last_ask {
                                                        ui.colored_label(
                                                            egui::Color32::from_rgb(200, 100, 100),
                                                            format!("${:.4}", ask),
                                                        );
                                                    } else {
                                                        ui.label("-");
                                                    }

                                                    // Last update
                                                    if let Some(last_update) = activity.last_update
                                                    {
                                                        let elapsed =
                                                            last_update.elapsed().as_secs();
                                                        if elapsed < 60 {
                                                            ui.label(format!("{}s", elapsed));
                                                        } else if elapsed < 3600 {
                                                            ui.label(format!("{}m", elapsed / 60));
                                                        } else {
                                                            ui.label(format!(
                                                                "{}h",
                                                                elapsed / 3600
                                                            ));
                                                        }
                                                    } else {
                                                        ui.label("-");
                                                    }

                                                    ui.end_row();
                                                }
                                            });
                                    });
                            }
                        } else {
                            ui.label("Loading stream data...");
                        }
                    } else {
                        ui.label("Not currently streaming. Start streaming to see market data.");

                        if ui.button("▶️ Start Streaming").clicked() {
                            self.show_dataset_selector = true;
                            self.show_streams_overview = false;
                        }
                    }

                    ui.separator();

                    if ui.button("Close").clicked() {
                        self.show_streams_overview = false;
                    }
                });
        }

        // Dataset Selector Dialog
        if self.show_dataset_selector {
            egui::Window::new("📊 Select Dataset for Streaming")
                .default_width(800.0)
                .default_height(600.0)
                .show(ctx, |ui| {
                    ui.label("Select markets to stream from available datasets:");

                    ui.separator();

                    // Refresh datasets button
                    if ui.button("🔄 Refresh Datasets").clicked() {
                        self.available_datasets = Self::load_available_datasets(&self.data_paths);
                        info!("Refreshed datasets, found: {}", self.available_datasets.len());
                    }

                    ui.separator();

                    // Show available datasets in a scrollable area
                    egui::ScrollArea::vertical()
                        .id_salt("dataset_selector_scroll")
                        .max_height(400.0)
                        .show(ui, |ui| {
                            if self.available_datasets.is_empty() {
                                ui.label("No datasets found in data/datasets directory");
                                ui.label("Use the CLI to fetch market data first:");
                                ui.code("polybot fetch-all-markets --dataset-name raw_markets/$(date +%Y-%m-%d)");
                            } else {
                                for dataset in &self.available_datasets {
                                    ui.group(|ui| {
                                        ui.horizontal(|ui| {
                                            let mut is_selected = self.selected_datasets.contains(&dataset.name);

                                            if ui.checkbox(&mut is_selected, "").clicked() {
                                                if is_selected {
                                                    self.selected_datasets.insert(dataset.name.clone());
                                                } else {
                                                    self.selected_datasets.remove(&dataset.name);
                                                }
                                            }

                                            ui.vertical(|ui| {
                                                ui.heading(&dataset.name);
                                                ui.label(format!("📁 Path: {}", dataset.path.display()));
                                                ui.label(format!("📊 Assets: {} | 💾 Size: {:.1} MB",
                                                    dataset.asset_count, dataset.size_mb));
                                            });
                                        });
                                    });
                                }
                            }
                        });

                    ui.separator();

                    // Show selected count
                    ui.label(format!("Selected {} dataset(s)", self.selected_datasets.len()));

                    ui.separator();

                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.show_dataset_selector = false;
                            self.selected_datasets.clear();
                        }

                        if ui.button("Start Streaming").clicked() && !self.selected_datasets.is_empty() {
                            info!("Starting streaming with {} datasets", self.selected_datasets.len());
                            self.show_dataset_selector = false;
                            self.start_streaming_with_datasets();
                        }
                    });
                });
        }
    }

    /// Take a screenshot and save it to the screenshots directory
    fn take_screenshot(&mut self, ctx: &egui::Context) {
        // Create screenshots directory if it doesn't exist
        let screenshots_dir = std::path::Path::new("screenshots");
        if let Err(e) = fs::create_dir_all(screenshots_dir) {
            error!("Failed to create screenshots directory: {}", e);
            self.screenshot_message = Some(("Failed to create screenshots directory".to_string(), std::time::Instant::now()));
            return;
        }

        // Generate timestamp for filename
        let now: DateTime<Local> = Local::now();
        let timestamp = now.format("%Y-%m-%d_%H-%M-%S");
        let filename = format!("screenshot_{}.png", timestamp);
        let filepath = screenshots_dir.join(&filename);

        // Request screenshot - this is the proper way in egui/eframe
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        
        // Store the pending screenshot info
        self.pending_screenshot = Some((filepath, filename.clone()));
        
        // Show immediate feedback
        self.screenshot_message = Some(("📸 Taking screenshot...".to_string(), std::time::Instant::now()));
        
        info!("Screenshot requested - will be saved to: screenshots/{}", filename);
    }
}

impl TradingApp {
    /// Save current layout to file
    fn save_layout_to_file(&mut self) {
        // Create layouts directory if it doesn't exist
        let layouts_dir = self.data_paths.data().join("config").join("layouts");
        if let Err(e) = fs::create_dir_all(&layouts_dir) {
            error!("Failed to create layouts directory: {}", e);
            return;
        }

        // Generate filename with timestamp
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        let filename = format!("layout_{}.json", timestamp);
        let filepath = layouts_dir.join(&filename);

        // Create saved layout with pinned tiles
        let saved_layout = SavedLayout {
            tree: self.tree.clone(),
            pinned_tiles: self.pinned_tiles.iter().cloned().collect(),
        };
        
        // Serialize the layout
        match serde_json::to_string_pretty(&saved_layout) {
            Ok(json) => {
                match fs::write(&filepath, json) {
                    Ok(_) => {
                        info!("Layout saved to: {} (with {} pinned tiles)", 
                              filepath.display(), self.pinned_tiles.len());
                        self.has_unsaved_layout_changes = false;
                        // TODO: Show success message to user
                    }
                    Err(e) => {
                        error!("Failed to write layout file: {}", e);
                    }
                }
            }
            Err(e) => {
                error!("Failed to serialize layout: {}", e);
            }
        }
    }

    /// Load layout from file
    fn load_layout_from_file(&mut self) {
        let layouts_dir = self.data_paths.data().join("config").join("layouts");
        
        // Check if layouts directory exists
        if !layouts_dir.exists() {
            warn!("No layouts directory found");
            return;
        }

        // List available layout files
        match fs::read_dir(&layouts_dir) {
            Ok(entries) => {
                let mut layout_files: Vec<PathBuf> = entries
                    .filter_map(|entry| entry.ok())
                    .map(|entry| entry.path())
                    .filter(|path| {
                        path.extension()
                            .and_then(|ext| ext.to_str())
                            .map(|ext| ext == "json")
                            .unwrap_or(false)
                    })
                    .collect();

                if layout_files.is_empty() {
                    warn!("No layout files found");
                    return;
                }

                // Sort by modification time (newest first)
                layout_files.sort_by_key(|path| {
                    fs::metadata(path)
                        .and_then(|meta| meta.modified())
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                });
                layout_files.reverse();

                // For now, load the most recent layout
                // TODO: Show file picker dialog
                if let Some(latest_file) = layout_files.first() {
                    match fs::read_to_string(latest_file) {
                        Ok(content) => {
                            // Try to load new format first (with pinned tiles)
                            match serde_json::from_str::<SavedLayout>(&content) {
                                Ok(saved_layout) => {
                                    self.tree = saved_layout.tree;
                                    self.pinned_tiles = saved_layout.pinned_tiles.into_iter().collect();
                                    self.focused_tile_id = None;
                                    self.has_unsaved_layout_changes = false;
                                    info!("Layout loaded from: {} (with {} pinned tiles)", 
                                          latest_file.display(), self.pinned_tiles.len());
                                }
                                Err(_) => {
                                    // Fall back to old format (just tree)
                                    match serde_json::from_str::<Tree<Pane>>(&content) {
                                        Ok(tree) => {
                                            self.tree = tree;
                                            self.pinned_tiles.clear();
                                            self.focused_tile_id = None;
                                            self.has_unsaved_layout_changes = false;
                                            info!("Layout loaded from: {} (legacy format)", latest_file.display());
                                        }
                                        Err(e) => {
                                            error!("Failed to deserialize layout: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to read layout file: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to read layouts directory: {}", e);
            }
        }
    }

    /// Save screenshot data to file
    fn save_screenshot(
        screenshot: &egui::ColorImage,
        filepath: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Get raw image data
        let raw_data = screenshot.as_raw();
        let width = screenshot.size[0] as u32;
        let height = screenshot.size[1] as u32;
        
        // The raw data is in RGBA format
        if let Some(img) = image::RgbaImage::from_raw(width, height, raw_data.to_vec()) {
            // Save to file
            img.save(filepath)?;
            Ok(())
        } else {
            Err("Failed to create image from raw data".into())
        }
    }

    /// Crop and save screenshot data to file
    fn crop_and_save_screenshot(
        screenshot: &egui::ColorImage,
        tile_rect: &egui::Rect,
        filepath: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Get raw image data
        let raw_data = screenshot.as_raw();
        let full_width = screenshot.size[0] as u32;
        let full_height = screenshot.size[1] as u32;
        
        // Calculate crop bounds (converting from logical to physical pixels if needed)
        let scale_factor = 1.0; // Assuming no DPI scaling for now
        let crop_x = (tile_rect.min.x * scale_factor).max(0.0) as u32;
        let crop_y = (tile_rect.min.y * scale_factor).max(0.0) as u32;
        let crop_width = ((tile_rect.width() * scale_factor) as u32).min(full_width - crop_x);
        let crop_height = ((tile_rect.height() * scale_factor) as u32).min(full_height - crop_y);
        
        // Create full image first
        if let Some(full_img) = image::RgbaImage::from_raw(full_width, full_height, raw_data.to_vec()) {
            // Crop the image to the tile bounds
            let cropped = image::imageops::crop_imm(
                &full_img,
                crop_x,
                crop_y,
                crop_width,
                crop_height
            ).to_image();
            
            // Save to file
            cropped.save(filepath)?;
            Ok(())
        } else {
            Err("Failed to create image from raw data".into())
        }
    }

    fn poll_streaming_progress(&mut self, ctx: &egui::Context) {
        // Poll progress updates
        if let Some(rx) = &mut self.streaming_progress_rx {
            match rx.try_recv() {
                Ok((progress, message)) => {
                    self.streaming_state = StreamingState::Initializing { progress, message };
                    ctx.request_repaint(); // Request UI update
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                    // No new progress
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    // Channel closed
                    self.streaming_progress_rx = None;
                }
            }
        }

        // Poll for final result
        if let Some(rx) = &mut self.streaming_result_rx {
            match rx.try_recv() {
                Ok(Ok(streaming_service)) => {
                    info!("Streaming service initialized successfully");

                    // Subscribe to events
                    self.event_receiver = Some(streaming_service.subscribe_events());

                    self.streaming_service = Some(streaming_service);
                    self.streaming_state = StreamingState::Connected;
                    self.streaming_result_rx = None;
                    self.streaming_progress_rx = None;

                    // Start background task to update cached data
                    self.start_data_update_task();

                    ctx.request_repaint();
                }
                Ok(Err(e)) => {
                    error!("Failed to initialize streaming: {}", e);
                    self.streaming_state = StreamingState::Error(e.to_string());
                    self.streaming_result_rx = None;
                    self.streaming_progress_rx = None;
                    ctx.request_repaint();
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                    // Not ready yet
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                    // Channel closed without sending
                    self.streaming_state =
                        StreamingState::Error("Streaming initialization cancelled".to_string());
                    self.streaming_result_rx = None;
                    self.streaming_progress_rx = None;
                    ctx.request_repaint();
                }
            }
        }

        // Request continuous updates while initializing
        if matches!(self.streaming_state, StreamingState::Initializing { .. }) {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

impl eframe::App for TradingApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Add debug logging to confirm GUI is running (thread-safe)
        if self.first_update {
            info!("🎮 First GUI update() call - interface is running!");
            self.first_update = false;
        }

        // Apply minimal styling changes - only what's necessary
        ctx.style_mut(|style| {
            // Basic spacing adjustments
            style.spacing.item_spacing = egui::vec2(8.0, 4.0);
            style.spacing.indent = 18.0;
            
            // IMPORTANT: DO NOT make widgets transparent or change their bg_fill!
            // Previous attempts to hide the resize handle by setting:
            //   style.visuals.widgets.inactive.bg_fill = egui::Color32::TRANSPARENT
            // caused ALL tiles to become invisible/transparent.
            // The resize handle cannot be fully hidden without affecting other widgets.
            
            // Only adjust panel-specific settings
            let bg_color = style.visuals.window_fill();
            style.visuals.panel_fill = bg_color;
            
            // Minimize visual separator lines
            style.visuals.window_stroke = egui::Stroke::new(0.0, bg_color);
            
            // Make resize corner less prominent
            style.visuals.resize_corner_size = 0.0;
            
            // Minimal margin adjustments for seamless panel connections  
            style.spacing.window_margin = egui::Margin::ZERO;
            style.spacing.menu_margin = egui::Margin::same(4);
            style.spacing.button_padding = egui::vec2(8.0, 4.0);
        });

        // Handle keyboard shortcuts with improved error handling
        ctx.input(|i| {
            // F11 to toggle fullscreen (proper toggling with error handling)
            if i.key_pressed(egui::Key::F11) {
                self.is_fullscreen = !self.is_fullscreen;
                if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.is_fullscreen));
                })) {
                    error!("❌ F11 fullscreen command failed: {:?}", e);
                    // Reset state on failure
                    self.is_fullscreen = !self.is_fullscreen;
                } else {
                    info!("🖥️ F11 pressed - toggling fullscreen to: {}", self.is_fullscreen);
                }
            }

            // ESC to exit fullscreen (safe approach with error handling)
            if i.key_pressed(egui::Key::Escape) {
                if self.is_fullscreen {
                    if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
                    })) {
                        error!("❌ ESC fullscreen exit failed: {:?}", e);
                    } else {
                        self.is_fullscreen = false;
                        info!("🖥️ ESC pressed - exiting fullscreen");
                    }
                }
            }

            // Ctrl/Cmd+Q to quit (with error handling)
            if i.modifiers.command && i.key_pressed(egui::Key::Q) {
                info!("🚪 User pressed Cmd/Ctrl+Q to quit");
                if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                })) {
                    error!("❌ Quit command failed: {:?}", e);
                } else {
                    info!("🚪 Close command sent");
                }
            }
        });

        // Initialize portfolio service if needed
        self.init_portfolio_service_if_needed();

        // Poll for streaming progress updates first
        self.poll_streaming_progress(ctx);

        // Poll for cached data updates from background task
        self.poll_cached_data_updates();

        // Poll for streaming events
        self.poll_streaming_events(ctx);

        // Show menu bar
        self.show_menu_bar(ctx);

        // Show sidebar
        self.show_sidebar(ctx);

        // Show dialogs on top
        self.show_dialogs(ctx);

        // Main content area with tiles - ensure no gaps between sidebar and central panel
        let window_fill = ctx.style().visuals.window_fill();
        egui::CentralPanel::default()
            .frame(egui::Frame::default()
                .fill(window_fill)  // Use same fill color as sidebar
                .inner_margin(egui::Margin {
                    left: 0,  // No left margin to meet sidebar perfectly
                    right: 8,
                    top: 8,
                    bottom: 8,
                })  // Proper margins except on left where it meets sidebar
                .outer_margin(egui::Margin::ZERO)  // Remove any outer margins that could create gaps
                .stroke(egui::Stroke::NONE)  // Ensure no stroke/border
            )
            .show(ctx, |ui| {
            // Create a separate scope to isolate the borrow
            let tree = &mut self.tree;
            let mut behavior = TradingBehavior {
                _portfolio_manager: &self.portfolio_manager,
                portfolio_service: &self.portfolio_service,
                streaming_state: &self.streaming_state,
                streaming_assets: &self.streaming_assets,
                current_token_id: &mut self.current_token_id,
                _current_bids: &self.current_bids,
                _current_asks: &self.current_asks,
                orderbook_changes: &self.orderbook_changes,
                show_dataset_selector: &mut self.show_dataset_selector,
                _last_position_fetch: &mut self._last_position_fetch,
                _is_fetching_positions: &mut self._is_fetching_positions,
                token_activities: &self.token_activities,
                streaming_service: &self.streaming_service,
                cached_streaming_stats: &self.cached_streaming_stats,
                pending_new_orderbook: &mut self.pending_new_orderbook,
                pending_new_worker_details: &mut self.pending_new_worker_details,
                cached_orderbook: &self.cached_orderbook,
                cached_last_trade_price: &self.cached_last_trade_price,
                cached_worker_statuses: &self.cached_worker_statuses,
                selected_worker_id: &mut self.selected_worker_id,
                worker_stream_events: &mut self.worker_stream_events,
                worker_stream_max_events: &self.worker_stream_max_events,
                event_receiver: &mut self.event_receiver,
                current_token_sender: &self.current_token_sender,
                focused_tile_id: &mut self.focused_tile_id,
                tiles_to_close: &mut self.tiles_to_close,
                screenshot_message: &mut self.screenshot_message,
                pending_tile_screenshot: &mut self.pending_tile_screenshot,
                tile_bounds: &mut self.tile_bounds,
                pinned_tiles: &mut self.pinned_tiles,
                has_unsaved_layout_changes: &mut self.has_unsaved_layout_changes,
            };
            tree.ui(&mut behavior, ui);

            // Handle tiles marked for closing
            for tile_id in self.tiles_to_close.drain(..) {
                info!("Closing tile: {:?}", tile_id);
                tree.tiles.remove(tile_id);
                self.has_unsaved_layout_changes = true;
            }
            
            // Detect drag/drop or other tree structure changes
            // Calculate a simple hash of the tree structure
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            
            // Hash the tree structure (this will capture tile positions and hierarchy)
            if let Some(root) = tree.root {
                root.hash(&mut hasher);
                // Also hash the tiles to detect structural changes
                for (id, _) in tree.tiles.iter() {
                    id.hash(&mut hasher);
                }
            }
            
            let current_hash = hasher.finish();
            
            // Check if tree structure has changed
            if let Some(prev_hash) = self.previous_tree_hash {
                if prev_hash != current_hash && !self.tiles_to_close.is_empty() == false {
                    // Tree structure changed and it wasn't due to closing tiles
                    self.has_unsaved_layout_changes = true;
                    info!("Layout changed due to drag/drop or restructuring");
                }
            }
            
            self.previous_tree_hash = Some(current_hash);
        });

        // Handle screenshot messages
        if let Some((message, timestamp)) = &self.screenshot_message {
            if timestamp.elapsed().as_secs() < 3 {
                // Show screenshot message as a toast/notification
                egui::Window::new("Screenshot")
                    .anchor(egui::Align2::RIGHT_BOTTOM, [-10.0, -10.0])
                    .fixed_size([250.0, 60.0])
                    .frame(egui::Frame::popup(&ctx.style()))
                    .show(ctx, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.add_space(10.0);
                            ui.label(message);
                        });
                    });
            } else {
                self.screenshot_message = None;
            }
        }

        // Handle screenshot events
        ctx.input(|i| {
            for event in &i.raw.events {
                if let egui::Event::Screenshot { viewport_id: _, image, user_data: _ } = event {
                    // Check if this is a tile screenshot
                    if let Some((tile_id, filepath, filename)) = self.pending_tile_screenshot.take() {
                        // Get the tile bounds
                        if let Some(tile_rect) = self.tile_bounds.get(&tile_id) {
                            // Crop the screenshot to just the tile area
                            match Self::crop_and_save_screenshot(image, tile_rect, &filepath) {
                                Ok(_) => {
                                    info!("Tile screenshot saved successfully to: {}", filepath.display());
                                    self.screenshot_message = Some((
                                        format!("✅ Tile screenshot saved: {}", &filename),
                                        std::time::Instant::now(),
                                    ));
                                }
                                Err(e) => {
                                    error!("Failed to save tile screenshot: {}", e);
                                    self.screenshot_message = Some((
                                        format!("❌ Failed to save tile screenshot: {}", e),
                                        std::time::Instant::now(),
                                    ));
                                }
                            }
                        } else {
                            error!("Tile bounds not found for tile {:?}", tile_id);
                            self.screenshot_message = Some((
                                "❌ Failed to save tile screenshot: bounds not found".to_string(),
                                std::time::Instant::now(),
                            ));
                        }
                    } else if let Some((filepath, filename)) = self.pending_screenshot.take() {
                        // Regular full screenshot
                        match Self::save_screenshot(image, &filepath) {
                            Ok(_) => {
                                info!("Screenshot saved successfully to: {}", filepath.display());
                                self.screenshot_message = Some((
                                    format!("✅ Screenshot saved: {}", &filename),
                                    std::time::Instant::now(),
                                ));
                            }
                            Err(e) => {
                                error!("Failed to save screenshot: {}", e);
                                self.screenshot_message = Some((
                                    format!("❌ Failed to save screenshot: {}", e),
                                    std::time::Instant::now(),
                                ));
                            }
                        }
                    }
                }
            }
        });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let saved_layout = SavedLayout {
            tree: self.tree.clone(),
            pinned_tiles: self.pinned_tiles.iter().cloned().collect(),
        };
        
        if let Ok(serialized) = serde_json::to_string(&saved_layout) {
            storage.set_string("trading_workspace", serialized);
        }
    }
}

// Tile behavior implementation
struct TradingBehavior<'a> {
    _portfolio_manager: &'a Arc<PortfolioManager>,
    portfolio_service: &'a PortfolioService,
    streaming_state: &'a StreamingState,
    streaming_assets: &'a Vec<String>,
    current_token_id: &'a mut Option<String>,
    _current_bids: &'a Vec<crate::core::types::market::PriceLevel>,
    _current_asks: &'a Vec<crate::core::types::market::PriceLevel>,
    orderbook_changes: &'a Vec<OrderBookChange>,
    show_dataset_selector: &'a mut bool,
    _last_position_fetch: &'a mut Option<std::time::Instant>,
    _is_fetching_positions: &'a mut bool,
    token_activities: &'a Arc<RwLock<HashMap<String, TokenActivity>>>,
    streaming_service: &'a Option<Arc<StreamingService>>,
    cached_streaming_stats: &'a Option<crate::core::services::streaming::traits::StreamingStats>,
    pending_new_orderbook: &'a mut Option<String>,
    pending_new_worker_details: &'a mut Option<usize>,
    cached_orderbook: &'a Option<crate::core::ws::OrderBook>,
    cached_last_trade_price: &'a Option<(Decimal, u64)>,
    cached_worker_statuses: &'a Vec<crate::core::services::streaming::traits::WorkerStatus>,
    selected_worker_id: &'a mut Option<usize>,
    worker_stream_events: &'a mut Vec<PolyEvent>,
    worker_stream_max_events: &'a usize,
    event_receiver: &'a mut Option<tokio::sync::broadcast::Receiver<PolyEvent>>,
    current_token_sender: &'a Option<std::sync::mpsc::Sender<Option<String>>>,
    focused_tile_id: &'a mut Option<egui_tiles::TileId>,
    tiles_to_close: &'a mut Vec<egui_tiles::TileId>,
    // Screenshot-related fields
    screenshot_message: &'a mut Option<(String, std::time::Instant)>,
    pending_tile_screenshot: &'a mut Option<(TileId, std::path::PathBuf, String)>,
    tile_bounds: &'a mut std::collections::HashMap<TileId, egui::Rect>,
    // Pin state
    pinned_tiles: &'a mut HashSet<TileId>,
    // Layout change tracking
    has_unsaved_layout_changes: &'a mut bool,
}

impl egui_tiles::Behavior<Pane> for TradingBehavior<'_> {
    fn tab_title_for_pane(&mut self, pane: &Pane) -> egui::WidgetText {
        pane.tab_title().into()
    }

    /// Enable drag and drop preview when dragging panes
    fn preview_dragged_panes(&self) -> bool {
        true // Show preview of pane being dragged
    }

    /// Check if a tab can be closed - all panes can now be closed
    fn is_tab_closable(
        &self,
        _tiles: &egui_tiles::Tiles<Pane>,
        _tile_id: egui_tiles::TileId,
    ) -> bool {
        // All panes can be closed now
        true
    }

    /// Handle tab close events
    fn on_tab_close(
        &mut self,
        tiles: &mut egui_tiles::Tiles<Pane>,
        tile_id: egui_tiles::TileId,
    ) -> bool {
        tiles.remove(tile_id);
        true // Allow closing
    }

    /// Handle tab button interactions - including middle click
    fn on_tab_button(
        &mut self,
        _tiles: &egui_tiles::Tiles<Pane>,
        tile_id: egui_tiles::TileId,
        response: egui::Response,
    ) -> egui::Response {
        // Track focus when tile is clicked
        if response.clicked() {
            *self.focused_tile_id = Some(tile_id);
        }

        // Check for middle mouse button click (like Chrome tabs) - all panes can be closed
        if response.middle_clicked() {
            self.close_pane(tile_id);
        }

        response
    }

    /// Customize the tab bar height for better drag targets
    fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
        32.0 // Larger tab bar for easier dragging
    }

    /// Add visual feedback during drag operations
    fn dragged_overlay_color(&self, _visuals: &egui::Visuals) -> egui::Color32 {
        egui::Color32::from_rgba_unmultiplied(0, 100, 200, 50) // Light blue overlay
    }

    /// Paint drag preview with a distinct look
    fn paint_drag_preview(
        &self,
        visuals: &egui::Visuals,
        painter: &egui::Painter,
        _parent_rect: Option<egui::Rect>,
        preview_rect: egui::Rect,
    ) {
        // Draw a semi-transparent rectangle with a border
        painter.rect_filled(
            preview_rect,
            egui::CornerRadius::same(4),
            egui::Color32::from_rgba_unmultiplied(0, 100, 200, 30),
        );
        painter.rect_stroke(
            preview_rect,
            egui::CornerRadius::same(4),
            egui::Stroke::new(2.0, visuals.widgets.active.bg_stroke.color),
            egui::epaint::StrokeKind::Inside,
        );
    }

    /// Show UI while dragging a tab
    fn drag_ui(
        &mut self,
        tiles: &egui_tiles::Tiles<Pane>,
        ui: &mut egui::Ui,
        tile_id: egui_tiles::TileId,
    ) {
        if let Some(tile) = tiles.get(tile_id) {
            if let egui_tiles::Tile::Pane(pane) = tile {
                // Show a compact version of the pane title while dragging
                ui.group(|ui| {
                    ui.set_min_size(egui::vec2(120.0, 60.0));
                    ui.vertical_centered(|ui| {
                        ui.add_space(10.0);
                        ui.heading(pane.tab_title());
                        ui.label("Drop to place");
                    });
                });
            }
        }
    }

    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        tile_id: TileId,
        pane: &mut Pane,
    ) -> egui_tiles::UiResponse {
        // Set minimum size constraints for the pane
        let min_size = match pane {
            Pane::Orders => egui::vec2(400.0, 300.0),
            Pane::Streams => egui::vec2(600.0, 400.0),
            Pane::Portfolio => egui::vec2(500.0, 300.0),
            Pane::Tokens => egui::vec2(400.0, 300.0),
            Pane::MarketDepth(_) => egui::vec2(350.0, 400.0),
            Pane::Charts => egui::vec2(500.0, 400.0),
            Pane::TradeHistory => egui::vec2(400.0, 300.0),
            Pane::Balances => egui::vec2(350.0, 200.0),
            Pane::WebSocketManager => egui::vec2(700.0, 500.0),
            Pane::WorkerDetails(_) => egui::vec2(600.0, 500.0),
        };

        ui.set_min_size(min_size);

        // Check if this tile is focused and add visual feedback
        let is_focused = *self.focused_tile_id == Some(tile_id);

        // Create a frame with proper spacing and aesthetic borders
        let frame = if is_focused {
            egui::Frame::default()
                .inner_margin(egui::Margin::same(2)) // Reduced inner spacing
                .outer_margin(egui::Margin::ZERO) // Remove outer margin to prevent gaps
                .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(46, 204, 113))) // Elegant green border (3px)
                .fill(ui.visuals().panel_fill)
                .corner_radius(egui::CornerRadius::same(4)) // Subtle rounded corners for aesthetics
                .shadow(egui::epaint::Shadow {
                    offset: [1, 1],
                    blur: 6,  // Subtle glow effect
                    spread: 1, // Minimal spread to avoid overlap
                    color: egui::Color32::from_rgba_unmultiplied(46, 204, 113, 120), // Subtle green glow
                })
        } else {
            egui::Frame::default()
                .inner_margin(egui::Margin::same(2)) // Reduced inner spacing
                .outer_margin(egui::Margin::ZERO) // Remove outer margin to prevent gaps
                .stroke(egui::Stroke::NONE)  // No stroke for unfocused tiles to prevent gaps
                .fill(ui.visuals().panel_fill)
                .corner_radius(egui::CornerRadius::same(2)) // Subtle rounded corners
        };

        let mut drag_started = false;
        
        // Get the full tile rect before showing the frame to detect clicks anywhere in the tile
        let full_tile_rect = ui.available_rect_before_wrap();
        
        // Store the tile bounds for screenshot functionality
        self.tile_bounds.insert(tile_id, full_tile_rect);

        let frame_response = frame.show(ui, |ui| {
            ui.vertical(|ui| {
                // Custom title bar with drag and close functionality
                let title_bar_response = self.show_custom_title_bar(ui, tile_id, pane);

                // Check for drag operations on title bar
                if title_bar_response.drag_started() {
                    drag_started = true;
                    return;
                }

                ui.separator();

                // Show the pane content
                match pane {
                    Pane::Orders => self.show_orders_pane(ui),
                    Pane::Streams => self.show_streams_pane(ui),
                    Pane::Portfolio => self.show_portfolio_pane(ui),
                    Pane::Tokens => self.show_tokens_pane(ui),
                    Pane::MarketDepth(token_id) => self.show_market_depth_pane(ui, token_id),
                    Pane::Charts => self.show_charts_pane(ui),
                    Pane::TradeHistory => self.show_trade_history_pane(ui),
                    Pane::Balances => self.show_balances_pane(ui),
                    Pane::WebSocketManager => self.show_websocket_manager_pane(ui),
                    Pane::WorkerDetails(worker_id) => self.show_worker_details_pane(ui, *worker_id),
                }
            });
        });

        // Track focus when clicking ANYWHERE in the tile (frame or content)
        // Check both the frame response and the full tile area to ensure comprehensive click detection
        if frame_response.response.clicked() || 
            (ui.input(|i| i.pointer.any_pressed()) && ui.rect_contains_pointer(full_tile_rect))
        {
            *self.focused_tile_id = Some(tile_id);
        }

        // Handle drag start
        if drag_started {
            egui_tiles::UiResponse::DragStarted
        } else {
            egui_tiles::UiResponse::None
        }
    }

    /// Configure tile simplification to prevent tab creation and optimize grid layout
    /// This ensures we never create tab containers and always use linear splits
    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        egui_tiles::SimplificationOptions {
            prune_empty_tabs: true,               // Remove empty tab containers
            prune_empty_containers: true,         // Remove empty containers
            prune_single_child_tabs: true,        // Convert single-child tabs to direct panes
            prune_single_child_containers: false, // Keep containers to maintain grid structure
            all_panes_must_have_tabs: false,      // NEVER force tabs - use linear splits only
            join_nested_linear_containers: true,  // Optimize nested linear containers
        }
    }
    
    /// Set the gap width between tiles to prevent black strips
    fn gap_width(&self, _style: &egui::Style) -> f32 {
        0.0 // No gap between tiles to prevent black strips
    }
}

impl TradingBehavior<'_> {
    /// Take a screenshot of a specific tile
    fn take_tile_screenshot(&mut self, ctx: &egui::Context, tile_id: TileId, pane: &Pane) {
        // Create screenshots directory if it doesn't exist
        let screenshots_dir = std::path::Path::new("screenshots");
        if let Err(e) = fs::create_dir_all(screenshots_dir) {
            error!("Failed to create screenshots directory: {}", e);
            *self.screenshot_message = Some(("Failed to create screenshots directory".to_string(), std::time::Instant::now()));
            return;
        }

        // Generate timestamp for filename with pane name
        let now: DateTime<Local> = Local::now();
        let timestamp = now.format("%Y-%m-%d_%H-%M-%S");
        let pane_name = match pane {
            Pane::Orders => "orders",
            Pane::Streams => "streams",
            Pane::Portfolio => "portfolio",
            Pane::Tokens => "tokens",
            Pane::MarketDepth(_) => "market-depth",
            Pane::Charts => "charts",
            Pane::TradeHistory => "trade-history",
            Pane::Balances => "balances",
            Pane::WebSocketManager => "websocket-manager",
            Pane::WorkerDetails(_) => "worker-details",
        };
        let filename = format!("tile_{}_{}.png", pane_name, timestamp);
        let filepath = screenshots_dir.join(&filename);

        // Store the tile ID for the screenshot
        *self.pending_tile_screenshot = Some((tile_id, filepath.clone(), filename.clone()));
        
        // Request screenshot of the entire viewport
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        
        // Show immediate feedback
        *self.screenshot_message = Some((format!("📷 Taking screenshot of {}...", pane.tab_title()), std::time::Instant::now()));
        
        info!("Tile screenshot requested - will be saved to: screenshots/{}", filename);
    }

    /// Show custom title bar with drag functionality and close button
    fn show_custom_title_bar(
        &mut self,
        ui: &mut egui::Ui,
        tile_id: TileId,
        pane: &Pane,
    ) -> egui::Response {
        let title_height = 32.0;
        let close_button_size = 20.0;
        let screenshot_button_size = 20.0;
        let pin_button_size = 20.0;

        let (title_rect, mut response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), title_height),
            egui::Sense::click_and_drag(),
        );

        // Draw title bar background
        let title_bg_color = if response.hovered() {
            ui.visuals().widgets.hovered.weak_bg_fill
        } else {
            ui.visuals().widgets.noninteractive.weak_bg_fill
        };

        ui.painter()
            .rect_filled(title_rect, egui::CornerRadius::same(4), title_bg_color);

        // Handle title bar interactions
        if response.clicked() {
            *self.focused_tile_id = Some(tile_id);
        }

        // Handle middle click to close (like browser tabs)
        if response.middle_clicked() {
            self.close_pane(tile_id);
        }

        // Layout buttons - close button on the right, then screenshot, then pin
        let close_button_rect = egui::Rect::from_min_size(
            egui::pos2(
                title_rect.max.x - close_button_size - 6.0,
                title_rect.min.y + 6.0,
            ),
            egui::vec2(close_button_size, close_button_size),
        );

        let screenshot_button_rect = egui::Rect::from_min_size(
            egui::pos2(
                close_button_rect.min.x - screenshot_button_size - 4.0,
                title_rect.min.y + 6.0,
            ),
            egui::vec2(screenshot_button_size, screenshot_button_size),
        );
        
        let pin_button_rect = egui::Rect::from_min_size(
            egui::pos2(
                screenshot_button_rect.min.x - pin_button_size - 4.0,
                title_rect.min.y + 6.0,
            ),
            egui::vec2(pin_button_size, pin_button_size),
        );

        let title_text_rect = egui::Rect::from_min_max(
            egui::pos2(title_rect.min.x + 8.0, title_rect.min.y),
            egui::pos2(pin_button_rect.min.x - 4.0, title_rect.max.y),
        );

        // Draw pane title and icon
        let title_text = pane.tab_title();
        ui.painter().text(
            title_text_rect.center(),
            egui::Align2::CENTER_CENTER,
            &title_text,
            egui::FontId::proportional(14.0),
            ui.visuals().text_color(),
        );

        // Draw pin button
        let pin_button_response = ui.allocate_rect(pin_button_rect, egui::Sense::click());
        
        let is_pinned = self.pinned_tiles.contains(&tile_id);
        let pin_button_color = if pin_button_response.hovered() {
            egui::Color32::from_rgb(255, 200, 50) // Gold on hover
        } else if is_pinned {
            egui::Color32::from_rgb(255, 180, 0) // Orange when pinned
        } else {
            ui.visuals().text_color()
        };

        // Draw pin icon
        let pin_icon = if is_pinned { "🔒" } else { "📌" };
        ui.painter().text(
            pin_button_rect.center(),
            egui::Align2::CENTER_CENTER,
            pin_icon,
            egui::FontId::proportional(14.0),
            pin_button_color,
        );

        if pin_button_response.clicked() {
            if is_pinned {
                self.pinned_tiles.remove(&tile_id);
                info!("Unpinned tile: {:?}", tile_id);
            } else {
                self.pinned_tiles.insert(tile_id);
                info!("Pinned tile: {:?}", tile_id);
            }
            *self.has_unsaved_layout_changes = true;
        }

        if pin_button_response.hovered() {
            let tooltip = if is_pinned {
                "Unpin this tile (currently locked)"
            } else {
                "Pin this tile (prevent moving/arranging)"
            };
            pin_button_response.on_hover_text(tooltip);
        }

        // Draw screenshot button
        let screenshot_button_response = ui.allocate_rect(screenshot_button_rect, egui::Sense::click());
        
        let screenshot_button_color = if screenshot_button_response.hovered() {
            egui::Color32::from_rgb(100, 150, 255) // Blue on hover
        } else {
            ui.visuals().text_color()
        };

        // Draw camera icon for screenshot
        ui.painter().text(
            screenshot_button_rect.center(),
            egui::Align2::CENTER_CENTER,
            "📷",
            egui::FontId::proportional(14.0),
            screenshot_button_color,
        );

        if screenshot_button_response.clicked() {
            self.take_tile_screenshot(ui.ctx(), tile_id, pane);
        }

        if screenshot_button_response.hovered() {
            screenshot_button_response.on_hover_text("Take screenshot of this tile");
        }

        // Draw close button for all panes
        let close_button_response = ui.allocate_rect(close_button_rect, egui::Sense::click());

        let close_button_color = if close_button_response.hovered() {
            egui::Color32::from_rgb(220, 50, 50) // Red on hover
        } else {
            ui.visuals().text_color()
        };

        // Draw X symbol
        self.draw_close_button_x(ui, close_button_rect.center(), close_button_color);

        if close_button_response.clicked() {
            self.close_pane(tile_id);
        }

        // Show tooltip on hover
        if close_button_response.hovered() {
            close_button_response.on_hover_text("Close pane");
        }

        // Visual feedback for dragging (only if not pinned)
        if !self.pinned_tiles.contains(&tile_id) {
            if response.drag_started() || response.dragged() {
                ui.painter().rect_stroke(
                    title_rect,
                    egui::CornerRadius::same(4),
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(0, 150, 255)),
                    egui::epaint::StrokeKind::Inside,
                );
            }
        } else {
            // Disable dragging for pinned tiles
            response = response.clone();
        }

        response
    }

    fn show_orders_pane(&mut self, ui: &mut egui::Ui) {
        // Show user address if available
        if let Some(user_address) = self.portfolio_service.get_user_address_sync() {
            ui.label(format!("👤 User: {}", user_address));
            ui.separator();
        }

        // Show refresh status and time elapsed
        ui.horizontal(|ui| {
            if self.portfolio_service.is_refreshing_sync() {
                ui.spinner();
                ui.label("Refreshing orders...");
            } else {
                // Show time since last refresh
                if let Some(elapsed) = self.portfolio_service.time_since_last_refresh_sync() {
                    let seconds = elapsed.as_secs();
                    let time_str = if seconds < 60 {
                        format!("{}s ago", seconds)
                    } else if seconds < 3600 {
                        format!("{}m {}s ago", seconds / 60, seconds % 60)
                    } else {
                        format!("{}h {}m ago", seconds / 3600, (seconds % 3600) / 60)
                    };
                    ui.label(format!("🕒 Last refresh: {}", time_str));
                } else {
                    ui.label("🕒 Never refreshed");
                }

                if ui.button("🔄 Refresh Now").clicked() {
                    info!("Manual refresh triggered");
                    self.portfolio_service.refresh_data_async();
                }
            }
        });
        ui.separator();

        // Get orders from portfolio service
        let orders = self.portfolio_service.get_orders_sync();

        if orders.is_empty() && !self.portfolio_service.is_refreshing_sync() {
            ui.label("✅ Successfully authenticated with Polymarket API");
            ui.label("📋 No active orders found");
            ui.separator();
            ui.label("💡 To place orders, use the trading commands or visit your profile page");
        } else {
            ui.label(format!("📋 Active Orders ({} total)", orders.len()));
            ui.separator();

            // Orders table
            egui::ScrollArea::vertical()
                .id_salt("orders_pane_scroll")
                .max_height(400.0)
                .show(ui, |ui| {
                    egui::Grid::new("orders_grid")
                        .num_columns(7)
                        .spacing([8.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            // Header
                            ui.heading("Order ID");
                            ui.heading("Market");
                            ui.heading("Side");
                            ui.heading("Price");
                            ui.heading("Size");
                            ui.heading("Remaining");
                            ui.heading("Status");
                            ui.end_row();

                            // Order rows
                            for order in &orders {
                                // Order ID (truncated)
                                let order_id_display = if order.id.len() > 12 {
                                    format!("{}...", &order.id[..12])
                                } else {
                                    order.id.clone()
                                };
                                ui.monospace(order_id_display);

                                // Market (get from market_info or use asset_id)
                                let market_display = if let Some(market_info) = &order.market_info {
                                    if let Some(question) = &market_info.market_question {
                                        if question.len() > 25 {
                                            format!("{}...", &question[..25])
                                        } else {
                                            question.clone()
                                        }
                                    } else {
                                        format!(
                                            "Asset: {}",
                                            &order.asset_id
                                                [..std::cmp::min(25, order.asset_id.len())]
                                        )
                                    }
                                } else {
                                    format!(
                                        "Asset: {}",
                                        &order.asset_id[..std::cmp::min(25, order.asset_id.len())]
                                    )
                                };
                                ui.label(market_display);

                                // Side with color
                                let side_str = match order.side {
                                    crate::core::execution::orders::OrderSide::Buy => "BUY",
                                    crate::core::execution::orders::OrderSide::Sell => "SELL",
                                };
                                let side_color = match order.side {
                                    crate::core::execution::orders::OrderSide::Buy => {
                                        egui::Color32::from_rgb(100, 200, 100)
                                    }
                                    crate::core::execution::orders::OrderSide::Sell => {
                                        egui::Color32::from_rgb(200, 100, 100)
                                    }
                                };
                                ui.colored_label(side_color, side_str);

                                // Price
                                ui.label(format!("${:.4}", order.price));

                                // Size
                                ui.label(format!("{:.2}", order.size));

                                // Remaining
                                let remaining = order.remaining_size;
                                ui.label(format!("{:.2}", remaining));

                                // Status with color
                                let status_str = match order.status {
                                    crate::core::execution::orders::OrderStatus::Open => "OPEN",
                                    crate::core::execution::orders::OrderStatus::Filled => "FILLED",
                                    crate::core::execution::orders::OrderStatus::Cancelled => "CANCELLED",
                                    crate::core::execution::orders::OrderStatus::PartiallyFilled => {
                                        "PARTIALLY_FILLED"
                                    }
                                    crate::core::execution::orders::OrderStatus::Rejected => "REJECTED",
                                    crate::core::execution::orders::OrderStatus::Pending => "PENDING",
                                };
                                let status_color = match order.status {
                                    crate::core::execution::orders::OrderStatus::Open => {
                                        egui::Color32::from_rgb(100, 200, 100)
                                    }
                                    crate::core::execution::orders::OrderStatus::PartiallyFilled => {
                                        egui::Color32::from_rgb(255, 200, 100)
                                    }
                                    crate::core::execution::orders::OrderStatus::Filled => {
                                        egui::Color32::from_rgb(100, 100, 200)
                                    }
                                    crate::core::execution::orders::OrderStatus::Cancelled => {
                                        egui::Color32::from_rgb(200, 100, 100)
                                    }
                                    crate::core::execution::orders::OrderStatus::Rejected => {
                                        egui::Color32::from_rgb(200, 80, 80)
                                    }
                                    crate::core::execution::orders::OrderStatus::Pending => {
                                        egui::Color32::from_rgb(255, 255, 100)
                                    }
                                };
                                ui.colored_label(status_color, status_str);

                                ui.end_row();
                            }
                        });
                });
        }

        ui.separator();

        if ui.button("🔄 Refresh Orders").clicked() {
            info!("Manual refresh triggered from orders pane");
            self.portfolio_service.refresh_data_async();
        }
    }

    fn show_streams_pane(&mut self, ui: &mut egui::Ui) {
        // Show comprehensive stream statistics
        if let Ok(activities) = self.token_activities.try_read() {
            // Calculate time-based statistics for all streams
            let active_streams_5m = activities
                .values()
                .filter(|activity| {
                    if let Some(last_update) = activity.last_update {
                        last_update.elapsed().as_secs() <= 300
                    } else {
                        false
                    }
                })
                .count();

            // All-time active streams (tokens that have ever received events)
            let active_streams_all = activities
                .values()
                .filter(|activity| activity.event_count > 0)
                .count();

            let total_streams = activities.len();

            ui.horizontal(|ui| {
                ui.heading("📡 Market Streams");
                ui.separator();
                ui.label(format!(
                    "📊 {} active (5m) / {} active (all) / {} total",
                    active_streams_5m, active_streams_all, total_streams
                ));
            });
        } else {
            ui.horizontal(|ui| {
                ui.heading("📡 Market Streams");
                ui.separator();
                ui.colored_label(egui::Color32::GRAY, "⏳ Loading stream statistics...");
            });
        }

        ui.separator();

        // Show streaming status
        use crate::gui::components::market_data::streaming_status;
        let is_streaming = matches!(self.streaming_state, StreamingState::Connected);
        streaming_status(ui, is_streaming, self.streaming_assets.len());

        ui.separator();

        if !is_streaming {
            ui.label("Not currently streaming. Click 'Start Streaming' in the sidebar to begin.");

            if ui.button("▶️ Start Streaming").clicked() {
                *self.show_dataset_selector = true;
            }
        } else {
            // Create a vertical split - top for event stream, bottom for most active markets
            let available_height = ui.available_height();
            let half_height = available_height / 2.0;

            // Top section: Real-time Event Stream
            ui.group(|ui| {
                ui.set_height(half_height - 10.0);
                ui.vertical(|ui| {
                    ui.heading("🔴 Live Event Stream");
                    ui.separator();

                    egui::ScrollArea::vertical()
                        .id_salt("live_event_stream_scroll")
                        .max_height(half_height - 60.0)
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            // Show recent events (we'll need to implement event storage)
                            if let Ok(activities) = self.token_activities.try_read() {
                                // Collect all recent events sorted by time
                                let mut recent_events = Vec::new();

                                for activity in activities.values() {
                                    if let Some(last_update) = activity.last_update {
                                        recent_events.push((
                                            last_update,
                                            activity.token_id.clone(),
                                            activity.last_bid,
                                            activity.last_ask,
                                            activity.event_count,
                                        ));
                                    }
                                }

                                // Sort by timestamp (most recent first)
                                recent_events.sort_by(|a, b| b.0.cmp(&a.0));

                                // Display last 100 events
                                for (timestamp, token_id, bid, ask, _) in
                                    recent_events.iter().take(100)
                                {
                                    ui.horizontal(|ui| {
                                        // Timestamp
                                        let elapsed = timestamp.elapsed();
                                        let time_str = if elapsed.as_secs() < 60 {
                                            format!("{}s ago", elapsed.as_secs())
                                        } else if elapsed.as_secs() < 3600 {
                                            format!("{}m ago", elapsed.as_secs() / 60)
                                        } else {
                                            format!("{}h ago", elapsed.as_secs() / 3600)
                                        };
                                        ui.monospace(format!("[{}]", time_str));

                                        // Token ID (truncated)
                                        let token_display = if token_id.len() > 16 {
                                            format!("{}...", &token_id[..16])
                                        } else {
                                            token_id.clone()
                                        };
                                        ui.monospace(&token_display);

                                        // Bid/Ask
                                        if let Some(bid_price) = bid {
                                            ui.colored_label(
                                                egui::Color32::from_rgb(100, 200, 100),
                                                format!("BID: ${:.4}", bid_price),
                                            );
                                        }
                                        if let Some(ask_price) = ask {
                                            ui.colored_label(
                                                egui::Color32::from_rgb(200, 100, 100),
                                                format!("ASK: ${:.4}", ask_price),
                                            );
                                        }
                                    });
                                }

                                if recent_events.is_empty() {
                                    ui.label("Waiting for events...");
                                }
                            }
                        });
                });
            });

            ui.separator();

            // Bottom section: Most Active Markets
            ui.group(|ui| {
                ui.set_height(half_height - 10.0);
                ui.vertical(|ui| {
                    ui.heading("🔥 Most Active Markets");
                    ui.separator();

                    // Show all active token streams in a table
                    if let Ok(activities) = self.token_activities.try_read() {
                        if activities.is_empty() {
                            ui.label("Waiting for streaming data...");
                        } else {
                            // Sort activities by event count (most active first)
                            let mut sorted_activities: Vec<_> = activities.values().cloned().collect();
                            sorted_activities.sort_by(|a, b| b.event_count.cmp(&a.event_count));

                            ui.label(format!("Active streams: {} tokens", sorted_activities.len()));
                            ui.separator();

                            // Create scrollable table
                            egui::ScrollArea::vertical()
                                .id_salt("most_active_markets_scroll")
                                .max_height(half_height - 120.0)
                                .show(ui, |ui| {
                                    egui::Grid::new("active_markets_grid")
                                        .num_columns(8)
                                        .spacing([8.0, 4.0])
                                        .striped(true)
                                        .show(ui, |ui| {
                                    // Header
                                    ui.heading("Token");
                                    ui.heading("Events");
                                    ui.heading("Trades");
                                    ui.heading("Volume");
                                    ui.heading("Bid");
                                    ui.heading("Ask");
                                    ui.heading("Last Trade");
                                    ui.heading("Last Update");
                                    ui.end_row();

                                    // Data rows
                                    for activity in sorted_activities.iter().take(50) { // Limit display
                                        // Token ID (truncated)
                                        let token_display = if activity.token_id.len() > 12 {
                                            format!("{}...", &activity.token_id[..12])
                                        } else {
                                            activity.token_id.clone()
                                        };

                                        // Make clickable to select
                                        let response = ui.selectable_label(
                                            self.current_token_id.as_ref() == Some(&activity.token_id),
                                            &token_display
                                        );

                                        if response.clicked() {
                                            // Left click - open new orderbook
                                            *self.pending_new_orderbook = Some(activity.token_id.clone());
                                            info!("Left-clicked token to open new orderbook: {}", activity.token_id);
                                        }

                                        // Right click for additional options (future use)
                                        if response.secondary_clicked() {
                                            // Could add context menu here
                                            *self.pending_new_orderbook = Some(activity.token_id.clone());
                                            info!("Right-clicked token for new orderbook: {}", activity.token_id);
                                        }

                                        // Add tooltip
                                        response.on_hover_text("Click to open orderbook");

                                        // Event count
                                        ui.label(format!("{}", activity.event_count));

                                        // Trade count
                                        ui.label(format!("{}", activity.trade_count));

                                        // Volume
                                        ui.label(format!("${:.2}", activity.total_volume));

                                        // Bid price
                                        if let Some(bid) = activity.last_bid {
                                            ui.colored_label(
                                                egui::Color32::from_rgb(100, 200, 100),
                                                format!("${:.4}", bid)
                                            );
                                        } else {
                                            ui.label("-");
                                        }

                                        // Ask price
                                        if let Some(ask) = activity.last_ask {
                                            ui.colored_label(
                                                egui::Color32::from_rgb(200, 100, 100),
                                                format!("${:.4}", ask)
                                            );
                                        } else {
                                            ui.label("-");
                                        }

                                        // Last trade price
                                        if let Some(last_trade) = activity.last_trade_price {
                                            ui.colored_label(
                                                egui::Color32::from_rgb(150, 150, 255),
                                                format!("${:.4}", last_trade)
                                            );
                                        } else {
                                            ui.label("-");
                                        }

                                        // Last update time
                                        if let Some(last_update) = activity.last_update {
                                            let elapsed = last_update.elapsed().as_secs();
                                            if elapsed < 60 {
                                                ui.label(format!("{}s ago", elapsed));
                                            } else if elapsed < 3600 {
                                                ui.label(format!("{}m ago", elapsed / 60));
                                            } else {
                                                ui.label(format!("{}h ago", elapsed / 3600));
                                            }
                                        } else {
                                            ui.label("-");
                                        }

                                        ui.end_row();
                                    }
                                        });
                                });
                        }
                    } else {
                        ui.label("Loading stream data...");
                    }
                });
            });
        }
    }

    fn show_portfolio_pane(&mut self, ui: &mut egui::Ui) {
        // Show user address if available
        if let Some(user_address) = self.portfolio_service.get_user_address_sync() {
            ui.label(format!("👤 User: {}", user_address));
            ui.label(format!(
                "🔗 Profile: https://polymarket.com/profile/{}",
                user_address
            ));
            ui.separator();
        }

        // Show loading indicator
        if self.portfolio_service.is_refreshing_sync() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Fetching portfolio data...");
            });
            ui.separator();
        }

        // Show balance information if available
        if let Some(balance) = self.portfolio_service.get_balance_sync() {
            ui.horizontal(|ui| {
                ui.group(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("💰 Cash");
                        ui.label(format!("${:.2}", balance.cash));
                    });
                });

                ui.group(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("🎯 Bets");
                        ui.label(format!("${:.2}", balance.bets));
                    });
                });

                ui.group(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("📊 Total Equity");
                        ui.label(format!("${:.2}", balance.equity_total));
                    });
                });
            });
            ui.separator();
        }

        // Show portfolio stats from service
        if let Some(stats) = self.portfolio_service.get_stats_sync() {
            ui.horizontal(|ui| {
                ui.group(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("Total P&L");
                        let pnl = stats.total_pnl();
                        let color = if pnl >= rust_decimal::Decimal::ZERO {
                            egui::Color32::from_rgb(100, 200, 100)
                        } else {
                            egui::Color32::from_rgb(200, 100, 100)
                        };
                        ui.colored_label(color, format!("${:.2}", pnl));
                    });
                });

                ui.group(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("Win Rate");
                        if let Some(win_rate) = stats.win_rate {
                            ui.label(format!("{:.1}%", win_rate));
                        } else {
                            ui.label("N/A");
                        }
                    });
                });

                ui.group(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("Active Positions");
                        ui.label(format!("{}", stats.total_positions));
                    });
                });
            });
            ui.separator();
        }

        // Show positions from service
        let positions = self.portfolio_service.get_positions_sync();
        if positions.is_empty() && !self.portfolio_service.is_refreshing_sync() {
            ui.label("📊 No positions found");
            ui.label("💡 Your positions will appear here after placing trades");
        } else if !positions.is_empty() {
            ui.label(format!("📊 Positions ({} total)", positions.len()));
            ui.separator();

            // Positions table
            egui::ScrollArea::vertical()
                .id_salt("positions_pane_scroll")
                .max_height(400.0)
                .show(ui, |ui| {
                    egui::Grid::new("positions_grid")
                        .num_columns(8)
                        .spacing([8.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            // Header
                            ui.heading("Market");
                            ui.heading("Outcome");
                            ui.heading("Side");
                            ui.heading("Size");
                            ui.heading("Avg Price");
                            ui.heading("Current");
                            ui.heading("P&L");
                            ui.heading("Status");
                            ui.end_row();

                            // Position rows
                            for position in &positions {
                                // Market (truncated)
                                let market_display = if position.market_id.len() > 25 {
                                    format!("{}...", &position.market_id[..25])
                                } else {
                                    position.market_id.clone()
                                };
                                ui.label(market_display);

                                // Outcome
                                ui.label(&position.outcome);

                                // Side with color
                                let side_color = match position.side {
                                    crate::core::portfolio::PositionSide::Long => {
                                        egui::Color32::from_rgb(100, 200, 100)
                                    }
                                    crate::core::portfolio::PositionSide::Short => {
                                        egui::Color32::from_rgb(200, 100, 100)
                                    }
                                };
                                ui.colored_label(side_color, format!("{:?}", position.side));

                                // Size
                                ui.label(format!("{:.2}", position.size));

                                // Average price
                                ui.label(format!("${:.4}", position.average_price));

                                // Current price
                                let current_price = position
                                    .current_price
                                    .map(|p| format!("${:.4}", p))
                                    .unwrap_or_else(|| "N/A".to_string());
                                ui.label(current_price);

                                // P&L with color
                                let pnl = position.total_pnl();
                                let pnl_color = if pnl >= rust_decimal::Decimal::ZERO {
                                    egui::Color32::from_rgb(100, 200, 100)
                                } else {
                                    egui::Color32::from_rgb(200, 100, 100)
                                };
                                let pnl_str = if pnl >= rust_decimal::Decimal::ZERO {
                                    format!("+${:.2}", pnl)
                                } else {
                                    format!("-${:.2}", pnl.abs())
                                };
                                ui.colored_label(pnl_color, pnl_str);

                                // Status
                                let status_color = match position.status {
                                    crate::core::portfolio::PositionStatus::Open => {
                                        egui::Color32::from_rgb(100, 200, 100)
                                    }
                                    crate::core::portfolio::PositionStatus::Closed => egui::Color32::GRAY,
                                    crate::core::portfolio::PositionStatus::Liquidated => {
                                        egui::Color32::from_rgb(200, 100, 100)
                                    }
                                };
                                ui.colored_label(status_color, format!("{:?}", position.status));

                                ui.end_row();
                            }
                        });
                });
        }

        ui.separator();

        if ui.button("🔄 Refresh Portfolio").clicked() {
            info!("Portfolio is automatically updated via WebSocket stream");
            // Note: Portfolio is updated automatically via WebSocket events
        }
    }

    fn show_tokens_pane(&mut self, ui: &mut egui::Ui) {
        ui.label("Token data and selection interface coming soon.");

        // TODO: Port token functionality from ratatui version
    }

    fn show_market_depth_pane(&mut self, ui: &mut egui::Ui, pane_token_id: &Option<String>) {
        if let Some(token_id) = pane_token_id {
            ui.label(format!("Market depth for: {}", token_id));
            ui.separator();

            // Update current token if this pane has a different token
            if self.current_token_id.as_ref() != Some(token_id) {
                *self.current_token_id = Some(token_id.clone());

                // Notify background task of current token change
                if let Some(sender) = self.current_token_sender {
                    let _ = sender.send(Some(token_id.clone()));
                }
            }

            // Get orderbook data for this specific token
            if let Some(_streaming_service) = &self.streaming_service {
                // Display cached last trade price if available
                if let Some((last_price, timestamp)) = &self.cached_last_trade_price {
                    ui.horizontal(|ui| {
                        ui.group(|ui| {
                            ui.vertical_centered(|ui| {
                                ui.label("Last Trade Price");
                                ui.heading(format!("${:.4}", last_price));

                                // Format timestamp
                                let dt = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(
                                    *timestamp as i64,
                                )
                                .unwrap_or_else(chrono::Utc::now);
                                ui.label(format!("{}", dt.format("%H:%M:%S UTC")));
                            });
                        });
                    });
                    ui.separator();
                }

                if let Some(order_book) = &self.cached_orderbook {
                    let bids = order_book.get_bids();
                    let asks = order_book.get_asks();

                    // Enhanced order book display with depth visualization
                    use crate::gui::components::market_data::order_book_display_enhanced;

                    // Filter changes for this specific token
                    let changes: Vec<(Decimal, Decimal, Instant, bool)> = self
                        .orderbook_changes
                        .iter()
                        .filter(|change| change.token_id == *token_id)
                        .map(|change| (change.price, change.size, change.changed_at, change.is_bid))
                        .collect();

                    order_book_display_enhanced(ui, &bids[..], &asks[..], &changes);

                    // Additional market depth metrics
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.group(|ui| {
                            ui.vertical_centered(|ui| {
                                ui.label("Bid Levels");
                                ui.heading(format!("{}", bids.len()));
                            });
                        });

                        ui.group(|ui| {
                            ui.vertical_centered(|ui| {
                                ui.label("Ask Levels");
                                ui.heading(format!("{}", asks.len()));
                            });
                        });

                        if !bids.is_empty() && !asks.is_empty() {
                            let spread = asks[0].price - bids[0].price;
                            let spread_pct = (spread / asks[0].price) * Decimal::from(100);

                            ui.group(|ui| {
                                ui.vertical_centered(|ui| {
                                    ui.label("Spread");
                                    ui.heading(format!("{:.2}%", spread_pct));
                                });
                            });
                        }
                    });
                } else {
                    ui.label("No orderbook data available");
                    ui.label("Waiting for orderbook data...");

                    // Show helpful information
                    ui.separator();
                    ui.label("Troubleshooting:");
                    ui.label("• Ensure the token is being streamed");
                    ui.label("• Check that streaming service is connected");
                    ui.label(format!("• Current token: {}", token_id));

                    if let Some(_streaming_service) = &self.streaming_service {
                        // Show debug info
                        ui.separator();
                        ui.label("Debug info:");
                        ui.label("• Background task should be fetching orderbook data");
                        ui.label("• Check application logs for fetch attempts");
                    }
                }
            } else {
                ui.label("Streaming service not available");
                ui.label("Start streaming to see market depth data.");

                ui.separator();
                ui.label("To start streaming:");
                ui.label("1. Go to the Streams tab");
                ui.label("2. Click 'Start Streaming'");
                ui.label("3. Add tokens to stream");
            }
        } else {
            ui.label("This orderbook pane has no token assigned");

            if ui.button("📡 Open Streams Panel").clicked() {
                info!("User requested to open streams panel");
            }
        }
    }

    fn show_charts_pane(&mut self, ui: &mut egui::Ui) {
        ui.label("Price charts and technical analysis coming soon.");

        // TODO: Implement charting with egui_plot
    }

    fn show_trade_history_pane(&mut self, ui: &mut egui::Ui) {
        ui.label("Historical trade data will be displayed here.");

        // TODO: Implement trade history display
    }

    fn show_balances_pane(&mut self, ui: &mut egui::Ui) {
        ui.label("Account balance information coming soon.");

        // TODO: Implement balance display
    }

    fn show_websocket_manager_pane(&mut self, ui: &mut egui::Ui) {
        ui.label("🔌 WebSocket Manager");
        ui.separator();

        // Check if streaming service is available
        if self.streaming_service.is_none() {
            ui.label("⚠️ No streaming service available");
            ui.label("Start streaming to see WebSocket connections");
            return;
        }

        ui.spacing_mut().item_spacing.y = 8.0;

        // Aggregate Stats at the top
        ui.group(|ui| {
            ui.label("📊 Service Statistics");

            if let Some(stats) = &self.cached_streaming_stats {
                // Calculate active vs total streams with time-based statistics
                let token_activities = if let Ok(activities) = self.token_activities.try_read() {
                    activities.clone()
                } else {
                    HashMap::new()
                };

                // Active streams in last 5 minutes (300 seconds)
                let active_streams_5m = token_activities
                    .values()
                    .filter(|activity| {
                        if let Some(last_update) = activity.last_update {
                            last_update.elapsed().as_secs() <= 300
                        } else {
                            false
                        }
                    })
                    .count();

                // All-time active streams (tokens that have ever received events)
                let active_streams_all = token_activities
                    .values()
                    .filter(|activity| activity.event_count > 0)
                    .count();

                let total_streams = token_activities.len();

                ui.horizontal(|ui| {
                    ui.label(format!("📦 Total Events: {}", stats.total_events_received));
                    ui.separator();

                    ui.label(format!(
                        "📊 Streams: {} active (5m) / {} active (all) / {} total",
                        active_streams_5m, active_streams_all, total_streams
                    ));
                    ui.separator();

                    // Calculate total bitrate (approximation based on events/sec)
                    let bitrate_kb = stats.events_per_second * 0.5; // Assume ~500 bytes per event
                    ui.label(format!("📊 Bitrate: {:.1} KB/s", bitrate_kb));
                    ui.separator();

                    // Format uptime
                    let uptime_hours = stats.uptime_seconds / 3600;
                    let uptime_mins = (stats.uptime_seconds % 3600) / 60;
                    let uptime_secs = stats.uptime_seconds % 60;
                    let uptime_display = if uptime_hours > 0 {
                        format!("{}h {}m {}s", uptime_hours, uptime_mins, uptime_secs)
                    } else if uptime_mins > 0 {
                        format!("{}m {}s", uptime_mins, uptime_secs)
                    } else {
                        format!("{}s", uptime_secs)
                    };
                    ui.label(format!("⏰ Service Uptime: {}", uptime_display));
                });
            } else {
                ui.label("⏳ Loading statistics...");
            }
        });

        ui.separator();

        // Workers Table
        ui.group(|ui| {
            ui.label("👷 Worker Connections");
            ui.small("💡 Tip: Double-click a worker row to open detailed view");

            // Use cached worker statuses
            let worker_statuses = &self.cached_worker_statuses;

            if worker_statuses.is_empty() {
                ui.label("No active workers");
            } else {
                // Create table with improved scroll area
                egui::ScrollArea::vertical()
                    .id_salt("websocket_manager_worker_table_scroll")
                    .max_height(200.0)
                    .auto_shrink([false; 2])
                    .stick_to_bottom(false)
                    .show(ui, |ui| {
                        egui::Grid::new("worker_table")
                            .striped(true)
                            .min_col_width(80.0)
                            .show(ui, |ui| {
                                // Header
                                ui.strong("Worker ID");
                                ui.strong("Status");
                                ui.strong("Events Count ▼"); // Default sort indicator
                                ui.strong("Events/sec");
                                ui.strong("Tokens");
                                ui.strong("Last Activity");
                                ui.strong("Uptime");
                                ui.strong("Errors");
                                ui.end_row();

                                // Sort workers by events count (descending)
                                let mut sorted_workers = worker_statuses.to_vec();
                                sorted_workers
                                    .sort_by(|a, b| b.events_processed.cmp(&a.events_processed));

                                for worker in sorted_workers {
                                    let is_selected =
                                        *self.selected_worker_id == Some(worker.worker_id);

                                    // Create clickable area for entire row
                                    let row_response = ui.allocate_response(
                                        egui::vec2(
                                            ui.available_width(),
                                            ui.spacing().interact_size.y + 4.0,
                                        ),
                                        egui::Sense::click(),
                                    );

                                    // Handle row interactions
                                    if row_response.clicked() {
                                        *self.selected_worker_id = Some(worker.worker_id);
                                        // Clear previous worker events when selecting new worker
                                        self.worker_stream_events.clear();
                                    }

                                    if row_response.double_clicked() {
                                        // Open worker details pane on double-click
                                        *self.pending_new_worker_details = Some(worker.worker_id);
                                    }

                                    // Highlight selected row
                                    if is_selected {
                                        ui.painter().rect_filled(
                                            row_response.rect,
                                            2.0,
                                            egui::Color32::from_rgba_premultiplied(
                                                100, 149, 237, 40,
                                            ),
                                        );
                                    }

                                    // Highlight row on hover
                                    if row_response.hovered() {
                                        ui.painter().rect_stroke(
                                            row_response.rect,
                                            2.0,
                                            egui::Stroke::new(
                                                1.0,
                                                egui::Color32::from_rgba_premultiplied(
                                                    100, 149, 237, 80,
                                                ),
                                            ),
                                            egui::epaint::StrokeKind::Inside,
                                        );
                                    }

                                    // Worker ID
                                    ui.label(format!("#{}", worker.worker_id));

                                    // Status
                                    if worker.is_connected {
                                        ui.colored_label(egui::Color32::GREEN, "● Connected");
                                    } else if worker.last_error.is_some() {
                                        ui.colored_label(egui::Color32::RED, "● Failed");
                                    } else {
                                        ui.colored_label(egui::Color32::YELLOW, "● Disconnected");
                                    }

                                    // Events Count
                                    ui.label(format!("{}", worker.events_processed));

                                    // Events/sec (calculate from stats)
                                    let events_per_sec =
                                        if let Some(stats) = &self.cached_streaming_stats {
                                            if stats.active_connections > 0 {
                                                stats.events_per_second
                                                    / stats.active_connections as f64
                                            } else {
                                                0.0
                                            }
                                        } else {
                                            0.0
                                        };
                                    ui.label(format!("{:.1}", events_per_sec));

                                    // Tokens Count
                                    ui.label(format!("{}", worker.assigned_tokens.len()));

                                    // Last Activity (time ago format)
                                    let elapsed = worker.last_activity.elapsed();
                                    let activity_str = if elapsed.as_secs() < 60 {
                                        format!("{}s ago", elapsed.as_secs())
                                    } else if elapsed.as_secs() < 3600 {
                                        format!("{}m ago", elapsed.as_secs() / 60)
                                    } else {
                                        format!("{}h ago", elapsed.as_secs() / 3600)
                                    };
                                    ui.label(activity_str);

                                    // Uptime (same as service uptime for now)
                                    if let Some(stats) = &self.cached_streaming_stats {
                                        let mins = stats.uptime_seconds / 60;
                                        if mins < 60 {
                                            ui.label(format!("{}m", mins));
                                        } else {
                                            ui.label(format!("{}h {}m", mins / 60, mins % 60));
                                        }
                                    } else {
                                        ui.label("-");
                                    }

                                    // Errors Count
                                    if worker.last_error.is_some() {
                                        ui.colored_label(egui::Color32::ORANGE, "1");
                                    } else {
                                        ui.label("0");
                                    }

                                    ui.end_row();
                                }
                            });
                    });
            }
        });

        ui.separator();

        // Worker Event Stream (when worker is selected)
        if let Some(worker_id) = *self.selected_worker_id {
            ui.group(|ui| {
                ui.label(format!("📡 Event Stream - Worker #{}", worker_id));

                // Collect events for this worker from event receiver
                if let Some(receiver) = &mut self.event_receiver {
                    // Try to receive events without blocking
                    while let Ok(event) = receiver.try_recv() {
                        // Add to worker events (keep last N events)
                        self.worker_stream_events.push(event);
                        if self.worker_stream_events.len() > *self.worker_stream_max_events {
                            self.worker_stream_events.remove(0);
                        }
                    }
                }

                // Display events as a proper table
                egui::ScrollArea::vertical()
                    .id_salt("websocket_manager_events_scroll")
                    .max_height(300.0)
                    .auto_shrink([false; 2])
                    .stick_to_bottom(false)
                    .show(ui, |ui| {
                        if self.worker_stream_events.is_empty() {
                            ui.label("No events captured yet...");
                        } else {
                            // Create table for events
                            egui::Grid::new("worker_events_table")
                                .striped(true)
                                .min_col_width(60.0)
                                .show(ui, |ui| {
                                    // Table header
                                    ui.strong("Timestamp");
                                    ui.strong("Event Type");
                                    ui.strong("Asset ID");
                                    ui.strong("Price");
                                    ui.strong("Size");
                                    ui.strong("Side");
                                    ui.end_row();

                                    // Show events in reverse order (newest first)
                                    for event in self.worker_stream_events.iter().rev() {
                                        // Timestamp
                                        let timestamp = chrono::Local::now().format("%H:%M:%S");
                                        ui.monospace(format!("{}", timestamp));

                                        // Event type with color coding
                                        match event {
                                            PolyEvent::Trade { .. } => {
                                                ui.colored_label(egui::Color32::GREEN, "TRADE");
                                            }
                                            PolyEvent::PriceChange { .. } => {
                                                ui.colored_label(
                                                    egui::Color32::LIGHT_BLUE,
                                                    "PRICE",
                                                );
                                            }
                                            PolyEvent::Book { .. } => {
                                                ui.colored_label(egui::Color32::YELLOW, "BOOK");
                                            }
                                            PolyEvent::TickSizeChange { .. } => {
                                                ui.colored_label(egui::Color32::GRAY, "TICK_SIZE");
                                            }
                                            PolyEvent::LastTradePrice { .. } => {
                                                ui.colored_label(
                                                    egui::Color32::LIGHT_GRAY,
                                                    "LAST_PRICE",
                                                );
                                            }
                                            PolyEvent::MyOrder { .. } => {
                                                ui.colored_label(egui::Color32::BLUE, "MY_ORDER");
                                            }
                                            PolyEvent::MyTrade { .. } => {
                                                ui.colored_label(
                                                    egui::Color32::DARK_GREEN,
                                                    "MY_TRADE",
                                                );
                                            }
                                        }

                                        // Extract data for table columns
                                        let (asset_id, price, size, side) = match event {
                                            PolyEvent::Trade {
                                                asset_id,
                                                price,
                                                size,
                                                side,
                                            } => (
                                                asset_id.clone(),
                                                Some(*price),
                                                Some(*size),
                                                Some(format!("{:?}", side)),
                                            ),
                                            PolyEvent::PriceChange {
                                                asset_id,
                                                price,
                                                size,
                                                side,
                                                ..
                                            } => (
                                                asset_id.clone(),
                                                Some(*price),
                                                Some(*size),
                                                Some(format!("{:?}", side)),
                                            ),
                                            PolyEvent::Book { asset_id, .. } => {
                                                (asset_id.clone(), None, None, None)
                                            }
                                            PolyEvent::TickSizeChange { asset_id, .. } => {
                                                (asset_id.clone(), None, None, None)
                                            }
                                            PolyEvent::LastTradePrice {
                                                asset_id, price, ..
                                            } => (asset_id.clone(), Some(*price), None, None),
                                            PolyEvent::MyOrder {
                                                asset_id,
                                                price,
                                                size,
                                                side,
                                                ..
                                            } => (
                                                asset_id.clone(),
                                                Some(*price),
                                                Some(*size),
                                                Some(format!("{:?}", side)),
                                            ),
                                            PolyEvent::MyTrade {
                                                asset_id,
                                                price,
                                                size,
                                                side,
                                            } => (
                                                asset_id.clone(),
                                                Some(*price),
                                                Some(*size),
                                                Some(format!("{:?}", side)),
                                            ),
                                        };

                                        // Asset ID (truncated)
                                        let display_asset_id = if asset_id.len() > 12 {
                                            format!("{}...", &asset_id[..12])
                                        } else {
                                            asset_id
                                        };
                                        ui.monospace(display_asset_id);

                                        // Price
                                        if let Some(price) = price {
                                            ui.monospace(format!("{:.4}", price));
                                        } else {
                                            ui.label("-");
                                        }

                                        // Size
                                        if let Some(size) = size {
                                            ui.monospace(format!("{:.2}", size));
                                        } else {
                                            ui.label("-");
                                        }

                                        // Side
                                        if let Some(side) = side {
                                            match side.as_str() {
                                                "Buy" => {
                                                    ui.colored_label(egui::Color32::GREEN, "BUY")
                                                }
                                                "Sell" => {
                                                    ui.colored_label(egui::Color32::RED, "SELL")
                                                }
                                                _ => ui.label(side),
                                            };
                                        } else {
                                            ui.label("-");
                                        }

                                        ui.end_row();
                                    }
                                });
                        }
                    });

                // Event stats for this worker
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "📊 Showing last {} events",
                        self.worker_stream_events.len()
                    ));
                    if ui.button("Clear").clicked() {
                        self.worker_stream_events.clear();
                    }
                });
            });
        } else {
            ui.group(|ui| {
                ui.label("📡 Event Stream");
                ui.label("Select a worker from the table above to view its event stream");
            });
        }
    }

    fn show_worker_details_pane(&mut self, ui: &mut egui::Ui, worker_id: usize) {
        // Calculate active vs total streams for this worker with time-based statistics
        let worker_status = self
            .cached_worker_statuses
            .iter()
            .find(|w| w.worker_id == worker_id);

        let (active_streams_5m, active_streams_all, total_streams) =
            if let Some(worker) = worker_status {
                let token_activities = if let Ok(activities) = self.token_activities.try_read() {
                    activities.clone()
                } else {
                    HashMap::new()
                };

                // Active streams in last 5 minutes (300 seconds)
                let active_count_5m = worker
                    .assigned_tokens
                    .iter()
                    .filter(|token_id| {
                        if let Some(activity) = token_activities.get(*token_id) {
                            if let Some(last_update) = activity.last_update {
                                last_update.elapsed().as_secs() <= 300
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    })
                    .count();

                // All-time active streams (tokens that have ever received events)
                let active_count_all = worker
                    .assigned_tokens
                    .iter()
                    .filter(|token_id| {
                        if let Some(activity) = token_activities.get(*token_id) {
                            activity.event_count > 0
                        } else {
                            false
                        }
                    })
                    .count();

                (
                    active_count_5m,
                    active_count_all,
                    worker.assigned_tokens.len(),
                )
            } else {
                (0, 0, 0)
            };

        ui.horizontal(|ui| {
            ui.label(format!("👷 Worker #{} Details", worker_id));
            ui.separator();
            ui.label(format!(
                "📊 {} active (5m) / {} active (all) / {} total streams",
                active_streams_5m, active_streams_all, total_streams
            ));
        });
        ui.separator();

        // Check if streaming service is available
        if self.streaming_service.is_none() {
            ui.label("⚠️ No streaming service available");
            ui.label("Start streaming to see worker details");
            return;
        }

        // Find the specific worker in the cached statuses
        let worker_status = self
            .cached_worker_statuses
            .iter()
            .find(|w| w.worker_id == worker_id);

        if let Some(worker) = worker_status {
            // Worker Information Section
            ui.group(|ui| {
                ui.label("📋 Worker Information");
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Status:");
                    if worker.is_connected {
                        ui.colored_label(egui::Color32::GREEN, "● Connected");
                    } else if worker.last_error.is_some() {
                        ui.colored_label(egui::Color32::RED, "● Failed");
                    } else {
                        ui.colored_label(egui::Color32::YELLOW, "● Disconnected");
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Events Processed:");
                    ui.strong(format!("{}", worker.events_processed));
                });

                ui.horizontal(|ui| {
                    ui.label("Last Activity:");
                    let elapsed = worker.last_activity.elapsed();
                    let activity_str = if elapsed.as_secs() < 60 {
                        format!("{}s ago", elapsed.as_secs())
                    } else if elapsed.as_secs() < 3600 {
                        format!("{}m ago", elapsed.as_secs() / 60)
                    } else {
                        format!("{}h ago", elapsed.as_secs() / 3600)
                    };
                    ui.label(activity_str);
                });

                if let Some(error) = &worker.last_error {
                    ui.horizontal(|ui| {
                        ui.label("Last Error:");
                        ui.colored_label(egui::Color32::RED, error);
                    });
                }
            });

            ui.add_space(8.0);

            // Section 2: Current token selection
            ui.group(|ui| {
                ui.label("🪙 Token Selection");
                ui.separator();

                if worker.assigned_tokens.is_empty() {
                    ui.label("No tokens assigned to this worker");
                } else {
                    // Create a scrollable area for tokens
                    egui::ScrollArea::vertical()
                        .id_salt("worker_details_tokens_scroll")
                        .max_height(200.0)
                        .show(ui, |ui| {
                            // Create a table for tokens
                            egui::Grid::new("worker_tokens_table")
                                .striped(true)
                                .min_col_width(100.0)
                                .show(ui, |ui| {
                                    // Header
                                    ui.strong("Token ID");
                                    ui.strong("Events");
                                    ui.strong("Active");
                                    ui.strong("Last Update");
                                    ui.end_row();

                                    // Get token activities for comparison
                                    let token_activities =
                                        if let Ok(activities) = self.token_activities.try_read() {
                                            activities.clone()
                                        } else {
                                            HashMap::new()
                                        };

                                    // Sort tokens by event count (descending)
                                    let mut sorted_tokens = worker.assigned_tokens.clone();
                                    sorted_tokens.sort_by(|a, b| {
                                        let count_a = token_activities
                                            .get(a)
                                            .map(|ta| ta.event_count)
                                            .unwrap_or(0);
                                        let count_b = token_activities
                                            .get(b)
                                            .map(|ta| ta.event_count)
                                            .unwrap_or(0);
                                        count_b.cmp(&count_a)
                                    });

                                    for token_id in sorted_tokens {
                                        // Truncate token ID for display
                                        let display_id = if token_id.len() > 16 {
                                            format!("{}...", &token_id[..16])
                                        } else {
                                            token_id.clone()
                                        };
                                        ui.monospace(display_id);

                                        // Event count from token activities
                                        if let Some(activity) = token_activities.get(&token_id) {
                                            ui.label(format!("{}", activity.event_count));

                                            // Active indicator (recent activity within 5 minutes)
                                            if let Some(last_update) = activity.last_update {
                                                let elapsed = last_update.elapsed();
                                                if elapsed.as_secs() <= 300 {
                                                    ui.colored_label(
                                                        egui::Color32::GREEN,
                                                        "● Active",
                                                    );
                                                } else {
                                                    ui.colored_label(
                                                        egui::Color32::GRAY,
                                                        "○ Inactive",
                                                    );
                                                }

                                                // Last update
                                                let update_str = if elapsed.as_secs() < 60 {
                                                    format!("{}s ago", elapsed.as_secs())
                                                } else if elapsed.as_secs() < 3600 {
                                                    format!("{}m ago", elapsed.as_secs() / 60)
                                                } else {
                                                    format!("{}h ago", elapsed.as_secs() / 3600)
                                                };
                                                ui.label(update_str);
                                            } else {
                                                ui.colored_label(
                                                    egui::Color32::GRAY,
                                                    "○ No activity",
                                                );
                                                ui.label("-");
                                            }
                                        } else {
                                            ui.label("0");
                                            ui.colored_label(egui::Color32::GRAY, "○ No data");
                                            ui.label("-");
                                        }

                                        ui.end_row();
                                    }
                                });
                        });

                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(format!("Total Tokens: {}", worker.assigned_tokens.len()));
                        ui.separator();

                        // Count active tokens
                        let token_activities =
                            if let Ok(activities) = self.token_activities.try_read() {
                                activities.clone()
                            } else {
                                HashMap::new()
                            };

                        let active_count_5m = worker
                            .assigned_tokens
                            .iter()
                            .filter(|token_id| {
                                if let Some(activity) = token_activities.get(*token_id) {
                                    if let Some(last_update) = activity.last_update {
                                        last_update.elapsed().as_secs() <= 300
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                }
                            })
                            .count();

                        let active_count_all = worker
                            .assigned_tokens
                            .iter()
                            .filter(|token_id| {
                                if let Some(activity) = token_activities.get(*token_id) {
                                    activity.event_count > 0
                                } else {
                                    false
                                }
                            })
                            .count();

                        ui.label(format!(
                            "Active: {} (5m) / {} (all)",
                            active_count_5m, active_count_all
                        ));
                    });
                }
            });

            ui.add_space(8.0);

            // Section 1: Stream of data - Show real-time events for this specific worker
            ui.group(|ui| {
                ui.label("📡 Event Stream");
                ui.separator();

                // Collect events for this worker from event receiver
                if let Some(receiver) = &mut self.event_receiver {
                    // Try to receive events without blocking
                    while let Ok(event) = receiver.try_recv() {
                        // Add to worker events (keep last N events)
                        self.worker_stream_events.push(event);
                        if self.worker_stream_events.len() > *self.worker_stream_max_events {
                            self.worker_stream_events.remove(0);
                        }
                    }
                }

                // Display events as a proper table
                egui::ScrollArea::vertical()
                    .id_salt("worker_details_events_scroll")
                    .max_height(300.0)
                    .auto_shrink([false; 2])
                    .stick_to_bottom(false)
                    .show(ui, |ui| {
                        if self.worker_stream_events.is_empty() {
                            ui.label("No events captured yet...");
                            ui.label("Events will appear here when the worker processes them");
                        } else {
                            // Create table for events
                            egui::Grid::new("worker_details_events_table")
                                .striped(true)
                                .min_col_width(60.0)
                                .show(ui, |ui| {
                                    // Table header
                                    ui.strong("Timestamp");
                                    ui.strong("Event Type");
                                    ui.strong("Asset ID");
                                    ui.strong("Price");
                                    ui.strong("Size");
                                    ui.strong("Side");
                                    ui.end_row();

                                    // Show events in reverse order (newest first)
                                    for event in self.worker_stream_events.iter().rev() {
                                        // Timestamp
                                        let timestamp = chrono::Local::now().format("%H:%M:%S");
                                        ui.monospace(format!("{}", timestamp));

                                        // Event type with color coding
                                        match event {
                                            PolyEvent::Trade { .. } => {
                                                ui.colored_label(egui::Color32::GREEN, "TRADE");
                                            }
                                            PolyEvent::PriceChange { .. } => {
                                                ui.colored_label(
                                                    egui::Color32::LIGHT_BLUE,
                                                    "PRICE",
                                                );
                                            }
                                            PolyEvent::Book { .. } => {
                                                ui.colored_label(egui::Color32::YELLOW, "BOOK");
                                            }
                                            PolyEvent::TickSizeChange { .. } => {
                                                ui.colored_label(egui::Color32::GRAY, "TICK_SIZE");
                                            }
                                            PolyEvent::LastTradePrice { .. } => {
                                                ui.colored_label(
                                                    egui::Color32::LIGHT_GRAY,
                                                    "LAST_PRICE",
                                                );
                                            }
                                            PolyEvent::MyOrder { .. } => {
                                                ui.colored_label(egui::Color32::BLUE, "MY_ORDER");
                                            }
                                            PolyEvent::MyTrade { .. } => {
                                                ui.colored_label(
                                                    egui::Color32::DARK_GREEN,
                                                    "MY_TRADE",
                                                );
                                            }
                                        }

                                        // Extract data for table columns
                                        let (asset_id, price, size, side) = match event {
                                            PolyEvent::Trade {
                                                asset_id,
                                                price,
                                                size,
                                                side,
                                            } => (
                                                asset_id.clone(),
                                                Some(*price),
                                                Some(*size),
                                                Some(format!("{:?}", side)),
                                            ),
                                            PolyEvent::PriceChange {
                                                asset_id,
                                                price,
                                                size,
                                                side,
                                                ..
                                            } => (
                                                asset_id.clone(),
                                                Some(*price),
                                                Some(*size),
                                                Some(format!("{:?}", side)),
                                            ),
                                            PolyEvent::Book { asset_id, .. } => {
                                                (asset_id.clone(), None, None, None)
                                            }
                                            PolyEvent::TickSizeChange { asset_id, .. } => {
                                                (asset_id.clone(), None, None, None)
                                            }
                                            PolyEvent::LastTradePrice {
                                                asset_id, price, ..
                                            } => (asset_id.clone(), Some(*price), None, None),
                                            PolyEvent::MyOrder {
                                                asset_id,
                                                price,
                                                size,
                                                side,
                                                ..
                                            } => (
                                                asset_id.clone(),
                                                Some(*price),
                                                Some(*size),
                                                Some(format!("{:?}", side)),
                                            ),
                                            PolyEvent::MyTrade {
                                                asset_id,
                                                price,
                                                size,
                                                side,
                                            } => (
                                                asset_id.clone(),
                                                Some(*price),
                                                Some(*size),
                                                Some(format!("{:?}", side)),
                                            ),
                                        };

                                        // Asset ID (truncated)
                                        let display_asset_id = if asset_id.len() > 12 {
                                            format!("{}...", &asset_id[..12])
                                        } else {
                                            asset_id
                                        };
                                        ui.monospace(display_asset_id);

                                        // Price
                                        if let Some(price) = price {
                                            ui.monospace(format!("{:.4}", price));
                                        } else {
                                            ui.label("-");
                                        }

                                        // Size
                                        if let Some(size) = size {
                                            ui.monospace(format!("{:.2}", size));
                                        } else {
                                            ui.label("-");
                                        }

                                        // Side
                                        if let Some(side) = side {
                                            match side.as_str() {
                                                "Buy" => {
                                                    ui.colored_label(egui::Color32::GREEN, "BUY")
                                                }
                                                "Sell" => {
                                                    ui.colored_label(egui::Color32::RED, "SELL")
                                                }
                                                _ => ui.label(side),
                                            };
                                        } else {
                                            ui.label("-");
                                        }

                                        ui.end_row();
                                    }
                                });
                        }
                    });

                // Event stats and controls
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "📊 Showing last {} events for Worker #{}",
                        self.worker_stream_events.len(),
                        worker_id
                    ));
                    if ui.button("Clear").clicked() {
                        self.worker_stream_events.clear();
                    }
                });
            });
        } else {
            ui.label(format!("⚠️ Worker #{} not found", worker_id));
            ui.label("This worker may no longer be active");
        }
    }

    /// Abstract close logic for panes - handles adding tile to close list
    fn close_pane(&mut self, tile_id: TileId) {
        self.tiles_to_close.push(tile_id);
        info!("Pane {} queued for closing", tile_id.0);
    }

    /// Draw the X symbol for close buttons (abstracted for reuse)
    fn draw_close_button_x(&self, ui: &mut egui::Ui, center: egui::Pos2, color: egui::Color32) {
        let size = 6.0;
        ui.painter().line_segment(
            [
                center + egui::vec2(-size, -size),
                center + egui::vec2(size, size),
            ],
            egui::Stroke::new(2.0, color),
        );
        ui.painter().line_segment(
            [
                center + egui::vec2(-size, size),
                center + egui::vec2(size, -size),
            ],
            egui::Stroke::new(2.0, color),
        );
    }
}
