//! Simple strategy implementation
//!
//! A basic strategy that monitors orderbook spreads and trade activity,
//! logging insights and demonstrating the strategy interface.

use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::core::execution::orders::PolyBot;
use crate::core::types::common::Side;
use crate::core::ws::OrderBook;
use crate::strategy::{SingleTokenStrategy, TradeEvent};

/// Configuration for the simple strategy
#[derive(Debug, Clone)]
pub struct SimpleStrategyConfig {
    /// Minimum spread (in price units) to consider market wide
    pub min_spread_threshold: Decimal,
    /// Maximum spread (in price units) to consider market tight
    pub max_spread_threshold: Decimal,
    /// Time window for trade volume analysis
    pub volume_window: Duration,
    /// Log frequency for orderbook updates (only log every Nth update)
    pub log_frequency: u32,
}

impl Default for SimpleStrategyConfig {
    fn default() -> Self {
        Self {
            min_spread_threshold: Decimal::new(1, 3), // 0.001
            max_spread_threshold: Decimal::new(1, 2), // 0.01
            volume_window: Duration::from_secs(300),  // 5 minutes
            log_frequency: 10, // Log every 10th orderbook update
        }
    }
}

/// Simple strategy state
#[derive(Debug)]
struct StrategyState {
    /// Number of orderbook updates received
    update_count: u64,
    /// Last spread observed
    last_spread: Option<Decimal>,
    /// Recent trades for volume analysis
    recent_trades: Vec<(Instant, TradeEvent)>,
    /// Total buy volume in window
    buy_volume: Decimal,
    /// Total sell volume in window
    sell_volume: Decimal,
}

/// A simple strategy implementation
pub struct SimpleStrategy {
    /// Strategy configuration
    config: SimpleStrategyConfig,
    /// Token ID this strategy monitors
    token_id: String,
    /// Strategy name
    name: String,
    /// Reference to the polybot coordinator
    polybot: Arc<PolyBot>,
    /// Internal state
    state: Arc<RwLock<StrategyState>>,
}

impl SimpleStrategy {
    /// Create a new simple strategy
    pub fn new(
        token_id: String,
        config: SimpleStrategyConfig,
        polybot: Arc<PolyBot>,
    ) -> Self {
        let name = format!("SimpleStrategy-{}", &token_id[..8.min(token_id.len())]);
        
        Self {
            config,
            token_id,
            name,
            polybot,
            state: Arc::new(RwLock::new(StrategyState {
                update_count: 0,
                last_spread: None,
                recent_trades: Vec::new(),
                buy_volume: Decimal::ZERO,
                sell_volume: Decimal::ZERO,
            })),
        }
    }
    
    /// Create with default configuration
    pub fn with_defaults(token_id: String, polybot: Arc<PolyBot>) -> Self {
        Self::new(token_id, SimpleStrategyConfig::default(), polybot)
    }
    
    /// Clean up old trades outside the volume window
    async fn cleanup_old_trades(&self, state: &mut StrategyState) {
        let cutoff = Instant::now() - self.config.volume_window;
        
        // Remove old trades and update volumes
        state.recent_trades.retain(|(timestamp, trade)| {
            if *timestamp < cutoff {
                // Remove from volume tracking
                match trade.side {
                    Side::Buy => state.buy_volume -= trade.size,
                    Side::Sell => state.sell_volume -= trade.size,
                }
                false
            } else {
                true
            }
        });
    }
    
    /// Analyze current market conditions
    async fn analyze_market(&self, orderbook: &OrderBook, state: &StrategyState) {
        if let (Some(best_bid), Some(best_ask)) = (orderbook.best_bid(), orderbook.best_ask()) {
            let spread = best_ask.price - best_bid.price;
            let mid_price = (best_bid.price + best_ask.price) / Decimal::TWO;
            let spread_percentage = (spread / mid_price) * Decimal::ONE_HUNDRED;
            
            // Determine market condition
            let market_condition = if spread <= self.config.min_spread_threshold {
                "VERY TIGHT"
            } else if spread <= self.config.max_spread_threshold {
                "NORMAL"
            } else {
                "WIDE"
            };
            
            // Calculate order imbalance
            let bid_depth: Decimal = orderbook.bids.values()
                .take(5)
                .copied()
                .sum();
                
            let ask_depth: Decimal = orderbook.asks.values()
                .take(5)
                .copied()
                .sum();
                
            let total_depth = bid_depth + ask_depth;
            let imbalance = if total_depth > Decimal::ZERO {
                ((bid_depth - ask_depth) / total_depth) * Decimal::ONE_HUNDRED
            } else {
                Decimal::ZERO
            };
            
            // Log market analysis
            info!(
                "[{}] Market Analysis - Spread: ${:.4} ({:.2}%) [{}] | Mid: ${:.4} | Imbalance: {:.1}% | Buy Vol: {} | Sell Vol: {}",
                self.name,
                spread,
                spread_percentage,
                market_condition,
                mid_price,
                imbalance,
                state.buy_volume,
                state.sell_volume
            );
            
            // Check for significant changes
            if let Some(last_spread) = state.last_spread {
                let spread_change = ((spread - last_spread).abs() / last_spread) * Decimal::ONE_HUNDRED;
                if spread_change > Decimal::TEN {
                    warn!(
                        "[{}] Significant spread change detected: {:.1}% (${:.4} → ${:.4})",
                        self.name,
                        spread_change,
                        last_spread,
                        spread
                    );
                }
            }
        }
    }
}

#[async_trait]
impl SingleTokenStrategy for SimpleStrategy {
    async fn orderbook_update(&self, orderbook: &OrderBook) -> Result<()> {
        let mut state = self.state.write().await;
        state.update_count += 1;
        
        // Log first update to confirm we're receiving data
        if state.update_count == 1 {
            info!("[{}] First orderbook update received!", self.name);
        }
        
        // Only log periodically to avoid spam
        if state.update_count % self.config.log_frequency as u64 != 0 {
            return Ok(());
        }
        
        // Clean up old trades
        self.cleanup_old_trades(&mut state).await;
        
        // Update spread tracking
        if let (Some(best_bid), Some(best_ask)) = (orderbook.best_bid(), orderbook.best_ask()) {
            state.last_spread = Some(best_ask.price - best_bid.price);
        }
        
        // Analyze market conditions
        self.analyze_market(orderbook, &state).await;
        
        // Access order manager stats
        let order_stats = self.polybot.order.get_statistics().await;
        if order_stats.orders_placed > 0 {
            info!(
                "[{}] Order Stats - Placed: {} | Success: {} | Failed: {} | Volume: ${:.2}",
                self.name,
                order_stats.orders_placed,
                order_stats.successful_orders,
                order_stats.failed_orders,
                order_stats.total_volume_traded
            );
        }
        
        Ok(())
    }
    
    async fn trade_event(&self, trade: &TradeEvent) -> Result<()> {
        let mut state = self.state.write().await;
        
        // Add to recent trades
        state.recent_trades.push((Instant::now(), trade.clone()));
        
        // Update volume tracking
        match trade.side {
            Side::Buy => state.buy_volume += trade.size,
            Side::Sell => state.sell_volume += trade.size,
        }
        
        // Clean up old trades
        self.cleanup_old_trades(&mut state).await;
        
        // Log significant trades
        let total_volume = state.buy_volume + state.sell_volume;
        if trade.size > Decimal::ZERO && total_volume > Decimal::ZERO {
            let trade_percentage = (trade.size / total_volume) * Decimal::ONE_HUNDRED;
            
            if trade_percentage > Decimal::ONE {
                info!(
                    "[{}] Significant Trade: {} {} @ ${:.4} ({:.1}% of {}m volume)",
                    self.name,
                    match trade.side {
                        Side::Buy => "BUY",
                        Side::Sell => "SELL",
                    },
                    trade.size,
                    trade.price,
                    trade_percentage,
                    self.config.volume_window.as_secs() / 60
                );
            }
        }
        
        Ok(())
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    fn token_id(&self) -> &str {
        &self.token_id
    }
}