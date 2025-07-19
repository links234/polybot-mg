use crate::core::types::market::PriceLevel;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Row, Table},
    Frame,
};
use rust_decimal::Decimal;

#[derive(Debug, Clone)]
struct OrderBookLevel {
    price: Decimal,
    bid_size: Option<Decimal>,
    ask_size: Option<Decimal>,
    is_mid: bool,
    cumulative_bid_total: Option<Decimal>, // Total USD to clear all bids from best to this level
    cumulative_ask_total: Option<Decimal>, // Total USD to clear all asks from best to this level
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

pub fn render_order_book(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    bids: &[PriceLevel],
    asks: &[PriceLevel],
    token_id: &str,
    scroll: usize,
) {
    // Create temporary app-like structure for compatibility
    let mut temp_bids = bids.to_vec();
    let mut temp_asks = asks.to_vec();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Min(0),    // Order book
        ])
        .split(area);

    // Title
    // Show full token ID for easy copying
    let title = Paragraph::new(format!("Order Book | Token ID: {}", token_id))
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, chunks[0]);

    // Create levels for the order book display
    let mut levels = Vec::new();

    // Add all ask levels (above mid) - sorted low to high
    temp_asks.sort_by(|a, b| a.price.cmp(&b.price)); // Low to high

    for level in &temp_asks {
        levels.push(OrderBookLevel {
            price: level.price,
            bid_size: None,
            ask_size: Some(level.size),
            is_mid: false,
            cumulative_bid_total: None,
            cumulative_ask_total: None,
        });
    }

    // Calculate and add mid point
    if let (Some(best_bid), Some(best_ask)) = (
        temp_bids.iter().max_by_key(|level| level.price),
        temp_asks.iter().min_by_key(|level| level.price),
    ) {
        if best_bid.price < best_ask.price {
            let mid_price = (best_bid.price + best_ask.price) / Decimal::from(2);
            levels.push(OrderBookLevel {
                price: mid_price,
                bid_size: None,
                ask_size: None,
                is_mid: true,
                cumulative_bid_total: None,
                cumulative_ask_total: None,
            });
        } else {
            // Crossed market - show error
            levels.push(OrderBookLevel {
                price: Decimal::ZERO,
                bid_size: None,
                ask_size: None,
                is_mid: true,
                cumulative_bid_total: None,
                cumulative_ask_total: None,
            });
        }
    }

    // Add all bid levels (below mid) - sorted high to low
    temp_bids.sort_by(|a, b| b.price.cmp(&a.price)); // High to low

    for level in &temp_bids {
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

    // Apply scrolling and render
    let available_height = chunks[1].height.saturating_sub(3) as usize; // Account for borders and header
    
    // Calculate actual scroll position with bounds checking
    let actual_scroll = if levels.len() > available_height {
        // Clamp scroll to valid range
        scroll.min(levels.len().saturating_sub(available_height))
    } else {
        // If all content fits, no scrolling needed
        0
    };
    
    let start_idx = actual_scroll;
    let end_idx = (start_idx + available_height).min(levels.len());
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
                    ])
                    .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                } else {
                    // Normal mid-point row
                    Row::new(vec![
                        format!("--- MID ${:.4} ---", level.price),
                        "".to_string(),
                        "".to_string(),
                    ])
                    .style(
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )
                }
            } else {
                // Regular order level
                let (size, total, style) = if let Some(bid_size) = level.bid_size {
                    // Bid level (green)
                    let cumulative_total = level.cumulative_bid_total.unwrap_or(Decimal::ZERO);
                    (
                        format!("{:.2}", bid_size),
                        format!("${:.2}", cumulative_total),
                        Style::default().fg(Color::Green),
                    )
                } else if let Some(ask_size) = level.ask_size {
                    // Ask level (red)
                    let cumulative_total = level.cumulative_ask_total.unwrap_or(Decimal::ZERO);
                    (
                        format!("{:.2}", ask_size),
                        format!("${:.2}", cumulative_total),
                        Style::default().fg(Color::Red),
                    )
                } else {
                    // Empty level
                    (
                        "".to_string(),
                        "".to_string(),
                        Style::default().fg(Color::Gray),
                    )
                };

                let price_str = format!("${:.4}", level.price);

                Row::new(vec![price_str, size, total]).style(style)
            }
        })
        .collect();

    let table = Table::new(
        rows,
        &[
            Constraint::Percentage(35), // Price
            Constraint::Percentage(25), // Size
            Constraint::Percentage(40), // Total USD
        ],
    )
    .header(Row::new(vec!["Price", "Size", "Total USD"]).style(Style::default().fg(Color::Yellow)))
    .block(Block::default().borders(Borders::ALL).title("Order Book"));

    frame.render_widget(table, chunks[1]);
}