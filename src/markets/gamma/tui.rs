//! Interactive TUI for Gamma data exploration and querying
//! 
//! This module provides a comprehensive terminal interface for:
//! - Browsing markets, events, trades, and positions
//! - Interactive search and filtering
//! - Real-time data visualization
//! - Export capabilities

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, List, ListItem, ListState, Paragraph, Tabs, Wrap,
    },
    Frame, Terminal,
};
use rust_decimal::prelude::ToPrimitive;
use std::io;
use std::time::{Duration, Instant};

use super::types::*;
use super::storage::GammaStorage;
use super::search::{GammaSearchEngine, MarketAnalytics};

/// Main TUI application state
pub struct GammaTui {
    /// Current tab index
    current_tab: usize,
    /// Tab names
    tabs: Vec<&'static str>,
    /// Market browser state
    market_browser: MarketBrowser,
    /// Event browser state  
    event_browser: EventBrowser,
    /// Trade viewer state
    trade_viewer: TradeViewer,
    /// Position viewer state
    position_viewer: PositionViewer,
    /// Search interface state
    search_interface: SearchInterface,
    /// Analytics dashboard state
    analytics_dashboard: AnalyticsDashboard,
    /// Storage and search engine
    storage: GammaStorage,
    search_engine: GammaSearchEngine,
    /// Status messages
    status_message: String,
    /// Whether to show help
    show_help: bool,
    /// Last refresh time
    last_refresh: Instant,
}

impl GammaTui {
    /// Create new TUI application
    pub fn new(storage: GammaStorage) -> Self {
        let search_engine = GammaSearchEngine::new(storage.clone());
        
        Self {
            current_tab: 0,
            tabs: vec!["Markets", "Events", "Trades", "Positions", "Search", "Analytics"],
            market_browser: MarketBrowser::new(),
            event_browser: EventBrowser::new(),
            trade_viewer: TradeViewer::new(),
            position_viewer: PositionViewer::new(),
            search_interface: SearchInterface::new(),
            analytics_dashboard: AnalyticsDashboard::new(),
            storage,
            search_engine,
            status_message: "Ready".to_string(),
            show_help: false,
            last_refresh: Instant::now(),
        }
    }

    /// Run the TUI application
    pub fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Load initial data
        self.refresh_data()?;

        // Main event loop
        let result = self.run_event_loop(&mut terminal);

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    /// Main event loop
    fn run_event_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
        loop {
            // Draw UI
            terminal.draw(|f| self.draw(f))?;

            // Handle events
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') => break,
                            KeyCode::Char('h') => self.show_help = !self.show_help,
                            KeyCode::Char('r') => self.refresh_data()?,
                            KeyCode::Tab => self.next_tab(),
                            KeyCode::BackTab => self.prev_tab(),
                            KeyCode::Char('1') => self.current_tab = 0,
                            KeyCode::Char('2') => self.current_tab = 1,
                            KeyCode::Char('3') => self.current_tab = 2,
                            KeyCode::Char('4') => self.current_tab = 3,
                            KeyCode::Char('5') => self.current_tab = 4,
                            KeyCode::Char('6') => self.current_tab = 5,
                            _ => {
                                // Pass event to current tab
                                match self.current_tab {
                                    0 => self.market_browser.handle_key(key.code)?,
                                    1 => self.event_browser.handle_key(key.code)?,
                                    2 => self.trade_viewer.handle_key(key.code)?,
                                    3 => self.position_viewer.handle_key(key.code)?,
                                    4 => self.search_interface.handle_key(key.code)?,
                                    5 => self.analytics_dashboard.handle_key(key.code)?,
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }

            // Auto-refresh every 30 seconds
            if self.last_refresh.elapsed() > Duration::from_secs(30) {
                self.refresh_data()?;
            }
        }

        Ok(())
    }

    /// Draw the UI
    fn draw(&mut self, f: &mut Frame) {
        let size = f.area();

        // Create main layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Tabs
                Constraint::Min(0),    // Content
                Constraint::Length(3), // Status
            ])
            .split(size);

        // Draw tabs
        self.draw_tabs(f, chunks[0]);

        // Draw content based on current tab
        match self.current_tab {
            0 => self.market_browser.draw(f, chunks[1]),
            1 => self.event_browser.draw(f, chunks[1]),
            2 => self.trade_viewer.draw(f, chunks[1]),
            3 => self.position_viewer.draw(f, chunks[1]),
            4 => self.search_interface.draw(f, chunks[1]),
            5 => self.analytics_dashboard.draw(f, chunks[1]),
            _ => {}
        }

        // Draw status bar
        self.draw_status(f, chunks[2]);

        // Draw help overlay if requested
        if self.show_help {
            self.draw_help(f, size);
        }
    }

    /// Draw tab bar
    fn draw_tabs(&self, f: &mut Frame, area: Rect) {
        let titles: Vec<Line> = self.tabs
            .iter()
            .map(|t| Line::from(*t))
            .collect();

        let tabs = Tabs::new(titles)
            .block(Block::default().borders(Borders::ALL).title("Gamma Explorer"))
            .select(self.current_tab)
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

        f.render_widget(tabs, area);
    }

    /// Draw status bar
    fn draw_status(&self, f: &mut Frame, area: Rect) {
        let storage_stats = self.storage.get_stats().unwrap_or_default();
        
        let status_text = format!(
            " {} | Markets: {} | Events: {} | Trades: {} | Positions: {} | Press 'h' for help, 'q' to quit ",
            self.status_message,
            storage_stats.total_markets,
            storage_stats.total_events, 
            storage_stats.total_trades,
            storage_stats.total_positions
        );

        let status = Paragraph::new(status_text)
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Left);

        f.render_widget(status, area);
    }

    /// Draw help overlay
    fn draw_help(&self, f: &mut Frame, area: Rect) {
        let help_text = vec![
            Line::from(vec![
                Span::styled("Gamma Explorer Help", Style::default().add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from("Global Controls:"),
            Line::from("  q          - Quit application"),
            Line::from("  h          - Toggle this help"),
            Line::from("  r          - Refresh data"),
            Line::from("  Tab        - Next tab"),
            Line::from("  Shift+Tab  - Previous tab"),
            Line::from("  1-6        - Jump to tab"),
            Line::from(""),
            Line::from("Navigation:"),
            Line::from("  ↑/↓        - Navigate lists"),
            Line::from("  Enter      - Select/View details"),
            Line::from("  /          - Search (where applicable)"),
            Line::from("  Esc        - Cancel/Back"),
            Line::from(""),
            Line::from("Tab-Specific:"),
            Line::from("  Markets    - Browse and filter markets"),
            Line::from("  Events     - View events with nested markets"),
            Line::from("  Trades     - Historical trade data"),
            Line::from("  Positions  - User position tracking"),
            Line::from("  Search     - Advanced filtering interface"),
            Line::from("  Analytics  - Statistics and insights"),
        ];

        let help_popup = Paragraph::new(help_text)
            .block(
                Block::default()
                    .title("Help")
                    .borders(Borders::ALL)
                    .style(Style::default().bg(Color::Black))
            )
            .wrap(Wrap { trim: true });

        let popup_area = centered_rect(60, 70, area);
        f.render_widget(Clear, popup_area);
        f.render_widget(help_popup, popup_area);
    }

    /// Navigate to next tab
    fn next_tab(&mut self) {
        self.current_tab = (self.current_tab + 1) % self.tabs.len();
    }

    /// Navigate to previous tab
    fn prev_tab(&mut self) {
        if self.current_tab > 0 {
            self.current_tab -= 1;
        } else {
            self.current_tab = self.tabs.len() - 1;
        }
    }

    /// Refresh all data
    fn refresh_data(&mut self) -> Result<()> {
        self.status_message = "Refreshing data...".to_string();
        
        // Refresh each component
        self.market_browser.refresh(&self.search_engine)?;
        self.event_browser.refresh(&self.storage)?;
        self.trade_viewer.refresh(&self.storage)?;
        self.position_viewer.refresh(&self.storage)?;
        self.analytics_dashboard.refresh(&self.search_engine)?;
        
        self.status_message = "Data refreshed".to_string();
        self.last_refresh = Instant::now();
        
        Ok(())
    }
}

/// Market browser component
struct MarketBrowser {
    markets: Vec<GammaMarket>,
    state: ListState,
    selected_market: Option<GammaMarket>,
    show_details: bool,
}

impl MarketBrowser {
    fn new() -> Self {
        Self {
            markets: Vec::new(),
            state: ListState::default(),
            selected_market: None,
            show_details: false,
        }
    }

    fn refresh(&mut self, search_engine: &GammaSearchEngine) -> Result<()> {
        // Load top markets by volume
        self.markets = search_engine.get_top_markets_by_volume(100)?;
        if !self.markets.is_empty() && self.state.selected().is_none() {
            self.state.select(Some(0));
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::Up => self.previous(),
            KeyCode::Down => self.next(),
            KeyCode::Enter => self.toggle_details(),
            KeyCode::Esc => self.show_details = false,
            _ => {}
        }
        Ok(())
    }

    fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.markets.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
        self.selected_market = self.markets.get(i).cloned();
    }

    fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.markets.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
        self.selected_market = self.markets.get(i).cloned();
    }

    fn toggle_details(&mut self) {
        if self.selected_market.is_some() {
            self.show_details = !self.show_details;
        }
    }

    fn draw(&mut self, f: &mut Frame, area: Rect) {
        if self.show_details && self.selected_market.is_some() {
            self.draw_market_details(f, area);
        } else {
            self.draw_market_list(f, area);
        }
    }

    fn draw_market_list(&mut self, f: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self.markets
            .iter()
            .map(|market| {
                let volume = format!("${:.0}", market.volume());
                let status = if market.active { "🟢" } else { "🔴" };
                
                ListItem::new(Line::from(vec![
                    Span::raw(status),
                    Span::raw(" "),
                    Span::styled(&market.question, Style::default().fg(Color::White)),
                    Span::raw(" "),
                    Span::styled(volume, Style::default().fg(Color::Green)),
                ]))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title(format!("Markets ({}) - Press Enter for details", self.markets.len()))
                    .borders(Borders::ALL)
            )
            .highlight_style(Style::default().bg(Color::Blue).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");

        f.render_stateful_widget(list, area, &mut self.state);
    }

    fn draw_market_details(&self, f: &mut Frame, area: Rect) {
        if let Some(ref market) = self.selected_market {
            let details = vec![
                Line::from(vec![Span::styled("Market Details", Style::default().add_modifier(Modifier::BOLD))]),
                Line::from(""),
                Line::from(vec![Span::raw("Question: "), Span::styled(&market.question, Style::default().fg(Color::Yellow))]),
                Line::from(vec![Span::raw("Volume: "), Span::styled(format!("${:.2}", market.volume()), Style::default().fg(Color::Green))]),
                Line::from(vec![Span::raw("Liquidity: "), Span::styled(format!("${:.2}", market.liquidity.unwrap_or_default()), Style::default().fg(Color::Blue))]),
                Line::from(vec![Span::raw("Active: "), Span::styled(if market.active { "Yes" } else { "No" }, Style::default().fg(if market.active { Color::Green } else { Color::Red }))]),
                Line::from(""),
                Line::from("Outcomes:"),
            ];

            let mut all_lines = details;
            for (i, outcome) in market.outcomes.iter().enumerate() {
                let price = market.outcome_prices.as_ref()
                    .and_then(|prices| prices.get(i))
                    .unwrap_or(&rust_decimal::Decimal::ZERO);
                all_lines.push(Line::from(vec![
                    Span::raw(format!("  {}: ", outcome)),
                    Span::styled(format!("${:.3}", price), Style::default().fg(Color::Cyan))
                ]));
            }

            let paragraph = Paragraph::new(all_lines)
                .block(Block::default().title("Market Details - Press Esc to go back").borders(Borders::ALL))
                .wrap(Wrap { trim: true });

            f.render_widget(paragraph, area);
        }
    }
}

/// Event browser component
struct EventBrowser {
    events: Vec<GammaEvent>,
    state: ListState,
    selected_event: Option<GammaEvent>,
    show_details: bool,
    market_state: ListState,
}

impl EventBrowser {
    fn new() -> Self {
        Self {
            events: Vec::new(),
            state: ListState::default(),
            selected_event: None,
            show_details: false,
            market_state: ListState::default(),
        }
    }

    fn refresh(&mut self, _storage: &GammaStorage) -> Result<()> {
        // TODO: Load events from storage
        // For now, initialize with empty list
        if !self.events.is_empty() && self.state.selected().is_none() {
            self.state.select(Some(0));
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::Up => {
                if self.show_details && self.selected_event.is_some() {
                    self.previous_market();
                } else {
                    self.previous_event();
                }
            }
            KeyCode::Down => {
                if self.show_details && self.selected_event.is_some() {
                    self.next_market();
                } else {
                    self.next_event();
                }
            }
            KeyCode::Enter => self.toggle_details(),
            KeyCode::Esc => {
                self.show_details = false;
                self.market_state = ListState::default();
            }
            _ => {}
        }
        Ok(())
    }

    fn next_event(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.events.len().saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
        self.selected_event = self.events.get(i).cloned();
    }

    fn previous_event(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.events.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
        self.selected_event = self.events.get(i).cloned();
    }

    fn next_market(&mut self) {
        if let Some(ref event) = self.selected_event {
            let i = match self.market_state.selected() {
                Some(i) => {
                    if i >= event.markets.len().saturating_sub(1) {
                        0
                    } else {
                        i + 1
                    }
                }
                None => 0,
            };
            self.market_state.select(Some(i));
        }
    }

    fn previous_market(&mut self) {
        if let Some(ref event) = self.selected_event {
            let i = match self.market_state.selected() {
                Some(i) => {
                    if i == 0 {
                        event.markets.len().saturating_sub(1)
                    } else {
                        i - 1
                    }
                }
                None => 0,
            };
            self.market_state.select(Some(i));
        }
    }

    fn toggle_details(&mut self) {
        if self.selected_event.is_some() {
            self.show_details = !self.show_details;
            if self.show_details {
                self.market_state.select(Some(0));
            }
        }
    }

    fn draw(&mut self, f: &mut Frame, area: Rect) {
        if self.show_details && self.selected_event.is_some() {
            self.draw_event_details(f, area);
        } else {
            self.draw_event_list(f, area);
        }
    }

    fn draw_event_list(&mut self, f: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self.events
            .iter()
            .map(|event| {
                let volume = format!("${:.0}", event.volume_num.or(event.volume).unwrap_or_default());
                let status = match event.active {
                    Some(true) => "🟢",
                    Some(false) => "🔴",
                    None => "⚪",
                };
                
                ListItem::new(Line::from(vec![
                    Span::raw(status),
                    Span::raw(" "),
                    Span::styled(&event.title, Style::default().fg(Color::White)),
                    Span::raw(" "),
                    Span::styled(volume, Style::default().fg(Color::Green)),
                ]))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title(format!("Events ({}) - Press Enter for details", self.events.len()))
                    .borders(Borders::ALL)
            )
            .highlight_style(Style::default().bg(Color::Blue).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");

        f.render_stateful_widget(list, area, &mut self.state);
    }

    fn draw_event_details(&self, f: &mut Frame, area: Rect) {
        if let Some(ref event) = self.selected_event {
            // Split area into event info and market list
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(8),  // Event info
                    Constraint::Min(0),     // Markets list
                ])
                .split(area);

            // Draw event info
            let volume_str = event.volume_num.or(event.volume)
                .map(|v| format!("${:.2}", v))
                .unwrap_or_else(|| "N/A".to_string());
            let liquidity_str = event.liquidity_num.or(event.liquidity)
                .map(|l| format!("${:.2}", l))
                .unwrap_or_else(|| "N/A".to_string());
            let active_str = match event.active {
                Some(true) => ("Yes", Color::Green),
                Some(false) => ("No", Color::Red),
                None => ("Unknown", Color::Gray),
            };
            
            let event_info = vec![
                Line::from(vec![Span::styled("Event Details", Style::default().add_modifier(Modifier::BOLD))]),
                Line::from(""),
                Line::from(vec![Span::raw("Title: "), Span::styled(&event.title, Style::default().fg(Color::Yellow))]),
                Line::from(vec![Span::raw("Volume: "), Span::styled(volume_str, Style::default().fg(Color::Green))]),
                Line::from(vec![Span::raw("Liquidity: "), Span::styled(liquidity_str, Style::default().fg(Color::Blue))]),
                Line::from(vec![Span::raw("Active: "), Span::styled(active_str.0, Style::default().fg(active_str.1))]),
            ];

            let event_paragraph = Paragraph::new(event_info)
                .block(Block::default().title("Event Info - Press Esc to go back").borders(Borders::ALL));

            f.render_widget(event_paragraph, chunks[0]);

            // Draw markets list
            let market_items: Vec<ListItem> = event.markets
                .iter()
                .map(|market| {
                    let volume = format!("${:.0}", market.volume());
                    let status = if market.active { "🟢" } else { "🔴" };
                    
                    ListItem::new(Line::from(vec![
                        Span::raw(status),
                        Span::raw(" "),
                        Span::styled(&market.question, Style::default().fg(Color::White)),
                        Span::raw(" "),
                        Span::styled(volume, Style::default().fg(Color::Green)),
                    ]))
                })
                .collect();

            let markets_list = List::new(market_items)
                .block(
                    Block::default()
                        .title("Markets in Event")
                        .borders(Borders::ALL)
                )
                .highlight_style(Style::default().bg(Color::DarkGray))
                .highlight_symbol("> ");

            f.render_stateful_widget(markets_list, chunks[1], &mut self.market_state.clone());
        }
    }
}

/// Trade viewer component
struct TradeViewer {
    trades: Vec<GammaTrade>,
    state: ListState,
    selected_trade: Option<GammaTrade>,
    show_details: bool,
    _filter_user: Option<String>,
}

impl TradeViewer {
    fn new() -> Self {
        Self {
            trades: Vec::new(),
            state: ListState::default(),
            selected_trade: None,
            show_details: false,
            _filter_user: None,
        }
    }

    fn refresh(&mut self, _storage: &GammaStorage) -> Result<()> {
        // TODO: Load trades from storage
        // For now, initialize with empty list
        if !self.trades.is_empty() && self.state.selected().is_none() {
            self.state.select(Some(0));
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::Up => self.previous(),
            KeyCode::Down => self.next(),
            KeyCode::Enter => self.toggle_details(),
            KeyCode::Esc => self.show_details = false,
            _ => {}
        }
        Ok(())
    }

    fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.trades.len().saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
        self.selected_trade = self.trades.get(i).cloned();
    }

    fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.trades.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
        self.selected_trade = self.trades.get(i).cloned();
    }

    fn toggle_details(&mut self) {
        if self.selected_trade.is_some() {
            self.show_details = !self.show_details;
        }
    }

    fn draw(&mut self, f: &mut Frame, area: Rect) {
        if self.show_details && self.selected_trade.is_some() {
            self.draw_trade_details(f, area);
        } else {
            self.draw_trade_list(f, area);
        }
    }

    fn draw_trade_list(&mut self, f: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self.trades
            .iter()
            .map(|trade| {
                let side_color = match trade.side {
                    TradeSide::Buy => Color::Green,
                    TradeSide::Sell => Color::Red,
                };
                let side_text = match trade.side {
                    TradeSide::Buy => "BUY ",
                    TradeSide::Sell => "SELL",
                };
                let value = trade.size * trade.price;
                
                ListItem::new(Line::from(vec![
                    Span::styled(side_text, Style::default().fg(side_color)),
                    Span::raw(" "),
                    Span::styled(format!("{} @ ${:.3}", trade.size, trade.price), Style::default().fg(Color::White)),
                    Span::raw(" "),
                    Span::styled(format!("${:.2}", value), Style::default().fg(Color::Yellow)),
                    Span::raw(" "),
                    Span::styled(trade.timestamp.format("%H:%M:%S").to_string(), Style::default().fg(Color::DarkGray)),
                ]))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title(format!("Trades ({}) - Press Enter for details", self.trades.len()))
                    .borders(Borders::ALL)
            )
            .highlight_style(Style::default().bg(Color::Blue).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");

        f.render_stateful_widget(list, area, &mut self.state);
    }

    fn draw_trade_details(&self, f: &mut Frame, area: Rect) {
        if let Some(ref trade) = self.selected_trade {
            let side_color = match trade.side {
                TradeSide::Buy => Color::Green,
                TradeSide::Sell => Color::Red,
            };
            
            let details = vec![
                Line::from(vec![Span::styled("Trade Details", Style::default().add_modifier(Modifier::BOLD))]),
                Line::from(""),
                Line::from(vec![Span::raw("Market: "), Span::styled(&trade.title, Style::default().fg(Color::Yellow))]),
                Line::from(vec![Span::raw("Outcome: "), Span::styled(&trade.outcome, Style::default().fg(Color::Cyan))]),
                Line::from(vec![Span::raw("Side: "), Span::styled(format!("{:?}", trade.side), Style::default().fg(side_color))]),
                Line::from(vec![Span::raw("Size: "), Span::styled(trade.size.to_string(), Style::default().fg(Color::White))]),
                Line::from(vec![Span::raw("Price: "), Span::styled(format!("${:.4}", trade.price), Style::default().fg(Color::White))]),
                Line::from(vec![Span::raw("Value: "), Span::styled(format!("${:.2}", trade.size * trade.price), Style::default().fg(Color::Yellow))]),
                Line::from(vec![Span::raw("Time: "), Span::styled(trade.timestamp.to_string(), Style::default().fg(Color::White))]),
                Line::from(""),
                Line::from(vec![Span::raw("User: "), Span::styled(&trade.proxy_wallet.0, Style::default().fg(Color::Blue))]),
                Line::from(vec![Span::raw("Tx Hash: "), Span::styled(&trade.transaction_hash.0, Style::default().fg(Color::DarkGray))]),
            ];

            let paragraph = Paragraph::new(details)
                .block(Block::default().title("Trade Details - Press Esc to go back").borders(Borders::ALL))
                .wrap(Wrap { trim: true });

            f.render_widget(paragraph, area);
        }
    }
}

/// Position viewer component
struct PositionViewer {
    positions: Vec<GammaPosition>,
    state: ListState,
    selected_position: Option<GammaPosition>,
    show_details: bool,
    user_address: String,
}

impl PositionViewer {
    fn new() -> Self {
        Self {
            positions: Vec::new(),
            state: ListState::default(),
            selected_position: None,
            show_details: false,
            user_address: String::new(),
        }
    }

    fn refresh(&mut self, _storage: &GammaStorage) -> Result<()> {
        // TODO: Load positions from storage
        // For now, initialize with empty list
        if !self.positions.is_empty() && self.state.selected().is_none() {
            self.state.select(Some(0));
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::Up => self.previous(),
            KeyCode::Down => self.next(),
            KeyCode::Enter => self.toggle_details(),
            KeyCode::Esc => self.show_details = false,
            _ => {}
        }
        Ok(())
    }

    fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.positions.len().saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
        self.selected_position = self.positions.get(i).cloned();
    }

    fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.positions.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
        self.selected_position = self.positions.get(i).cloned();
    }

    fn toggle_details(&mut self) {
        if self.selected_position.is_some() {
            self.show_details = !self.show_details;
        }
    }

    fn draw(&mut self, f: &mut Frame, area: Rect) {
        if self.show_details && self.selected_position.is_some() {
            self.draw_position_details(f, area);
        } else {
            self.draw_position_list(f, area);
        }
    }

    fn draw_position_list(&mut self, f: &mut Frame, area: Rect) {
        if self.positions.is_empty() && !self.user_address.is_empty() {
            let msg = Paragraph::new("No positions found. Set user address with 'polybot gamma positions --user <address>'")
                .block(Block::default().title("Positions").borders(Borders::ALL));
            f.render_widget(msg, area);
            return;
        }

        let items: Vec<ListItem> = self.positions
            .iter()
            .map(|position| {
                let pnl_color = if position.cash_pnl >= rust_decimal::Decimal::ZERO {
                    Color::Green
                } else {
                    Color::Red
                };
                let pnl_sign = if position.cash_pnl >= rust_decimal::Decimal::ZERO { "+" } else { "" };
                
                ListItem::new(Line::from(vec![
                    Span::styled(&position.outcome, Style::default().fg(Color::White)),
                    Span::raw(" "),
                    Span::styled(format!("{} shares", position.size), Style::default().fg(Color::Cyan)),
                    Span::raw(" "),
                    Span::styled(format!("${:.2}", position.current_value), Style::default().fg(Color::Yellow)),
                    Span::raw(" "),
                    Span::styled(format!("{}{:.2}%", pnl_sign, position.percent_pnl), Style::default().fg(pnl_color)),
                ]))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title(format!("Positions ({}) - Press Enter for details", self.positions.len()))
                    .borders(Borders::ALL)
            )
            .highlight_style(Style::default().bg(Color::Blue).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");

        f.render_stateful_widget(list, area, &mut self.state);
    }

    fn draw_position_details(&self, f: &mut Frame, area: Rect) {
        if let Some(ref position) = self.selected_position {
            let pnl_color = if position.cash_pnl >= rust_decimal::Decimal::ZERO {
                Color::Green
            } else {
                Color::Red
            };
            
            let details = vec![
                Line::from(vec![Span::styled("Position Details", Style::default().add_modifier(Modifier::BOLD))]),
                Line::from(""),
                Line::from(vec![Span::raw("Market: "), Span::styled(&position.title, Style::default().fg(Color::Yellow))]),
                Line::from(vec![Span::raw("Outcome: "), Span::styled(&position.outcome, Style::default().fg(Color::Cyan))]),
                Line::from(""),
                Line::from("Position Info:"),
                Line::from(vec![Span::raw("  Size: "), Span::styled(format!("{} shares", position.size), Style::default().fg(Color::White))]),
                Line::from(vec![Span::raw("  Avg Price: "), Span::styled(format!("${:.4}", position.avg_price), Style::default().fg(Color::White))]),
                Line::from(vec![Span::raw("  Initial Value: "), Span::styled(format!("${:.2}", position.initial_value), Style::default().fg(Color::White))]),
                Line::from(vec![Span::raw("  Current Value: "), Span::styled(format!("${:.2}", position.current_value), Style::default().fg(Color::Yellow))]),
                Line::from(""),
                Line::from("P&L:"),
                Line::from(vec![Span::raw("  Cash P&L: "), Span::styled(format!("${:.2}", position.cash_pnl), Style::default().fg(pnl_color))]),
                Line::from(vec![Span::raw("  Percent P&L: "), Span::styled(format!("{:.2}%", position.percent_pnl), Style::default().fg(pnl_color))]),
                Line::from(vec![Span::raw("  Realized P&L: "), Span::styled(format!("${:.2}", position.realized_pnl), Style::default().fg(Color::Blue))]),
                Line::from(""),
                Line::from(vec![Span::raw("Redeemable: "), Span::styled(if position.redeemable { "Yes" } else { "No" }, Style::default().fg(if position.redeemable { Color::Green } else { Color::Red }))]),
                Line::from(vec![Span::raw("End Date: "), Span::styled(position.end_date.format("%Y-%m-%d %H:%M").to_string(), Style::default().fg(Color::White))]),
            ];

            let paragraph = Paragraph::new(details)
                .block(Block::default().title("Position Details - Press Esc to go back").borders(Borders::ALL))
                .wrap(Wrap { trim: true });

            f.render_widget(paragraph, area);
        }
    }
}

/// Search interface component
struct SearchInterface {
    search_query: String,
    search_results: Vec<GammaMarket>,
    state: ListState,
    selected_market: Option<GammaMarket>,
    show_details: bool,
    input_mode: bool,
}

impl SearchInterface {
    fn new() -> Self {
        Self {
            search_query: String::new(),
            search_results: Vec::new(),
            state: ListState::default(),
            selected_market: None,
            show_details: false,
            input_mode: false,
        }
    }

    fn handle_key(&mut self, key: KeyCode) -> Result<()> {
        if self.input_mode {
            match key {
                KeyCode::Enter => {
                    self.input_mode = false;
                    // TODO: Execute search
                }
                KeyCode::Esc => {
                    self.input_mode = false;
                }
                KeyCode::Backspace => {
                    self.search_query.pop();
                }
                KeyCode::Char(c) => {
                    self.search_query.push(c);
                }
                _ => {}
            }
        } else {
            match key {
                KeyCode::Char('/') => {
                    self.input_mode = true;
                    self.search_query.clear();
                }
                KeyCode::Up => self.previous(),
                KeyCode::Down => self.next(),
                KeyCode::Enter => self.toggle_details(),
                KeyCode::Esc => self.show_details = false,
                _ => {}
            }
        }
        Ok(())
    }

    fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.search_results.len().saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
        self.selected_market = self.search_results.get(i).cloned();
    }

    fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.search_results.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
        self.selected_market = self.search_results.get(i).cloned();
    }

    fn toggle_details(&mut self) {
        if self.selected_market.is_some() {
            self.show_details = !self.show_details;
        }
    }

    fn draw(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Search box
                Constraint::Min(0),     // Results
            ])
            .split(area);

        // Draw search box
        let search_box = Paragraph::new(self.search_query.as_str())
            .block(Block::default()
                .title(if self.input_mode { "Search (Enter to search, Esc to cancel)" } else { "Search (Press / to search)" })
                .borders(Borders::ALL)
                .border_style(if self.input_mode { Style::default().fg(Color::Yellow) } else { Style::default() }));
        
        f.render_widget(search_box, chunks[0]);

        // Draw results
        if self.show_details && self.selected_market.is_some() {
            self.draw_market_details(f, chunks[1]);
        } else {
            self.draw_search_results(f, chunks[1]);
        }
    }

    fn draw_search_results(&mut self, f: &mut Frame, area: Rect) {
        if self.search_results.is_empty() {
            let msg = Paragraph::new("No results. Press / to search.")
                .block(Block::default().title("Search Results").borders(Borders::ALL));
            f.render_widget(msg, area);
            return;
        }

        let items: Vec<ListItem> = self.search_results
            .iter()
            .map(|market| {
                let volume = format!("${:.0}", market.volume());
                let status = if market.active { "🟢" } else { "🔴" };
                
                ListItem::new(Line::from(vec![
                    Span::raw(status),
                    Span::raw(" "),
                    Span::styled(&market.question, Style::default().fg(Color::White)),
                    Span::raw(" "),
                    Span::styled(volume, Style::default().fg(Color::Green)),
                ]))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title(format!("Search Results ({}) - Press Enter for details", self.search_results.len()))
                    .borders(Borders::ALL)
            )
            .highlight_style(Style::default().bg(Color::Blue).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");

        f.render_stateful_widget(list, area, &mut self.state);
    }

    fn draw_market_details(&self, f: &mut Frame, area: Rect) {
        if let Some(ref market) = self.selected_market {
            let details = vec![
                Line::from(vec![Span::styled("Market Details", Style::default().add_modifier(Modifier::BOLD))]),
                Line::from(""),
                Line::from(vec![Span::raw("Question: "), Span::styled(&market.question, Style::default().fg(Color::Yellow))]),
                Line::from(vec![Span::raw("Volume: "), Span::styled(format!("${:.2}", market.volume()), Style::default().fg(Color::Green))]),
                Line::from(vec![Span::raw("Liquidity: "), Span::styled(format!("${:.2}", market.liquidity.unwrap_or_default()), Style::default().fg(Color::Blue))]),
                Line::from(vec![Span::raw("Active: "), Span::styled(if market.active { "Yes" } else { "No" }, Style::default().fg(if market.active { Color::Green } else { Color::Red }))]),
                Line::from(""),
                Line::from("Outcomes:"),
            ];

            let mut all_lines = details;
            for (i, outcome) in market.outcomes.iter().enumerate() {
                let price = market.outcome_prices.as_ref()
                    .and_then(|prices| prices.get(i))
                    .unwrap_or(&rust_decimal::Decimal::ZERO);
                all_lines.push(Line::from(vec![
                    Span::raw(format!("  {}: ", outcome)),
                    Span::styled(format!("${:.3}", price), Style::default().fg(Color::Cyan))
                ]));
            }

            let paragraph = Paragraph::new(all_lines)
                .block(Block::default().title("Market Details - Press Esc to go back").borders(Borders::ALL))
                .wrap(Wrap { trim: true });

            f.render_widget(paragraph, area);
        }
    }
}

struct AnalyticsDashboard {
    analytics: Option<MarketAnalytics>,
}

impl AnalyticsDashboard {
    fn new() -> Self {
        Self {
            analytics: None,
        }
    }

    fn refresh(&mut self, search_engine: &GammaSearchEngine) -> Result<()> {
        self.analytics = Some(search_engine.get_market_analytics()?);
        Ok(())
    }

    fn handle_key(&mut self, _key: KeyCode) -> Result<()> { Ok(()) }

    fn draw(&mut self, f: &mut Frame, area: Rect) {
        if let Some(ref analytics) = self.analytics {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(8),  // Summary stats
                    Constraint::Min(0),     // Categories and tags
                ])
                .split(area);

            // Draw summary statistics
            self.draw_summary_stats(f, chunks[0], analytics);

            // Draw categories and tags
            self.draw_categories_tags(f, chunks[1], analytics);
        } else {
            let placeholder = Paragraph::new("Loading analytics...")
                .block(Block::default().title("Analytics").borders(Borders::ALL));
            f.render_widget(placeholder, area);
        }
    }

    fn draw_summary_stats(&self, f: &mut Frame, area: Rect, analytics: &MarketAnalytics) {
        let stats_text = vec![
            Line::from(vec![Span::styled("Market Statistics", Style::default().add_modifier(Modifier::BOLD))]),
            Line::from(""),
            Line::from(vec![Span::raw("Total Markets: "), Span::styled(analytics.total_markets.to_string(), Style::default().fg(Color::Yellow))]),
            Line::from(vec![Span::raw("Active: "), Span::styled(analytics.active_markets.to_string(), Style::default().fg(Color::Green))]),
            Line::from(vec![Span::raw("Closed: "), Span::styled(analytics.closed_markets.to_string(), Style::default().fg(Color::Red))]),
            Line::from(vec![Span::raw("Total Volume: "), Span::styled(format!("${:.0}", analytics.total_volume), Style::default().fg(Color::Cyan))]),
            Line::from(vec![Span::raw("Average Volume: "), Span::styled(format!("${:.0}", analytics.avg_volume), Style::default().fg(Color::Cyan))]),
        ];

        let stats = Paragraph::new(stats_text)
            .block(Block::default().title("Summary").borders(Borders::ALL));

        f.render_widget(stats, area);
    }

    fn draw_categories_tags(&self, f: &mut Frame, area: Rect, analytics: &MarketAnalytics) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        // Draw top categories
        let category_items: Vec<ListItem> = analytics.top_categories
            .iter()
            .map(|(name, count)| {
                ListItem::new(Line::from(vec![
                    Span::raw(name),
                    Span::raw(" "),
                    Span::styled(format!("({})", count), Style::default().fg(Color::Yellow)),
                ]))
            })
            .collect();

        let categories = List::new(category_items)
            .block(Block::default().title("Top Categories").borders(Borders::ALL));

        f.render_widget(categories, chunks[0]);

        // Draw top tags  
        let tag_items: Vec<ListItem> = analytics.top_tags
            .iter()
            .take(10) // Limit to top 10 for display
            .map(|(name, count)| {
                ListItem::new(Line::from(vec![
                    Span::raw(name),
                    Span::raw(" "),
                    Span::styled(format!("({})", count), Style::default().fg(Color::Yellow)),
                ]))
            })
            .collect();

        let tags = List::new(tag_items)
            .block(Block::default().title("Top Tags").borders(Borders::ALL));

        f.render_widget(tags, chunks[1]);
    }
}

/// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
/// Interactive markets browser for searching and navigating markets
pub struct MarketsBrowser {
    markets: Vec<GammaMarket>,
    filtered_markets: Vec<usize>, // indices into markets
    selected: usize,
    scroll_offset: usize,
    search_mode: bool,
    search_query: String,
    items_per_page: usize,
}

impl MarketsBrowser {
    pub fn new(markets: Vec<GammaMarket>) -> Self {
        let filtered_markets: Vec<usize> = (0..markets.len()).collect();
        Self {
            markets,
            filtered_markets,
            selected: 0,
            scroll_offset: 0,
            search_mode: false,
            search_query: String::new(),
            items_per_page: 20,
        }
    }
    
    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title + search bar
                Constraint::Min(0),    // Markets list
                Constraint::Length(4), // Help text + status
            ])
            .split(area);
        
        // Update items per page based on available space
        self.items_per_page = (chunks[1].height as usize).saturating_sub(2);
        
        self.render_header(f, chunks[0]);
        self.render_markets_list(f, chunks[1]);
        self.render_help(f, chunks[2]);
    }
    
    fn render_header(&self, f: &mut Frame, area: Rect) {
        let header_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(area);
        
        // Title with scrolling indicator
        let current_page = if self.filtered_markets.is_empty() { 0 } else { self.selected / self.items_per_page + 1 };
        let total_pages = if self.filtered_markets.is_empty() { 0 } else { (self.filtered_markets.len() - 1) / self.items_per_page + 1 };
        
        let title = if self.search_mode {
            format!("🔍 Markets Search - {} results found [Page {}/{}]", 
                self.filtered_markets.len(), current_page, total_pages)
        } else {
            let total_str = if self.markets.len() >= 10000 {
                format!("{}K+", self.markets.len() / 1000)
            } else {
                self.markets.len().to_string()
            };
            format!("📊 Polymarket Markets Browser - {} total markets [Page {}/{}]", 
                total_str, current_page, total_pages)
        };
        
        let title_paragraph = Paragraph::new(title)
            .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .block(Block::default().borders(Borders::BOTTOM));
        f.render_widget(title_paragraph, header_chunks[0]);
        
        // Search bar
        let search_text = if self.search_mode {
            format!("Search: {}_", self.search_query)
        } else {
            "Press \"/\" to search, ↑↓ to navigate, Enter for details, q to quit".to_string()
        };
        
        let search_style = if self.search_mode {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Gray)
        };
        
        let search_paragraph = Paragraph::new(search_text).style(search_style);
        f.render_widget(search_paragraph, header_chunks[1]);
    }
    
    fn render_markets_list(&self, f: &mut Frame, area: Rect) {
        let visible_start = self.scroll_offset;
        let visible_end = (visible_start + self.items_per_page).min(self.filtered_markets.len());
        
        let items: Vec<ListItem> = self.filtered_markets[visible_start..visible_end]
            .iter()
            .enumerate()
            .map(|(i, &market_idx)| {
                let market = &self.markets[market_idx];
                let is_selected = visible_start + i == self.selected;
                
                let volume = market.volume().to_string();
                let liquidity = market.liquidity.unwrap_or_default();
                let status = if market.active { "🟢" } else if market.closed { "🔴" } else { "🟡" };
                
                // Format volume with better readability
                let volume_val: f64 = volume.parse().unwrap_or(0.0);
                let volume_str = if volume_val >= 1_000_000.0 {
                    format!("${:.1}M", volume_val / 1_000_000.0)
                } else if volume_val >= 1_000.0 {
                    format!("${:.1}K", volume_val / 1_000.0)
                } else {
                    format!("${:.0}", volume_val)
                };
                
                // Format liquidity
                let liquidity_str = if liquidity >= rust_decimal::Decimal::new(1_000_000, 0) {
                    format!("${:.1}M", liquidity.to_f64().unwrap_or(0.0) / 1_000_000.0)
                } else if liquidity >= rust_decimal::Decimal::new(1_000, 0) {
                    format!("${:.1}K", liquidity.to_f64().unwrap_or(0.0) / 1_000.0)
                } else {
                    format!("${:.0}", liquidity.to_f64().unwrap_or(0.0))
                };
                
                // Truncate question but add "..." if truncated
                let question_max_len = area.width.saturating_sub(40) as usize;
                let question_display = if market.question.len() > question_max_len {
                    format!("{}...", market.question.chars().take(question_max_len.saturating_sub(3)).collect::<String>())
                } else {
                    market.question.clone()
                };
                
                let line = format!(
                    "{} {} │ Vol: {} │ Liq: {}",
                    status,
                    question_display,
                    volume_str,
                    liquidity_str
                );
                
                let style = if is_selected {
                    Style::default().bg(Color::Blue).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                
                ListItem::new(line).style(style)
            })
            .collect();
        
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Markets"))
            .highlight_style(Style::default().bg(Color::Blue).add_modifier(Modifier::BOLD));
        
        f.render_widget(list, area);
        
        // Render scrollbar if needed
        if self.filtered_markets.len() > self.items_per_page {
            self.render_scrollbar(f, area);
        }
    }
    
    fn render_scrollbar(&self, f: &mut Frame, area: Rect) {
        let scrollbar_area = Rect {
            x: area.x + area.width - 1,
            y: area.y + 1,
            width: 1,
            height: area.height - 2,
        };
        
        let total_items = self.filtered_markets.len();
        let visible_items = self.items_per_page;
        let scroll_pos = self.scroll_offset;
        
        let scrollbar_height = scrollbar_area.height as usize;
        let thumb_size = ((visible_items * scrollbar_height) / total_items).max(1);
        let thumb_pos = (scroll_pos * scrollbar_height) / total_items;
        
        for y in 0..scrollbar_height {
            let style = if y >= thumb_pos && y < thumb_pos + thumb_size {
                Style::default().bg(Color::White)
            } else {
                Style::default().bg(Color::DarkGray)
            };
            
            f.render_widget(
                Block::default().style(style),
                Rect {
                    x: scrollbar_area.x,
                    y: scrollbar_area.y + y as u16,
                    width: 1,
                    height: 1,
                }
            );
        }
    }
    
    fn render_help(&self, f: &mut Frame, area: Rect) {
        let help_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(area);
        
        let help_text = if self.search_mode {
            "🔍 SEARCH MODE: Type keywords to filter markets | Enter: confirm search | Esc: cancel search | Backspace: delete char"
        } else {
            "Navigation: ↑↓ Arrow keys | PgUp/PgDn Fast scroll | Home/End Jump to start/end | / Search | Enter Details | q Quit"
        };
        
        // Status info
        let status_text = if self.filtered_markets.is_empty() {
            "No markets found".to_string()
        } else {
            format!("Market {}/{} selected | {} total markets loaded", 
                self.selected + 1, 
                self.filtered_markets.len(),
                self.markets.len()
            )
        };
        
        let help_paragraph = Paragraph::new(help_text)
            .style(Style::default().fg(Color::Cyan))
            .block(Block::default().borders(Borders::TOP))
            .wrap(Wrap { trim: true });
        f.render_widget(help_paragraph, help_chunks[0]);
        
        let status_paragraph = Paragraph::new(status_text)
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        f.render_widget(status_paragraph, help_chunks[1]);
    }
    
    pub fn start_search(&mut self) {
        self.search_mode = true;
        self.search_query.clear();
    }
    
    pub fn is_searching(&self) -> bool {
        self.search_mode
    }
    
    pub fn cancel_search(&mut self) {
        self.search_mode = false;
        self.search_query.clear();
        // Reset to show all markets
        self.filtered_markets = (0..self.markets.len()).collect();
        self.selected = 0;
        self.scroll_offset = 0;
    }
    
    pub fn handle_char(&mut self, c: char) {
        if self.search_mode {
            self.search_query.push(c);
            self.apply_search();
        }
    }
    
    pub fn handle_backspace(&mut self) {
        if self.search_mode && !self.search_query.is_empty() {
            self.search_query.pop();
            self.apply_search();
        }
    }
    
    pub fn handle_enter(&mut self) {
        if self.search_mode {
            self.search_mode = false;
        } else if let Some(&_market_idx) = self.filtered_markets.get(self.selected) {
            // Show detailed market information in a popup or expanded view
            // For now, just exit search mode
        }
    }
    
    fn apply_search(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_markets = (0..self.markets.len()).collect();
        } else {
            let query_lower = self.search_query.to_lowercase();
            self.filtered_markets = self.markets
                .iter()
                .enumerate()
                .filter(|(_, market)| {
                    market.question.to_lowercase().contains(&query_lower) ||
                    market.description.as_ref()
                        .map(|d| d.to_lowercase().contains(&query_lower))
                        .unwrap_or(false) ||
                    market.category.as_ref()
                        .map(|c| c.to_lowercase().contains(&query_lower))
                        .unwrap_or(false)
                })
                .map(|(i, _)| i)
                .collect();
        }
        
        self.selected = 0;
        self.scroll_offset = 0;
    }
    
    pub fn next(&mut self) {
        if !self.filtered_markets.is_empty() {
            self.selected = (self.selected + 1).min(self.filtered_markets.len() - 1);
            self.ensure_visible();
        }
    }
    
    pub fn previous(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.ensure_visible();
        }
    }
    
    pub fn page_down(&mut self) {
        if !self.filtered_markets.is_empty() {
            self.selected = (self.selected + self.items_per_page).min(self.filtered_markets.len() - 1);
            self.ensure_visible();
        }
    }
    
    pub fn page_up(&mut self) {
        self.selected = self.selected.saturating_sub(self.items_per_page);
        self.ensure_visible();
    }
    
    pub fn go_to_top(&mut self) {
        self.selected = 0;
        self.scroll_offset = 0;
    }
    
    pub fn go_to_bottom(&mut self) {
        if !self.filtered_markets.is_empty() {
            self.selected = self.filtered_markets.len() - 1;
            self.ensure_visible();
        }
    }
    
    fn ensure_visible(&mut self) {
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + self.items_per_page {
            self.scroll_offset = self.selected.saturating_sub(self.items_per_page - 1);
        }
    }
}
