use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use rust_decimal::Decimal;
use std::time::Instant;
use crate::ws::{PolyEvent, Side};
use crate::services::Streamer;
use crate::types::PriceLevel;

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
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            format_event(&event)
        })) {
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
            PolyEvent::PriceChange { asset_id, side, price, size, .. } => {
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
                    let activity = activities.entry(asset_id.clone()).or_insert_with(|| {
                        TokenActivity {
                            token_id: asset_id.clone(),
                            event_count: 0,
                            last_bid: None,
                            last_ask: None,
                                    last_update: None,
                            total_volume: Decimal::ZERO,
                            trade_count: 0,
                        }
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
            PolyEvent::Book { asset_id, bids, asks, .. } => {
                // Update order book if we're viewing this token
                if let AppState::OrderBook { token_id } = &self.state {
                    if token_id == asset_id {
                        self.current_bids = bids.clone();
                        self.current_asks = asks.clone();
                    }
                }
                
                // Update activity
                if let Ok(mut activities) = self.token_activities.try_write() {
                    let activity = activities.entry(asset_id.clone()).or_insert_with(|| {
                        TokenActivity {
                            token_id: asset_id.clone(),
                            event_count: 0,
                            last_bid: None,
                            last_ask: None,
                                    last_update: None,
                            total_volume: Decimal::ZERO,
                            trade_count: 0,
                        }
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
            PolyEvent::Trade { asset_id, price, size, .. } => {
                if let Ok(mut activities) = self.token_activities.try_write() {
                    let activity = activities.entry(asset_id.clone()).or_insert_with(|| {
                        TokenActivity {
                            token_id: asset_id.clone(),
                            event_count: 0,
                            last_bid: None,
                            last_ask: None,
                                    last_update: None,
                            total_volume: Decimal::ZERO,
                            trade_count: 0,
                        }
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
                token_id: activity.token_id.clone() 
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
            activities.get(token_id).map(|activity| activity.event_count).unwrap_or(0)
        } else {
            0
        }
    }
}

fn format_event(event: &PolyEvent) -> String {
    match event {
        PolyEvent::PriceChange { asset_id, side, price, size, .. } => {
            let action = if *size == Decimal::ZERO { "REMOVE" } else { "UPDATE" };
            let side_str = match side {
                Side::Buy => "BID",
                Side::Sell => "ASK",
            };
            format!("{} {} {} @ ${} ({})", 
                &asset_id[..16], action, side_str, price, size)
        }
        PolyEvent::Book { asset_id, bids, asks, .. } => {
            format!("{} BOOK {} bids, {} asks", 
                &asset_id[..16], bids.len(), asks.len())
        }
        PolyEvent::Trade { asset_id, side, price, size, .. } => {
            let side_str = match side {
                Side::Buy => "BUY",
                Side::Sell => "SELL",
            };
            format!("{} TRADE {} {} @ ${}", 
                &asset_id[..16], side_str, size, price)
        }
        _ => format!("Other event")
    }
}