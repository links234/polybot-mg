use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Row, Table, Wrap},
    Frame,
};
use rust_decimal::Decimal;
use crate::types::{PriceLevel, SpreadInfo};
use crate::tui::App;

#[derive(Debug, Clone)]
struct OrderBookLevel {
    price: Decimal,
    bid_size: Option<Decimal>,
    ask_size: Option<Decimal>,
    is_mid: bool,
    cumulative_bid_total: Option<Decimal>, // Total USD to clear all bids from best to this level
    cumulative_ask_total: Option<Decimal>, // Total USD to clear all asks from best to this level
}

pub fn draw_order_book(frame: &mut Frame<'_>, app: &mut App, token_id: &str) {
    // Determine if we have enough space for side panel (minimum 120 chars wide)
    let has_space_for_details = frame.area().width >= 120;
    
    let main_chunks = if has_space_for_details {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(70), // Main orderbook area
                Constraint::Percentage(30), // Token details sidebar
            ])
            .split(frame.area())
    } else {
        std::rc::Rc::new([frame.area()]) // Use full area if not enough space
    };
    
    // Layout for the main orderbook area
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),      // Title
            Constraint::Length(4),      // Token info & stats (compact)
            Constraint::Min(8),         // Order book combined view (compact)
            Constraint::Length(3),      // Help
        ])
        .split(main_chunks[0]);
    
    // Title
    let title = Paragraph::new("Order Book View")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, chunks[0]);
    
    // Prepare order book data
    let levels = prepare_order_book_levels(app);
    
    // Create multi-line token info display
    let spread_info = calculate_spread_info(&app.current_bids, &app.current_asks);
    let mid_price = spread_info.mid_price;
    let elapsed = app.elapsed_time();
    let elapsed_str = if elapsed.as_secs() >= 3600 {
        format!("{}h{}m{}s", elapsed.as_secs() / 3600, (elapsed.as_secs() % 3600) / 60, elapsed.as_secs() % 60)
    } else if elapsed.as_secs() >= 60 {
        format!("{}m{}s", elapsed.as_secs() / 60, elapsed.as_secs() % 60)
    } else {
        format!("{}s", elapsed.as_secs())
    };
    
    let token_events = app.get_token_event_count(token_id);
    
    // Use complete token ID
    let token_info = vec![
        format!("🏷️  Token: {}", token_id),
        format!("📊 {} bids, {} asks | 📈 {} events | ⏱️ {} | 💰 {}", 
               app.current_bids.len(), 
               app.current_asks.len(), 
               token_events, 
               elapsed_str,
               spread_info.description),
    ].join("\n");
    
    let info = Paragraph::new(token_info)
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title("📋 Market Information"));
    frame.render_widget(info, chunks[1]);
    
    // Combined order book view
    draw_combined_order_book(frame, chunks[2], &levels, app, mid_price);
    
    // Help
    let help = Paragraph::new("↑/↓: Scroll | M: Reset to mid | Total: Cumulative USD to clear to level | Esc: Back | q: Quit")
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(help, chunks[3]);
    
    // Draw token details sidebar if there's enough space
    if has_space_for_details {
        draw_token_details(frame, main_chunks[1], token_id);
    }
}

fn prepare_order_book_levels(app: &App) -> Vec<OrderBookLevel> {
    let mut levels = Vec::new();
    
    // Find best bid and ask
    let best_bid = app.current_bids.iter().map(|level| level.price).max();
    let best_ask = app.current_asks.iter().map(|level| level.price).min();
    
    // Add all ask levels (above mid) - sorted high to low
    let mut asks: Vec<_> = app.current_asks.iter().cloned().collect();
    asks.sort_by(|a, b| b.price.cmp(&a.price)); // High to low
    
    for level in asks {
        levels.push(OrderBookLevel {
            price: level.price,
            bid_size: None,
            ask_size: Some(level.size),
            is_mid: false,
            cumulative_bid_total: None,
            cumulative_ask_total: None,
        });
    }
    
    // Insert mid-point between best bid and ask (only if valid spread)
    if let (Some(bid), Some(ask)) = (best_bid, best_ask) {
        if bid < ask {
            let mid_price = (bid + ask) / Decimal::from(2);
            levels.push(OrderBookLevel {
                price: mid_price,
                bid_size: None,
                ask_size: None,
                is_mid: true,
                cumulative_bid_total: None,
                cumulative_ask_total: None,
            });
        } else {
            // If bid >= ask, we have a crossed market - show clear error
            levels.push(OrderBookLevel {
                price: Decimal::ZERO, // Will be displayed as error message
                bid_size: None,
                ask_size: None,
                is_mid: true, // Use mid styling but show error
                cumulative_bid_total: None,
                cumulative_ask_total: None,
            });
        }
    }
    
    // Add all bid levels (below mid) - sorted high to low
    let mut bids: Vec<_> = app.current_bids.iter().cloned().collect();
    bids.sort_by(|a, b| b.price.cmp(&a.price)); // High to low
    
    for level in bids {
        levels.push(OrderBookLevel {
            price: level.price,
            bid_size: Some(level.size),
            ask_size: None,
            is_mid: false,
            cumulative_bid_total: None,
            cumulative_ask_total: None,
        });
    }
    
    // Sort all levels by price (highest to lowest)
    levels.sort_by(|a, b| b.price.cmp(&a.price));
    
    // Calculate cumulative totals
    calculate_cumulative_totals(&mut levels);
    
    levels
}

fn calculate_cumulative_totals(levels: &mut [OrderBookLevel]) {
    // Calculate cumulative ask totals (from lowest ask price upward)
    let mut ask_running_total = Decimal::ZERO;
    
    // Process asks from highest to lowest price (reverse order for asks)
    for level in levels.iter_mut().rev() {
        if let Some(ask_size) = level.ask_size {
            ask_running_total += ask_size * level.price;
            level.cumulative_ask_total = Some(ask_running_total);
        }
    }
    
    // Calculate cumulative bid totals (from highest bid price downward)
    let mut bid_running_total = Decimal::ZERO;
    
    // Process bids from highest to lowest price (forward order for bids)
    for level in levels.iter_mut() {
        if let Some(bid_size) = level.bid_size {
            bid_running_total += bid_size * level.price;
            level.cumulative_bid_total = Some(bid_running_total);
        }
    }
}

fn calculate_spread_info(bids: &[PriceLevel], asks: &[PriceLevel]) -> SpreadInfo {
    let best_bid = bids.iter().map(|level| level.price).max();
    let best_ask = asks.iter().map(|level| level.price).min();
    
    match (best_bid, best_ask) {
        (Some(bid), Some(ask)) => {
            let spread = ask - bid;
            if spread <= Decimal::ZERO {
                // Invalid spread - crossed market
                SpreadInfo::crossed_market(bid, ask)
            } else {
                let mid = (bid + ask) / Decimal::from(2);
                let spread_pct = if mid > Decimal::ZERO {
                    (spread / mid) * Decimal::from(100)
                } else {
                    Decimal::ZERO
                };
                SpreadInfo::normal_market(bid, ask, spread_pct.to_string().parse().unwrap_or(0.0))
            }
        }
        _ => SpreadInfo::new("No spread available".to_string(), None)
    }
}

fn draw_combined_order_book(
    frame: &mut Frame<'_>, 
    area: ratatui::layout::Rect, 
    levels: &[OrderBookLevel], 
    app: &mut App,
    _mid_price: Option<Decimal>
) {
    if levels.is_empty() {
        let empty = Paragraph::new("No order book data available")
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title("Order Book"));
        frame.render_widget(empty, area);
        return;
    }
    
    // Calculate available rows for data (subtract header + borders)
    let available_rows = area.height.saturating_sub(3) as usize;
    
    // Find mid position in levels
    let mid_index = levels.iter().position(|l| l.is_mid).unwrap_or(levels.len() / 2);
    
    // Check if we need to center on mid (special value)
    if app.orderbook_scroll == usize::MAX {
        // Reset to actual centered position
        let half_view = available_rows / 2;
        app.orderbook_scroll = mid_index.saturating_sub(half_view);
    }
    
    // Calculate scroll bounds to keep mid visible
    let max_scroll = if levels.len() > available_rows {
        levels.len() - available_rows
    } else {
        0 // All levels fit, no scrolling needed
    };
    
    // Clamp scroll to valid range
    let scroll_pos = std::cmp::min(app.orderbook_scroll, max_scroll);
    
    // Determine start index
    let start_idx = if levels.len() <= available_rows {
        0
    } else {
        scroll_pos
    };
    
    let end_idx = std::cmp::min(start_idx + available_rows, levels.len());
    let visible_levels = &levels[start_idx..end_idx];
    
    let rows: Vec<Row> = visible_levels
        .iter()
        .map(|level| {
            if level.is_mid {
                if level.price == Decimal::ZERO {
                    // Crossed market error
                    Row::new(vec![
                        "⚠️ CROSSED MARKET ERROR ⚠️".to_string(),
                        "BID >= ASK".to_string(),
                        "INVALID SPREAD".to_string(),
                    ]).style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD | Modifier::SLOW_BLINK))
                } else {
                    // Normal mid-point row
                    Row::new(vec![
                        format!("--- MID ${:.4} ---", level.price),
                        "".to_string(),
                        "".to_string(),
                    ]).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
                }
            } else {
                // Regular order level
                let (size, total, style) = if let Some(bid_size) = level.bid_size {
                    // Bid level (green)
                    let cumulative_total = level.cumulative_bid_total.unwrap_or(Decimal::ZERO);
                    (
                        format!("{:.2}", bid_size),
                        format!("${:.2}", cumulative_total),
                        Style::default().fg(Color::Green)
                    )
                } else if let Some(ask_size) = level.ask_size {
                    // Ask level (red)
                    let cumulative_total = level.cumulative_ask_total.unwrap_or(Decimal::ZERO);
                    (
                        format!("{:.2}", ask_size),
                        format!("${:.2}", cumulative_total),
                        Style::default().fg(Color::Red)
                    )
                } else {
                    // Empty level (shouldn't happen with new logic)
                    ("".to_string(), "".to_string(), Style::default().fg(Color::Gray))
                };
                
                let price_str = format!("${:.4}", level.price);
                
                Row::new(vec![
                    price_str,
                    size,
                    total,
                ]).style(style)
            }
        })
        .collect();
    
    let scroll_indicator = if levels.len() > available_rows {
        format!(" ({}‑{} of {})", start_idx + 1, end_idx, levels.len())
    } else {
        String::new()
    };
    
    let table = Table::new(
        rows,
        &[
            Constraint::Percentage(35), // Price (first)
            Constraint::Percentage(25), // Size
            Constraint::Percentage(40), // Total USD
        ],
    )
    .header(Row::new(vec!["Price Level", "Size", "Total USD"])
        .style(Style::default().fg(Color::Yellow)))
    .block(Block::default()
        .borders(Borders::ALL)
        .title(format!("Order Book{}", scroll_indicator)));
    
    frame.render_widget(table, area);
}

fn draw_token_details(frame: &mut Frame<'_>, area: ratatui::layout::Rect, token_id: &str) {
    // Try to load token details from markets data
    let details = load_token_details(token_id);
    
    let content = if let Some(details) = details {
        vec![
            "📋 Token Details".to_string(),
            "".to_string(),
            format!("🏷️  ID: {}", token_id),
            "".to_string(),
            format!("📝  Question: {}", details.question.unwrap_or_else(|| "N/A".to_string())),
            "".to_string(),
            format!("🏪  Market: {}", details.market_slug.unwrap_or_else(|| "N/A".to_string())),
            "".to_string(),
            format!("🔗  Condition: {}", details.condition_id.unwrap_or_else(|| "N/A".to_string())),
            "".to_string(),
            format!("⏰  Created: {}", details.start_date.unwrap_or_else(|| "N/A".to_string())),
            "".to_string(),
            format!("🏁  Ends: {}", details.end_date.unwrap_or_else(|| "N/A".to_string())),
            "".to_string(),
            format!("💰  Min Bet: ${}", details.minimum_order_size.unwrap_or_else(|| "N/A".to_string())),
            "".to_string(),
            format!("📊  Active: {}", if details.active.unwrap_or(false) { "Yes" } else { "No" }),
            "".to_string(),
            "Outcomes:".to_string(),
        ]
    } else {
        vec![
            "📋 Token Details".to_string(),
            "".to_string(),
            format!("🏷️  ID: {}", token_id),
            "".to_string(),
            "⚠️  No market data available".to_string(),
            "".to_string(),
            "This could mean:".to_string(),
            "• Token not in markets.json".to_string(),
            "• Market data not loaded".to_string(),
            "• Token ID not recognized".to_string(),
        ]
    };
    
    let details_text = content.join("\n");
    
    let details_widget = Paragraph::new(details_text)
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true })
        .block(Block::default()
            .borders(Borders::ALL)
            .title("📋 Token Details")
            .style(Style::default().fg(Color::Cyan)));
    
    frame.render_widget(details_widget, area);
}

#[derive(Debug)]
struct TokenDetails {
    question: Option<String>,
    market_slug: Option<String>,
    condition_id: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    minimum_order_size: Option<String>,
    active: Option<bool>,
}

fn load_token_details(token_id: &str) -> Option<TokenDetails> {
    // Try to load from markets.json in the data directory
    // This is a simplified implementation - in a real scenario you'd want to
    // pass the markets data through the app state or load it dynamically
    
    use std::fs;
    use serde_json::Value;
    
    // Try to find markets.json in common locations
    let possible_paths = [
        "./data/datasets/bitcoin_price_bets/2025-06-15/2025-06-15_19-48-01/markets.json",
        "./markets.json",
        "./data/markets.json",
    ];
    
    for path in &possible_paths {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(markets) = serde_json::from_str::<Value>(&content) {
                if let Some(markets_array) = markets.as_array() {
                    for market in markets_array {
                        if let Some(tokens) = market.get("tokens") {
                            if let Some(tokens_array) = tokens.as_array() {
                                for token in tokens_array {
                                    if let Some(id) = token.get("token_id") {
                                        if id.as_str() == Some(token_id) {
                                            return Some(TokenDetails {
                                                question: market.get("question").and_then(|v| v.as_str()).map(String::from),
                                                market_slug: market.get("market_slug").and_then(|v| v.as_str()).map(String::from),
                                                condition_id: market.get("condition_id").and_then(|v| v.as_str()).map(String::from),
                                                start_date: market.get("start_date").and_then(|v| v.as_str()).map(String::from),
                                                end_date: market.get("end_date").and_then(|v| v.as_str()).map(String::from),
                                                minimum_order_size: market.get("minimum_order_size").and_then(|v| v.as_str()).map(String::from),
                                                active: market.get("active").and_then(|v| v.as_bool()),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    None
}