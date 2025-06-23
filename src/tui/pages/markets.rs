use crate::tui::App;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Cell, List, ListItem, Paragraph, Row, Table},
};

pub struct MarketsPage {
    pub selected_market: usize,
}

impl MarketsPage {
    pub fn new() -> Self {
        Self { selected_market: 0 }
    }

    fn render_markets_list(&self, frame: &mut Frame, area: Rect, app: &App) {
        // For now, show active tokens as a proxy for markets
        let tokens = app.get_all_active_tokens();

        let header = Row::new(vec!["Token ID", "Events", "Last Bid", "Last Ask", "Volume"]).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

        let rows: Vec<Row> = tokens
            .iter()
            .take(20) // Limit to 20 for display
            .enumerate()
            .map(|(i, activity)| {
                let style = if i == self.selected_market {
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

                let volume_str = if activity.total_volume > rust_decimal::Decimal::ZERO {
                    format!("${:.0}", activity.total_volume)
                } else {
                    "-".to_string()
                };

                Row::new(vec![
                    Cell::from(&activity.token_id[..16]),
                    Cell::from(activity.event_count.to_string()),
                    Cell::from(bid_str),
                    Cell::from(ask_str),
                    Cell::from(volume_str),
                ])
                .style(style)
            })
            .collect();

        let table = Table::new(
            rows,
            &[
                Constraint::Percentage(35),
                Constraint::Percentage(15),
                Constraint::Percentage(15),
                Constraint::Percentage(15),
                Constraint::Percentage(20),
            ],
        )
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(format!(
            "Active Market Tokens ({}) - Use ↑↓ to select, Enter to view",
            tokens.len()
        )));

        frame.render_widget(table, area);
    }

    fn render_market_details(&self, frame: &mut Frame, area: Rect, app: &App) {
        let tokens = app.get_all_active_tokens();

        let content = if let Some(activity) = tokens.get(self.selected_market) {
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
                "Token Details:\n\nToken ID: {}\n\nActivity:\n• Events: {}\n• Trades: {}\n• Volume: ${:.2}\n• Event Rate: {:.2}/min\n\nPricing:\n• Last Bid: {}\n• Last Ask: {}\n• Spread: {}\n\nTiming:\n• Last Update: {}s ago\n\nControls:\nEnter - View in Stream\nT - View in Tokens page\nS - Subscribe",
                activity.token_id,
                activity.event_count,
                activity.trade_count,
                activity.total_volume,
                if elapsed > 0 { activity.event_count as f64 / (elapsed as f64 / 60.0) } else { 0.0 },
                activity.last_bid.map(|p| format!("${:.4}", p)).unwrap_or_else(|| "N/A".to_string()),
                activity.last_ask.map(|p| format!("${:.4}", p)).unwrap_or_else(|| "N/A".to_string()),
                spread,
                elapsed
            )
        } else {
            "No token selected\n\nMarkets Integration:\nThis page shows active trading tokens as a proxy for markets.\nTo see full market data, use the markets CLI commands:\n\n• polybot markets list\n• polybot fetch-all-markets\n• polybot analyze".to_string()
        };

        let paragraph = Paragraph::new(content).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Token/Market Details"),
        );

        frame.render_widget(paragraph, area);
    }

    fn render_market_actions(&self, frame: &mut Frame, area: Rect, _app: &App) {
        let actions = vec![
            "Enter - View Market Tokens",
            "I - Market Information",
            "F - Add to Favorites",
            "S - Market Statistics",
            "R - Refresh Markets",
        ];

        let items: Vec<ListItem> = actions
            .iter()
            .map(|action| ListItem::new(*action))
            .collect();

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Available Actions"),
        );

        frame.render_widget(list, area);
    }
}

impl super::Page for MarketsPage {
    fn render(&self, frame: &mut Frame, area: Rect, app: &App) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area);

        // Left side: Markets list
        self.render_markets_list(frame, chunks[0], app);

        // Right side: Market details and actions
        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(chunks[1]);

        self.render_market_details(frame, right_chunks[0], app);
        self.render_market_actions(frame, right_chunks[1], app);
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut App) -> bool {
        match key.code {
            KeyCode::Up => {
                self.selected_market = self.selected_market.saturating_sub(1);
                true
            }
            KeyCode::Down => {
                let tokens_count = app.get_all_active_tokens().len();
                if tokens_count > 0 {
                    self.selected_market = (self.selected_market + 1).min(tokens_count.min(20) - 1);
                    // Limit to 20 displayed
                }
                true
            }
            KeyCode::Enter => {
                // Handle view market tokens
                true
            }
            KeyCode::Char('i') | KeyCode::Char('I') => {
                // Handle market info
                true
            }
            KeyCode::Char('f') | KeyCode::Char('F') => {
                // Handle add to favorites
                true
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                // Handle market statistics
                true
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                // Handle refresh
                true
            }
            _ => false,
        }
    }
}

impl Default for MarketsPage {
    fn default() -> Self {
        Self::new()
    }
}
