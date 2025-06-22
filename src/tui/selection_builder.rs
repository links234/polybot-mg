//! TUI for building token selections

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io::{self, Stdout};
use tracing::info;

use crate::datasets::DatasetType;
use crate::storage::DatasetDiscovery;

#[allow(dead_code)]
pub struct SelectionBuilderResult {
    pub name: String,
    pub description: Option<String>,
    pub tokens: Vec<String>,
    pub cancelled: bool,
}

#[allow(dead_code)]
pub struct SelectionBuilder {
    /// All available tokens grouped by dataset
    available_tokens: Vec<TokenOption>,
    /// Currently selected tokens
    selected_tokens: Vec<String>,
    /// Current selection name
    name: String,
    /// Current description
    description: String,
    /// UI state
    list_state: ListState,
    /// Current mode
    mode: BuilderMode,
    /// Input buffer for name/description
    input_buffer: String,
    /// Search filter
    search_filter: String,
    /// Filtered token indices
    filtered_indices: Vec<usize>,
}

#[derive(Debug, Clone)]
struct TokenOption {
    token_id: String,
    dataset_name: String,
    dataset_type: DatasetType,
    market_info: Option<String>,
}

#[derive(Debug, PartialEq)]
enum BuilderMode {
    SelectingTokens,
    EnteringName,
    EnteringDescription,
    Confirming,
}

#[allow(dead_code)]
impl SelectionBuilder {
    pub async fn run(initial_name: Option<String>) -> Result<SelectionBuilderResult> {
        // Discover available tokens
        info!("Discovering available tokens from datasets...");
        let tokens = Self::discover_tokens().await?;

        if tokens.is_empty() {
            return Ok(SelectionBuilderResult {
                name: String::new(),
                description: None,
                tokens: Vec::new(),
                cancelled: true,
            });
        }

        // Setup terminal
        enable_raw_mode()?;
        io::stdout().execute(EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(io::stdout());
        let mut terminal = Terminal::new(backend)?;

        // Create builder
        let has_initial_name = initial_name.is_some();
        let mut builder = Self {
            available_tokens: tokens,
            selected_tokens: Vec::new(),
            name: initial_name.unwrap_or_default(),
            description: String::new(),
            list_state: ListState::default(),
            mode: if has_initial_name {
                BuilderMode::SelectingTokens
            } else {
                BuilderMode::EnteringName
            },
            input_buffer: String::new(),
            search_filter: String::new(),
            filtered_indices: Vec::new(),
        };

        // Initialize filtered indices
        builder.update_filter();
        if !builder.filtered_indices.is_empty() {
            builder.list_state.select(Some(0));
        }

        let result = builder.run_loop(&mut terminal).await?;

        // Cleanup terminal
        disable_raw_mode()?;
        io::stdout().execute(LeaveAlternateScreen)?;

        Ok(result)
    }

    async fn discover_tokens() -> Result<Vec<TokenOption>> {
        let mut tokens = Vec::new();

        // Use the dataset discovery to find all datasets
        let discovery = DatasetDiscovery::new("./data");
        let datasets = discovery.discover_datasets().await?;

        for dataset in datasets {
            // Skip pipeline datasets as they don't contain streamable tokens
            if matches!(
                dataset.dataset_info.dataset_type,
                DatasetType::Pipeline { .. }
            ) {
                continue;
            }

            // Extract token ID from the dataset
            let token_option = TokenOption {
                token_id: dataset.token_id.clone(),
                dataset_name: dataset.dataset_info.name.clone(),
                dataset_type: dataset.dataset_info.dataset_type.clone(),
                market_info: Some(dataset.market.clone()),
            };

            // Avoid duplicates
            if !tokens
                .iter()
                .any(|t: &TokenOption| t.token_id == token_option.token_id)
            {
                tokens.push(token_option);
            }
        }

        // Sort by dataset name for better organization
        tokens.sort_by(|a, b| a.dataset_name.cmp(&b.dataset_name));

        Ok(tokens)
    }

    async fn run_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<SelectionBuilderResult> {
        loop {
            terminal.draw(|f| self.render(f))?;

            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match self.mode {
                        BuilderMode::SelectingTokens => {
                            match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => {
                                    return Ok(SelectionBuilderResult {
                                        name: String::new(),
                                        description: None,
                                        tokens: Vec::new(),
                                        cancelled: true,
                                    });
                                }
                                KeyCode::Enter | KeyCode::Char(' ') => {
                                    self.toggle_selection();
                                }
                                KeyCode::Up => {
                                    self.move_selection(-1);
                                }
                                KeyCode::Down => {
                                    self.move_selection(1);
                                }
                                KeyCode::PageUp => {
                                    self.move_selection(-10);
                                }
                                KeyCode::PageDown => {
                                    self.move_selection(10);
                                }
                                KeyCode::Char('s') => {
                                    if !self.selected_tokens.is_empty() {
                                        if self.name.is_empty() {
                                            self.mode = BuilderMode::EnteringName;
                                            self.input_buffer.clear();
                                        } else {
                                            self.mode = BuilderMode::Confirming;
                                        }
                                    }
                                }
                                KeyCode::Char('a') => {
                                    // Select all filtered
                                    for idx in &self.filtered_indices {
                                        let token_id = self.available_tokens[*idx].token_id.clone();
                                        if !self.selected_tokens.contains(&token_id) {
                                            self.selected_tokens.push(token_id);
                                        }
                                    }
                                }
                                KeyCode::Char('n') => {
                                    // Deselect all
                                    self.selected_tokens.clear();
                                }
                                KeyCode::Char('/') => {
                                    self.search_filter.clear();
                                }
                                KeyCode::Char(c) if c.is_alphanumeric() => {
                                    self.search_filter.push(c);
                                    self.update_filter();
                                }
                                KeyCode::Backspace => {
                                    self.search_filter.pop();
                                    self.update_filter();
                                }
                                _ => {}
                            }
                        }
                        BuilderMode::EnteringName => match key.code {
                            KeyCode::Esc => {
                                self.mode = BuilderMode::SelectingTokens;
                            }
                            KeyCode::Enter => {
                                if !self.input_buffer.is_empty() {
                                    self.name = self.input_buffer.clone();
                                    self.mode = BuilderMode::EnteringDescription;
                                    self.input_buffer.clear();
                                }
                            }
                            KeyCode::Char(c) => {
                                if c.is_alphanumeric() || c == '-' || c == '_' {
                                    self.input_buffer.push(c);
                                }
                            }
                            KeyCode::Backspace => {
                                self.input_buffer.pop();
                            }
                            _ => {}
                        },
                        BuilderMode::EnteringDescription => match key.code {
                            KeyCode::Esc => {
                                self.mode = BuilderMode::EnteringName;
                            }
                            KeyCode::Enter => {
                                self.description = self.input_buffer.clone();
                                self.mode = BuilderMode::Confirming;
                            }
                            KeyCode::Char(c) => {
                                self.input_buffer.push(c);
                            }
                            KeyCode::Backspace => {
                                self.input_buffer.pop();
                            }
                            _ => {}
                        },
                        BuilderMode::Confirming => match key.code {
                            KeyCode::Char('y') | KeyCode::Enter => {
                                return Ok(SelectionBuilderResult {
                                    name: self.name.clone(),
                                    description: if self.description.is_empty() {
                                        None
                                    } else {
                                        Some(self.description.clone())
                                    },
                                    tokens: self.selected_tokens.clone(),
                                    cancelled: false,
                                });
                            }
                            KeyCode::Char('n') | KeyCode::Esc => {
                                self.mode = BuilderMode::SelectingTokens;
                            }
                            _ => {}
                        },
                    }
                }
            }
        }
    }

    fn toggle_selection(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if let Some(&actual_idx) = self.filtered_indices.get(selected) {
                let token_id = self.available_tokens[actual_idx].token_id.clone();

                if let Some(pos) = self.selected_tokens.iter().position(|t| t == &token_id) {
                    self.selected_tokens.remove(pos);
                } else {
                    self.selected_tokens.push(token_id);
                }
            }
        }
    }

    fn move_selection(&mut self, delta: i32) {
        if self.filtered_indices.is_empty() {
            return;
        }

        let current = self.list_state.selected().unwrap_or(0) as i32;
        let new = (current + delta)
            .max(0)
            .min(self.filtered_indices.len() as i32 - 1) as usize;

        self.list_state.select(Some(new));
    }

    fn update_filter(&mut self) {
        self.filtered_indices.clear();

        let filter = self.search_filter.to_lowercase();

        for (idx, token) in self.available_tokens.iter().enumerate() {
            if filter.is_empty()
                || token.token_id.to_lowercase().contains(&filter)
                || token.dataset_name.to_lowercase().contains(&filter)
                || token
                    .market_info
                    .as_ref()
                    .map(|m| m.to_lowercase().contains(&filter))
                    .unwrap_or(false)
            {
                self.filtered_indices.push(idx);
            }
        }

        // Reset selection if current is out of bounds
        if let Some(selected) = self.list_state.selected() {
            if selected >= self.filtered_indices.len() && !self.filtered_indices.is_empty() {
                self.list_state.select(Some(0));
            }
        }
    }

    fn render(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(10),   // Main content
                Constraint::Length(4), // Instructions
            ])
            .split(f.area());

        // Header
        let header = Paragraph::new("Token Selection Builder")
            .style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            );
        f.render_widget(header, chunks[0]);

        // Main content
        match self.mode {
            BuilderMode::SelectingTokens => {
                self.render_token_selection(f, chunks[1]);
            }
            BuilderMode::EnteringName => {
                self.render_name_input(f, chunks[1]);
            }
            BuilderMode::EnteringDescription => {
                self.render_description_input(f, chunks[1]);
            }
            BuilderMode::Confirming => {
                self.render_confirmation(f, chunks[1]);
            }
        }

        // Instructions
        self.render_instructions(f, chunks[2]);
    }

    fn render_token_selection(&mut self, f: &mut Frame, area: ratatui::prelude::Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(area);

        // Token list
        let items: Vec<ListItem> = self
            .filtered_indices
            .iter()
            .map(|&idx| {
                let token = &self.available_tokens[idx];
                let is_selected = self.selected_tokens.contains(&token.token_id);

                let checkbox = if is_selected { "[✓]" } else { "[ ]" };
                let icon = token.dataset_type.icon();

                let mut spans = vec![
                    Span::styled(
                        checkbox,
                        if is_selected {
                            Style::default().fg(Color::Green)
                        } else {
                            Style::default().fg(Color::Gray)
                        },
                    ),
                    Span::raw(" "),
                    Span::raw(icon),
                    Span::raw(" "),
                ];

                // Truncate token ID for display
                let token_display = if token.token_id.len() > 20 {
                    format!(
                        "{}...{}",
                        &token.token_id[..10],
                        &token.token_id[token.token_id.len() - 10..]
                    )
                } else {
                    token.token_id.clone()
                };

                spans.push(Span::raw(token_display));

                if let Some(market) = &token.market_info {
                    let market_display = if market.len() > 30 {
                        format!(" - {}...", &market[..27])
                    } else {
                        format!(" - {}", market)
                    };
                    spans.push(Span::styled(
                        market_display,
                        Style::default().fg(Color::Gray),
                    ));
                }

                ListItem::new(Line::from(spans))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title(format!(
                        "Available Tokens ({} found, {} selected)",
                        self.filtered_indices.len(),
                        self.selected_tokens.len()
                    ))
                    .borders(Borders::ALL),
            )
            .highlight_style(Style::default().bg(Color::DarkGray));

        f.render_stateful_widget(list, chunks[0], &mut self.list_state);

        // Selection summary
        let summary_text = if self.selected_tokens.is_empty() {
            vec![
                Line::from("No tokens selected"),
                Line::from(""),
                Line::from("Use ↑/↓ to navigate"),
                Line::from("Press Enter or Space to select"),
                Line::from("Press 's' to save selection"),
            ]
        } else {
            let mut lines = vec![
                Line::from(format!("Selected: {} tokens", self.selected_tokens.len())),
                Line::from(""),
            ];

            // Show first few selected tokens
            for (_i, token) in self.selected_tokens.iter().take(5).enumerate() {
                let display = if token.len() > 25 {
                    format!("{}...{}", &token[..10], &token[token.len() - 10..])
                } else {
                    token.clone()
                };
                lines.push(Line::from(format!("• {}", display)));
            }

            if self.selected_tokens.len() > 5 {
                lines.push(Line::from(format!(
                    "... and {} more",
                    self.selected_tokens.len() - 5
                )));
            }

            lines
        };

        let summary = Paragraph::new(summary_text)
            .block(
                Block::default()
                    .title("Selection Summary")
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: true });

        f.render_widget(summary, chunks[1]);

        // Search filter display
        if !self.search_filter.is_empty() {
            let search_display = format!("Filter: {}", self.search_filter);
            let search_area = ratatui::prelude::Rect {
                x: chunks[0].x + 1,
                y: chunks[0].y + chunks[0].height - 2,
                width: search_display.len() as u16 + 2,
                height: 1,
            };

            let search = Paragraph::new(search_display).style(Style::default().fg(Color::Yellow));
            f.render_widget(search, search_area);
        }
    }

    fn render_name_input(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        let popup_area = Self::centered_rect(60, 20, area);

        f.render_widget(Clear, popup_area);

        let block = Block::default()
            .title("Enter Selection Name")
            .borders(Borders::ALL)
            .border_type(BorderType::Double);

        let inner = block.inner(popup_area);
        f.render_widget(block, popup_area);

        let input = Paragraph::new(self.input_buffer.as_str())
            .style(Style::default().fg(Color::Yellow))
            .block(Block::default().borders(Borders::ALL));

        f.render_widget(input, inner);
    }

    fn render_description_input(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        let popup_area = Self::centered_rect(70, 25, area);

        f.render_widget(Clear, popup_area);

        let block = Block::default()
            .title("Enter Description (optional)")
            .borders(Borders::ALL)
            .border_type(BorderType::Double);

        let inner = block.inner(popup_area);
        f.render_widget(block, popup_area);

        let input = Paragraph::new(self.input_buffer.as_str())
            .style(Style::default().fg(Color::Cyan))
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::ALL));

        f.render_widget(input, inner);
    }

    fn render_confirmation(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        let popup_area = Self::centered_rect(60, 30, area);

        f.render_widget(Clear, popup_area);

        let block = Block::default()
            .title("Confirm Selection")
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(Color::Green));

        let inner = block.inner(popup_area);
        f.render_widget(block, popup_area);

        let mut lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::raw("Name: "),
                Span::styled(
                    &self.name,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
        ];

        if !self.description.is_empty() {
            lines.push(Line::from(vec![
                Span::raw("Description: "),
                Span::styled(&self.description, Style::default().fg(Color::Cyan)),
            ]));
            lines.push(Line::from(""));
        }

        lines.push(Line::from(vec![
            Span::raw("Tokens: "),
            Span::styled(
                format!("{} selected", self.selected_tokens.len()),
                Style::default().fg(Color::Green),
            ),
        ]));

        lines.push(Line::from(""));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Press 'y' to save, 'n' to go back",
            Style::default().fg(Color::Gray),
        )));

        let confirm = Paragraph::new(lines).alignment(Alignment::Center);

        f.render_widget(confirm, inner);
    }

    fn render_instructions(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        let instructions = match self.mode {
            BuilderMode::SelectingTokens => {
                vec![
                    Line::from(vec![
                        Span::raw("↑/↓: Navigate | "),
                        Span::raw("Enter/Space: Toggle | "),
                        Span::raw("a: Select all | "),
                        Span::raw("n: Clear all | "),
                        Span::raw("s: Save | "),
                        Span::raw("q: Cancel"),
                    ]),
                    Line::from(vec![
                        Span::raw("Type to filter | "),
                        Span::raw("/: Clear filter | "),
                        Span::raw("PgUp/PgDn: Fast scroll"),
                    ]),
                ]
            }
            BuilderMode::EnteringName => {
                vec![
                    Line::from("Enter a name for this selection (alphanumeric, -, _)"),
                    Line::from("Press Enter to continue, Esc to go back"),
                ]
            }
            BuilderMode::EnteringDescription => {
                vec![
                    Line::from("Enter an optional description for this selection"),
                    Line::from("Press Enter to continue, Esc to go back"),
                ]
            }
            BuilderMode::Confirming => {
                vec![
                    Line::from("Review your selection"),
                    Line::from("Press 'y' to save, 'n' to go back"),
                ]
            }
        };

        let instructions_widget = Paragraph::new(instructions)
            .style(Style::default().fg(Color::Gray))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            );

        f.render_widget(instructions_widget, area);
    }

    fn centered_rect(
        percent_x: u16,
        percent_y: u16,
        r: ratatui::prelude::Rect,
    ) -> ratatui::prelude::Rect {
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
}
