//! Interactive TUI for dataset management

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;

use super::{DatasetInfo, DatasetManager, DatasetManagerConfig, DatasetSummary};

/// Dataset management TUI application state
pub struct DatasetTui {
    /// Dataset manager
    manager: DatasetManager,
    /// Current selection in the list
    list_state: ListState,
    /// Whether to show dataset details
    show_details: bool,
    /// Current view mode
    view_mode: ViewMode,
    /// Whether the app should quit
    should_quit: bool,
    /// Datasets marked for deletion
    marked_for_deletion: Vec<String>,
    /// Current operation result message
    status_message: Option<StatusMessage>,
}

/// Different view modes for the TUI
#[derive(Debug, Clone, PartialEq)]
pub enum ViewMode {
    /// List all datasets
    List,
    /// Show summary statistics
    Summary,
    /// Confirmation dialog for deletion
    DeleteConfirmation,
}

/// Status message for user feedback
#[derive(Debug, Clone)]
pub struct StatusMessage {
    pub text: String,
    pub is_error: bool,
}

impl DatasetTui {
    /// Create a new dataset TUI application
    pub fn new(config: DatasetManagerConfig) -> Result<Self> {
        let mut manager = DatasetManager::new(config);
        manager.scan_datasets()?;

        let mut app = Self {
            manager,
            list_state: ListState::default(),
            show_details: false,
            view_mode: ViewMode::List,
            should_quit: false,
            marked_for_deletion: Vec::new(),
            status_message: None,
        };

        // Select first dataset if available
        if !app.manager.get_datasets().is_empty() {
            app.list_state.select(Some(0));
        }

        Ok(app)
    }

    /// Get the currently selected dataset
    fn selected_dataset(&self) -> Option<&DatasetInfo> {
        if let Some(selected) = self.list_state.selected() {
            self.manager.get_datasets().get(selected)
        } else {
            None
        }
    }

    /// Handle key events
    fn handle_key_event(&mut self, key: KeyCode) {
        match self.view_mode {
            ViewMode::List => self.handle_list_keys(key),
            ViewMode::Summary => self.handle_summary_keys(key),
            ViewMode::DeleteConfirmation => self.handle_delete_confirmation_keys(key),
        }
    }

    /// Handle keys in list view mode
    fn handle_list_keys(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.next_dataset();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.previous_dataset();
            }
            KeyCode::Char(' ') | KeyCode::Tab => {
                self.show_details = !self.show_details;
            }
            KeyCode::Char('r') => {
                self.refresh_datasets();
            }
            KeyCode::Char('s') => {
                self.view_mode = ViewMode::Summary;
            }
            KeyCode::Char('d') => {
                if let Some(dataset) = self.selected_dataset() {
                    self.toggle_mark_for_deletion(dataset.name.clone());
                }
            }
            KeyCode::Char('D') => {
                if !self.marked_for_deletion.is_empty() {
                    self.view_mode = ViewMode::DeleteConfirmation;
                }
            }
            KeyCode::Char('c') => {
                self.marked_for_deletion.clear();
                self.set_status_message("Cleared deletion marks", false);
            }
            _ => {}
        }
    }

    /// Handle keys in summary view mode
    fn handle_summary_keys(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.view_mode = ViewMode::List;
            }
            KeyCode::Char('r') => {
                self.refresh_datasets();
            }
            _ => {}
        }
    }

    /// Handle keys in delete confirmation mode
    fn handle_delete_confirmation_keys(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.delete_marked_datasets();
                self.view_mode = ViewMode::List;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.view_mode = ViewMode::List;
            }
            _ => {}
        }
    }

    /// Move to next dataset
    fn next_dataset(&mut self) {
        let datasets = self.manager.get_datasets();
        if datasets.is_empty() {
            return;
        }

        let selected = self.list_state.selected().unwrap_or(0);
        let next = if selected >= datasets.len() - 1 {
            0
        } else {
            selected + 1
        };
        self.list_state.select(Some(next));
    }

    /// Move to previous dataset
    fn previous_dataset(&mut self) {
        let datasets = self.manager.get_datasets();
        if datasets.is_empty() {
            return;
        }

        let selected = self.list_state.selected().unwrap_or(0);
        let previous = if selected == 0 {
            datasets.len() - 1
        } else {
            selected - 1
        };
        self.list_state.select(Some(previous));
    }

    /// Refresh datasets by rescanning
    fn refresh_datasets(&mut self) {
        match self.manager.scan_datasets() {
            Ok(()) => {
                let count = self.manager.get_datasets().len();
                self.set_status_message(&format!("Refreshed: {} datasets found", count), false);

                // Reset selection if needed
                if self.list_state.selected().is_some() {
                    let datasets = self.manager.get_datasets();
                    if datasets.is_empty() {
                        self.list_state.select(None);
                    } else if self.list_state.selected().unwrap() >= datasets.len() {
                        self.list_state.select(Some(datasets.len() - 1));
                    }
                }
            }
            Err(e) => {
                self.set_status_message(&format!("Refresh failed: {}", e), true);
            }
        }
    }

    /// Toggle a dataset for deletion
    fn toggle_mark_for_deletion(&mut self, dataset_name: String) {
        if let Some(pos) = self
            .marked_for_deletion
            .iter()
            .position(|x| x == &dataset_name)
        {
            self.marked_for_deletion.remove(pos);
            self.set_status_message(&format!("Unmarked {} for deletion", dataset_name), false);
        } else {
            self.marked_for_deletion.push(dataset_name.clone());
            self.set_status_message(&format!("Marked {} for deletion", dataset_name), false);
        }
    }

    /// Delete all marked datasets
    fn delete_marked_datasets(&mut self) {
        if self.marked_for_deletion.is_empty() {
            return;
        }

        match self.manager.delete_datasets(&self.marked_for_deletion) {
            Ok(deleted) => {
                self.set_status_message(
                    &format!("Successfully deleted {} datasets", deleted.len()),
                    false,
                );
                self.marked_for_deletion.clear();

                // Reset selection if needed
                let datasets = self.manager.get_datasets();
                if datasets.is_empty() {
                    self.list_state.select(None);
                } else if let Some(selected) = self.list_state.selected() {
                    if selected >= datasets.len() {
                        self.list_state.select(Some(datasets.len() - 1));
                    }
                }
            }
            Err(e) => {
                self.set_status_message(&format!("Deletion failed: {}", e), true);
            }
        }
    }

    /// Set a status message
    fn set_status_message(&mut self, text: &str, is_error: bool) {
        self.status_message = Some(StatusMessage {
            text: text.to_string(),
            is_error,
        });
    }

    /// Run the TUI application
    pub async fn run(mut self) -> Result<()> {
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

    /// Main application loop
    async fn run_app(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        loop {
            terminal.draw(|f| self.ui(f))?;

            if self.should_quit {
                break;
            }

            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    self.handle_key_event(key.code);
                }
            }
        }

        Ok(())
    }

    /// Draw the UI
    fn ui(&mut self, f: &mut Frame) {
        match self.view_mode {
            ViewMode::List => self.draw_list_view(f),
            ViewMode::Summary => self.draw_summary_view(f),
            ViewMode::DeleteConfirmation => self.draw_delete_confirmation(f),
        }
    }

    /// Draw the list view
    fn draw_list_view(&mut self, f: &mut Frame) {
        let size = f.area();

        // Create layout
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(if self.show_details { 60 } else { 100 }),
                Constraint::Percentage(if self.show_details { 40 } else { 0 }),
            ])
            .split(size);

        // Main panel
        self.draw_dataset_list(f, chunks[0]);

        // Details panel (if enabled)
        if self.show_details {
            self.draw_dataset_details(f, chunks[1]);
        }

        // Help footer
        self.draw_help(f, size);

        // Status message
        if let Some(status) = &self.status_message {
            self.draw_status_message(f, size, status);
        }
    }

    /// Draw the dataset list
    fn draw_dataset_list(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
        let datasets = self.manager.get_datasets();
        let title = if datasets.is_empty() {
            "📊 No Datasets Found"
        } else {
            &format!(
                "📊 Datasets ({} total, {} marked)",
                datasets.len(),
                self.marked_for_deletion.len()
            )
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue));

        if datasets.is_empty() {
            let empty_msg = Paragraph::new(Text::from(vec![
                Line::from("No datasets found."),
                Line::from(""),
                Line::from("Run some pipelines to generate datasets."),
                Line::from(""),
                Line::from("Press 'q' to quit or 'r' to refresh."),
            ]))
            .block(block)
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Center);

            f.render_widget(empty_msg, area);
            return;
        }

        let items: Vec<ListItem> = datasets
            .iter()
            .map(|dataset| {
                let is_marked = self.marked_for_deletion.contains(&dataset.name);
                let mark_icon = if is_marked { "🗑️" } else { " " };

                let style = if is_marked {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::White)
                };

                let content = vec![
                    Line::from(vec![
                        Span::styled(mark_icon, Style::default().fg(Color::Red)),
                        Span::styled(dataset.status_icon(), Style::default()),
                        Span::styled(" ", Style::default()),
                        Span::styled(dataset.dataset_type.icon(), Style::default()),
                        Span::styled(" ", Style::default()),
                        Span::styled(&dataset.name, style.add_modifier(Modifier::BOLD)),
                        Span::styled(
                            format!(" ({})", dataset.formatted_size()),
                            Style::default().fg(Color::Yellow),
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled(
                            format!(
                                "{} • {} files • {}",
                                dataset.dataset_type.display_name(),
                                dataset.file_count,
                                dataset.age()
                            ),
                            Style::default().fg(Color::Gray),
                        ),
                    ]),
                ];

                ListItem::new(content)
            })
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(Style::default().bg(Color::Blue).fg(Color::Black))
            .highlight_symbol("▶ ");

        f.render_stateful_widget(list, area, &mut self.list_state);
    }

    /// Draw dataset details panel
    fn draw_dataset_details(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let block = Block::default()
            .title("📖 Dataset Details")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green));

        if let Some(dataset) = self.selected_dataset() {
            let mut content = vec![
                Line::from(vec![
                    Span::styled("Name: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::from(dataset.name.clone()),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Type: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled(dataset.dataset_type.icon(), Style::default()),
                    Span::from(" "),
                    Span::from(dataset.dataset_type.display_name()),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Size: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::from(dataset.formatted_size()),
                ]),
                Line::from(vec![
                    Span::styled("Files: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::from(dataset.file_count.to_string()),
                ]),
                Line::from(vec![
                    Span::styled("Created: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::from(dataset.age()),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Status: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled(dataset.status_icon(), Style::default()),
                    Span::from(
                        if matches!(dataset.health_status, super::DatasetHealthStatus::Healthy) {
                            " Complete"
                        } else {
                            " Incomplete"
                        },
                    ),
                ]),
            ];

            // Show command information
            if !dataset.command_info.detected_commands.is_empty() {
                content.push(Line::from(""));
                content.push(Line::from(vec![Span::styled(
                    "Commands: ",
                    Style::default().add_modifier(Modifier::BOLD),
                )]));
                for (i, command) in dataset.command_info.detected_commands.iter().enumerate() {
                    let prefix = if i == 0 { "  • " } else { "  • " };
                    content.push(Line::from(vec![
                        Span::styled(prefix, Style::default().fg(Color::Blue)),
                        Span::styled(&command.command, Style::default().fg(Color::Blue)),
                    ]));
                }
                content.push(Line::from(vec![
                    Span::styled("  Confidence: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        format!("{:.1}%", dataset.command_info.confidence * 100.0),
                        Style::default().fg(if dataset.command_info.confidence > 0.7 {
                            Color::Green
                        } else {
                            Color::Yellow
                        }),
                    ),
                ]));
            }

            if !dataset.warnings.is_empty() {
                content.push(Line::from(""));
                content.push(Line::from(vec![Span::styled(
                    "Warnings:",
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(Color::Yellow),
                )]));
                for warning in &dataset.warnings {
                    content.push(Line::from(vec![
                        Span::styled("  • ", Style::default().fg(Color::Yellow)),
                        Span::styled(&warning.message, Style::default().fg(Color::Yellow)),
                    ]));
                }
            }

            content.push(Line::from(""));
            content.push(Line::from(vec![Span::styled(
                "Path: ",
                Style::default().add_modifier(Modifier::BOLD),
            )]));
            content.push(Line::from(vec![Span::styled(
                dataset.path.to_string_lossy(),
                Style::default().fg(Color::Gray),
            )]));

            let details = Paragraph::new(content)
                .block(block)
                .wrap(Wrap { trim: true });

            f.render_widget(details, area);
        } else {
            let no_selection = Paragraph::new("No dataset selected")
                .block(block)
                .alignment(Alignment::Center);

            f.render_widget(no_selection, area);
        }
    }

    /// Draw summary view
    fn draw_summary_view(&self, f: &mut Frame) {
        let size = f.area();
        let summary = self.manager.get_summary();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(size);

        // Title
        let title = Paragraph::new("📊 Dataset Summary")
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center);
        f.render_widget(title, chunks[0]);

        // Summary content
        self.draw_summary_content(f, chunks[1], &summary);

        // Help footer
        self.draw_help(f, size);
    }

    /// Draw summary content
    fn draw_summary_content(
        &self,
        f: &mut Frame,
        area: ratatui::layout::Rect,
        summary: &DatasetSummary,
    ) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green));

        let mut content = vec![
            Line::from(vec![
                Span::styled(
                    "Total Datasets: ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    summary.total_datasets.to_string(),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    "Total Size: ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    summary.formatted_total_size(),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    "Total Files: ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    summary.total_files.to_string(),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    "Created Today: ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    summary.datasets_today.to_string(),
                    Style::default().fg(Color::Green),
                ),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "By Type:",
                Style::default().add_modifier(Modifier::BOLD),
            )]),
        ];

        for (dataset_type, count) in &summary.type_counts {
            content.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(dataset_type.icon(), Style::default()),
                Span::styled(" ", Style::default()),
                Span::styled(dataset_type.display_name(), Style::default()),
                Span::styled(": ", Style::default()),
                Span::styled(count.to_string(), Style::default().fg(Color::Yellow)),
            ]));
        }

        if let Some(last_scan) = summary.last_scan {
            content.push(Line::from(""));
            content.push(Line::from(vec![
                Span::styled("Last Scan: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    last_scan.format("%Y-%m-%d %H:%M:%S").to_string(),
                    Style::default().fg(Color::Gray),
                ),
            ]));
        }

        let summary_paragraph = Paragraph::new(content)
            .block(block)
            .wrap(Wrap { trim: true });

        f.render_widget(summary_paragraph, area);
    }

    /// Draw delete confirmation dialog
    fn draw_delete_confirmation(&self, f: &mut Frame) {
        let size = f.area();

        // Calculate popup area
        let popup_area = ratatui::layout::Rect {
            x: size.width / 4,
            y: size.height / 3,
            width: size.width / 2,
            height: size.height / 3,
        };

        // Clear the area
        f.render_widget(Clear, popup_area);

        let block = Block::default()
            .title("🗑️ Confirm Deletion")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red));

        let mut content = vec![
            Line::from(vec![
                Span::styled("Are you sure you want to delete ", Style::default()),
                Span::styled(
                    format!("{} dataset(s)", self.marked_for_deletion.len()),
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::styled("?", Style::default()),
            ]),
            Line::from(""),
            Line::from("This action cannot be undone!"),
            Line::from(""),
        ];

        for dataset_name in &self.marked_for_deletion {
            content.push(Line::from(vec![
                Span::styled("  • ", Style::default().fg(Color::Red)),
                Span::styled(dataset_name, Style::default()),
            ]));
        }

        content.push(Line::from(""));
        content.push(Line::from(vec![
            Span::styled("Press ", Style::default()),
            Span::styled(
                "Y",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to confirm, ", Style::default()),
            Span::styled(
                "N",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to cancel", Style::default()),
        ]));

        let confirmation = Paragraph::new(content)
            .block(block)
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Center);

        f.render_widget(confirmation, popup_area);
    }

    /// Draw help footer
    fn draw_help(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let help_area = ratatui::layout::Rect {
            x: area.x,
            y: area.height - 3,
            width: area.width,
            height: 3,
        };

        let help_text = match self.view_mode {
            ViewMode::List => {
                if self.manager.get_datasets().is_empty() {
                    "q: Quit • r: Refresh"
                } else {
                    "↑/↓ or j/k: Navigate • d: Mark for deletion • D: Delete marked • c: Clear marks • Space: Details • s: Summary • r: Refresh • q: Quit"
                }
            }
            ViewMode::Summary => "q/Esc: Back to list • r: Refresh",
            ViewMode::DeleteConfirmation => "Y: Confirm deletion • N/Esc: Cancel",
        };

        let help = Paragraph::new(help_text)
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(Color::Gray)),
            )
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });

        f.render_widget(Clear, help_area);
        f.render_widget(help, help_area);
    }

    /// Draw status message
    fn draw_status_message(
        &self,
        f: &mut Frame,
        area: ratatui::layout::Rect,
        status: &StatusMessage,
    ) {
        let status_area = ratatui::layout::Rect {
            x: area.x,
            y: area.height - 6,
            width: area.width,
            height: 3,
        };

        let style = if status.is_error {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::Green)
        };

        let icon = if status.is_error { "❌" } else { "✅" };

        let status_msg = Paragraph::new(format!("{} {}", icon, status.text))
            .block(Block::default().borders(Borders::ALL).border_style(style))
            .style(style)
            .alignment(Alignment::Center);

        f.render_widget(Clear, status_area);
        f.render_widget(status_msg, status_area);
    }
}
