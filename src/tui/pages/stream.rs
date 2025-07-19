use crate::tui::widgets::order_book::render_order_book;
use crate::tui::{App, AppState};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs},
};

#[derive(Debug, Clone, Copy, PartialEq)]
enum StreamTab {
    ActiveTokens,
    RecentEvents,
}

pub struct StreamPage {
    current_tab: StreamTab,
}

impl StreamPage {
    pub fn new() -> Self {
        Self {
            current_tab: StreamTab::ActiveTokens,
        }
    }

    fn render_overview(&self, frame: &mut Frame, area: Rect, app: &App) {
        // Split area for tabs at top and content below
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(area);

        // Render tabs
        let tab_titles = vec!["Active Tokens", "Recent Events"];
        let selected_tab = match self.current_tab {
            StreamTab::ActiveTokens => 0,
            StreamTab::RecentEvents => 1,
        };
        let tabs = Tabs::new(tab_titles)
            .block(Block::default().borders(Borders::ALL).title("Stream View"))
            .select(selected_tab)
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
        frame.render_widget(tabs, main_chunks[0]);

        // Render content based on selected tab
        match self.current_tab {
            StreamTab::ActiveTokens => {
                // Original layout for active tokens view
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                    .split(main_chunks[1]);

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
            StreamTab::RecentEvents => {
                // Full area for recent events with metrics at bottom
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(7)])
                    .split(main_chunks[1]);

                self.render_full_event_log(frame, chunks[0], app);
                self.render_metrics(frame, chunks[1], app);
            }
        }
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
                "Active Tokens ({}) - Use ↑↓ to select, Enter to view, ←→ to switch tabs",
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

    fn render_full_event_log(&self, frame: &mut Frame, area: Rect, app: &App) {
        let content_height = area.height.saturating_sub(2) as usize; // Account for borders
        let total_events = app.event_log.len();
        
        // Bound the scroll position to prevent going beyond content
        let max_scroll = if total_events > content_height {
            total_events.saturating_sub(content_height)
        } else {
            0
        };
        let bounded_scroll = app.event_log_scroll.min(max_scroll);
        
        let events: Vec<ListItem> = app
            .event_log
            .iter()
            .rev()
            .skip(bounded_scroll)
            .take(content_height)
            .map(|event| ListItem::new(event.clone()))
            .collect();

        // Calculate proper visible range
        let visible_start = if total_events > 0 {
            total_events.saturating_sub(bounded_scroll).saturating_sub(events.len()) + 1
        } else {
            0
        };
        let visible_end = total_events.saturating_sub(bounded_scroll);
        
        let title = if total_events > 0 {
            format!("All Events ({}-{} of {}) - Use ↑↓ to scroll, Space to center", visible_start, visible_end, total_events)
        } else {
            "All Events (0) - Waiting for events...".to_string()
        };

        let list = List::new(events).block(
            Block::default()
                .borders(Borders::ALL)
                .title(title),
        );

        frame.render_widget(list, area);
    }

    fn render_orderbook(&self, frame: &mut Frame, area: Rect, app: &App, token_id: &str) {
        // First split vertically to add token ID bar at top
        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(area);

        // Render token ID bar at the top for easy copying
        let token_id_widget = Paragraph::new(format!(" Token ID: {} (Press 'c' to copy)", token_id))
            .block(Block::default()
                .borders(Borders::ALL)
                .title("Token Information")
                .border_style(Style::default().fg(Color::Cyan)))
            .style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
        frame.render_widget(token_id_widget, vertical_chunks[0]);

        // Now split the remaining area horizontally
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(vertical_chunks[1]);

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

        // Show full token ID on separate line for easy copying
        let content = format!(
            "Token ID:\n{}\n\nEvents Received: {}\n\nBids: {}\nAsks: {}\n\nControls:\n↑↓ - Scroll\nSpace - Center\nc - Copy Token ID\nBackspace - Back",
            token_id,
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
                KeyCode::Left => {
                    self.current_tab = StreamTab::ActiveTokens;
                    true
                }
                KeyCode::Right => {
                    self.current_tab = StreamTab::RecentEvents;
                    // Bound the scroll position to valid range when switching tabs
                    let estimated_display_height = 25;
                    let max_scroll = if app.event_log.len() > estimated_display_height {
                        app.event_log.len().saturating_sub(estimated_display_height)
                    } else {
                        0
                    };
                    app.event_log_scroll = app.event_log_scroll.min(max_scroll);
                    true
                }
                KeyCode::Up => {
                    match self.current_tab {
                        StreamTab::ActiveTokens => app.select_previous(),
                        StreamTab::RecentEvents => app.scroll_event_log_up(),
                    }
                    true
                }
                KeyCode::Down => {
                    match self.current_tab {
                        StreamTab::ActiveTokens => app.select_next(),
                        StreamTab::RecentEvents => app.scroll_event_log_down(),
                    }
                    true
                }
                KeyCode::Char(' ') => {
                    if self.current_tab == StreamTab::RecentEvents {
                        // Center the event log view
                        let content_height = 25; // Reasonable default
                        if app.event_log.len() > content_height {
                            // Center by showing the middle portion of the log
                            app.event_log_scroll = (app.event_log.len().saturating_sub(content_height)) / 2;
                        } else {
                            // If all content fits, scroll to top
                            app.event_log_scroll = 0;
                        }
                        true
                    } else {
                        false
                    }
                }
                KeyCode::Enter => {
                    if self.current_tab == StreamTab::ActiveTokens {
                        app.select_token();
                        true
                    } else {
                        false
                    }
                }
                KeyCode::Char('t') | KeyCode::Char('T') => {
                    // Debug: Add test events
                    for i in 0..50 {
                        app.event_log.push(format!("🧪 Test event #{}: This is a longer test event to make scrolling more obvious", i));
                    }
                    true
                }
                _ => false,
            },
            AppState::OrderBook { token_id } => {
                let token_id_clone = token_id.clone();
                match key.code {
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
                    KeyCode::Char('c') | KeyCode::Char('C') => {
                        // Copy token ID to clipboard
                        let _ = app.copy_token_to_clipboard(&token_id_clone);
                        true
                    }
                    _ => false,
                }
            }
        }
    }
}

impl Default for StreamPage {
    fn default() -> Self {
        Self::new()
    }
}
