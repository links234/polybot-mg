//! Stream command for real-time WebSocket data

use anyhow::Result;
use clap::Args;
use tracing::{info, warn, error, debug};
use tokio::signal;
use owo_colors::OwoColorize;
use std::time::Duration;
use std::fs;
use serde_json::Value;

use crate::data_paths::DataPaths;
use crate::logging::{LogMode, LoggingConfig, init_logging};
use crate::services::{Streamer, StreamerConfig};
use crate::ws::{WsConfig, AuthPayload, PolyEvent, Side};
use crate::tui::{App, EventHandler, ui, events};
use crate::datasets::SelectionManager;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};
use std::sync::Arc;
use futures::FutureExt;

#[derive(Args, Clone)]
pub struct StreamArgs {
    /// Asset IDs to stream (comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub assets: Vec<String>,
    
    /// Path to markets.json file to load asset IDs from
    #[arg(long)]
    pub markets_path: Option<String>,
    
    /// Use a saved token selection
    #[arg(long)]
    pub selection: Option<String>,
    
    /// Markets for user feed (comma-separated, requires auth)
    #[arg(long, value_delimiter = ',')]
    pub markets: Option<Vec<String>>,
    
    /// API key for user feed authentication
    #[arg(long)]
    pub api_key: Option<String>,
    
    /// API secret for user feed authentication
    #[arg(long)]
    pub secret: Option<String>,
    
    /// API passphrase for user feed authentication
    #[arg(long)]
    pub passphrase: Option<String>,
    
    /// WebSocket heartbeat interval in seconds
    #[arg(long, default_value = "10")]
    pub heartbeat_interval: u64,
    
    /// Maximum reconnection attempts (0 = infinite)
    #[arg(long, default_value = "0")]
    pub max_reconnection_attempts: u32,
    
    /// Show order book updates
    #[arg(long)]
    pub show_book: bool,
    
    /// Show trade updates
    #[arg(long)]
    pub show_trades: bool,
    
    /// Show user order/trade updates
    #[arg(long)]
    pub show_user: bool,
    
    /// Print order book summary every N seconds
    #[arg(long)]
    pub summary_interval: Option<u64>,
    
    /// Use sandbox environment
    #[arg(long)]
    pub sandbox: bool,
    
    /// Use TUI interface (default: true, use --no-tui to disable)
    #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
    pub tui: bool,
}

pub struct StreamCommand {
    args: StreamArgs,
}

impl StreamCommand {
    pub fn new(args: StreamArgs) -> Self {
        Self { args }
    }

    pub async fn execute(&self, host: &str, data_paths: DataPaths) -> Result<()> {
        if self.args.tui {
            // For TUI mode: Use file-only logging but show nice console progress first
            self.execute_tui_with_progress(host, data_paths).await
        } else {
            // For CLI mode: Use console and file logging throughout
            let logging_config = LoggingConfig::new(LogMode::ConsoleAndFile, data_paths.clone());
            init_logging(logging_config)?;
            self.execute_cli(host, data_paths).await
        }
    }
    
    async fn execute_tui_with_progress(&self, host: &str, data_paths: DataPaths) -> Result<()> {
        // Initialize file-only logging for TUI mode
        let logging_config = LoggingConfig::new(LogMode::FileOnly, data_paths.clone());
        init_logging(logging_config)?;
        
        // Now use direct println! for nice console output without log prefixes
        println!("\n🚀 Starting Polymarket WebSocket Stream");
        println!("═══════════════════════════════════════════");
        
        // Load assets and show progress
        let assets = self.get_assets_for_streaming(&data_paths).await?;

        if assets.is_empty() {
            eprintln!("\n❌ Error: No assets were selected or provided");
            eprintln!("   Use --assets, --markets-path, or --selection");
            return Err(anyhow::anyhow!("No assets specified"));
        }

        // Show asset information
        println!("📊 Streaming {} assets:", assets.len());
        for (i, token) in assets.iter().take(5).enumerate() {
            println!("   {} {}", 
                if i == assets.len() - 1 || i == 4 { "└" } else { "├" },
                token
            );
        }
        if assets.len() > 5 {
            println!("   └ ... and {} more", assets.len() - 5);
        }

        // Configure WebSocket and show progress
        println!("\n🔧 Configuration:");
        println!("   ├ Host: {}", host);
        println!("   ├ Heartbeat: {}s", self.args.heartbeat_interval);
        println!("   └ Reconnect attempts: {}", 
            if self.args.max_reconnection_attempts == 0 { "unlimited".to_string() } 
            else { self.args.max_reconnection_attempts.to_string() }
        );

        let ws_config = WsConfig {
            heartbeat_interval: self.args.heartbeat_interval,
            max_reconnection_attempts: self.args.max_reconnection_attempts,
            ..Default::default()
        };

        // Configure authentication
        let user_auth = if let (Some(api_key), Some(_), Some(_)) = 
            (&self.args.api_key, &self.args.secret, &self.args.passphrase) {
            println!("\n🔐 Authentication:");
            println!("   └ API Key: {}...{}", 
                &api_key[..8.min(api_key.len())], 
                &api_key[api_key.len().saturating_sub(4)..]
            );
            Some(AuthPayload {
                api_key: self.args.api_key.clone().unwrap(),
                secret: self.args.secret.clone().unwrap(),
                passphrase: self.args.passphrase.clone().unwrap(),
            })
        } else {
            None
        };

        // Configure streamer
        let streamer_config = StreamerConfig {
            ws_config,
            market_assets: assets,
            user_markets: self.args.markets.clone(),
            user_auth,
            event_buffer_size: 1000,
            auto_sync_on_hash_mismatch: true,
        };

        // Create and start streamer with progress
        println!("\n🔌 Connecting to WebSocket...");
        let mut streamer = Streamer::new(streamer_config);
        
        match streamer.start(host, &data_paths).await {
            Ok(_) => {
                println!("✅ WebSocket connection established");
                println!("📡 Fetching initial orderbooks...");
            }
            Err(e) => {
                eprintln!("\n❌ Failed to connect: {}", e);
                return Err(anyhow::anyhow!("Connection failed: {}", e));
            }
        }
        
        // Wait for initial data with proper timeout handling and progress feedback
        self.wait_for_initial_data(&streamer).await?;
        
        println!("\n🎨 Starting TUI interface...");
        println!("💡 Keyboard shortcuts: ↑/↓ Navigate | Enter: Select | q: Quit");
        println!("📄 Logs: {}", data_paths.logs().display());
        
        // Small delay to let user see the message
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        
        // Continue with TUI execution
        self.execute_tui_inner_with_streamer(host, data_paths, streamer).await
    }
    
    async fn execute_tui_inner_with_streamer(&self, _host: &str, _data_paths: DataPaths, streamer: Streamer) -> Result<()> {
        // Set up panic hook for proper terminal cleanup
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            // Try to restore terminal on panic
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
            
            // Call original panic hook
            original_hook(panic_info);
        }));
        
        // Wrap the entire TUI execution in a catch_unwind for additional safety
        let result = std::panic::AssertUnwindSafe(self.execute_tui_inner_with_streamer_core(streamer))
            .catch_unwind()
            .await;
        
        // Restore original panic hook
        let _ = std::panic::take_hook();
        
        match result {
            Ok(Ok(res)) => Ok(res),
            Ok(Err(e)) => Err(e),
            Err(panic) => {
                let panic_msg = if let Some(s) = panic.downcast_ref::<String>() {
                    s.clone()
                } else if let Some(s) = panic.downcast_ref::<&str>() {
                    s.to_string()
                } else {
                    "Unknown panic occurred".to_string()
                };
                Err(anyhow::anyhow!("TUI panicked: {}", panic_msg))
            }
        }
    }
    
    async fn execute_tui_inner_with_streamer_core(&self, streamer: Streamer) -> Result<()> {
        let streamer_arc = Arc::new(streamer);
        
        // Setup terminal with error handling and fallback
        let mut terminal = match setup_terminal() {
            Ok(terminal) => terminal,
            Err(e) => {
                error!("Failed to setup terminal for TUI: {}", e);
                
                // Check if this is a terminal device error
                if e.to_string().contains("Device not configured") || 
                   e.to_string().contains("not a terminal") ||
                   e.to_string().contains("Inappropriate ioctl") {
                    warn!("Terminal not available for TUI, falling back to CLI mode with FileOnly logging");
                    // Would need to fall back to CLI here, but that's complex
                    return Err(anyhow::anyhow!("Terminal not available: {}", e));
                } else {
                    return Err(anyhow::anyhow!("Failed to setup terminal: {}", e));
                }
            }
        };
        
        // Create app
        let mut app = App::new(streamer_arc.clone());
        
        // Create event handler with balanced tick rate for UI responsiveness
        let mut event_handler = EventHandler::new(Duration::from_millis(50));
        
        // Get event stream
        let mut events = streamer_arc.events();
        
        info!("Starting TUI main loop");
        
        // Main loop with comprehensive error handling
        let result = loop {
            // Draw UI with error handling
            match terminal.draw(|f| {
                if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ui::draw(f, &mut app);
                })) {
                    error!("UI drawing panicked: {:?}", e);
                }
            }) {
                Ok(_) => {},
                Err(e) => {
                    error!("Terminal drawing error: {}", e);
                    break Err(anyhow::anyhow!("Terminal drawing failed: {}", e));
                }
            }
            
            // Handle events with balanced timeout for UI responsiveness
            let event_timeout = Duration::from_millis(25);
            
            tokio::select! {
                // Handle keyboard events first (higher priority for UI responsiveness)
                ui_event_opt = event_handler.next() => {
                    match ui_event_opt {
                        Some(ui_event) => {
                            match ui_event {
                                events::Event::Key(key) => {
                                    match key.code {
                                        KeyCode::Char('q') | KeyCode::Char('Q') => {
                                            info!("User requested quit");
                                            app.should_quit = true;
                                        }
                                        KeyCode::Up => {
                                            if matches!(app.state, crate::tui::AppState::OrderBook { .. }) {
                                                app.scroll_orderbook_up();
                                            } else {
                                                app.select_previous();
                                            }
                                        }
                                        KeyCode::Down => {
                                            if matches!(app.state, crate::tui::AppState::OrderBook { .. }) {
                                                app.scroll_orderbook_down();
                                            } else {
                                                app.select_next();
                                            }
                                        }
                                        KeyCode::Char('m') | KeyCode::Char('M') => {
                                            if matches!(app.state, crate::tui::AppState::OrderBook { .. }) {
                                                app.reset_orderbook_scroll();
                                            }
                                        }
                                        KeyCode::Enter => {
                                            if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                                app.select_token();
                                            })) {
                                                error!("Token selection panicked: {:?}", e);
                                                app.event_log.push("⚠️ Token selection error".to_string());
                                            }
                                        }
                                        KeyCode::Esc | KeyCode::Backspace => {
                                            app.go_back();
                                        }
                                        KeyCode::Char('r') => {
                                            // Add refresh functionality
                                            info!("User requested refresh");
                                            events = streamer_arc.events();
                                            app.event_log.push("🔄 Refreshed event stream".to_string());
                                        }
                                        _ => {}
                                    }
                                }
                                events::Event::Tick => {
                                    // Regular tick for UI updates
                                }
                                events::Event::Error(error_msg) => {
                                    error!("Event handler error: {}", error_msg);
                                    app.event_log.push(format!("❌ Input error: {}", error_msg));
                                }
                            }
                        }
                        None => {
                            warn!("Event handler channel closed");
                            app.event_log.push("⚠️ Input handler stopped".to_string());
                            break Err(anyhow::anyhow!("Input event handler stopped unexpectedly"));
                        }
                    }
                }
                
                // Handle websocket events (lower priority)
                ws_event = events.recv() => {
                    match ws_event {
                        Ok(event) => {
                            if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                app.handle_event(event);
                            })) {
                                error!("Event handling panicked: {:?}", e);
                                app.event_log.push("⚠️ Event processing error occurred".to_string());
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            warn!("Event channel lagged by {} messages, resubscribing", n);
                            events = streamer_arc.events();
                            app.event_log.push(format!("⚠️ Missed {} events due to lag", n));
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            error!("Event channel closed, stopping TUI");
                            app.event_log.push("❌ Event stream closed".to_string());
                            break Err(anyhow::anyhow!("WebSocket event stream closed unexpectedly"));
                        }
                    }
                }
                
                // Timeout to ensure we don't hang
                _ = tokio::time::sleep(event_timeout) => {
                    // Regular timeout, continue loop
                }
            }
            
            if app.should_quit {
                info!("Exiting TUI main loop");
                break Ok(());
            }
        };
        
        // Restore terminal with error handling
        if let Err(e) = restore_terminal(&mut terminal) {
            error!("Failed to restore terminal: {}", e);
            // Don't fail the entire operation for terminal restore errors
        }
        
        // Stop streamer with error handling
        info!("Stopping streamer...");
        match Arc::try_unwrap(streamer_arc) {
            Ok(mut streamer) => streamer.stop().await,
            Err(_) => {
                warn!("Could not unwrap streamer Arc, there are still references");
                // Streamer will be dropped when Arc goes out of scope
            }
        }
        
        result
    }

    async fn execute_cli(&self, host: &str, data_paths: DataPaths) -> Result<()> {
        // Load assets from various sources
        let assets = self.get_assets_for_streaming(&data_paths).await?;

        if assets.is_empty() {
            return Err(anyhow::anyhow!("At least one asset ID must be provided with --assets, --markets-path, or --selection"));
        }

        info!("🚀 Starting Polymarket WebSocket stream with {} assets", assets.len());

        // Configure WebSocket
        let ws_config = WsConfig {
            heartbeat_interval: self.args.heartbeat_interval,
            max_reconnection_attempts: self.args.max_reconnection_attempts,
            ..Default::default()
        };

        // Configure authentication if provided
        let user_auth = if let (Some(api_key), Some(secret), Some(passphrase)) = 
            (&self.args.api_key, &self.args.secret, &self.args.passphrase) {
            Some(AuthPayload {
                api_key: api_key.clone(),
                secret: secret.clone(),
                passphrase: passphrase.clone(),
            })
        } else if self.args.markets.is_some() {
            warn!("User markets specified but authentication not provided. User feed will be disabled.");
            None
        } else {
            None
        };

        // Configure streamer
        let streamer_config = StreamerConfig {
            ws_config,
            market_assets: assets,
            user_markets: self.args.markets.clone(),
            user_auth,
            event_buffer_size: 1000,
            auto_sync_on_hash_mismatch: true,
        };

        // Skip connectivity test for now - proceed directly to streaming
        println!("\n🔌 Starting WebSocket connection...");
        
        // Create and start streamer
        println!("\n🔗 Creating WebSocket streamer...");
        let mut streamer = Streamer::new(streamer_config);
        info!("🔌 Starting streamer for host: {}", host);
        
        match streamer.start(host, &data_paths).await {
            Ok(_) => {
                println!("✅ Streamer started successfully");
                info!("🔗 Streamer started successfully");
            }
            Err(e) => {
                eprintln!("\n❌ Failed to start streamer: {}", e);
                return Err(anyhow::anyhow!("Failed to start streamer: {}", e));
            }
        }

        // Set up event handling
        let mut events = streamer.events();
        
        // Set up summary timer
        let mut summary_timer = if let Some(interval) = self.args.summary_interval {
            Some(tokio::time::interval(Duration::from_secs(interval)))
        } else {
            None
        };

        info!("✅ Streaming started. Press Ctrl+C to stop.");

        // Main event loop
        loop {
            tokio::select! {
                // Handle events
                result = events.recv() => {
                    match result {
                        Ok(event) => {
                            self.handle_event(event);
                        }
                        Err(e) => {
                            warn!("Event receive error: {}", e);
                            // Try to resubscribe if the channel is lagging
                            match e {
                                tokio::sync::broadcast::error::RecvError::Lagged(n) => {
                                    warn!("Channel lagged by {} messages, resubscribing", n);
                                    events = streamer.events();
                                    continue;
                                }
                                tokio::sync::broadcast::error::RecvError::Closed => {
                                    error!("Event channel closed, stopping");
                                    break;
                                }
                            }
                        }
                    }
                }
                
                // Handle summary timer
                _ = async {
                    if let Some(ref mut timer) = summary_timer {
                        timer.tick().await;
                        Self::print_order_book_summary(&streamer);
                    } else {
                        // If no summary timer, sleep to prevent busy loop
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                } => {}
                
                // Handle Ctrl+C
                _ = signal::ctrl_c() => {
                    info!("🛑 Shutting down...");
                    break;
                }
            }
        }

        // Stop streamer
        streamer.stop().await;
        info!("✅ Stream stopped");

        Ok(())
    }

    /// Get assets for streaming from all possible sources with fallback to dataset selector
    async fn get_assets_for_streaming(&self, data_paths: &DataPaths) -> Result<Vec<String>> {
        // 1. Check direct assets argument
        if !self.args.assets.is_empty() {
            info!("Using {} directly specified assets", self.args.assets.len());
            return Ok(self.args.assets.clone());
        }

        // 2. Check markets_path argument
        if let Some(markets_path) = &self.args.markets_path {
            info!("Loading assets from markets file: {}", markets_path);
            return self.load_assets_from_markets_file(markets_path);
        }

        // 3. Check selection argument
        if let Some(selection_name) = &self.args.selection {
            info!("Loading assets from selection: {}", selection_name);
            return self.load_assets_from_selection(selection_name, data_paths);
        }

        // 4. No explicit source provided - trigger dataset selector
        info!("No assets specified, triggering dataset selector");
        
        // Check if we're in TUI mode for interactive selector
        if self.args.tui {
            match self.run_interactive_dataset_selector(data_paths).await {
                Ok(assets) => {
                    if !assets.is_empty() {
                        info!("Selected {} assets from dataset selector", assets.len());
                        return Ok(assets);
                    } else {
                        return Err(anyhow::anyhow!("No assets selected from dataset selector"));
                    }
                }
                Err(e) => {
                    warn!("Interactive dataset selector failed: {}", e);
                    // Fall through to non-interactive options
                }
            }
        }

        // 5. Fall back to non-interactive selection list
        self.show_available_selections_and_exit(data_paths).await
    }

    async fn run_interactive_dataset_selector(&self, _data_paths: &DataPaths) -> Result<Vec<String>> {
        println!("\n🔍 No assets specified - opening dataset selector...");
        println!("💡 You can also use --assets, --markets-path, or --selection");
        
        // TODO: Implement proper dataset selector once storage module is fixed
        // For now, this is a placeholder implementation that shows what would happen
        println!("📋 Dataset selector would show available datasets here");
        println!("🚧 Dataset selector temporarily unavailable (storage module needs bincode dependency)");
        println!("   Please use --assets, --markets-path, or --selection instead");
        
        Err(anyhow::anyhow!("Interactive dataset selector temporarily unavailable - please use --assets, --markets-path, or --selection"))
    }

    async fn show_available_selections_and_exit(&self, data_paths: &DataPaths) -> Result<Vec<String>> {
        println!("\n❌ No assets specified and interactive mode not available.");
        println!("\nYou can provide assets in several ways:");
        println!("  1. Direct tokens:     --assets TOKEN1,TOKEN2,TOKEN3");
        println!("  2. Markets file:      --markets-path path/to/markets.json");
        println!("  3. Saved selection:   --selection my-selection");
        
        // Show available selections
        let manager = SelectionManager::new(&data_paths.data());
        let selections = manager.list_all_selections()?;
        
        if !selections.is_empty() {
            println!("\nAvailable selections:");
            for selection in selections {
                println!("  • {}", selection);
            }
            println!("\nUse: polybot stream --selection <name>");
        } else {
            println!("\nNo saved selections found. Create one with: polybot selections create");
        }
        
        Err(anyhow::anyhow!("No assets specified"))
    }

    fn load_assets_from_selection(&self, selection_name: &str, data_paths: &DataPaths) -> Result<Vec<String>> {
        let manager = SelectionManager::new(&data_paths.data());
        manager.get_tokens(selection_name)
            .map_err(|e| anyhow::anyhow!("Failed to load selection '{}': {}", selection_name, e))
    }

    fn load_assets_from_markets_file(&self, markets_path: &str) -> Result<Vec<String>> {
        info!("📁 Loading markets from: {}", markets_path);
        
        // Read and parse the JSON file
        let contents = fs::read_to_string(markets_path)
            .map_err(|e| anyhow::anyhow!("Failed to read markets file '{}': {}", markets_path, e))?;
        
        let markets: Vec<Value> = serde_json::from_str(&contents)
            .map_err(|e| anyhow::anyhow!("Failed to parse markets JSON: {}", e))?;
        
        // Extract token_id from each market's tokens array
        let mut assets = Vec::new();
        for (index, market) in markets.iter().enumerate() {
            if let Some(tokens) = market.get("tokens").and_then(|v| v.as_array()) {
                for token in tokens {
                    if let Some(token_id) = token.get("token_id").and_then(|v| v.as_str()) {
                        assets.push(token_id.to_string());
                        if assets.len() <= 5 {  // Show first 5 for debugging
                            debug!("Token {}: {}", assets.len(), token_id);
                        }
                    }
                }
            } else {
                warn!("Market at index {} missing tokens array, skipping", index);
            }
        }
        
        info!("✅ Loaded {} assets from markets file", assets.len());
        
        Ok(assets)
    }

    fn handle_event(&self, event: PolyEvent) {
        match event {
            PolyEvent::Book { asset_id, bids, asks, .. } if self.args.show_book => {
                let best_bid = bids.first().map(|level| format!("${} ({})", level.price, level.size)).unwrap_or_default();
                let best_ask = asks.first().map(|level| format!("${} ({})", level.price, level.size)).unwrap_or_default();
                
                info!("📈 {} - Bid: {} Ask: {}", asset_id, best_bid, best_ask);
            }
            
            PolyEvent::PriceChange { asset_id, side, price, size, .. } if self.args.show_book => {
                let action = if size == rust_decimal::Decimal::ZERO { "REMOVE" } else { "UPDATE" };
                let side_str = match side {
                    Side::Buy => "BID".bright_green().to_string(),
                    Side::Sell => "ASK".bright_red().to_string(),
                };
                
                info!("📊 {} - {} {} ${} ({})", asset_id, action, side_str, price, size);
            }
            
            
            PolyEvent::Trade { asset_id, price, size, side, .. } if self.args.show_trades => {
                let side_str = match side {
                    Side::Buy => "BUY".bright_green().to_string(),
                    Side::Sell => "SELL".bright_red().to_string(),
                };
                
                info!("💰 {} - {} {} @ ${}", asset_id, side_str, size, price);
            }
            
            PolyEvent::MyOrder { asset_id, side, price, size, status, .. } if self.args.show_user => {
                let side_str = match side {
                    Side::Buy => "BUY".bright_green().to_string(),
                    Side::Sell => "SELL".bright_red().to_string(),
                };
                
                info!("📋 {} - Order {} {} @ ${} - {:?}", asset_id, side_str, size, price, status);
            }
            
            PolyEvent::MyTrade { asset_id, side, price, size, .. } if self.args.show_user => {
                let side_str = match side {
                    Side::Buy => "BOUGHT".bright_green().to_string(),
                    Side::Sell => "SOLD".bright_red().to_string(),
                };
                
                info!("✅ {} - {} {} @ ${}", asset_id, side_str, size, price);
            }
            
            PolyEvent::TickSizeChange { asset_id, tick_size } => {
                info!("Tick size changed for {}: {}", asset_id, tick_size);
            }
            
            _ => {} // Ignore other events or when show flags are disabled
        }
    }

    fn print_order_book_summary(streamer: &Streamer) {
        let summaries = streamer.summary();
        
        if !summaries.is_empty() {
            info!("📊 Order Book Summary:");
            for summary in summaries {
                info!("  {}", summary);
            }
        }
    }

    /// Wait for initial WebSocket data with progress indicators and timeout handling
    async fn wait_for_initial_data(&self, streamer: &Streamer) -> Result<()> {
        use std::time::Duration;
        
        println!("⏳ Waiting for WebSocket data...");
        info!("Waiting for initial WebSocket data before starting TUI...");
        
        let timeout_duration = Duration::from_secs(10); // Extended timeout to handle slow connections
        let check_interval = Duration::from_millis(500);
        let mut elapsed = Duration::from_secs(0);
        let mut events = streamer.events();
        let mut data_received = false;
        let mut connection_established = false;
        
        // Show progress dots
        let mut progress_dots = 0;
        
        while elapsed < timeout_duration {
            // Check for events with timeout
            let event_result = tokio::time::timeout(check_interval, events.recv()).await;
            
            match event_result {
                Ok(Ok(event)) => {
                    data_received = true;
                    connection_established = true;
                    
                    // Log the type of data received for debugging
                    match &event {
                        PolyEvent::Book { asset_id, .. } => {
                            println!("✅ Order book data received for {}", asset_id);
                            info!("Received initial order book for asset: {}", asset_id);
                        }
                        PolyEvent::Trade { asset_id, .. } => {
                            println!("✅ Trade data received for {}", asset_id);
                            info!("Received trade data for asset: {}", asset_id);
                        }
                        PolyEvent::PriceChange { asset_id, .. } => {
                            println!("✅ Price update received for {}", asset_id);
                            info!("Received price change for asset: {}", asset_id);
                        }
                        _ => {
                            println!("✅ WebSocket data received");
                            info!("Received WebSocket event: {:?}", event);
                        }
                    }
                    break;
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {
                    // Channel is lagged, but connection is working
                    println!("✅ WebSocket connection active (catching up on events)");
                    connection_established = true;
                    break;
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                    // Channel closed - this is a problem
                    let error_msg = "WebSocket event channel closed unexpectedly";
                    error!("{}", error_msg);
                    print!("\r                                                    \r");
                    println!("❌ {}", error_msg);
                    return Err(anyhow::anyhow!("WebSocket connection failed: {}", error_msg));
                }
                Err(_) => {
                    // Timeout on this check interval - update progress
                    elapsed += check_interval;
                    
                    // Update progress indicator
                    progress_dots = (progress_dots + 1) % 4;
                    let dots = ".".repeat(progress_dots + 1);
                    let spaces = " ".repeat(3 - progress_dots);
                    print!("\r⏳ Waiting for WebSocket data{}{} ({:.1}s)", dots, spaces, elapsed.as_secs_f32());
                    std::io::Write::flush(&mut std::io::stdout()).unwrap_or(());
                }
            }
        }
        
        // Clear the progress line
        print!("\r                                                    \r");
        std::io::Write::flush(&mut std::io::stdout()).unwrap_or(());
        
        if !data_received {
            // Check if we have any order books from REST API sync as fallback
            let order_books = streamer.get_all_order_books();
            if !order_books.is_empty() {
                println!("ℹ️  No real-time data yet, but {} order books available from REST API", order_books.len());
                info!("Proceeding with {} order books from REST API", order_books.len());
                connection_established = true;
            }
        }
        
        if !connection_established {
            let error_msg = "No WebSocket data received within timeout period";
            error!("{}", error_msg);
            println!("❌ {}", error_msg);
            println!("🔍 Troubleshooting suggestions:");
            println!("   • Check network connection");
            println!("   • Verify asset IDs are correct and active");
            println!("   • Check if markets are currently trading");
            println!("   • Try with fewer assets if subscribing to many");
            println!("   • Use --verbose flag for detailed logs");
            println!("   • Check logs: {}", crate::data_paths::DataPaths::new("./data").logs().display());
            
            // Allow users to bypass with environment variable for debugging
            if std::env::var("POLYBOT_SKIP_DATA_WAIT").is_ok() {
                println!("⚠️  POLYBOT_SKIP_DATA_WAIT set - continuing anyway");
                warn!("Bypassing data wait due to POLYBOT_SKIP_DATA_WAIT environment variable");
            } else {
                return Err(anyhow::anyhow!(
                    "WebSocket timeout: {}. Set POLYBOT_SKIP_DATA_WAIT=1 to bypass this check.", 
                    error_msg
                ));
            }
        }
        
        if data_received {
            println!("🎯 Real-time data stream established successfully");
            info!("Real-time WebSocket data stream is active");
        } else {
            println!("⚠️  Starting with REST API data, real-time updates may follow");
            info!("Starting TUI with REST API data only");
        }
        
        Ok(())
    }
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    use std::io::IsTerminal;
    
    // Check if stdout is a terminal
    if !io::stdout().is_terminal() {
        return Err(anyhow::anyhow!("stdout is not a terminal"));
    }
    
    // Check if stderr is a terminal (for user interaction)
    if !io::stderr().is_terminal() {
        return Err(anyhow::anyhow!("stderr is not a terminal"));
    }
    
    enable_raw_mode()
        .map_err(|e| anyhow::anyhow!("Failed to enable raw mode: {}", e))?;
    
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .map_err(|e| anyhow::anyhow!("Failed to setup terminal screen: {}", e))?;
    
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)
        .map_err(|e| anyhow::anyhow!("Failed to create terminal: {}", e))?;
    
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}