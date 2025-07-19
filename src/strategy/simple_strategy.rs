//! Simple strategy implementation
//!
//! A basic strategy that monitors orderbook spreads and trade activity,
//! logging insights and demonstrating the strategy interface.

use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc, oneshot};
use tokio::time::{interval, MissedTickBehavior};
use tracing::{info, warn, debug, error};
use polymarket_rs_client::ClobClient;
use owo_colors::OwoColorize;

use crate::core::execution::orders::{PolyBot, OrderPlacementResponse};
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
    /// Interval for checking if we should place orders
    pub order_check_interval: Duration,
    /// Maximum number of active orders to maintain
    pub max_active_orders: usize,
    /// Base discount percentage for orders (e.g., 0.005 = 0.5%)
    pub base_discount_percent: Decimal,
    /// Discount increment per order (e.g., 0.005 = 0.5%)
    pub discount_increment: Decimal,
    /// Tick size for price rounding (e.g., 0.01 for cent precision)
    pub tick_size: Decimal,
    /// Base order size in shares (for testing, keep small)
    pub base_order_size: Decimal,
    /// Maximum total value per order in USD
    pub max_order_value: Decimal,
}

impl Default for SimpleStrategyConfig {
    fn default() -> Self {
        Self {
            min_spread_threshold: Decimal::new(1, 3), // 0.001
            max_spread_threshold: Decimal::new(1, 2), // 0.01
            volume_window: Duration::from_secs(300),  // 5 minutes
            log_frequency: 10, // Log every 10th orderbook update
            order_check_interval: Duration::from_secs(10), // Check every 10 seconds
            max_active_orders: 3, // Max 3 active orders
            base_discount_percent: Decimal::new(5, 3), // 0.005 = 0.5%
            discount_increment: Decimal::new(5, 3), // 0.005 = 0.5% increment
            tick_size: Decimal::new(1, 2), // 0.01 = cent precision
            base_order_size: Decimal::new(5, 0), // 5 shares base size
            max_order_value: Decimal::new(250, 2), // $2.50 max per order
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
    /// Number of active orders
    active_order_count: usize,
    /// Last orderbook snapshot for order decisions
    last_orderbook: Option<OrderBook>,
    /// Track placed order IDs
    active_order_ids: Vec<String>,
    /// Pending approved orders to be placed
    pending_orders: Vec<ProposedOrder>,
    /// Event count at last market analysis log
    last_analysis_event_count: u64,
    /// Total events processed since strategy start
    total_events_processed: u64,
}

/// Command for order placement
#[derive(Debug)]
enum OrderCommand {
    ProposeOrders(Vec<ProposedOrder>, OrderBook, oneshot::Sender<Option<Vec<ProposedOrder>>>),
    Shutdown,
}

/// Proposed order details
#[derive(Debug, Clone)]
pub struct ProposedOrder {
    pub price: Decimal,
    pub size: Decimal,
    pub side: Side,
    pub discount_percent: Decimal,
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
    /// Command channel for order placement
    order_command_tx: mpsc::Sender<OrderCommand>,
    /// ClobClient for placing orders
    clob_client: Option<Arc<tokio::sync::Mutex<ClobClient>>>,
    /// Shutdown flag for graceful termination
    shutdown_flag: Arc<AtomicBool>,
}

impl SimpleStrategy {
    /// Create a new simple strategy
    pub fn new(
        token_id: String,
        config: SimpleStrategyConfig,
        polybot: Arc<PolyBot>,
    ) -> Self {
        let name = format!("SimpleStrategy-{}", &token_id[..8.min(token_id.len())]);
        
        let (order_command_tx, order_command_rx) = mpsc::channel(100);
        let shutdown_flag = Arc::new(AtomicBool::new(false));
        
        let strategy = Self {
            config: config.clone(),
            token_id: token_id.clone(),
            name: name.clone(),
            polybot: polybot.clone(),
            state: Arc::new(RwLock::new(StrategyState {
                update_count: 0,
                last_spread: None,
                recent_trades: Vec::new(),
                buy_volume: Decimal::ZERO,
                sell_volume: Decimal::ZERO,
                active_order_count: 0,
                last_orderbook: None,
                active_order_ids: Vec::new(),
                pending_orders: Vec::new(),
                last_analysis_event_count: 0,
                total_events_processed: 0,
            })),
            order_command_tx,
            clob_client: None,
            shutdown_flag: shutdown_flag.clone(),
        };
        
        // Spawn the order timer task
        let state_clone = strategy.state.clone();
        let config_clone = config.clone();
        let polybot_clone = polybot.clone();
        let token_id_clone = token_id.clone();
        let name_clone = name.clone();
        let order_tx = strategy.order_command_tx.clone();
        
        tokio::spawn(async move {
            Self::order_timer_task(
                state_clone,
                config_clone,
                polybot_clone,
                token_id_clone,
                name_clone,
                order_tx,
            ).await;
        });
        
        // Spawn the user input handler task
        let name_clone = name.clone();
        let shutdown_flag_clone = shutdown_flag.clone();
        tokio::spawn(async move {
            Self::user_input_handler(order_command_rx, name_clone, shutdown_flag_clone).await;
        });
        
        strategy
    }
    
    /// Create with default configuration
    pub fn with_defaults(token_id: String, polybot: Arc<PolyBot>) -> Self {
        Self::new(token_id, SimpleStrategyConfig::default(), polybot)
    }
    
    /// Set the ClobClient for order placement
    pub fn set_clob_client(&mut self, client: Arc<tokio::sync::Mutex<ClobClient>>) {
        self.clob_client = Some(client);
    }
    
    /// Check if shutdown has been requested
    pub fn is_shutdown_requested(&self) -> bool {
        self.shutdown_flag.load(Ordering::Relaxed)
    }
    
    /// Shutdown the strategy cleanly
    pub async fn shutdown(&self) -> Result<()> {
        // Set shutdown flag and send shutdown command
        self.shutdown_flag.store(true, Ordering::Relaxed);
        let _ = self.order_command_tx.send(OrderCommand::Shutdown).await;
        info!("[{}] Strategy shutdown initiated", self.name);
        Ok(())
    }
    
    /// Check if we have pending orders to place and process them
    pub async fn process_pending_orders(&self) -> Result<()> {
        // Get pending orders from state
        let pending_orders = {
            let mut state = self.state.write().await;
            let orders = state.pending_orders.clone();
            state.pending_orders.clear(); // Clear after taking
            orders
        };
        
        if pending_orders.is_empty() {
            return Ok(());
        }
        
        // Display order placement header
        println!("\n{}", "🚀 Placing Orders".bright_cyan().bold());
        println!("{}", "═".repeat(60).bright_black());
        
        info!("[{}] Processing {} pending orders", self.name, pending_orders.len());
        
        // Place the orders and show results
        let responses = self.place_orders(pending_orders.clone()).await?;
        
        // Display results
        println!("\n{}", "📊 Order Placement Results:".bright_yellow());
        
        let mut total_placed_value = Decimal::ZERO;
        let mut successful_orders = Vec::new();
        let mut failed_orders = Vec::new();
        
        for (i, (proposal, response)) in pending_orders.iter().zip(responses.iter()).enumerate() {
            let total_value = proposal.size * proposal.price;
            
            if response.success {
                successful_orders.push((proposal, response, total_value));
                total_placed_value += total_value;
                
                println!("   {} {} {} {} @ ${} = ${} {}",
                    format!("[{}]", i + 1).bright_white(),
                    "✅".bright_green(),
                    match proposal.side {
                        Side::Buy => "BUY".bright_green().to_string(),
                        Side::Sell => "SELL".bright_red().to_string(),
                    },
                    format!("{} shares", proposal.size).bright_white(),
                    format!("{:.4}", proposal.price).bright_cyan(),
                    format!("{:.2}", total_value).bright_yellow(),
                    if let Some(order_id) = &response.order_id {
                        format!("(Order: {})", &order_id[..8.min(order_id.len())]).bright_black().to_string()
                    } else {
                        "".to_string()
                    }
                );
            } else {
                failed_orders.push((proposal, response, total_value));
                
                println!("   {} {} {} {} @ ${} = ${} {}",
                    format!("[{}]", i + 1).bright_white(),
                    "❌".bright_red(),
                    match proposal.side {
                        Side::Buy => "BUY".bright_green().to_string(),
                        Side::Sell => "SELL".bright_red().to_string(),
                    },
                    format!("{} shares", proposal.size).bright_white(),
                    format!("{:.4}", proposal.price).bright_cyan(),
                    format!("{:.2}", total_value).bright_yellow(),
                    if let Some(error) = &response.error_message {
                        format!("({})", error).bright_red().to_string()
                    } else {
                        "(Unknown error)".bright_red().to_string()
                    }
                );
            }
        }
        
        // Summary
        println!("\n{}", "─".repeat(60).bright_black());
        
        let successful_count = successful_orders.len();
        let failed_count = failed_orders.len();
        let total_count = responses.len();
        
        println!("   {} {} {} {}",
            "Summary:".bright_white().bold(),
            format!("{}/{}", successful_count, total_count).bright_green(),
            "orders placed successfully".bright_white(),
            if failed_count > 0 {
                format!("({} failed)", failed_count).bright_red().to_string()
            } else {
                "".to_string()
            }
        );
        
        if successful_count > 0 {
            println!("   {} ${}", 
                "Total Value Placed:".bright_white(),
                format!("{:.2}", total_placed_value).bright_yellow().bold()
            );
        }
        
        println!("{}", "═".repeat(60).bright_black());
        
        info!("[{}] Successfully placed {} out of {} orders", self.name, successful_count, total_count);
        
        Ok(())
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
    async fn analyze_market(&self, orderbook: &OrderBook, state: &mut StrategyState) {
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
            
            // Calculate events since last analysis
            let events_since_last = state.total_events_processed - state.last_analysis_event_count;
            
            // Log market analysis with event count
            info!(
                "[{}] Market Analysis ({} events) - Spread: ${:.4} ({:.2}%) [{}] | Mid: ${:.4} | Imbalance: {:.1}% | Buy Vol: {} | Sell Vol: {}",
                self.name,
                events_since_last,
                spread,
                spread_percentage,
                market_condition,
                mid_price,
                imbalance,
                state.buy_volume,
                state.sell_volume
            );
            
            // Update last analysis event count
            state.last_analysis_event_count = state.total_events_processed;
            
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
    
    /// Order timer task that checks every N seconds if we should place orders
    async fn order_timer_task(
        state: Arc<RwLock<StrategyState>>,
        config: SimpleStrategyConfig,
        _polybot: Arc<PolyBot>,
        _token_id: String,
        name: String,
        order_tx: mpsc::Sender<OrderCommand>,
    ) {
        let mut timer = interval(config.order_check_interval);
        timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
        
        info!("[{}] Order timer task started (checking every {} seconds)", name, config.order_check_interval.as_secs());
        
        loop {
            timer.tick().await;
            
            // Check if we should propose orders
            let should_propose = {
                let state_guard = state.read().await;
                state_guard.active_order_count < config.max_active_orders && 
                state_guard.last_orderbook.is_some()
            };
            
            if !should_propose {
                continue;
            }
            
            // Get orderbook and prepare proposals
            let (proposals, orderbook) = {
                let state_guard = state.read().await;
                if let Some(orderbook) = &state_guard.last_orderbook {
                    match Self::calculate_order_proposals(
                        orderbook,
                        &config,
                        state_guard.active_order_count,
                    ) {
                        Ok(props) => (props, orderbook.clone()),
                        Err(e) => {
                            warn!("[{}] Failed to calculate order proposals: {}", name, e);
                            continue;
                        }
                    }
                } else {
                    continue;
                }
            };
            
            if proposals.is_empty() {
                debug!("[{}] No orders to propose", name);
                continue;
            }
            
            // Send proposals to user input handler with orderbook
            let (response_tx, response_rx) = oneshot::channel();
            if let Err(e) = order_tx.send(OrderCommand::ProposeOrders(proposals, orderbook, response_tx)).await {
                error!("[{}] Failed to send order proposals: {}", name, e);
                break;
            }
            
            // Wait for user response
            match response_rx.await {
                Ok(Some(approved_orders)) => {
                    info!("[{}] User approved {} orders", name, approved_orders.len());
                    
                    // Store approved orders in state for placement
                    let mut state_guard = state.write().await;
                    state_guard.pending_orders.extend(approved_orders);
                    
                    info!("[{}] {} orders pending placement", name, state_guard.pending_orders.len());
                }
                Ok(None) => {
                    info!("[{}] User rejected order proposals", name);
                }
                Err(e) => {
                    error!("[{}] Failed to receive user response: {}", name, e);
                }
            }
        }
        
        info!("[{}] Order timer task shutting down", name);
    }
    
    /// Round a price to the nearest tick size
    fn round_to_tick_size(price: Decimal, tick_size: Decimal) -> Decimal {
        if tick_size == Decimal::ZERO {
            return price;
        }
        
        // Calculate how many ticks fit into the price
        let ticks = price / tick_size;
        
        // Round to nearest integer
        let rounded_ticks = ticks.round();
        
        // Return the rounded price
        rounded_ticks * tick_size
    }
    
    /// Calculate market depth and price impact for a given volume
    fn calculate_market_depth(
        orderbook: &OrderBook,
        side: Side,
        volume_percent: Decimal,
    ) -> (Decimal, Decimal, Vec<(Decimal, Decimal)>) {
        // Get the side of the book we're hitting
        let levels: Vec<_> = match side {
            Side::Buy => {
                // For buying, we hit the asks (sorted low to high)
                let mut asks: Vec<_> = orderbook.asks.iter().collect();
                asks.sort_by(|a, b| a.0.cmp(b.0));
                asks.into_iter().map(|(price, size)| (*price, *size)).collect()
            }
            Side::Sell => {
                // For selling, we hit the bids (sorted high to low)
                let mut bids: Vec<_> = orderbook.bids.iter().collect();
                bids.sort_by(|a, b| b.0.cmp(a.0));
                bids.into_iter().map(|(price, size)| (*price, *size)).collect()
            }
        };
        
        if levels.is_empty() {
            return (Decimal::ZERO, Decimal::ZERO, vec![]);
        }
        
        // Calculate total volume available
        let total_volume: Decimal = levels.iter().map(|(_, size)| size).sum();
        let target_volume = total_volume * volume_percent / Decimal::ONE_HUNDRED;
        
        // Walk through levels until we consume target volume
        let mut remaining_volume = target_volume;
        let mut total_cost = Decimal::ZERO;
        let mut levels_consumed = Vec::new();
        let mut last_price = levels[0].0;
        
        for (price, size) in &levels {
            if remaining_volume <= Decimal::ZERO {
                break;
            }
            
            let volume_at_level = (*size).min(remaining_volume);
            total_cost += volume_at_level * price;
            remaining_volume -= volume_at_level;
            last_price = *price;
            
            if volume_at_level > Decimal::ZERO {
                levels_consumed.push((*price, volume_at_level));
            }
        }
        
        // Calculate average price (VWAP)
        let volume_filled = target_volume - remaining_volume;
        let avg_price = if volume_filled > Decimal::ZERO {
            total_cost / volume_filled
        } else {
            Decimal::ZERO
        };
        
        // Calculate price impact
        let best_price = levels[0].0;
        let price_impact = ((last_price - best_price).abs() / best_price) * Decimal::ONE_HUNDRED;
        
        (avg_price, price_impact, levels_consumed)
    }
    
    /// Calculate order proposals based on current orderbook
    fn calculate_order_proposals(
        orderbook: &OrderBook,
        config: &SimpleStrategyConfig,
        current_order_count: usize,
    ) -> Result<Vec<ProposedOrder>> {
        let mut proposals = Vec::new();
        
        // Get best bid
        let best_bid = orderbook.best_bid()
            .ok_or_else(|| anyhow::Error::msg("No best bid available"))?;
        
        // Calculate how many more orders we can place
        let orders_to_place = config.max_active_orders.saturating_sub(current_order_count);
        
        // Generate proposals with increasing discounts
        for i in 0..orders_to_place {
            let discount_multiplier = Decimal::from(current_order_count + i);
            let total_discount = config.base_discount_percent + (config.discount_increment * discount_multiplier);
            
            // Calculate discounted price
            let discounted_price = best_bid.price * (Decimal::ONE - total_discount);
            
            // Round to tick size after discount calculation
            let price = Self::round_to_tick_size(discounted_price, config.tick_size);
            
            // Calculate size based on base size and max order value
            // Size in shares, not USDC - for Polymarket, 1 share = price in USDC
            // So if price is $0.75, then 5 shares = $3.75 value
            let mut size = config.base_order_size;
            
            // Ensure total value doesn't exceed max_order_value
            let total_value = size * price;
            if total_value > config.max_order_value {
                // Reduce size to stay under max value
                size = config.max_order_value / price;
                // Round down to nearest whole share
                size = size.floor();
                // Ensure at least 1 share
                if size < Decimal::ONE {
                    size = Decimal::ONE;
                }
            }
            
            proposals.push(ProposedOrder {
                price,
                size,
                side: Side::Buy,
                discount_percent: total_discount * Decimal::ONE_HUNDRED,
            });
        }
        
        Ok(proposals)
    }
    
    /// User input handler task
    async fn user_input_handler(
        mut order_rx: mpsc::Receiver<OrderCommand>,
        name: String,
        shutdown_flag: Arc<AtomicBool>,
    ) {
        info!("[{}] User input handler started", name);
        
        while let Some(command) = order_rx.recv().await {
            match command {
                OrderCommand::ProposeOrders(proposals, orderbook, response_tx) => {
                    use std::io::Write;
                    
                    // Calculate total value and side cost
                    let total_order_value: Decimal = proposals.iter()
                        .map(|o| o.size * o.price)
                        .sum();
                    
                    // Calculate market depth for multiple thresholds
                    let thresholds = vec![
                        (Decimal::new(5, 0), "5%"),
                        (Decimal::new(20, 0), "20%"),
                        (Decimal::new(50, 0), "50%"),
                    ];
                    
                    let mut depth_analysis = Vec::new();
                    let best_ask_price = orderbook.best_ask().map(|l| l.price).unwrap_or(Decimal::ZERO);
                    
                    for (percent, label) in thresholds {
                        let (avg_price, impact, _) = Self::calculate_market_depth(
                            &orderbook,
                            Side::Buy,
                            percent,
                        );
                        depth_analysis.push((label, avg_price, impact));
                    }
                    
                    // Clear screen and display enhanced proposals
                    print!("\x1B[2J\x1B[1;1H"); // Clear screen and move to top
                    println!("\n{}", "🎯 Order Proposals".bright_cyan().bold());
                    println!("{} {}", 
                        format!("[{}]", name).bright_black(),
                        format!("@ {}", chrono::Local::now().format("%H:%M:%S")).bright_black()
                    );
                    println!("{}", "═".repeat(60).bright_black());
                    
                    // Market depth info
                    println!("\n{}", "📊 Market Depth Analysis:".bright_yellow());
                    for (label, avg_price, impact) in depth_analysis {
                        let color = if impact > Decimal::new(5, 0) {
                            "red"
                        } else if impact > Decimal::new(2, 0) {
                            "yellow"
                        } else {
                            "green"
                        };
                        
                        let impact_str = match color {
                            "red" => format!("{}{:.2}%", if impact > Decimal::ZERO { "+" } else { "" }, impact).bright_red().to_string(),
                            "yellow" => format!("{}{:.2}%", if impact > Decimal::ZERO { "+" } else { "" }, impact).bright_yellow().to_string(),
                            _ => format!("{}{:.2}%", if impact > Decimal::ZERO { "+" } else { "" }, impact).bright_green().to_string(),
                        };
                        
                        println!("   {} ${:.4} → ${:.4} ({})",
                            format!("{:>3} market order:", label).bright_black(),
                            best_ask_price,
                            avg_price,
                            impact_str
                        );
                    }
                    
                    println!("\n{}", "📋 Proposed Orders:".bright_yellow());
                    
                    for (i, order) in proposals.iter().enumerate() {
                        let total_value = order.size * order.price;
                        let side_str = match order.side {
                            Side::Buy => "BUY".bright_green().to_string(),
                            Side::Sell => "SELL".bright_red().to_string(),
                        };
                        
                        println!("   {}. {} {} {} @ ${} = ${} ({}{:.1}%)",
                            format!("{}", i + 1).bright_white().bold(),
                            side_str,
                            format!("{}", order.size).bright_white(),
                            "shares".bright_black(),
                            format!("{:.4}", order.price).bright_cyan(),
                            format!("{:.2}", total_value).bright_yellow().bold(),
                            "-".bright_black(),
                            order.discount_percent
                        );
                    }
                    
                    println!("\n   {}: ${}", 
                        "Total Cost".bright_white().bold(),
                        format!("{:.2}", total_order_value).bright_yellow().bold().underline()
                    );
                    
                    println!("\n{}", "─".repeat(60).bright_black());
                    print!("\n{} {} {} ",
                        "?".bright_green().bold().blink(),
                        "Approve these orders? (y/n)".bright_white(),
                        "[CTRL+C to exit]".bright_black()
                    );
                    print!(": ");
                    let _ = std::io::stdout().flush();
                    
                    // Read user input with proper shutdown handling
                    let (tx, mut rx) = oneshot::channel();
                    
                    // Spawn a task to read input - use std::thread to avoid blocking tokio runtime shutdown
                    std::thread::spawn(move || {
                        use std::io::{self, BufRead, Write};
                        let stdin = io::stdin();
                        let mut stdout = io::stdout();
                        
                        // Flush stdout to ensure prompt is visible
                        let _ = stdout.flush();
                        
                        // Read input
                        let mut input = String::new();
                        if let Ok(_) = stdin.lock().read_line(&mut input) {
                            let _ = tx.send(input.trim().to_lowercase() == "y");
                        }
                    });
                    
                    // Wait for input, timeout, or shutdown
                    let mut check_interval = tokio::time::interval(Duration::from_millis(100));
                    let timeout = tokio::time::sleep(Duration::from_secs(60));
                    tokio::pin!(timeout);
                    
                    let approved = loop {
                        tokio::select! {
                            // Check if we got user input
                            Ok(result) = &mut rx => {
                                break result;
                            }
                            // Check for timeout
                            _ = &mut timeout => {
                                println!("\n⏱️ Order approval timed out after 60 seconds");
                                break false;
                            }
                            // Periodically check shutdown flag
                            _ = check_interval.tick() => {
                                if shutdown_flag.load(Ordering::Relaxed) {
                                    println!("\n🛑 Strategy shutdown detected, cancelling order prompt");
                                    println!("   (Press Enter to complete shutdown if needed)");
                                    break false;
                                }
                            }
                        }
                    };
                    
                    // Send response with approved orders if accepted
                    let response = if approved {
                        Some(proposals)
                    } else {
                        None
                    };
                    let _ = response_tx.send(response);
                }
                OrderCommand::Shutdown => {
                    info!("[{}] User input handler shutting down", name);
                    break;
                }
            }
        }
    }
    
    /// Place approved orders
    pub async fn place_orders(&self, proposals: Vec<ProposedOrder>) -> Result<Vec<OrderPlacementResponse>> {
        let clob_client = self.clob_client.as_ref()
            .ok_or_else(|| anyhow::Error::msg("ClobClient not set"))?;
        
        let mut responses = Vec::new();
        
        for proposal in proposals {
            let total_value = proposal.size * proposal.price;
            info!(
                "[{}] Placing {} order: {} shares @ ${:.4} = ${:.2} total ({:.1}% discount)",
                self.name,
                match proposal.side {
                    Side::Buy => "BUY",
                    Side::Sell => "SELL",
                },
                proposal.size,
                proposal.price,
                total_value,
                proposal.discount_percent,
            );
            
            let mut client_guard = clob_client.lock().await;
            let response = match proposal.side {
                Side::Buy => {
                    self.polybot.order.place_buy_order(
                        &mut *client_guard,
                        &self.token_id,
                        proposal.price,
                        proposal.size,
                    ).await
                }
                Side::Sell => {
                    self.polybot.order.place_sell_order(
                        &mut *client_guard,
                        &self.token_id,
                        proposal.price,
                        proposal.size,
                    ).await
                }
            };
            
            match response {
                Ok(resp) => {
                    if resp.success {
                        info!("[{}] Order placed successfully: {:?}", self.name, resp.order_id);
                        
                        // Track the order ONLY if successful
                        if let Some(order_id) = &resp.order_id {
                            let mut state = self.state.write().await;
                            state.active_order_ids.push(order_id.clone());
                            state.active_order_count = state.active_order_ids.len();
                        }
                    } else {
                        warn!("[{}] Order placement failed: {:?}", self.name, resp.error_message);
                    }
                    responses.push(resp);
                }
                Err(e) => {
                    error!("[{}] Order placement error: {}", self.name, e);
                    responses.push(OrderPlacementResponse {
                        success: false,
                        order_id: None,
                        error_message: Some(e.to_string()),
                        order_details: None,
                        placement_time: chrono::Utc::now(),
                    });
                }
            }
        }
        
        Ok(responses)
    }
}

#[async_trait]
impl SingleTokenStrategy for SimpleStrategy {
    async fn orderbook_update(&self, orderbook: &OrderBook) -> Result<()> {
        let mut state = self.state.write().await;
        state.update_count += 1;
        state.total_events_processed += 1;
        
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
        
        // Update spread tracking and store orderbook
        if let (Some(best_bid), Some(best_ask)) = (orderbook.best_bid(), orderbook.best_ask()) {
            state.last_spread = Some(best_ask.price - best_bid.price);
        }
        
        // Store orderbook snapshot for order decisions
        state.last_orderbook = Some(orderbook.clone());
        
        // Analyze market conditions
        self.analyze_market(orderbook, &mut state).await;
        
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
        state.total_events_processed += 1;
        
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
    
    fn set_clob_client(&mut self, client: Arc<tokio::sync::Mutex<ClobClient>>) {
        self.clob_client = Some(client);
    }
    
    async fn process_pending_orders(&self) -> Result<()> {
        self.process_pending_orders().await
    }
    
    async fn shutdown(&self) -> Result<()> {
        self.shutdown().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    
    #[test]
    fn test_round_to_tick_size() {
        // Test with tick size 0.01
        let tick_size = dec!(0.01);
        
        // Test rounding up
        assert_eq!(SimpleStrategy::round_to_tick_size(dec!(0.7562), tick_size), dec!(0.76));
        assert_eq!(SimpleStrategy::round_to_tick_size(dec!(0.755), tick_size), dec!(0.76));
        
        // Test rounding down
        assert_eq!(SimpleStrategy::round_to_tick_size(dec!(0.7524), tick_size), dec!(0.75));
        assert_eq!(SimpleStrategy::round_to_tick_size(dec!(0.754), tick_size), dec!(0.75));
        
        // Test exact tick
        assert_eq!(SimpleStrategy::round_to_tick_size(dec!(0.75), tick_size), dec!(0.75));
        
        // Test with different tick sizes
        assert_eq!(SimpleStrategy::round_to_tick_size(dec!(0.1234), dec!(0.001)), dec!(0.123));
        assert_eq!(SimpleStrategy::round_to_tick_size(dec!(0.1235), dec!(0.001)), dec!(0.124));
        
        // Test with zero tick size (should return original price)
        assert_eq!(SimpleStrategy::round_to_tick_size(dec!(0.123456), dec!(0)), dec!(0.123456));
    }
}