use crate::data_paths::DataPaths;
use crate::execution::orderbook::PriceLevel;
use crate::execution::orders::{EnhancedOrder, OrderManager};
use crate::portfolio::manager::PortfolioManager;
use crate::services::Streamer;
use crate::tui::navigation::Navigation;
use crate::tui::pages::{MarketsPage, OrdersPage, PortfolioPage, StreamPage, TokensPage};
use crate::ws::{PolyEvent, Side};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

#[derive(Debug, Clone)]
pub struct TokenActivity {
    pub token_id: String,
    pub event_count: usize,
    pub last_bid: Option<Decimal>,
    pub last_ask: Option<Decimal>,
    pub last_update: Option<Instant>,
    pub total_volume: Decimal,
    pub trade_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Overview,
    OrderBook { token_id: String },
}

pub struct App {
    pub state: AppState,
    pub should_quit: bool,
    pub event_log: Vec<String>,
    pub token_activities: Arc<RwLock<HashMap<String, TokenActivity>>>,
    pub selected_token_index: usize,
    pub streamer: Arc<Streamer>,

    // Navigation and pages
    pub navigation: Navigation,
    pub stream_page: StreamPage,
    pub orders_page: OrdersPage,
    pub tokens_page: TokensPage,
    pub markets_page: MarketsPage,
    pub portfolio_page: PortfolioPage,

    // Portfolio and order management
    pub portfolio_manager: Arc<PortfolioManager>,
    pub order_manager: OrderManager,

    // Data fetching state
    pub data_paths: Option<DataPaths>,
    pub host: Option<String>,
    pub is_fetching_orders: bool,
    pub last_orders_fetch: Option<Instant>,
    pub orders_cache: Arc<RwLock<Vec<EnhancedOrder>>>,
    pub refresh_orders_requested: bool,
    pub is_fetching_orders_flag: Arc<AtomicBool>,

    // Order book state for selected token
    pub current_bids: Vec<PriceLevel>,
    pub current_asks: Vec<PriceLevel>,
    pub current_token_id: Option<String>,

    // Order book scroll state
    pub orderbook_scroll: usize,

    // Global metrics
    pub total_events_received: usize,
    pub start_time: Instant,
}

impl App {
    pub fn new(streamer: Arc<Streamer>) -> Self {
        Self {
            state: AppState::Overview,
            should_quit: false,
            event_log: Vec::new(),
            token_activities: Arc::new(RwLock::new(HashMap::new())),
            selected_token_index: 0,
            streamer,

            // Initialize navigation and pages
            navigation: Navigation::new(),
            stream_page: StreamPage::new(),
            orders_page: OrdersPage::new(),
            tokens_page: TokensPage::new(),
            markets_page: MarketsPage::new(),
            portfolio_page: PortfolioPage::new(),

            // Initialize portfolio manager
            portfolio_manager: Arc::new(PortfolioManager::new()),
            order_manager: OrderManager::new(),

            // Initialize data fetching state
            data_paths: None,
            host: None,
            is_fetching_orders: false,
            last_orders_fetch: None,
            orders_cache: Arc::new(RwLock::new(Vec::new())),
            refresh_orders_requested: false,
            is_fetching_orders_flag: Arc::new(AtomicBool::new(false)),

            current_bids: Vec::new(),
            current_asks: Vec::new(),
            current_token_id: None,
            orderbook_scroll: 0,
            total_events_received: 0,
            start_time: Instant::now(),
        }
    }

    pub fn handle_event(&mut self, event: PolyEvent) {
        // Increment total events counter
        self.total_events_received += 1;

        // Add to event log with error handling
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| format_event(&event))) {
            Ok(log_entry) => {
                self.event_log.push(log_entry);
            }
            Err(_) => {
                self.event_log.push("⚠️ Failed to format event".to_string());
            }
        }

        // Keep only last 100 events
        if self.event_log.len() > 100 {
            self.event_log.remove(0);
        }

        // Update token activity
        match &event {
            PolyEvent::PriceChange {
                asset_id,
                side,
                price,
                size,
                ..
            } => {
                // Update order book if we're viewing this token
                if let AppState::OrderBook { token_id } = &self.state {
                    if token_id == asset_id {
                        // Get updated orderbook from streamer
                        if let Some(order_book) = self.streamer.get_order_book(asset_id) {
                            self.current_bids = order_book.get_bids().to_vec();
                            self.current_asks = order_book.get_asks().to_vec();
                        }
                    }
                }

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
                            });

                    activity.event_count += 1;
                    activity.last_update = Some(Instant::now());

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
                // Update order book if we're viewing this token
                if let AppState::OrderBook { token_id } = &self.state {
                    if token_id == asset_id {
                        self.current_bids = bids.clone();
                        self.current_asks = asks.clone();
                    }
                }

                // Update activity
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
                            });

                    activity.event_count += 1;
                    activity.last_update = Some(Instant::now());
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
                            });

                    activity.event_count += 1;
                    activity.last_update = Some(Instant::now());
                    activity.trade_count += 1;
                    activity.total_volume += price * size;
                }
            }
            _ => {}
        }
    }

    pub fn get_all_active_tokens(&self) -> Vec<TokenActivity> {
        // Use try_read to avoid blocking in UI context
        if let Ok(activities) = self.token_activities.try_read() {
            let mut sorted: Vec<_> = activities.values().cloned().collect();
            sorted.sort_by(|a, b| b.event_count.cmp(&a.event_count));
            sorted
        } else {
            // Return empty vec if we can't read (lock contention)
            Vec::new()
        }
    }

    pub fn select_next(&mut self) {
        let max_tokens = self.get_all_active_tokens().len();
        if max_tokens > 0 {
            self.selected_token_index = (self.selected_token_index + 1).min(max_tokens - 1);
        }
    }

    pub fn select_previous(&mut self) {
        self.selected_token_index = self.selected_token_index.saturating_sub(1);
    }

    pub fn select_token(&mut self) {
        let all_tokens = self.get_all_active_tokens();
        if let Some(activity) = all_tokens.get(self.selected_token_index) {
            self.current_token_id = Some(activity.token_id.clone());
            self.state = AppState::OrderBook {
                token_id: activity.token_id.clone(),
            };

            // Get current order book from streamer
            if let Some(order_book) = self.streamer.get_order_book(&activity.token_id) {
                self.current_bids = order_book.get_bids().to_vec();
                self.current_asks = order_book.get_asks().to_vec();
            }
        }
    }

    pub fn go_back(&mut self) {
        self.state = AppState::Overview;
        self.current_bids.clear();
        self.current_asks.clear();
        self.current_token_id = None;
        self.orderbook_scroll = 0;
    }

    pub fn scroll_orderbook_up(&mut self) {
        self.orderbook_scroll = self.orderbook_scroll.saturating_sub(1);
    }

    pub fn scroll_orderbook_down(&mut self) {
        self.orderbook_scroll = self.orderbook_scroll.saturating_add(1);
    }

    pub fn reset_orderbook_scroll(&mut self) {
        // Instead of resetting to 0, calculate the scroll position that centers the mid-point
        // This will be handled by the orderbook rendering logic which already centers on mid
        // We use a special value to signal that we want to center on mid
        self.orderbook_scroll = usize::MAX; // Special value to indicate "center on mid"
    }

    pub fn elapsed_time(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }

    pub fn get_token_event_count(&self, token_id: &str) -> usize {
        if let Ok(activities) = self.token_activities.try_read() {
            activities
                .get(token_id)
                .map(|activity| activity.event_count)
                .unwrap_or(0)
        } else {
            0
        }
    }

    /// Configure the app with data paths and host for API calls
    pub fn configure_data_access(&mut self, data_paths: DataPaths, host: String) {
        self.data_paths = Some(data_paths);
        self.host = Some(host);
    }

    /// Check if we should fetch fresh orders data
    pub fn should_fetch_orders(&self) -> bool {
        if self.is_fetching_orders {
            return false; // Already fetching
        }

        match self.last_orders_fetch {
            None => true,                                            // Never fetched
            Some(last_fetch) => last_fetch.elapsed().as_secs() > 30, // Fetch every 30 seconds
        }
    }

    /// Mark that we're starting to fetch orders
    pub fn start_fetching_orders(&mut self) {
        self.is_fetching_orders = true;
        self.is_fetching_orders_flag.store(true, Ordering::Relaxed);
    }

    /// Request a refresh of orders data
    pub fn request_orders_refresh(&mut self) {
        self.refresh_orders_requested = true;
    }

    /// Update fetching status from the atomic flag (call this in the main loop)
    pub fn update_fetching_status(&mut self) {
        if !self.is_fetching_orders_flag.load(Ordering::Relaxed) && self.is_fetching_orders {
            self.is_fetching_orders = false;
            self.last_orders_fetch = Some(Instant::now());
        }
    }

    /// Handle order refresh requests by spawning fetch task
    pub fn handle_orders_refresh_request(&mut self) {
        if self.refresh_orders_requested && !self.is_fetching_orders {
            self.refresh_orders_requested = false;

            // Clone data before mutable borrow
            let data_paths_opt = self.data_paths.clone();
            let host_opt = self.host.clone();

            if let (Some(data_paths), Some(host)) = (data_paths_opt, host_opt) {
                info!("Processing order refresh request");
                self.start_fetching_orders();

                // Spawn async task to fetch orders
                let order_manager = self.order_manager.clone();
                let orders_cache = self.orders_cache.clone();
                let portfolio_manager = self.portfolio_manager.clone();
                let is_fetching_flag = self.is_fetching_orders_flag.clone();

                tokio::spawn(async move {
                    match Self::fetch_orders_task(
                        &order_manager,
                        &host,
                        &data_paths,
                        &orders_cache,
                        &portfolio_manager,
                    )
                    .await
                    {
                        Ok(orders_count) => {
                            info!("Successfully fetched and cached {} orders", orders_count);
                        }
                        Err(e) => {
                            error!("Failed to fetch orders: {}", e);
                        }
                    }

                    // Mark fetching as complete
                    is_fetching_flag.store(false, Ordering::Relaxed);
                });
            } else {
                warn!("Cannot fetch orders: data paths or host not configured");
                self.refresh_orders_requested = false;
            }
        }
    }

    /// Async task to fetch orders and update cache
    async fn fetch_orders_task(
        order_manager: &OrderManager,
        host: &str,
        data_paths: &crate::data_paths::DataPaths,
        orders_cache: &Arc<RwLock<Vec<EnhancedOrder>>>,
        portfolio_manager: &Arc<PortfolioManager>,
    ) -> Result<usize, anyhow::Error> {
        use crate::config;
        use crate::ethereum_utils;

        // Load private key to derive user address
        let private_key = config::load_private_key(data_paths).await.map_err(|e| {
            anyhow::anyhow!("No private key found. Run 'cargo run -- init' first: {}", e)
        })?;

        // Derive user's Ethereum address
        let user_address = ethereum_utils::derive_address_from_private_key(&private_key)?;

        info!("Fetching orders for user: {}", user_address);

        // Fetch orders using the order manager
        let fetched_orders = order_manager
            .fetch_orders(host, data_paths, &user_address)
            .await?;

        // Update the orders cache
        {
            let mut cache = orders_cache.write().await;
            *cache = fetched_orders.clone();
        }

        info!("Updated orders cache with {} orders", fetched_orders.len());

        // Convert to ActiveOrder for portfolio manager compatibility
        let active_orders: Result<Vec<crate::portfolio::types::ActiveOrder>, _> = fetched_orders
            .iter()
            .map(|enhanced_order| Self::convert_enhanced_to_active_order(enhanced_order))
            .collect();

        match active_orders {
            Ok(orders) => {
                // Update portfolio manager with fetched orders
                let mut active_orders_map = portfolio_manager.active_orders().write().await;
                active_orders_map.clear();
                for order in orders {
                    active_orders_map.insert(order.order_id.clone(), order);
                }
                info!(
                    "Updated portfolio manager with {} active orders",
                    active_orders_map.len()
                );
            }
            Err(e) => {
                warn!("Failed to convert orders for portfolio manager: {}", e);
            }
        }

        Ok(fetched_orders.len())
    }

    /// Convert EnhancedOrder to ActiveOrder for portfolio manager compatibility
    fn convert_enhanced_to_active_order(
        enhanced: &EnhancedOrder,
    ) -> Result<crate::portfolio::types::ActiveOrder, anyhow::Error> {
        use crate::portfolio::types::{
            ActiveOrder, OrderSide, OrderStatus, OrderType, TimeInForce,
        };
        use chrono::Utc;
        use rust_decimal::prelude::FromPrimitive;
        use rust_decimal::Decimal;

        // Convert side
        let side = match enhanced.side {
            crate::execution::orders::OrderSide::Buy => OrderSide::Buy,
            crate::execution::orders::OrderSide::Sell => OrderSide::Sell,
        };

        // Convert status
        let status = match enhanced.status {
            crate::execution::orders::OrderStatus::Open => OrderStatus::Open,
            crate::execution::orders::OrderStatus::Filled => OrderStatus::Filled,
            crate::execution::orders::OrderStatus::Cancelled => OrderStatus::Cancelled,
            crate::execution::orders::OrderStatus::PartiallyFilled => OrderStatus::PartiallyFilled,
            crate::execution::orders::OrderStatus::Rejected => OrderStatus::Rejected,
            crate::execution::orders::OrderStatus::Pending => OrderStatus::Pending,
        };

        // Extract outcome from additional fields or market info
        let outcome = enhanced
            .market_info
            .as_ref()
            .and_then(|info| info.token_outcome.clone())
            .or_else(|| {
                enhanced
                    .additional_fields
                    .get("outcome")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "YES".to_string());

        // Extract market_id from additional fields or use asset_id as fallback
        let market_id = enhanced
            .additional_fields
            .get("market")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| enhanced.asset_id.clone());

        Ok(ActiveOrder {
            order_id: enhanced.id.clone(),
            market_id,
            token_id: enhanced.asset_id.clone(),
            outcome,
            side,
            order_type: OrderType::Limit, // Default to limit, API doesn't always provide this
            price: Decimal::from_f64(enhanced.price).unwrap_or_default(),
            size: Decimal::from_f64(enhanced.original_size).unwrap_or_default(),
            filled_size: Decimal::from_f64(enhanced.filled_size).unwrap_or_default(),
            remaining_size: Decimal::from_f64(enhanced.remaining_size).unwrap_or_default(),
            status,
            created_at: enhanced.created_at,
            updated_at: enhanced.updated_at.unwrap_or_else(|| Utc::now()),
            time_in_force: TimeInForce::GTC, // Default to Good Till Cancelled
            post_only: false,                // Default, not provided by API
            reduce_only: false,              // Default, not provided by API
        })
    }

}

fn format_event(event: &PolyEvent) -> String {
    match event {
        PolyEvent::PriceChange {
            asset_id,
            side,
            price,
            size,
            ..
        } => {
            let action = if *size == Decimal::ZERO {
                "REMOVE"
            } else {
                "UPDATE"
            };
            let side_str = match side {
                Side::Buy => "BID",
                Side::Sell => "ASK",
            };
            format!(
                "{} {} {} @ ${} ({})",
                &asset_id[..16],
                action,
                side_str,
                price,
                size
            )
        }
        PolyEvent::Book {
            asset_id,
            bids,
            asks,
            ..
        } => {
            format!(
                "{} BOOK {} bids, {} asks",
                &asset_id[..16],
                bids.len(),
                asks.len()
            )
        }
        PolyEvent::Trade {
            asset_id,
            side,
            price,
            size,
            ..
        } => {
            let side_str = match side {
                Side::Buy => "BUY",
                Side::Sell => "SELL",
            };
            format!(
                "{} TRADE {} {} @ ${}",
                &asset_id[..16],
                side_str,
                size,
                price
            )
        }
        _ => format!("Other event"),
    }
}
