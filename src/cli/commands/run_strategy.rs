//! Run strategy command implementation

use anyhow::{Context, Result};
use clap::Args;
use rust_decimal::Decimal;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tracing::{debug, error, info, warn};

use crate::auth::get_authenticated_client;
use crate::config;
use crate::core::execution::orders::{OrderConfig, PolyBot};
use crate::core::services::{Streamer, StreamerConfig};
use crate::core::ws::{AuthPayload, PolyEvent, WsConfig};
use crate::data_paths::DataPaths;
use crate::logging::{init_logging, LogMode, LoggingConfig};
use crate::strategy::{simple_strategy, SimpleStrategy, SingleTokenStrategy, TradeEvent};

#[derive(Args, Clone)]
pub struct RunStrategyArgs {
    /// Token ID to run strategy on (comma-separated for multiple)
    #[arg(long, value_delimiter = ',')]
    pub token_id: Vec<String>,
    
    /// Subscribe to all markets (for testing)
    #[arg(long)]
    pub all_markets: bool,
    
    /// Show example asset IDs and exit
    #[arg(long)]
    pub show_examples: bool,
    
    /// Strategy type to run (currently only 'simple' is supported)
    #[arg(long, default_value = "simple")]
    pub strategy: String,
    
    /// Minimum spread threshold (in price units)
    #[arg(long, default_value = "0.001")]
    pub min_spread: f64,
    
    /// Maximum spread threshold (in price units)  
    #[arg(long, default_value = "0.01")]
    pub max_spread: f64,
    
    /// Volume analysis window in seconds
    #[arg(long, default_value = "300")]
    pub volume_window: u64,
    
    /// How often to log orderbook updates (every Nth update)
    #[arg(long, default_value = "10")]
    pub log_frequency: u32,
    
    /// Maximum retry attempts for connection (0 = infinite)
    #[arg(long, default_value = "0")]
    pub max_reconnection_attempts: u32,
    
    /// Silence hash mismatch warnings
    #[arg(long)]
    pub quiet_hash_mismatch: bool,
}

pub struct RunStrategyCommand {
    args: RunStrategyArgs,
}

impl RunStrategyCommand {
    pub fn new(args: RunStrategyArgs) -> Self {
        Self { args }
    }
    
    /// Resolve token IDs from prefix by searching in orderbook files
    async fn resolve_token_ids(&self, prefix: &str, data_paths: &DataPaths) -> Result<Vec<String>> {
        let orderbook_path = data_paths.root().join("market_data").join("orderbooks");
        
        debug!("Looking for orderbooks in: {:?}", orderbook_path);
        
        if !orderbook_path.exists() {
            debug!("Orderbook path does not exist: {:?}", orderbook_path);
            return Ok(vec![]);
        }
        
        let mut matched_tokens = Vec::new();
        
        // Read directory and find matching snapshot files
        let entries = tokio::fs::read_dir(&orderbook_path).await?;
        let mut entries = entries;
        
        while let Some(entry) = entries.next_entry().await? {
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();
            
            // Look for snapshot files only (to avoid duplicates from update files)
            if file_name_str.ends_with("_snapshot.fb") {
                // Extract token ID from filename
                let token_id = file_name_str
                    .strip_suffix("_snapshot.fb")
                    .unwrap_or(&file_name_str);
                
                // Check if token ID starts with the prefix
                if token_id.starts_with(prefix) {
                    debug!("Found matching token: {}", token_id);
                    matched_tokens.push(token_id.to_string());
                }
            }
        }
        
        // Sort for consistent ordering
        matched_tokens.sort();
        matched_tokens.dedup(); // Remove any duplicates
        
        Ok(matched_tokens)
    }
    
    pub async fn execute(&self, _host: &str, data_paths: &DataPaths) -> Result<()> {
        // Initialize logging
        let logging_config = LoggingConfig::new(
            LogMode::ConsoleAndFile,
            data_paths.clone(),
        );
        init_logging(logging_config)?;
        
        // Show examples if requested
        if self.args.show_examples {
            info!("📚 Example Polymarket Asset IDs:");
            info!("");
            info!("Active assets (as of snapshot):");
            info!("  108468416668663017133298741485453125150952822149773262784582671647441799250111");
            info!("  86964554239869134298526596083850367463176146674230300967120111145834668642528");
            info!("  21742633143463906290569050155826241533067272736897614950488156847949938836455");
            info!("");
            info!("Usage:");
            info!("  cargo run -- run-strategy --token-id 108468416668663017133298741485453125150952822149773262784582671647441799250111");
            info!("");
            info!("To find more active assets:");
            info!("  1. Use 'polybot markets list' to see current markets");
            info!("  2. Look for the token IDs in each market");
            info!("  3. Use 'polybot stream' to test WebSocket connectivity");
            return Ok(());
        }
        
        // Process token IDs with auto-detection
        let mut resolved_tokens = Vec::new();
        let expected_token_length = 70; // Polymarket token IDs are typically 70+ chars
        
        for token in &self.args.token_id {
            if token.len() < expected_token_length && !token.starts_with("0x") {
                info!("🔍 Auto-detecting token ID for prefix: {}", token);
                
                let matches = self.resolve_token_ids(token, data_paths).await?;
                
                match matches.len() {
                    0 => {
                        error!("❌ No tokens found matching prefix: {}", token);
                        error!("   Make sure the token exists in data/market_data/orderbooks/");
                        return Err(anyhow::anyhow!("No matching tokens found for prefix: {}", token));
                    }
                    1 => {
                        let full_token_id = &matches[0];
                        info!("✅ Auto-detected token: {}", full_token_id);
                        resolved_tokens.push(full_token_id.clone());
                    }
                    _ => {
                        // Multiple matches found - allow user selection
                        println!("\n🔍 Found {} tokens matching prefix '{}':", matches.len(), token);
                        println!("Please select one by entering the number:\n");
                        
                        for (idx, matched_token) in matches.iter().enumerate() {
                            println!("  [{}] {}", idx + 1, matched_token);
                        }
                        
                        print!("\nEnter selection (1-{}): ", matches.len());
                        std::io::Write::flush(&mut std::io::stdout())?;
                        
                        let mut input = String::new();
                        std::io::stdin().read_line(&mut input)?;
                        
                        let selection: usize = input
                            .trim()
                            .parse()
                            .context("Invalid selection")?;
                        
                        if selection == 0 || selection > matches.len() {
                            return Err(anyhow::anyhow!("Selection must be between 1 and {}", matches.len()));
                        }
                        
                        let selected = matches[selection - 1].clone();
                        info!("✅ Selected token: {}", selected);
                        resolved_tokens.push(selected);
                    }
                }
            } else {
                // Token ID looks complete, use as-is
                resolved_tokens.push(token.clone());
            }
        }
        
        info!("🚀 Starting {} strategy for {} token(s)", self.args.strategy, resolved_tokens.len());
        for token in &resolved_tokens {
            info!("  - Token: {}", token);
        }
        
        // Load credentials
        let api_creds = config::load_credentials(&data_paths).await?;
        
        // Create authenticated ClobClient for order placement
        let clob_client = get_authenticated_client(_host, &data_paths).await?;
        let clob_client = Arc::new(tokio::sync::Mutex::new(clob_client));
        info!("🔐 Created authenticated ClobClient for order placement");
        
        // Create PolyBot coordinator
        let order_config = OrderConfig {
            enable_detailed_logging: false, // Can be controlled via RUST_LOG env var
            ..Default::default()
        };
        let polybot = Arc::new(PolyBot::with_order_config(order_config));
        
        // For now, we'll use the first token ID for the strategy
        let primary_token = resolved_tokens.first()
            .ok_or_else(|| anyhow::anyhow!("No token ID provided"))?
            .clone();
        
        // Create strategy based on type
        let mut strategy: Box<dyn SingleTokenStrategy> = match self.args.strategy.as_str() {
            "simple" => {
                let config = simple_strategy::SimpleStrategyConfig {
                    min_spread_threshold: Decimal::try_from(self.args.min_spread)?,
                    max_spread_threshold: Decimal::try_from(self.args.max_spread)?,
                    volume_window: Duration::from_secs(self.args.volume_window),
                    log_frequency: self.args.log_frequency,
                    order_check_interval: Duration::from_secs(10),
                    max_active_orders: 3,
                    base_discount_percent: Decimal::new(5, 3), // 0.005 = 0.5%
                    discount_increment: Decimal::new(5, 3), // 0.005 = 0.5%
                    tick_size: Decimal::new(1, 2), // 0.01 = cent precision
                    base_order_size: Decimal::new(5, 0), // 5 shares base
                    max_order_value: Decimal::new(250, 2), // $2.50 max per order
                };
                Box::new(SimpleStrategy::new(
                    primary_token,
                    config,
                    polybot.clone(),
                ))
            }
            _ => {
                return Err(anyhow::anyhow!("Unknown strategy type: {}", self.args.strategy));
            }
        };
        
        // Set the ClobClient on the strategy
        strategy.set_clob_client(clob_client.clone());
        info!("✅ ClobClient attached to strategy");
        
        info!("📊 Strategy Configuration:");
        info!("   Strategy: {}", strategy.name());
        info!("   Primary Token: {}", strategy.token_id());
        info!("   Min Spread: ${:.4}", self.args.min_spread);
        info!("   Max Spread: ${:.4}", self.args.max_spread);
        info!("   Volume Window: {}s", self.args.volume_window);
        info!("   Log Frequency: every {} updates", self.args.log_frequency);
        if self.args.quiet_hash_mismatch {
            info!("   Hash Mismatch Warnings: SILENCED");
        }
        
        // Create WebSocket configuration
        let ws_config = WsConfig {
            market_url: "wss://ws-subscriptions-clob.polymarket.com/ws/market".to_string(),
            user_url: "wss://ws-subscriptions-clob.polymarket.com/ws/user".to_string(),
            heartbeat_interval: 10,
            max_reconnection_attempts: self.args.max_reconnection_attempts,
            initial_reconnection_delay: 1000,
            max_reconnection_delay: 60000,
            skip_hash_verification: true, // Use default - hash verification is disabled
            quiet_hash_mismatch: self.args.quiet_hash_mismatch,
        };
        
        // Create streamer configuration
        let market_assets = if self.args.all_markets {
            info!("⚠️  Subscribing to ALL markets (testing mode)");
            vec![] // Empty array subscribes to all markets
        } else {
            resolved_tokens.clone()
        };
        
        // Create user auth payload from API credentials
        let user_auth = AuthPayload {
            api_key: api_creds.api_key.clone(),
            secret: api_creds.secret.clone(),
            passphrase: api_creds.passphrase.clone(),
        };
        
        // Get user markets (same as token IDs for now)
        let user_markets = if self.args.all_markets {
            vec![] // Subscribe to all user markets
        } else {
            // For user feed, we need market IDs not token IDs
            // For now, we'll subscribe to the same as market assets
            resolved_tokens.clone()
        };
        
        let streamer_config = StreamerConfig {
            market_assets: market_assets.clone(),
            user_markets: Some(user_markets.clone()),
            user_auth: Some(user_auth),
            ws_config,
            event_buffer_size: 1000,
            auto_sync_on_hash_mismatch: true,
        };
        
        info!("🔌 Configuring WebSocket with user authentication");
        info!("   Market feed: {} assets", market_assets.len());
        info!("   User feed: {} markets", user_markets.len());
        
        // Create and start the streamer
        let mut streamer = Streamer::new(streamer_config);
        streamer.start(_host, data_paths).await?;
        let mut event_receiver = streamer.events();
        
        info!("🔌 Connected to WebSocket, streaming market data...");
        info!("   Press Ctrl+C to stop");
        info!("");
        info!("📡 Subscribed to {} asset(s):", market_assets.len());
        for asset in &market_assets {
            info!("   - {}", asset);
        }
        info!("");
        info!("⏳ Waiting for market events...");
        
        // Create shutdown signal handler
        let mut shutdown_signal = Box::pin(signal::ctrl_c());
        
        // Track events for status updates
        let mut last_event_time = std::time::Instant::now();
        let mut event_count = 0u64;
        let mut order_event_count = 0u64;
        let mut last_order_process = std::time::Instant::now();
        
        // Set up periodic timers
        let mut order_check_timer = tokio::time::interval(Duration::from_secs(1)); // Check every second
        order_check_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        
        let mut status_timer = tokio::time::interval(Duration::from_secs(30));
        status_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        
        // Main event loop
        loop {
            tokio::select! {
                // Handle shutdown signal
                _ = &mut shutdown_signal => {
                    info!("\n⏹️  Shutting down strategy...");
                    // Notify strategy of shutdown
                    if let Err(e) = strategy.shutdown().await {
                        error!("Failed to shutdown strategy cleanly: {}", e);
                    }
                    break;
                }
                
                // Periodic order processing (every second)
                _ = order_check_timer.tick() => {
                    let now = std::time::Instant::now();
                    if now.duration_since(last_order_process) >= Duration::from_secs(1) {
                        last_order_process = now;
                        if let Err(e) = strategy.process_pending_orders().await {
                            error!("Failed to process pending orders: {}", e);
                        }
                    }
                }
                
                // Periodic status update
                _ = status_timer.tick() => {
                    if event_count == 0 {
                        info!("💓 Strategy running - waiting for market events...");
                    } else {
                        info!("💓 Strategy running - {} events ({} order events) processed, last event {} seconds ago", 
                              event_count, order_event_count, last_event_time.elapsed().as_secs());
                    }
                }
                
                // Handle streaming events
                Ok(event) = event_receiver.recv() => {
                    event_count += 1;
                    last_event_time = std::time::Instant::now();
                    // Log important events, debug log orderbook updates
                    match &event {
                        PolyEvent::Book { asset_id, bids, asks, .. } => {
                            if asset_id == strategy.token_id() {
                                debug!("📊 Book update: {} bids, {} asks", bids.len(), asks.len());
                            }
                        }
                        PolyEvent::PriceChange { asset_id, side, price, size, .. } => {
                            if asset_id == strategy.token_id() {
                                debug!("💱 Price change: {:?} ${:.4} x {}", side, price, size);
                            }
                        }
                        PolyEvent::Trade { asset_id, price, size, side } => {
                            if asset_id == strategy.token_id() {
                                info!("💰 Trade: {:?} ${:.4} x {} = ${:.2}", side, price, size, price * size);
                            }
                        }
                        PolyEvent::LastTradePrice { asset_id, price, .. } => {
                            if asset_id == strategy.token_id() {
                                debug!("📈 Last trade price: ${:.4}", price);
                            }
                        }
                        PolyEvent::MyOrder { asset_id, side, price, size, status } => {
                            order_event_count += 1;
                            
                            // Use different emoji based on order status
                            let emoji = match status {
                                crate::core::types::common::OrderStatus::Open => "📋",
                                crate::core::types::common::OrderStatus::Filled => "✅",
                                crate::core::types::common::OrderStatus::Cancelled => "❌",
                                crate::core::types::common::OrderStatus::PartiallyFilled => "📊",
                            };
                            
                            info!("{} Order Update - Status: {:?}", emoji, status);
                            info!("   Asset: {}", asset_id);
                            info!("   Side: {:?} | Price: ${:.4} | Size: {}", side, price, size);
                            
                            // Only show target match for our monitored assets
                            if asset_id == strategy.token_id() {
                                info!("   ✨ This is our monitored token!");
                            }
                        }
                        PolyEvent::MyTrade { asset_id, side, price, size } => {
                            order_event_count += 1;
                            info!("💵 Trade Executed!");
                            info!("   Asset: {}", asset_id);
                            info!("   Side: {:?} | Price: ${:.4} | Size: {}", side, price, size);
                            info!("   Value: ${:.2}", price * size);
                            
                            if asset_id == strategy.token_id() {
                                info!("   ✨ This is our monitored token!");
                            }
                        }
                        _ => {
                            // Log any other event types we might receive
                            debug!("Received other event type");
                        }
                    }
                    
                    // Check if this is a trade event
                    if let Some(trade_event) = Option::<TradeEvent>::from(&event) {
                        if trade_event.asset_id == strategy.token_id() {
                            if let Err(e) = strategy.trade_event(&trade_event).await {
                                error!("Strategy trade event error: {}", e);
                            }
                        }
                    }
                    
                    // Check if this is an orderbook update
                    match &event {
                        PolyEvent::Book { asset_id, .. } => {
                            if asset_id == strategy.token_id() {
                                // Get the current orderbook from streamer
                                if let Some(orderbook) = streamer.get_order_book(asset_id) {
                                    if let Err(e) = strategy.orderbook_update(&orderbook).await {
                                        error!("Strategy orderbook update error: {}", e);
                                    }
                                } else {
                                    warn!("No orderbook found for asset: {}", asset_id);
                                }
                            }
                        }
                        PolyEvent::PriceChange { asset_id, .. } => {
                            if asset_id == strategy.token_id() {
                                // Get the updated orderbook from streamer
                                if let Some(orderbook) = streamer.get_order_book(asset_id) {
                                    if let Err(e) = strategy.orderbook_update(&orderbook).await {
                                        error!("Strategy orderbook update error: {}", e);
                                    }
                                } else {
                                    warn!("No orderbook found for asset: {}", asset_id);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                
                else => {
                    // Channel closed or error
                    error!("Event receiver closed, shutting down");
                    break;
                }
            }
        }
        
        // Shutdown
        info!("🛑 Stopping WebSocket streamer...");
        streamer.stop().await;
        
        // Give a moment for any pending operations
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Print final statistics
        let order_stats = polybot.order.get_statistics().await;
        info!("\n📊 Final Session Statistics:");
        info!("═══════════════════════════════════════");
        info!("📈 Market Events:");
        info!("   Total Events Processed: {}", event_count);
        info!("   Order Events: {}", order_event_count);
        info!("");
        info!("📋 Order Statistics:");
        info!("   Orders Placed: {}", order_stats.orders_placed);
        info!("   Successful Orders: {}", order_stats.successful_orders);
        info!("   Failed Orders: {}", order_stats.failed_orders);
        info!("   Total Volume: ${:.2}", order_stats.total_volume_traded);
        info!("═══════════════════════════════════════");
        info!("");
        info!("✅ Strategy shutdown complete");
        
        // Give a final moment for threads to clean up
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        Ok(())
    }
}