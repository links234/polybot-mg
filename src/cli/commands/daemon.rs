//! Daemon command for long-running WebSocket streaming with sample strategy

use anyhow::Result;
use clap::Args;
use owo_colors::OwoColorize;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::str::FromStr;
use std::time::Duration;
use tokio::signal;
use tracing::{info, warn};

use crate::data_paths::DataPaths;
use crate::services::{Streamer, StreamerConfig};
use crate::ws::{AuthPayload, PolyEvent, WsConfig};

#[derive(Args, Clone)]
pub struct DaemonArgs {
    /// Asset IDs to stream (comma-separated)
    #[arg(short, long, value_delimiter = ',')]
    pub assets: Vec<String>,

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

    /// Print order book summary every N seconds
    #[arg(long, default_value = "30")]
    pub summary_interval: u64,

    /// Use sandbox environment
    #[arg(long)]
    pub sandbox: bool,
}

pub struct DaemonCommand {
    args: DaemonArgs,
}

impl DaemonCommand {
    pub fn new(args: DaemonArgs) -> Self {
        Self { args }
    }

    pub async fn execute(&self, host: &str, data_paths: DataPaths) -> Result<()> {
        if self.args.assets.is_empty() {
            return Err(anyhow::anyhow!(
                "At least one asset ID must be provided with --assets"
            ));
        }

        info!(
            "{}",
            "🤖 Polymarket Streaming Daemon Starting...".bright_blue()
        );
        info!(
            "{}",
            format!("📊 Monitoring {} assets", self.args.assets.len()).bright_cyan()
        );

        // Configure WebSocket
        let ws_config = WsConfig {
            heartbeat_interval: self.args.heartbeat_interval,
            max_reconnection_attempts: 0, // Infinite retries for daemon
            ..Default::default()
        };

        // Configure authentication if provided
        let user_auth = if let (Some(api_key), Some(secret), Some(passphrase)) =
            (&self.args.api_key, &self.args.secret, &self.args.passphrase)
        {
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
            market_assets: self.args.assets.clone(),
            user_markets: self.args.markets.clone(),
            user_auth,
            event_buffer_size: 10000, // Larger buffer for daemon
            auto_sync_on_hash_mismatch: true,
        };

        // Create and start streamer
        let mut streamer = Streamer::new(streamer_config);
        streamer.start(host, &data_paths).await?;

        // Set up event handling for sample strategy
        let mut events = streamer.events();
        let mut summary_timer =
            tokio::time::interval(Duration::from_secs(self.args.summary_interval));

        info!(
            "{}",
            "✅ Daemon started. Press Ctrl+C to stop.".bright_green()
        );
        info!(
            "Streaming daemon running for assets: {:?}",
            self.args.assets
        );

        // Main event loop with sample strategy
        loop {
            tokio::select! {
                // Handle streaming events
                result = events.recv() => {
                    match result {
                        Ok(event) => {
                            self.handle_strategy_event(event).await;
                        }
                        Err(e) => {
                            warn!("Event receive error: {}", e);
                        }
                    }
                }

                // Periodic summary and strategy execution
                _ = summary_timer.tick() => {
                    self.execute_sample_strategy(&streamer).await;
                }

                // Handle shutdown signal
                _ = signal::ctrl_c() => {
                    info!("\n{}", "🛑 Shutdown signal received...".bright_yellow());
                    break;
                }
            }
        }

        // Stop streamer
        info!("Stopping streaming daemon");
        streamer.stop().await;
        info!("{}", "✅ Daemon stopped.".bright_green());

        Ok(())
    }

    /// Handle individual streaming events for strategy
    async fn handle_strategy_event(&self, event: PolyEvent) {
        match event {
            PolyEvent::Book {
                asset_id,
                bids,
                asks,
                ..
            } => {
                if let (Some(best_bid), Some(best_ask)) = (bids.first(), asks.first()) {
                    let spread = best_ask.price - best_bid.price;
                    let mid_price = (best_bid.price + best_ask.price) / Decimal::from(2);

                    // Log significant spread changes
                    if spread > Decimal::from_str("0.05").unwrap() {
                        info!(
                            "Wide spread detected: {} - mid: ${}, spread: ${} ({:.2}%)",
                            asset_id,
                            mid_price,
                            spread,
                            (spread / mid_price * Decimal::from(100))
                                .to_f64()
                                .unwrap_or(0.0)
                        );
                    }
                }
            }

            PolyEvent::Trade {
                asset_id,
                price,
                size,
                side,
                ..
            } => {
                // Log large trades
                if size > Decimal::from(1000) {
                    info!(
                        "Large trade: {} - {:?} {} @ ${}",
                        asset_id, side, size, price
                    );
                }
            }

            PolyEvent::MyTrade {
                asset_id,
                side,
                price,
                size,
                ..
            } => {
                info!(
                    "My trade executed: {} - {:?} {} @ ${}",
                    asset_id, side, size, price
                );
            }

            _ => {} // Ignore other events
        }
    }

    /// Execute sample strategy on periodic intervals
    async fn execute_sample_strategy(&self, streamer: &Streamer) {
        let order_books = streamer.get_all_order_books();

        if order_books.is_empty() {
            warn!("No order books available for strategy execution");
            return;
        }

        info!("\n{}", "📈 Strategy Analysis:".bright_blue());

        for asset_order_book in order_books {
            let asset_id = &asset_order_book.asset_id;
            let book = &asset_order_book.order_book;
            if let (Some(best_bid), Some(best_ask)) = (book.best_bid(), book.best_ask()) {
                let spread = best_ask.price - best_bid.price;
                let spread_pct = (spread / best_bid.price * Decimal::from(100))
                    .to_f64()
                    .unwrap_or(0.0);
                let mid_price = (best_bid.price + best_ask.price) / Decimal::from(2);

                // Sample strategy: identify arbitrage opportunities
                if spread_pct > 2.0 {
                    info!(
                        "  🎯 {} - Wide spread opportunity: {:.2}% (mid: ${})",
                        asset_id.bright_cyan(),
                        spread_pct,
                        mid_price
                    );
                }

                // Sample strategy: identify liquidity imbalances
                let liquidity_ratio = best_bid.size / best_ask.size;
                if liquidity_ratio > Decimal::from(2) {
                    info!(
                        "  📊 {} - Bid heavy: {:.1}:1 ratio (price pressure up?)",
                        asset_id.bright_cyan(),
                        liquidity_ratio.to_f64().unwrap_or(0.0)
                    );
                } else if liquidity_ratio < Decimal::from_str("0.5").unwrap() {
                    info!(
                        "  📊 {} - Ask heavy: 1:{:.1} ratio (price pressure down?)",
                        asset_id.bright_cyan(),
                        (Decimal::from(1) / liquidity_ratio).to_f64().unwrap_or(0.0)
                    );
                }

                // Basic market summary
                info!(
                    "  💹 {} - Mid: ${}, Spread: {:.2}%, Depth: {}/${} (bid/ask)",
                    asset_id.bright_white(),
                    mid_price,
                    spread_pct,
                    best_bid.size,
                    best_ask.size
                );
            } else {
                info!("  ⚠️  {} - Incomplete order book", asset_id.bright_yellow());
            }
        }

        info!("");
    }
}
