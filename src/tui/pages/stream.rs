use crate::tui::widgets::order_book::render_order_book;
use crate::tui::{App, AppState};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

pub struct StreamPage {}

impl StreamPage {
    pub fn new() -> Self {
        Self {}
    }

    fn render_overview(&self, frame: &mut Frame, area: Rect, app: &App) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area);

        // Left side: Active tokens list
        self.render_tokens_list(frame, chunks[0], app);

        // Right side: Event log and metrics
        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(chunks[1]);

        self.render_event_log(frame, right_chunks[0], app);
        self.render_metrics(frame, right_chunks[1], app);
    }

    fn render_tokens_list(&self, frame: &mut Frame, area: Rect, app: &App) {
        let tokens = app.get_all_active_tokens();

        let items: Vec<ListItem> = tokens
            .iter()
            .enumerate()
            .map(|(i, activity)| {
                let is_selected = i == app.selected_token_index;
                let style = if is_selected {
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

                let elapsed = activity
                    .last_update
                    .map(|t| t.elapsed().as_secs())
                    .unwrap_or(0);

                let content = format!(
                    "{} | Events: {} | Bid: {} | Ask: {} | {}s ago",
                    &activity.token_id[..16],
                    activity.event_count,
                    bid_str,
                    ask_str,
                    elapsed
                );

                ListItem::new(content).style(style)
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(format!(
                "Active Tokens ({}) - Use ↑↓ to select, Enter to view",
                tokens.len()
            )))
            .highlight_style(Style::default().fg(Color::Yellow));

        frame.render_widget(list, area);
    }

    fn render_event_log(&self, frame: &mut Frame, area: Rect, app: &App) {
        let events: Vec<ListItem> = app
            .event_log
            .iter()
            .rev()
            .take(area.height.saturating_sub(2) as usize)
            .map(|event| ListItem::new(event.clone()))
            .collect();

        let list = List::new(events).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Recent Events"),
        );

        frame.render_widget(list, area);
    }

    fn render_metrics(&self, frame: &mut Frame, area: Rect, app: &App) {
        let elapsed = app.elapsed_time();
        let rate = if elapsed.as_secs() > 0 {
            app.total_events_received as f64 / elapsed.as_secs() as f64
        } else {
            0.0
        };

        let content = format!(
            "Runtime: {:02}:{:02}:{:02}\nTotal Events: {}\nEvent Rate: {:.1}/sec\nActive Tokens: {}",
            elapsed.as_secs() / 3600,
            (elapsed.as_secs() % 3600) / 60,
            elapsed.as_secs() % 60,
            app.total_events_received,
            rate,
            app.get_all_active_tokens().len()
        );

        let paragraph =
            Paragraph::new(content).block(Block::default().borders(Borders::ALL).title("Metrics"));

        frame.render_widget(paragraph, area);
    }

    fn render_orderbook(&self, frame: &mut Frame, area: Rect, app: &App, token_id: &str) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(area);

        // Left side: Order book
        render_order_book(
            frame,
            chunks[0],
            &app.current_bids,
            &app.current_asks,
            token_id,
            app.orderbook_scroll,
        );

        // Right side: Token info and controls
        self.render_token_info(frame, chunks[1], app, token_id);
    }

    fn render_token_info(&self, frame: &mut Frame, area: Rect, app: &App, token_id: &str) {
        let event_count = app.get_token_event_count(token_id);

        let content = format!(
            "Token: {}\n\nEvents Received: {}\n\nBids: {}\nAsks: {}\n\nControls:\n↑↓ - Scroll\nSpace - Center\nBackspace - Back",
            &token_id[..32],
            event_count,
            app.current_bids.len(),
            app.current_asks.len()
        );

        let paragraph = Paragraph::new(content).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Token Details"),
        );

        frame.render_widget(paragraph, area);
    }
}

impl super::Page for StreamPage {
    fn render(&self, frame: &mut Frame, area: Rect, app: &App) {
        match &app.state {
            AppState::Overview => {
                self.render_overview(frame, area, app);
            }
            AppState::OrderBook { token_id } => {
                self.render_orderbook(frame, area, app, token_id);
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut App) -> bool {
        match &app.state {
            AppState::Overview => match key.code {
                KeyCode::Up => {
                    app.select_previous();
                    true
                }
                KeyCode::Down => {
                    app.select_next();
                    true
                }
                KeyCode::Enter => {
                    app.select_token();
                    true
                }
                _ => false,
            },
            AppState::OrderBook { .. } => match key.code {
                KeyCode::Backspace => {
                    app.go_back();
                    true
                }
                KeyCode::Up => {
                    app.scroll_orderbook_up();
                    true
                }
                KeyCode::Down => {
                    app.scroll_orderbook_down();
                    true
                }
                KeyCode::Char(' ') => {
                    app.reset_orderbook_scroll();
                    true
                }
                _ => false,
            },
        }
    }
}

impl Default for StreamPage {
    fn default() -> Self {
        Self::new()
    }
}
