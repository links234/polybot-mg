use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table},
    Frame,
};
use rust_decimal::Decimal;

use crate::tui::{App, AppState};
use crate::tui::widgets::draw_order_book;

pub fn draw(frame: &mut Frame<'_>, app: &mut App) {
    match app.state.clone() {
        AppState::Overview => draw_overview(frame, app),
        AppState::OrderBook { token_id } => draw_order_book(frame, app, &token_id),
    }
}

fn draw_overview(frame: &mut Frame<'_>, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),      // Title
            Constraint::Percentage(40), // Event log
            Constraint::Percentage(55), // Top active tokens
            Constraint::Length(3),      // Help
        ])
        .split(frame.area());
    
    // Title with real-time global event count
    let elapsed = app.elapsed_time();
    let elapsed_str = if elapsed.as_secs() >= 3600 {
        format!("{}h{}m{}s", elapsed.as_secs() / 3600, (elapsed.as_secs() % 3600) / 60, elapsed.as_secs() % 60)
    } else if elapsed.as_secs() >= 60 {
        format!("{}m{}s", elapsed.as_secs() / 60, elapsed.as_secs() % 60)
    } else {
        format!("{}s", elapsed.as_secs())
    };
    
    let title_text = format!("Polymarket WebSocket Stream | 📊 {} total events | ⏱️ {}", 
                            app.total_events_received, elapsed_str);
    let title = Paragraph::new(title_text)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, chunks[0]);
    
    // Event log
    let events: Vec<ListItem> = app.event_log
        .iter()
        .rev()
        .take(20)
        .map(|e| {
            let content = Line::from(Span::raw(e));
            ListItem::new(content)
        })
        .collect();
    
    let events_list = List::new(events)
        .block(Block::default()
            .borders(Borders::ALL)
            .title("Recent Events"));
    frame.render_widget(events_list, chunks[1]);
    
    // All active tokens
    let all_tokens = app.get_all_active_tokens();
    let selected = app.selected_token_index;
    
    // Calculate visible rows for the token table
    let available_height = chunks[2].height.saturating_sub(3) as usize; // Subtract header + borders
    let total_tokens = all_tokens.len();
    
    // Update scroll position to keep selected item visible
    let scroll_offset = if total_tokens <= available_height {
        0
    } else {
        let half_view = available_height / 2;
        if selected >= half_view {
            let max_scroll = total_tokens.saturating_sub(available_height);
            (selected.saturating_sub(half_view)).min(max_scroll)
        } else {
            0
        }
    };
    
    // Get visible tokens based on scroll
    let visible_end = (scroll_offset + available_height).min(total_tokens);
    let visible_tokens = if total_tokens > 0 {
        &all_tokens[scroll_offset..visible_end]
    } else {
        &[]
    };
    
    let rows: Vec<Row> = visible_tokens
        .iter()
        .enumerate()
        .map(|(i, activity)| {
            let global_index = scroll_offset + i;
            let style = if global_index == selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };
            
            let spread = match (activity.last_bid, activity.last_ask) {
                (Some(bid), Some(ask)) if ask > bid => format!("${:.4}", ask - bid),
                (Some(_), Some(_)) => "CROSSED".to_string(),
                _ => "-".to_string(),
            };
            
            let volume = if activity.total_volume > Decimal::ZERO {
                format!("${:.0}", activity.total_volume)
            } else {
                "-".to_string()
            };
            
            let last_update = if let Some(update_time) = activity.last_update {
                let elapsed = update_time.elapsed();
                if elapsed.as_secs() < 60 {
                    format!("{}s", elapsed.as_secs())
                } else {
                    format!("{}m", elapsed.as_secs() / 60)
                }
            } else {
                "-".to_string()
            };
            
            // Calculate events per second for this token
            let events_per_sec = if let Some(update_time) = activity.last_update {
                let elapsed_secs = update_time.elapsed().as_secs().max(1) as f64;
                let rate = activity.event_count as f64 / elapsed_secs;
                if rate >= 1.0 {
                    format!("{:.1}/s", rate)
                } else if rate >= 0.1 {
                    format!("{:.2}/s", rate)
                } else {
                    format!("{:.3}/s", rate)
                }
            } else {
                "-".to_string()
            };
            
            Row::new(vec![
                format!("{}", global_index + 1),
                format!("{}...{}", &activity.token_id[..6], &activity.token_id[activity.token_id.len()-6..]),
                format!("📊{}", activity.event_count),
                format!("⚡{}", events_per_sec),
                format!("🔄{}", activity.trade_count),
                activity.last_bid.map(|p| format!("${:.4}", p)).unwrap_or_else(|| "-".to_string()),
                activity.last_ask.map(|p| format!("${:.4}", p)).unwrap_or_else(|| "-".to_string()),
                spread,
                volume,
                last_update,
            ]).style(style)
        })
        .collect();
    
    let table = Table::new(
        rows,
        &[
            Constraint::Length(3),   // #
            Constraint::Length(15),  // Token ID
            Constraint::Length(10),  // Events
            Constraint::Length(10),  // Events/sec
            Constraint::Length(8),   // Trades
            Constraint::Length(11),  // Last Bid
            Constraint::Length(11),  // Last Ask
            Constraint::Length(11),  // Spread
            Constraint::Length(11),  // Volume
            Constraint::Length(8),   // Last Update
        ],
    )
    .header(Row::new(vec!["#", "Token ID", "Events", "Rate", "Trades", "Bid", "Ask", "Spread", "Volume", "Updated"])
        .style(Style::default().fg(Color::Yellow)))
    .block(Block::default()
        .borders(Borders::ALL)
        .title(format!("All Active Tokens ({} total) - Showing {}-{} | 📈 {} global events", 
               total_tokens, 
               scroll_offset + 1, 
               visible_end,
               app.total_events_received)));
    
    frame.render_widget(table, chunks[2]);
    
    // Help
    let help = Paragraph::new("↑/↓: Navigate tokens (auto-scroll) | Enter: View order book | r: Refresh | q: Quit")
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(help, chunks[3]);
}