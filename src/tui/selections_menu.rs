//! Interactive TUI menu for managing token selections

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Alignment},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Wrap, Clear,
    },
    Frame, Terminal,
};
use std::io::{self, Stdout};

use crate::datasets::{SelectionManager, TokenSelection};
use crate::data_paths::DataPaths;
use crate::tui::SelectionBuilder;
use super::selection_builder::SelectionBuilderResult;

#[allow(dead_code)]
pub enum SelectionsMenuResult {
    Exit,
    CreateNew(SelectionBuilderResult),
}

#[allow(dead_code)]
pub struct SelectionsMenu {
    /// All available selections
    selections: Vec<TokenSelection>,
    /// UI state for selection list
    list_state: ListState,
    /// Current mode
    mode: _MenuMode,
    /// Selection manager
    manager: SelectionManager,
    /// Currently selected selection for viewing/editing
    current_selection: Option<TokenSelection>,
    /// Confirmation state
    confirm_delete: Option<String>,
}

#[derive(Debug, PartialEq)]
enum _MenuMode {
    SelectionList,
    ViewingSelection,
    ConfirmDelete,
}

#[allow(dead_code)]
impl SelectionsMenu {
    pub async fn run(data_paths: &DataPaths) -> Result<SelectionsMenuResult> {
        // Setup terminal
        enable_raw_mode()?;
        io::stdout().execute(EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(io::stdout());
        let mut terminal = Terminal::new(backend)?;
        
        // Create menu
        let manager = SelectionManager::new(&data_paths.data());
        let mut menu = Self {
            selections: Vec::new(),
            list_state: ListState::default(),
            mode: _MenuMode::SelectionList,
            manager,
            current_selection: None,
            confirm_delete: None,
        };
        
        // Load selections
        menu.refresh_selections()?;
        
        let result = menu.run_loop(&mut terminal).await?;
        
        // Cleanup terminal
        disable_raw_mode()?;
        io::stdout().execute(LeaveAlternateScreen)?;
        
        Ok(result)
    }
    
    fn refresh_selections(&mut self) -> Result<()> {
        let selection_names = self.manager.list_selections()?;
        self.selections.clear();
        
        for name in selection_names {
            if let Ok(selection) = self.manager.load_selection(&name) {
                self.selections.push(selection);
            }
        }
        
        // Sort by modified date (newest first)
        self.selections.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
        
        // Select first item if available
        if !self.selections.is_empty() && self.list_state.selected().is_none() {
            self.list_state.select(Some(0));
        }
        
        Ok(())
    }
    
    async fn run_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<SelectionsMenuResult> {
        loop {
            terminal.draw(|f| self.render(f))?;
            
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match self.mode {
                        _MenuMode::SelectionList => {
                            match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => {
                                    return Ok(SelectionsMenuResult::Exit);
                                }
                                KeyCode::Char('n') | KeyCode::Char('c') => {
                                    // Create new selection
                                    terminal.clear()?;
                                    disable_raw_mode()?;
                                    io::stdout().execute(LeaveAlternateScreen)?;
                                    
                                    let result = SelectionBuilder::run(None).await?;
                                    
                                    // Re-enable terminal
                                    enable_raw_mode()?;
                                    io::stdout().execute(EnterAlternateScreen)?;
                                    
                                    if !result.cancelled {
                                        return Ok(SelectionsMenuResult::CreateNew(result));
                                    }
                                    
                                    // Refresh the list
                                    self.refresh_selections()?;
                                }
                                KeyCode::Enter | KeyCode::Char(' ') => {
                                    if let Some(selected) = self.list_state.selected() {
                                        if let Some(selection) = self.selections.get(selected).cloned() {
                                            self.current_selection = Some(selection);
                                            self.mode = _MenuMode::ViewingSelection;
                                        }
                                    }
                                }
                                KeyCode::Char('d') => {
                                    if let Some(selected) = self.list_state.selected() {
                                        if let Some(selection) = self.selections.get(selected) {
                                            self.confirm_delete = Some(selection.name.clone());
                                            self.mode = _MenuMode::ConfirmDelete;
                                        }
                                    }
                                }
                                KeyCode::Char('e') => {
                                    if let Some(selected) = self.list_state.selected() {
                                        if let Some(selection) = self.selections.get(selected) {
                                            // Edit selection - launch builder with existing name
                                            terminal.clear()?;
                                            disable_raw_mode()?;
                                            io::stdout().execute(LeaveAlternateScreen)?;
                                            
                                            let result = SelectionBuilder::run(Some(selection.name.clone())).await?;
                                            
                                            // Re-enable terminal
                                            enable_raw_mode()?;
                                            io::stdout().execute(EnterAlternateScreen)?;
                                            
                                            if !result.cancelled {
                                                return Ok(SelectionsMenuResult::CreateNew(result));
                                            }
                                            
                                            // Refresh the list
                                            self.refresh_selections()?;
                                        }
                                    }
                                }
                                KeyCode::Up => {
                                    self.move_selection(-1);
                                }
                                KeyCode::Down => {
                                    self.move_selection(1);
                                }
                                KeyCode::Char('r') => {
                                    // Refresh
                                    self.refresh_selections()?;
                                }
                                _ => {}
                            }
                        }
                        _MenuMode::ViewingSelection => {
                            match key.code {
                                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => {
                                    self.mode = _MenuMode::SelectionList;
                                    self.current_selection = None;
                                }
                                _ => {}
                            }
                        }
                        _MenuMode::ConfirmDelete => {
                            match key.code {
                                KeyCode::Char('y') => {
                                    if let Some(name) = &self.confirm_delete {
                                        self.manager.delete_selection(name)?;
                                        self.refresh_selections()?;
                                    }
                                    self.confirm_delete = None;
                                    self.mode = _MenuMode::SelectionList;
                                }
                                KeyCode::Char('n') | KeyCode::Esc => {
                                    self.confirm_delete = None;
                                    self.mode = _MenuMode::SelectionList;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }
    
    fn move_selection(&mut self, delta: i32) {
        if self.selections.is_empty() {
            return;
        }
        
        let current = self.list_state.selected().unwrap_or(0) as i32;
        let new = (current + delta)
            .max(0)
            .min(self.selections.len() as i32 - 1) as usize;
        
        self.list_state.select(Some(new));
    }
    
    fn render(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Header
                Constraint::Min(10),    // Main content
                Constraint::Length(3),  // Instructions
            ])
            .split(f.area());
        
        // Header
        let header = Paragraph::new("Token Selections Manager")
            .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            );
        f.render_widget(header, chunks[0]);
        
        // Main content
        match self.mode {
            _MenuMode::SelectionList => {
                self.render_selection_list(f, chunks[1]);
            }
            _MenuMode::ViewingSelection => {
                if let Some(selection) = &self.current_selection {
                    self.render_selection_details(f, chunks[1], selection);
                }
            }
            _MenuMode::ConfirmDelete => {
                self.render_selection_list(f, chunks[1]);
                self.render_delete_confirmation(f, chunks[1]);
            }
        }
        
        // Instructions
        self.render_instructions(f, chunks[2]);
    }
    
    fn render_selection_list(&mut self, f: &mut Frame, area: ratatui::prelude::Rect) {
        if self.selections.is_empty() {
            let empty_msg = vec![
                Line::from(""),
                Line::from("No selections found."),
                Line::from(""),
                Line::from("Press 'n' to create a new selection."),
            ];
            
            let paragraph = Paragraph::new(empty_msg)
                .style(Style::default().fg(Color::Gray))
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .title("Selections")
                        .borders(Borders::ALL),
                );
            
            f.render_widget(paragraph, area);
            return;
        }
        
        let items: Vec<ListItem> = self.selections
            .iter()
            .map(|selection| {
                let mut spans = vec![
                    Span::styled("⭐ ", Style::default().fg(Color::Yellow)),
                    Span::styled(&selection.name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                ];
                
                if let Some(desc) = &selection.description {
                    let desc_preview = if desc.len() > 40 {
                        format!(" - {}...", &desc[..37])
                    } else {
                        format!(" - {}", desc)
                    };
                    spans.push(Span::styled(desc_preview, Style::default().fg(Color::Gray)));
                }
                
                spans.push(Span::styled(
                    format!(" ({} tokens)", selection.tokens.len()),
                    Style::default().fg(Color::Cyan),
                ));
                
                ListItem::new(Line::from(spans))
            })
            .collect();
        
        let list = List::new(items)
            .block(Block::default()
                .title(format!("Selections ({})", self.selections.len()))
                .borders(Borders::ALL))
            .highlight_style(Style::default().bg(Color::DarkGray));
        
        f.render_stateful_widget(list, area, &mut self.list_state);
    }
    
    fn render_selection_details(&self, f: &mut Frame, area: ratatui::prelude::Rect, selection: &TokenSelection) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(7),   // Basic info
                Constraint::Min(5),      // Tokens list
                Constraint::Length(3),   // Stats
            ])
            .split(area);
        
        // Basic info
        let mut info_lines = vec![
            Line::from(vec![
                Span::raw("Name: "),
                Span::styled(&selection.name, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ]),
        ];
        
        if let Some(desc) = &selection.description {
            info_lines.push(Line::from(vec![
                Span::raw("Description: "),
                Span::styled(desc, Style::default().fg(Color::Cyan)),
            ]));
        }
        
        info_lines.push(Line::from(vec![
            Span::raw("Created: "),
            Span::raw(selection.created_at.format("%Y-%m-%d %H:%M UTC").to_string()),
        ]));
        
        info_lines.push(Line::from(vec![
            Span::raw("Modified: "),
            Span::raw(selection.modified_at.format("%Y-%m-%d %H:%M UTC").to_string()),
        ]));
        
        if !selection.tags.is_empty() {
            info_lines.push(Line::from(vec![
                Span::raw("Tags: "),
                Span::raw(selection.tags.join(", ")),
            ]));
        }
        
        let info = Paragraph::new(info_lines)
            .block(Block::default()
                .title("Selection Details")
                .borders(Borders::ALL))
            .wrap(Wrap { trim: true });
        
        f.render_widget(info, chunks[0]);
        
        // Tokens list
        let token_items: Vec<ListItem> = selection.tokens
            .iter()
            .enumerate()
            .map(|(i, token)| {
                let mut spans = vec![
                    Span::styled(format!("{:3}. ", i + 1), Style::default().fg(Color::DarkGray)),
                    Span::raw(&token.token_id),
                ];
                
                if let Some(name) = &token.name {
                    spans.push(Span::styled(format!(" - {}", name), Style::default().fg(Color::Gray)));
                }
                
                ListItem::new(Line::from(spans))
            })
            .collect();
        
        let tokens_list = List::new(token_items)
            .block(Block::default()
                .title(format!("Tokens ({})", selection.tokens.len()))
                .borders(Borders::ALL));
        
        f.render_widget(tokens_list, chunks[1]);
        
        // Stats
        let stats = Paragraph::new(vec![
            Line::from(format!("Total tokens: {}", selection.tokens.len())),
        ])
        .block(Block::default()
            .borders(Borders::ALL));
        
        f.render_widget(stats, chunks[2]);
    }
    
    fn render_delete_confirmation(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        let popup_area = Self::centered_rect(50, 20, area);
        
        f.render_widget(Clear, popup_area);
        
        let block = Block::default()
            .title("Confirm Delete")
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(Color::Red));
        
        let inner = block.inner(popup_area);
        f.render_widget(block, popup_area);
        
        if let Some(name) = &self.confirm_delete {
            let confirm_text = vec![
                Line::from(""),
                Line::from(vec![
                    Span::raw("Delete selection '"),
                    Span::styled(name, Style::default().fg(Color::Yellow)),
                    Span::raw("'?"),
                ]),
                Line::from(""),
                Line::from("This action cannot be undone."),
                Line::from(""),
                Line::from("Press 'y' to confirm, 'n' to cancel"),
            ];
            
            let confirm = Paragraph::new(confirm_text)
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::White));
            
            f.render_widget(confirm, inner);
        }
    }
    
    fn render_instructions(&self, f: &mut Frame, area: ratatui::prelude::Rect) {
        let instructions = match self.mode {
            _MenuMode::SelectionList => {
                vec![
                    Line::from(vec![
                        Span::raw("n: New | "),
                        Span::raw("Enter: View | "),
                        Span::raw("e: Edit | "),
                        Span::raw("d: Delete | "),
                        Span::raw("r: Refresh | "),
                        Span::raw("q: Quit"),
                    ]),
                ]
            }
            _MenuMode::ViewingSelection => {
                vec![
                    Line::from("Press Enter or Esc to go back"),
                ]
            }
            _MenuMode::ConfirmDelete => {
                vec![
                    Line::from("Press 'y' to confirm delete, 'n' to cancel"),
                ]
            }
        };
        
        let instructions_widget = Paragraph::new(instructions)
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            );
        
        f.render_widget(instructions_widget, area);
    }
    
    fn centered_rect(percent_x: u16, percent_y: u16, r: ratatui::prelude::Rect) -> ratatui::prelude::Rect {
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