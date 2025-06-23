use crate::tui::App;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};

pub struct TokensPage {
    pub selected_token: usize,
}

impl TokensPage {
    pub fn new() -> Self {
        Self { selected_token: 0 }
    }

    fn render_tokens_overview(&self, frame: &mut Frame, area: Rect, app: &App) {
        let tokens = app.get_all_active_tokens();

        let header = Row::new(vec![
            "Token ID", "Events", "Last Bid", "Last Ask", "Volume", "Trades",
        ])
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

        let rows: Vec<Row> = tokens
            .iter()
            .enumerate()
            .map(|(i, activity)| {
                let style = if i == self.selected_token {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                let bid_str = activity
                    .last_bid
                    .map(|p| format!("${:.4}", p))
                    .unwrap_or_else(|| "-".to_string());

                let ask_str = activity
                    .last_ask
                    .map(|p| format!("${:.4}", p))
                    .unwrap_or_else(|| "-".to_string());

                Row::new(vec![
                    Cell::from(&activity.token_id[..16]),
                    Cell::from(activity.event_count.to_string()),
                    Cell::from(bid_str),
                    Cell::from(ask_str),
                    Cell::from(format!("${:.2}", activity.total_volume)),
                    Cell::from(activity.trade_count.to_string()),
                ])
                .style(style)
            })
            .collect();

        let table = Table::new(
            rows,
            &[
                Constraint::Percentage(25),
                Constraint::Percentage(12),
                Constraint::Percentage(15),
                Constraint::Percentage(15),
                Constraint::Percentage(15),
                Constraint::Percentage(18),
            ],
        )
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(format!(
            "Active Tokens ({}) - Use ↑↓ to select, Enter to view stream",
            tokens.len()
        )));

        frame.render_widget(table, area);
    }

    fn render_token_details(&self, frame: &mut Frame, area: Rect, app: &App) {
        let tokens = app.get_all_active_tokens();

        let content = if let Some(activity) = tokens.get(self.selected_token) {
            let elapsed = activity
                .last_update
                .map(|t| t.elapsed().as_secs())
                .unwrap_or(0);

            let spread = match (activity.last_bid, activity.last_ask) {
                (Some(bid), Some(ask)) if ask > bid => {
                    format!(
                        "${:.4} ({:.2}%)",
                        ask - bid,
                        ((ask - bid) / bid * rust_decimal::Decimal::from(100))
                            .to_string()
                            .parse::<f64>()
                            .unwrap_or(0.0)
                    )
                }
                _ => "N/A".to_string(),
            };

            format!(
                "Token Details:\n\nToken ID: {}\n\nActivity:\n• Events: {}\n• Trades: {}\n• Volume: ${:.2}\n\nPricing:\n• Last Bid: {}\n• Last Ask: {}\n• Spread: {}\n\nTiming:\n• Last Update: {}s ago\n\nControls:\nEnter - View in Stream\nS - Subscribe/Unsubscribe\nI - Token Info",
                activity.token_id,
                activity.event_count,
                activity.trade_count,
                activity.total_volume,
                activity.last_bid.map(|p| format!("${:.4}", p)).unwrap_or_else(|| "N/A".to_string()),
                activity.last_ask.map(|p| format!("${:.4}", p)).unwrap_or_else(|| "N/A".to_string()),
                spread,
                elapsed
            )
        } else {
            "No token selected".to_string()
        };

        let paragraph = Paragraph::new(content).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Token Details"),
        );

        frame.render_widget(paragraph, area);
    }

    fn render_token_stats(&self, frame: &mut Frame, area: Rect, app: &App) {
        let tokens = app.get_all_active_tokens();
        let total_events: usize = tokens.iter().map(|t| t.event_count).sum();
        let total_volume: rust_decimal::Decimal = tokens.iter().map(|t| t.total_volume).sum();
        let total_trades: usize = tokens.iter().map(|t| t.trade_count).sum();

        let content = format!(
            "Overall Statistics:\n\nActive Tokens: {}\nTotal Events: {}\nTotal Volume: ${:.2}\nTotal Trades: {}\n\nMost Active Token:\n{}",
            tokens.len(),
            total_events,
            total_volume,
            total_trades,
            tokens.first()
                .map(|t| format!("{} ({} events)", &t.token_id[..16], t.event_count))
                .unwrap_or_else(|| "None".to_string())
        );

        let paragraph = Paragraph::new(content)
            .block(Block::default().borders(Borders::ALL).title("Statistics"));

        frame.render_widget(paragraph, area);
    }
}

impl super::Page for TokensPage {
    fn render(&self, frame: &mut Frame, area: Rect, app: &App) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);

        // Left side: Tokens list
        self.render_tokens_overview(frame, chunks[0], app);

        // Right side: Token details and stats
        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(chunks[1]);

        self.render_token_details(frame, right_chunks[0], app);
        self.render_token_stats(frame, right_chunks[1], app);
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut App) -> bool {
        let tokens = app.get_all_active_tokens();

        match key.code {
            KeyCode::Up => {
                self.selected_token = self.selected_token.saturating_sub(1);
                true
            }
            KeyCode::Down => {
                if !tokens.is_empty() {
                    self.selected_token = (self.selected_token + 1).min(tokens.len() - 1);
                }
                true
            }
            KeyCode::Enter => {
                if let Some(activity) = tokens.get(self.selected_token) {
                    // Switch to stream page and select this token
                    app.current_token_id = Some(activity.token_id.clone());
                    app.state = crate::tui::AppState::OrderBook {
                        token_id: activity.token_id.clone(),
                    };

                    // Get current order book from streamer
                    if let Some(order_book) = app.streamer.get_order_book(&activity.token_id) {
                        app.current_bids = order_book.get_bids().to_vec();
                        app.current_asks = order_book.get_asks().to_vec();
                    }
                }
                true
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                // Handle subscribe/unsubscribe
                true
            }
            KeyCode::Char('i') | KeyCode::Char('I') => {
                // Handle token info
                true
            }
            _ => false,
        }
    }
}

impl Default for TokensPage {
    fn default() -> Self {
        Self::new()
    }
}
