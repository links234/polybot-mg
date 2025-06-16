//! Interactive TUI for pipeline selection

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
    widgets::{
        Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
    },
    Frame, Terminal,
};
use std::io;

use super::{Pipeline, PipelineConfig, PipelineRunner};

/// Pipeline TUI application state
pub struct PipelineTui {
    /// Available pipelines
    pipelines: Vec<PipelineInfo>,
    /// Current selection in the list
    list_state: ListState,
    /// Whether to show pipeline details
    show_details: bool,
    /// Configuration
    config: PipelineConfig,
    /// Whether the app should quit
    should_quit: bool,
    /// Selected pipeline to run (when user presses Enter)
    selected_pipeline: Option<String>,
}

/// Information about a pipeline for display
#[derive(Debug, Clone)]
struct PipelineInfo {
    name: String,
    display_name: String,
    description: String,
    steps: usize,
    path: String,
    valid: bool,
    error: Option<String>,
}

impl PipelineTui {
    /// Create a new TUI application
    pub fn new(config: PipelineConfig) -> Result<Self> {
        let mut app = Self {
            pipelines: Vec::new(),
            list_state: ListState::default(),
            show_details: false,
            config,
            should_quit: false,
            selected_pipeline: None,
        };

        app.load_pipelines()?;
        
        // Select first pipeline if available
        if !app.pipelines.is_empty() {
            app.list_state.select(Some(0));
        }

        Ok(app)
    }

    /// Load available pipelines
    fn load_pipelines(&mut self) -> Result<()> {
        let pipeline_names = PipelineRunner::list_pipelines(&self.config.pipelines_dir)?;
        
        self.pipelines = pipeline_names
            .into_iter()
            .map(|name| {
                let path = self.config.pipeline_path(&name);
                
                match Pipeline::from_file(&path) {
                    Ok(pipeline) => PipelineInfo {
                        name: name.clone(),
                        display_name: pipeline.name,
                        description: pipeline.description,
                        steps: pipeline.steps.len(),
                        path,
                        valid: true,
                        error: None,
                    },
                    Err(e) => PipelineInfo {
                        name: name.clone(),
                        display_name: name.clone(),
                        description: "Invalid pipeline".to_string(),
                        steps: 0,
                        path,
                        valid: false,
                        error: Some(e.to_string()),
                    },
                }
            })
            .collect();

        Ok(())
    }

    /// Get the currently selected pipeline
    fn selected_pipeline_info(&self) -> Option<&PipelineInfo> {
        if let Some(selected) = self.list_state.selected() {
            self.pipelines.get(selected)
        } else {
            None
        }
    }

    /// Handle key events
    fn handle_key_event(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.next_pipeline();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.previous_pipeline();
            }
            KeyCode::Enter => {
                if let Some(pipeline) = self.selected_pipeline_info() {
                    if pipeline.valid {
                        self.selected_pipeline = Some(pipeline.name.clone());
                        self.should_quit = true;
                    }
                }
            }
            KeyCode::Char(' ') | KeyCode::Tab => {
                self.show_details = !self.show_details;
            }
            KeyCode::Char('r') => {
                // Refresh pipeline list
                let _ = self.load_pipelines();
            }
            _ => {}
        }
    }

    /// Move to next pipeline
    fn next_pipeline(&mut self) {
        if self.pipelines.is_empty() {
            return;
        }

        let selected = self.list_state.selected().unwrap_or(0);
        let next = if selected >= self.pipelines.len() - 1 {
            0
        } else {
            selected + 1
        };
        self.list_state.select(Some(next));
    }

    /// Move to previous pipeline
    fn previous_pipeline(&mut self) {
        if self.pipelines.is_empty() {
            return;
        }

        let selected = self.list_state.selected().unwrap_or(0);
        let previous = if selected == 0 {
            self.pipelines.len() - 1
        } else {
            selected - 1
        };
        self.list_state.select(Some(previous));
    }

    /// Run the TUI application
    pub async fn run(mut self) -> Result<Option<String>> {
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

        result?;
        Ok(self.selected_pipeline)
    }

    /// Main application loop
    async fn run_app(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
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
        let size = f.area();

        // Create layout
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(if self.show_details { 50 } else { 100 }),
                Constraint::Percentage(if self.show_details { 50 } else { 0 }),
            ])
            .split(size);

        // Main panel
        self.draw_pipeline_list(f, chunks[0]);

        // Details panel (if enabled)
        if self.show_details {
            self.draw_pipeline_details(f, chunks[1]);
        }

        // Help footer
        self.draw_help(f, size);
    }

    /// Draw the pipeline list
    fn draw_pipeline_list(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
        let title = if self.pipelines.is_empty() {
            "📋 No Pipelines Found"
        } else {
            "📋 Available Pipelines"
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue));

        if self.pipelines.is_empty() {
            let empty_msg = Paragraph::new(Text::from(vec![
                Line::from("No pipeline files found."),
                Line::from(""),
                Line::from("Create YAML files in the pipelines/ directory to get started."),
                Line::from(""),
                Line::from("Press 'q' to quit or 'r' to refresh."),
            ]))
            .block(block)
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Center);

            f.render_widget(empty_msg, area);
            return;
        }

        let items: Vec<ListItem> = self
            .pipelines
            .iter()
            .map(|pipeline| {
                let style = if pipeline.valid {
                    Style::default().fg(Color::White)
                } else {
                    Style::default().fg(Color::Red)
                };

                let icon = if pipeline.valid { "📄" } else { "❌" };
                let steps_text = if pipeline.valid {
                    format!(" ({} steps)", pipeline.steps)
                } else {
                    " (invalid)".to_string()
                };

                let content = vec![
                    Line::from(vec![
                        Span::styled(format!("{} {}", icon, pipeline.display_name), style.add_modifier(Modifier::BOLD)),
                        Span::styled(steps_text, Style::default().fg(Color::Yellow)),
                    ]),
                    Line::from(Span::styled(
                        pipeline.description.clone(),
                        Style::default().fg(Color::Gray),
                    )),
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

    /// Draw pipeline details panel
    fn draw_pipeline_details(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let block = Block::default()
            .title("📖 Pipeline Details")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green));

        if let Some(pipeline) = self.selected_pipeline_info() {
            let content = if pipeline.valid {
                vec![
                    Line::from(vec![
                        Span::styled("Name: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::from(pipeline.display_name.clone()),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Description: ", Style::default().add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(pipeline.description.clone()),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Steps: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::from(pipeline.steps.to_string()),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Path: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::styled(pipeline.path.clone(), Style::default().fg(Color::Gray)),
                    ]),
                ]
            } else {
                vec![
                    Line::from(vec![
                        Span::styled("❌ Invalid Pipeline", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Error: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::styled(
                            pipeline.error.as_deref().unwrap_or("Unknown error"),
                            Style::default().fg(Color::Red),
                        ),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Path: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::styled(pipeline.path.clone(), Style::default().fg(Color::Gray)),
                    ]),
                ]
            };

            let details = Paragraph::new(content)
                .block(block)
                .wrap(Wrap { trim: true });

            f.render_widget(details, area);
        } else {
            let no_selection = Paragraph::new("No pipeline selected")
                .block(block)
                .alignment(Alignment::Center);

            f.render_widget(no_selection, area);
        }
    }

    /// Draw help footer
    fn draw_help(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let help_area = ratatui::layout::Rect {
            x: area.x,
            y: area.height - 3,
            width: area.width,
            height: 3,
        };

        let help_text = if self.pipelines.is_empty() {
            "Press 'q' to quit • 'r' to refresh"
        } else {
            "↑/↓ or j/k: Navigate • Enter: Run Pipeline • Space/Tab: Toggle Details • r: Refresh • q: Quit"
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

        // Clear the area first
        f.render_widget(Clear, help_area);
        f.render_widget(help, help_area);
    }
} 