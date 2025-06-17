use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs},
    Frame, Terminal,
};
use std::{
    io,
    time::{Duration, Instant},
};
use tracing::info;

use crate::data_paths::DataPaths;
use crate::typed_store::{
    TypedStore,
    models::{MarketTable, TokenTable, ConditionTable, RocksDbMarket, Token, Condition}
};

#[derive(Debug, Clone, Copy, PartialEq)]
enum TabMode {
    Markets,
    Tokens,
    Conditions,
}

impl TabMode {
    fn title(&self) -> &'static str {
        match self {
            TabMode::Markets => "Markets",
            TabMode::Tokens => "Tokens", 
            TabMode::Conditions => "Conditions",
        }
    }

    fn all() -> &'static [TabMode] {
        &[TabMode::Markets, TabMode::Tokens, TabMode::Conditions]
    }
}

pub struct MarketsTui {
    data_paths: DataPaths,
    store: Option<TypedStore>,
    current_tab: TabMode,
    search_query: String,
    search_mode: bool,
    
    // Markets data
    markets: Vec<RocksDbMarket>,
    markets_list_state: ListState,
    markets_page: usize,
    markets_total_pages: usize,
    
    // Tokens data
    tokens: Vec<Token>,
    tokens_list_state: ListState,
    tokens_page: usize,
    tokens_total_pages: usize,
    
    // Conditions data
    conditions: Vec<Condition>,
    conditions_list_state: ListState,
    conditions_page: usize,
    conditions_total_pages: usize,
    
    // UI state
    status_message: Option<String>,
    last_status_time: Option<Instant>,
    items_per_page: usize,
}

impl MarketsTui {
    pub fn new(data_paths: DataPaths) -> Result<Self> {
        let mut tui = Self {
            data_paths,
            store: None,
            current_tab: TabMode::Markets,
            search_query: String::new(),
            search_mode: false,
            
            markets: Vec::new(),
            markets_list_state: ListState::default(),
            markets_page: 0,
            markets_total_pages: 0,
            
            tokens: Vec::new(),
            tokens_list_state: ListState::default(),
            tokens_page: 0,
            tokens_total_pages: 0,
            
            conditions: Vec::new(),
            conditions_list_state: ListState::default(),
            conditions_page: 0,
            conditions_total_pages: 0,
            
            status_message: None,
            last_status_time: None,
            items_per_page: 20,
        };

        tui.initialize_database()?;
        Ok(tui)
    }

    pub async fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.run_app(&mut terminal).await;

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

    async fn run_app<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        // Load initial data
        self.load_current_tab_data()?;

        loop {
            terminal.draw(|f| self.ui(f))?;

            // Clear old status messages
            if let Some(last_time) = self.last_status_time {
                if last_time.elapsed() > Duration::from_secs(3) {
                    self.status_message = None;
                    self.last_status_time = None;
                }
            }

            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        if self.search_mode {
                            match key.code {
                                KeyCode::Enter => {
                                    self.search_mode = false;
                                    self.perform_search()?;
                                }
                                KeyCode::Esc => {
                                    self.search_mode = false;
                                    self.search_query.clear();
                                    self.load_current_tab_data()?;
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
                            match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                                KeyCode::Char('/') => {
                                    self.search_mode = true;
                                }
                                KeyCode::Tab => {
                                    self.next_tab();
                                }
                                KeyCode::BackTab => {
                                    self.previous_tab();
                                }
                                KeyCode::Up | KeyCode::Char('k') => {
                                    self.previous_item();
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    self.next_item();
                                }
                                KeyCode::PageUp => {
                                    self.previous_page()?;
                                }
                                KeyCode::PageDown => {
                                    self.next_page()?;
                                }
                                KeyCode::Home => {
                                    self.first_page()?;
                                }
                                KeyCode::End => {
                                    self.last_page()?;
                                }
                                KeyCode::Enter => {
                                    self.show_item_details();
                                }
                                KeyCode::Char('r') => {
                                    self.refresh_data()?;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }

    fn ui(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title + Search
                Constraint::Length(3), // Tabs
                Constraint::Min(10),   // Main content
                Constraint::Length(3), // Pagination info
                Constraint::Length(4), // Instructions
                Constraint::Length(3), // Status
            ])
            .split(f.area());

        // Title and search bar
        self.render_title_and_search(f, chunks[0]);

        // Tabs
        self.render_tabs(f, chunks[1]);

        // Main content
        match self.current_tab {
            TabMode::Markets => self.render_markets_list(f, chunks[2]),
            TabMode::Tokens => self.render_tokens_list(f, chunks[2]),
            TabMode::Conditions => self.render_conditions_list(f, chunks[2]),
        }

        // Pagination info
        self.render_pagination_info(f, chunks[3]);

        // Instructions
        self.render_instructions(f, chunks[4]);

        // Status
        self.render_status(f, chunks[5]);
    }

    fn render_title_and_search(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let title_text = if self.search_mode {
            format!("🔍 Search: {}_", self.search_query)
        } else if !self.search_query.is_empty() {
            format!("🗄️ Markets Database - Search: '{}'", self.search_query)
        } else {
            "🗄️ Markets Database Browser".to_string()
        };

        let title = Paragraph::new(title_text)
            .style(if self.search_mode {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            })
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(title, area);
    }

    fn render_tabs(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let titles: Vec<Line> = TabMode::all()
            .iter()
            .map(|tab| Line::from(tab.title()))
            .collect();

        let selected_tab = TabMode::all()
            .iter()
            .position(|&tab| tab == self.current_tab)
            .unwrap_or(0);

        let tabs = Tabs::new(titles)
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .select(selected_tab);

        f.render_widget(tabs, area);
    }

    fn render_markets_list(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
        let items: Vec<ListItem> = self
            .markets
            .iter()
            .map(|market| {
                let status = if market.active {
                    "🟢"
                } else if market.closed {
                    "🔴"
                } else {
                    "🟡"
                };
                
                let volume_str = market.volume
                    .map(|v| format!("${:.0}", v))
                    .unwrap_or_else(|| "N/A".to_string());

                let line = Line::from(vec![
                    Span::raw(format!("{} ", status)),
                    Span::styled(
                        truncate_text(&market.question, 60),
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(format!(" ({})", volume_str)),
                ]);
                ListItem::new(line)
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(format!(
                "Markets ({} total)",
                self.get_total_count()
            )))
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("► ");

        f.render_stateful_widget(list, area, &mut self.markets_list_state);
    }

    fn render_tokens_list(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
        let items: Vec<ListItem> = self
            .tokens
            .iter()
            .map(|token| {
                let winner_indicator = match token.winner {
                    Some(true) => "🏆 ",
                    Some(false) => "❌ ",
                    None => "⏳ ",
                };

                let line = Line::from(vec![
                    Span::raw(winner_indicator),
                    Span::styled(
                        &token.outcome,
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(format!(" (${:.3})", token.current_price)),
                    Span::styled(
                        format!(" - {}", truncate_text(&token.id, 20)),
                        Style::default().fg(Color::Gray),
                    ),
                ]);
                ListItem::new(line)
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(format!(
                "Tokens ({} total)",
                self.get_total_count()
            )))
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("► ");

        f.render_stateful_widget(list, area, &mut self.tokens_list_state);
    }

    fn render_conditions_list(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
        let items: Vec<ListItem> = self
            .conditions
            .iter()
            .map(|condition| {
                let unknown_category = "Unknown".to_string();
                let category_str = condition.category.as_ref().unwrap_or(&unknown_category);
                
                let line = Line::from(vec![
                    Span::styled(
                        truncate_text(&condition.question, 50),
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" [{}]", category_str),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled(
                        format!(" ({} markets)", condition.market_count),
                        Style::default().fg(Color::Gray),
                    ),
                ]);
                ListItem::new(line)
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(format!(
                "Conditions ({} total)",
                self.get_total_count()
            )))
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("► ");

        f.render_stateful_widget(list, area, &mut self.conditions_list_state);
    }

    fn render_pagination_info(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let (current_page, total_pages) = match self.current_tab {
            TabMode::Markets => (self.markets_page + 1, self.markets_total_pages),
            TabMode::Tokens => (self.tokens_page + 1, self.tokens_total_pages),
            TabMode::Conditions => (self.conditions_page + 1, self.conditions_total_pages),
        };

        let pagination_text = if total_pages > 0 {
            format!("Page {} of {} ({} items per page)", current_page, total_pages, self.items_per_page)
        } else {
            "No data available".to_string()
        };

        let pagination = Paragraph::new(pagination_text)
            .style(Style::default().fg(Color::Cyan))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(pagination, area);
    }

    fn render_instructions(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let instructions = if self.search_mode {
            vec![
                Line::from("Type to search, Enter to confirm, Esc to cancel"),
            ]
        } else {
            vec![
                Line::from(vec![
                    Span::raw("Tab: Switch tabs  "),
                    Span::raw("↑/↓: Navigate  "),
                    Span::raw("PgUp/PgDn: Page  "),
                    Span::raw("Home/End: First/Last"),
                ]),
                Line::from(vec![
                    Span::raw("/: Search  "),
                    Span::raw("Enter: Details  "),
                    Span::raw("r: Refresh  "),
                    Span::raw("q: Quit"),
                ]),
            ]
        };

        let instructions_widget = Paragraph::new(instructions)
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title("Controls"));
        f.render_widget(instructions_widget, area);
    }

    fn render_status(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let status_text = if let Some(ref msg) = self.status_message {
            msg.clone()
        } else {
            format!("Ready - {} {} loaded", self.get_items_count(), self.current_tab.title().to_lowercase())
        };

        let status = Paragraph::new(status_text)
            .style(Style::default().fg(Color::Green))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(status, area);
    }

    // Navigation methods
    fn next_tab(&mut self) {
        let tabs = TabMode::all();
        let current_index = tabs.iter().position(|&tab| tab == self.current_tab).unwrap_or(0);
        let next_index = (current_index + 1) % tabs.len();
        self.current_tab = tabs[next_index];
        let _ = self.load_current_tab_data();
    }

    fn previous_tab(&mut self) {
        let tabs = TabMode::all();
        let current_index = tabs.iter().position(|&tab| tab == self.current_tab).unwrap_or(0);
        let prev_index = if current_index == 0 { tabs.len() - 1 } else { current_index - 1 };
        self.current_tab = tabs[prev_index];
        let _ = self.load_current_tab_data();
    }

    fn previous_item(&mut self) {
        match self.current_tab {
            TabMode::Markets => {
                if self.markets.is_empty() { return; }
                let i = match self.markets_list_state.selected() {
                    Some(i) => if i == 0 { self.markets.len() - 1 } else { i - 1 },
                    None => 0,
                };
                self.markets_list_state.select(Some(i));
            }
            TabMode::Tokens => {
                if self.tokens.is_empty() { return; }
                let i = match self.tokens_list_state.selected() {
                    Some(i) => if i == 0 { self.tokens.len() - 1 } else { i - 1 },
                    None => 0,
                };
                self.tokens_list_state.select(Some(i));
            }
            TabMode::Conditions => {
                if self.conditions.is_empty() { return; }
                let i = match self.conditions_list_state.selected() {
                    Some(i) => if i == 0 { self.conditions.len() - 1 } else { i - 1 },
                    None => 0,
                };
                self.conditions_list_state.select(Some(i));
            }
        }
    }

    fn next_item(&mut self) {
        match self.current_tab {
            TabMode::Markets => {
                if self.markets.is_empty() { return; }
                let i = match self.markets_list_state.selected() {
                    Some(i) => if i >= self.markets.len() - 1 { 0 } else { i + 1 },
                    None => 0,
                };
                self.markets_list_state.select(Some(i));
            }
            TabMode::Tokens => {
                if self.tokens.is_empty() { return; }
                let i = match self.tokens_list_state.selected() {
                    Some(i) => if i >= self.tokens.len() - 1 { 0 } else { i + 1 },
                    None => 0,
                };
                self.tokens_list_state.select(Some(i));
            }
            TabMode::Conditions => {
                if self.conditions.is_empty() { return; }
                let i = match self.conditions_list_state.selected() {
                    Some(i) => if i >= self.conditions.len() - 1 { 0 } else { i + 1 },
                    None => 0,
                };
                self.conditions_list_state.select(Some(i));
            }
        }
    }

    fn previous_page(&mut self) -> Result<()> {
        match self.current_tab {
            TabMode::Markets => {
                if self.markets_page > 0 {
                    self.markets_page -= 1;
                    self.load_markets_page()?;
                }
            }
            TabMode::Tokens => {
                if self.tokens_page > 0 {
                    self.tokens_page -= 1;
                    self.load_tokens_page()?;
                }
            }
            TabMode::Conditions => {
                if self.conditions_page > 0 {
                    self.conditions_page -= 1;
                    self.load_conditions_page()?;
                }
            }
        }
        Ok(())
    }

    fn next_page(&mut self) -> Result<()> {
        match self.current_tab {
            TabMode::Markets => {
                if self.markets_page < self.markets_total_pages.saturating_sub(1) {
                    self.markets_page += 1;
                    self.load_markets_page()?;
                }
            }
            TabMode::Tokens => {
                if self.tokens_page < self.tokens_total_pages.saturating_sub(1) {
                    self.tokens_page += 1;
                    self.load_tokens_page()?;
                }
            }
            TabMode::Conditions => {
                if self.conditions_page < self.conditions_total_pages.saturating_sub(1) {
                    self.conditions_page += 1;
                    self.load_conditions_page()?;
                }
            }
        }
        Ok(())
    }

    fn first_page(&mut self) -> Result<()> {
        match self.current_tab {
            TabMode::Markets => {
                self.markets_page = 0;
                self.load_markets_page()?;
            }
            TabMode::Tokens => {
                self.tokens_page = 0;
                self.load_tokens_page()?;
            }
            TabMode::Conditions => {
                self.conditions_page = 0;
                self.load_conditions_page()?;
            }
        }
        Ok(())
    }

    fn last_page(&mut self) -> Result<()> {
        match self.current_tab {
            TabMode::Markets => {
                self.markets_page = self.markets_total_pages.saturating_sub(1);
                self.load_markets_page()?;
            }
            TabMode::Tokens => {
                self.tokens_page = self.tokens_total_pages.saturating_sub(1);
                self.load_tokens_page()?;
            }
            TabMode::Conditions => {
                self.conditions_page = self.conditions_total_pages.saturating_sub(1);
                self.load_conditions_page()?;
            }
        }
        Ok(())
    }

    // Data loading methods
    fn initialize_database(&mut self) -> Result<()> {
        let db_path = self.data_paths.datasets().join("markets.db");
        if !db_path.exists() {
            self.set_status_message("❌ Database not found. Run 'polybot index' first.".to_string());
            return Ok(());
        }

        self.store = Some(TypedStore::open(&db_path)?);
        self.set_status_message("✅ Database loaded successfully".to_string());
        Ok(())
    }

    fn load_current_tab_data(&mut self) -> Result<()> {
        match self.current_tab {
            TabMode::Markets => self.load_markets_page(),
            TabMode::Tokens => self.load_tokens_page(),
            TabMode::Conditions => self.load_conditions_page(),
        }
    }

    fn load_markets_page(&mut self) -> Result<()> {
        if let Some(ref store) = self.store {
            let all_markets = if self.search_query.is_empty() {
                store.scan::<MarketTable>()?
            } else {
                // Apply search filter
                let all = store.scan::<MarketTable>()?;
                all.into_iter()
                    .filter(|(_, market)| {
                        let query_lower = self.search_query.to_lowercase();
                        market.question.to_lowercase().contains(&query_lower) ||
                        market.category.as_ref().map_or(false, |c| c.to_lowercase().contains(&query_lower))
                    })
                    .collect()
            };

            self.markets_total_pages = (all_markets.len() + self.items_per_page - 1) / self.items_per_page;
            
            let start_idx = self.markets_page * self.items_per_page;
            let end_idx = (start_idx + self.items_per_page).min(all_markets.len());
            
            self.markets = all_markets
                .into_iter()
                .skip(start_idx)
                .take(end_idx - start_idx)
                .map(|(_, market)| market)
                .collect();

            if !self.markets.is_empty() && self.markets_list_state.selected().is_none() {
                self.markets_list_state.select(Some(0));
            }
        }
        Ok(())
    }

    fn load_tokens_page(&mut self) -> Result<()> {
        if let Some(ref store) = self.store {
            let all_tokens = if self.search_query.is_empty() {
                store.scan::<TokenTable>()?
            } else {
                let all = store.scan::<TokenTable>()?;
                all.into_iter()
                    .filter(|(_, token)| {
                        let query_lower = self.search_query.to_lowercase();
                        token.outcome.to_lowercase().contains(&query_lower) ||
                        token.id.to_lowercase().contains(&query_lower)
                    })
                    .collect()
            };

            self.tokens_total_pages = (all_tokens.len() + self.items_per_page - 1) / self.items_per_page;
            
            let start_idx = self.tokens_page * self.items_per_page;
            let end_idx = (start_idx + self.items_per_page).min(all_tokens.len());
            
            self.tokens = all_tokens
                .into_iter()
                .skip(start_idx)
                .take(end_idx - start_idx)
                .map(|(_, token)| token)
                .collect();

            if !self.tokens.is_empty() && self.tokens_list_state.selected().is_none() {
                self.tokens_list_state.select(Some(0));
            }
        }
        Ok(())
    }

    fn load_conditions_page(&mut self) -> Result<()> {
        if let Some(ref store) = self.store {
            let all_conditions = if self.search_query.is_empty() {
                store.scan::<ConditionTable>()?
            } else {
                let all = store.scan::<ConditionTable>()?;
                all.into_iter()
                    .filter(|(_, condition)| {
                        let query_lower = self.search_query.to_lowercase();
                        condition.question.to_lowercase().contains(&query_lower) ||
                        condition.category.as_ref().map_or(false, |c| c.to_lowercase().contains(&query_lower))
                    })
                    .collect()
            };

            self.conditions_total_pages = (all_conditions.len() + self.items_per_page - 1) / self.items_per_page;
            
            let start_idx = self.conditions_page * self.items_per_page;
            let end_idx = (start_idx + self.items_per_page).min(all_conditions.len());
            
            self.conditions = all_conditions
                .into_iter()
                .skip(start_idx)
                .take(end_idx - start_idx)
                .map(|(_, condition)| condition)
                .collect();

            if !self.conditions.is_empty() && self.conditions_list_state.selected().is_none() {
                self.conditions_list_state.select(Some(0));
            }
        }
        Ok(())
    }

    fn perform_search(&mut self) -> Result<()> {
        // Reset pagination when searching
        self.markets_page = 0;
        self.tokens_page = 0;
        self.conditions_page = 0;
        
        self.load_current_tab_data()?;
        
        let count = self.get_items_count();
        self.set_status_message(format!("🔍 Found {} {} matching '{}'", count, self.current_tab.title().to_lowercase(), self.search_query));
        
        Ok(())
    }

    fn refresh_data(&mut self) -> Result<()> {
        self.initialize_database()?;
        self.load_current_tab_data()?;
        self.set_status_message("🔄 Data refreshed".to_string());
        Ok(())
    }

    fn show_item_details(&self) {
        // This would show a detailed popup - simplified for now
        match self.current_tab {
            TabMode::Markets => {
                if let Some(i) = self.markets_list_state.selected() {
                    if let Some(market) = self.markets.get(i) {
                        info!("Market selected: {}", market.question);
                    }
                }
            }
            TabMode::Tokens => {
                if let Some(i) = self.tokens_list_state.selected() {
                    if let Some(token) = self.tokens.get(i) {
                        info!("Token selected: {} - {}", token.outcome, token.id);
                    }
                }
            }
            TabMode::Conditions => {
                if let Some(i) = self.conditions_list_state.selected() {
                    if let Some(condition) = self.conditions.get(i) {
                        info!("Condition selected: {}", condition.question);
                    }
                }
            }
        }
    }

    // Helper methods
    fn get_total_count(&self) -> usize {
        match self.current_tab {
            TabMode::Markets => self.markets_total_pages * self.items_per_page,
            TabMode::Tokens => self.tokens_total_pages * self.items_per_page,
            TabMode::Conditions => self.conditions_total_pages * self.items_per_page,
        }
    }

    fn get_items_count(&self) -> usize {
        match self.current_tab {
            TabMode::Markets => self.markets.len(),
            TabMode::Tokens => self.tokens.len(),
            TabMode::Conditions => self.conditions.len(),
        }
    }

    fn set_status_message(&mut self, message: String) {
        self.status_message = Some(message);
        self.last_status_time = Some(Instant::now());
    }
}

fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}...", &text[..max_len.saturating_sub(3)])
    }
}