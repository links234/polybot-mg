//! Run strategy command implementation

use anyhow::Result;
use clap::Args;
use rust_decimal::Decimal;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::signal;
use tracing::{debug, error, info, warn};

use crate::config;
use crate::core::execution::orders::{OrderConfig, PolyBot};
use crate::core::services::{Streamer, StreamerConfig};
use crate::core::ws::{PolyEvent, WsConfig};
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
}

pub struct RunStrategyCommand {
    args: RunStrategyArgs,
}

impl RunStrategyCommand {
    pub fn new(args: RunStrategyArgs) -> Self {
        Self { args }
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
        
        info!("🚀 Starting {} strategy for {} token(s)", self.args.strategy, self.args.token_id.len());
        for token in &self.args.token_id {
            info!("  - Token: {}", token);
        }
        
        // Validate token ID format
        for token in &self.args.token_id {
            if token.len() < 50 && !token.starts_with("0x") {
                warn!("⚠️  Token ID '{}' appears to be in incorrect format", token);
                warn!("   Polymarket uses long decimal asset IDs (70+ digits)");
                warn!("   Example: 108468416668663017133298741485453125150952822149773262784582671647441799250111");
            }
        }
        
        // Load credentials and initialize PolyBot
        let _api_creds = config::load_credentials(&data_paths).await?;
        
        // Create PolyBot coordinator
        let order_config = OrderConfig {
            enable_detailed_logging: false, // Can be controlled via RUST_LOG env var
            ..Default::default()
        };
        let polybot = Arc::new(PolyBot::with_order_config(order_config));
        
        // For now, we'll use the first token ID for the strategy
        let primary_token = self.args.token_id.first()
            .ok_or_else(|| anyhow::anyhow!("No token ID provided"))?
            .clone();
        
        // Create strategy based on type
        let strategy: Box<dyn SingleTokenStrategy> = match self.args.strategy.as_str() {
            "simple" => {
                let config = simple_strategy::SimpleStrategyConfig {
                    min_spread_threshold: Decimal::try_from(self.args.min_spread)?,
                    max_spread_threshold: Decimal::try_from(self.args.max_spread)?,
                    volume_window: Duration::from_secs(self.args.volume_window),
                    log_frequency: self.args.log_frequency,
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
        
        info!("📊 Strategy Configuration:");
        info!("   Strategy: {}", strategy.name());
        info!("   Primary Token: {}", strategy.token_id());
        info!("   Min Spread: ${:.4}", self.args.min_spread);
        info!("   Max Spread: ${:.4}", self.args.max_spread);
        info!("   Volume Window: {}s", self.args.volume_window);
        info!("   Log Frequency: every {} updates", self.args.log_frequency);
        
        // Create WebSocket configuration
        let ws_config = WsConfig {
            market_url: "wss://ws-subscriptions-clob.polymarket.com/ws/market".to_string(),
            user_url: "wss://ws-subscriptions-clob.polymarket.com/ws/user".to_string(),
            heartbeat_interval: 10,
            max_reconnection_attempts: self.args.max_reconnection_attempts,
            initial_reconnection_delay: 1000,
            max_reconnection_delay: 60000,
            skip_hash_verification: false,
        };
        
        // Create streamer configuration
        let market_assets = if self.args.all_markets {
            info!("⚠️  Subscribing to ALL markets (testing mode)");
            vec![] // Empty array subscribes to all markets
        } else {
            self.args.token_id.clone()
        };
        
        let streamer_config = StreamerConfig {
            market_assets: market_assets.clone(),
            user_markets: None,
            user_auth: None,
            ws_config,
            event_buffer_size: 1000,
            auto_sync_on_hash_mismatch: true,
        };
        
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
        
        // Main event loop
        loop {
            tokio::select! {
                // Handle shutdown signal
                _ = &mut shutdown_signal => {
                    info!("\n⏹️  Shutting down strategy...");
                    break;
                }
                
                // Periodic status update
                _ = tokio::time::sleep(Duration::from_secs(30)) => {
                    if event_count == 0 {
                        info!("💓 Strategy running - waiting for market events...");
                    } else {
                        info!("💓 Strategy running - {} events processed, last event {} seconds ago", 
                              event_count, last_event_time.elapsed().as_secs());
                    }
                }
                
                // Handle streaming events
                Ok(event) = event_receiver.recv() => {
                    event_count += 1;
                    last_event_time = std::time::Instant::now();
                    // Debug log to see what events we're receiving
                    match &event {
                        PolyEvent::Book { asset_id, bids, asks, .. } => {
                            info!("📊 Received Book event for asset: {}", asset_id);
                            info!("   Bids: {} levels, Asks: {} levels", bids.len(), asks.len());
                            info!("   Target match: {}", asset_id == strategy.token_id());
                        }
                        PolyEvent::PriceChange { asset_id, side, price, size, .. } => {
                            info!("💱 Received PriceChange event for asset: {}", asset_id);
                            info!("   Side: {:?}, Price: ${}, Size: {}", side, price, size);
                            info!("   Target match: {}", asset_id == strategy.token_id());
                        }
                        PolyEvent::Trade { asset_id, price, size, side } => {
                            info!("💰 Received Trade event for asset: {}", asset_id);
                            info!("   Side: {:?}, Price: ${}, Size: {}", side, price, size);
                            info!("   Target match: {}", asset_id == strategy.token_id());
                        }
                        PolyEvent::LastTradePrice { asset_id, price, timestamp } => {
                            info!("📈 Received LastTradePrice for asset: {}", asset_id);
                            info!("   Price: ${}, Timestamp: {}", price, timestamp);
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
        streamer.stop().await;
        
        // Print final statistics
        let order_stats = polybot.order.get_statistics().await;
        info!("\n📈 Final Strategy Statistics:");
        info!("   Orders Placed: {}", order_stats.orders_placed);
        info!("   Successful Orders: {}", order_stats.successful_orders);
        info!("   Failed Orders: {}", order_stats.failed_orders);
        info!("   Total Volume: ${:.2}", order_stats.total_volume_traded);
        
        Ok(())
    }
}