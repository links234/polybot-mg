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
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::{
    fs,
    io,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tracing::error;

use crate::data_paths::DataPaths;
use crate::cli::commands::index::{IndexArgs, IndexCommand};

pub struct IndexTui {
    data_paths: DataPaths,
    chunk_files: Vec<ChunkFileInfo>,
    selected_files: Vec<bool>,
    list_state: ListState,
    show_help: bool,
    status_message: Option<String>,
    last_status_time: Option<Instant>,
    // Indexing progress state
    is_indexing: bool,
    indexing_progress: Arc<Mutex<crate::tui::IndexingProgress>>,
    progress_receiver: Option<mpsc::UnboundedReceiver<crate::tui::ProgressUpdate>>,
}

#[derive(Debug, Clone)]
struct ChunkFileInfo {
    path: PathBuf,
    name: String,
    size_mb: f64,
    market_count_estimate: usize,
}

impl IndexTui {
    pub fn new(data_paths: DataPaths) -> Result<Self> {
        let chunk_files = match Self::discover_chunk_files(&data_paths) {
            Ok(files) => files,
            Err(e) => {
                error!("Failed to discover chunk files: {}", e);
                Vec::new()
            }
        };
        
        let selected_files = vec![false; chunk_files.len()];
        let mut list_state = ListState::default();
        if !chunk_files.is_empty() {
            list_state.select(Some(0));
        }

        let mut tui = Self {
            data_paths,
            chunk_files,
            selected_files,
            list_state,
            show_help: false,
            status_message: None,
            last_status_time: None,
            is_indexing: false,
            indexing_progress: Arc::new(Mutex::new(crate::tui::IndexingProgress::default())),
            progress_receiver: None,
        };
        
        // Set initial status message
        if tui.chunk_files.is_empty() {
            let datasets_dir = tui.data_paths.datasets();
            if !datasets_dir.exists() {
                tui.set_status_message(format!("⚠️ Datasets directory does not exist: {}", datasets_dir.display()));
            } else {
                tui.set_status_message("⚠️ No market data files found. Press 'r' to refresh.".to_string());
            }
        } else {
            tui.set_status_message(format!("✅ Found {} market data files", tui.chunk_files.len()));
        }

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
        loop {
            terminal.draw(|f| self.ui(f))?;

            // Process progress updates if indexing
            let updates_to_process = if let Some(receiver) = &mut self.progress_receiver {
                let mut updates = Vec::new();
                while let Ok(update) = receiver.try_recv() {
                    updates.push(update);
                }
                updates
            } else {
                Vec::new()
            };
            
            for update in updates_to_process {
                self.handle_progress_update(update);
            }

            // Clear old status messages
            if let Some(last_time) = self.last_status_time {
                if last_time.elapsed() > Duration::from_secs(3) && !self.is_indexing {
                    self.status_message = None;
                    self.last_status_time = None;
                }
            }

            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        // Block most inputs during indexing
                        if self.is_indexing {
                            match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => {
                                    // TODO: Implement cancellation
                                    self.set_status_message("⚠️ Indexing in progress, cannot quit yet".to_string());
                                }
                                _ => {}
                            }
                        } else {
                            match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                                KeyCode::Char('h') | KeyCode::F(1) => {
                                    self.show_help = !self.show_help;
                                }
                                KeyCode::Up | KeyCode::Char('k') => {
                                    self.previous_item();
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    self.next_item();
                                }
                                KeyCode::Char(' ') => {
                                    self.toggle_selection();
                                }
                                KeyCode::Char('a') => {
                                    self.select_all();
                                }
                                KeyCode::Char('n') => {
                                    self.select_none();
                                }
                                KeyCode::Enter => {
                                    if let Err(e) = self.start_indexing().await {
                                        error!("Indexing failed: {}", e);
                                        self.set_status_message(format!("❌ Indexing failed: {}", e));
                                    }
                                }
                                KeyCode::Char('r') => {
                                    if let Err(e) = self.refresh_file_list() {
                                        error!("Failed to refresh file list: {}", e);
                                        self.set_status_message(format!("❌ Refresh failed: {}", e));
                                    } else {
                                        self.set_status_message("🔄 File list refreshed".to_string());
                                    }
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
        if self.is_indexing {
            self.render_indexing_progress(f);
        } else {
            self.render_file_selection(f);
        }
    }

    fn render_file_selection(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Min(10),   // Main content
                Constraint::Length(4), // Instructions
                Constraint::Length(3), // Status
            ])
            .split(f.area());

        // Title
        let title = Paragraph::new("🗄️ RocksDB Market Data Indexer")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(title, chunks[0]);

        // Main content
        if self.chunk_files.is_empty() {
            let empty_msg = Paragraph::new(vec![
                Line::from(""),
                Line::from("No market data files found in datasets directory."),
                Line::from(""),
                Line::from("Looking for:"),
                Line::from("  • markets_chunk_*.json (raw market chunks)"),
                Line::from("  • markets.json (analyzed market data)"),
                Line::from(""),
                Line::from(format!("Search path: {}", self.data_paths.datasets().display())),
                Line::from(""),
                Line::from("Run one of these commands to generate data:"),
                Line::from("  • polybot fetch-all-markets"),
                Line::from("  • polybot analyze --input raw_markets"),
            ])
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::ALL).title("No Data Found"));
            f.render_widget(empty_msg, chunks[1]);
        } else {
            self.render_file_list(f, chunks[1]);
        }

        // Instructions
        let instructions = if self.chunk_files.is_empty() {
            Paragraph::new("Press 'q' to quit, 'r' to refresh")
        } else {
            Paragraph::new(vec![
                Line::from(vec![
                    Span::raw("↑/↓: Navigate  "),
                    Span::raw("Space: Toggle  "),
                    Span::raw("a: Select All  "),
                    Span::raw("n: Select None"),
                ]),
                Line::from(vec![
                    Span::raw("Enter: Start Index  "),
                    Span::raw("r: Refresh  "),
                    Span::raw("h: Help  "),
                    Span::raw("q: Quit"),
                ]),
            ])
        };

        let instructions_widget = instructions
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title("Controls"));
        f.render_widget(instructions_widget, chunks[2]);

        // Status
        let status_text = if let Some(ref msg) = self.status_message {
            msg.clone()
        } else if !self.chunk_files.is_empty() {
            let selected_count = self.selected_files.iter().filter(|&&x| x).count();
            format!(
                "📊 {} files available, {} selected",
                self.chunk_files.len(),
                selected_count
            )
        } else {
            "Ready".to_string()
        };

        let status = Paragraph::new(status_text)
            .style(Style::default().fg(Color::Green))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(status, chunks[3]);

        // Help popup
        if self.show_help {
            self.render_help_popup(f);
        }
    }

    fn render_indexing_progress(&mut self, f: &mut Frame) {
        let progress = self.indexing_progress.lock().unwrap();
        
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Title
                Constraint::Length(10), // Progress bars
                Constraint::Min(8),     // Event log
                Constraint::Length(3),  // Stats
            ])
            .split(f.area());

        // Title
        let title = Paragraph::new("🗄️ RocksDB Indexing Progress")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(title, chunks[0]);

        // Progress bars section
        let progress_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Overall progress
                Constraint::Length(3), // File progress
                Constraint::Length(3), // Current phase
            ])
            .split(chunks[1]);

        // Overall progress
        let overall_percent = if progress.total_files > 0 {
            (progress.current_file as f64 / progress.total_files as f64 * 100.0) as u16
        } else {
            0
        };

        let overall_gauge = Gauge::default()
            .block(Block::default().title("Overall Progress").borders(Borders::ALL))
            .gauge_style(Style::default().fg(Color::Green).bg(Color::Black))
            .percent(overall_percent)
            .label(format!(
                "File {}/{} - {} markets indexed",
                progress.current_file,
                progress.total_files,
                progress.total_markets_indexed
            ));
        f.render_widget(overall_gauge, progress_chunks[0]);

        // File progress
        let file_percent = if progress.markets_in_file > 0 {
            (progress.markets_processed as f64 / progress.markets_in_file as f64 * 100.0) as u16
        } else {
            0
        };

        let file_label = if !progress.current_file_name.is_empty() {
            format!(
                "{} - {}/{} markets",
                progress.current_file_name,
                progress.markets_processed,
                progress.markets_in_file
            )
        } else {
            "Waiting...".to_string()
        };

        let file_gauge = Gauge::default()
            .block(Block::default().title("Current File").borders(Borders::ALL))
            .gauge_style(Style::default().fg(Color::Yellow).bg(Color::Black))
            .percent(file_percent)
            .label(file_label);
        f.render_widget(file_gauge, progress_chunks[1]);

        // Current phase
        let (phase_label, phase_color) = match &progress.phase {
            crate::tui::IndexingPhase::Starting => ("Starting...".to_string(), Color::Gray),
            crate::tui::IndexingPhase::ProcessingFiles => ("Processing market files".to_string(), Color::Yellow),
            crate::tui::IndexingPhase::IndexingConditions => {
                (format!("Indexing {} conditions", progress.total_conditions), Color::Cyan)
            },
            crate::tui::IndexingPhase::IndexingTokens => {
                (format!("Indexing {} tokens", progress.total_tokens), Color::Magenta)
            },
            crate::tui::IndexingPhase::Finalizing => ("Finalizing database".to_string(), Color::Blue),
            crate::tui::IndexingPhase::Completed => ("✅ Indexing completed!".to_string(), Color::Green),
            crate::tui::IndexingPhase::Failed(_) => ("❌ Indexing failed".to_string(), Color::Red),
        };

        let phase_content = Paragraph::new(phase_label)
            .style(Style::default().fg(phase_color))
            .alignment(Alignment::Center)
            .block(Block::default().title("Current Phase").borders(Borders::ALL));
        f.render_widget(phase_content, progress_chunks[2]);

        // Event log
        let events: Vec<ListItem> = progress
            .events
            .iter()
            .rev()
            .take((chunks[2].height as usize).saturating_sub(2))
            .map(|e| ListItem::new(Line::from(e.as_str())))
            .collect();

        let event_list = List::new(events)
            .block(Block::default().title("Event Log").borders(Borders::ALL))
            .style(Style::default().fg(Color::Gray));
        f.render_widget(event_list, chunks[2]);

        // Stats
        let elapsed = progress.start_time.elapsed();
        let rate = if elapsed.as_secs() > 0 {
            progress.total_markets_indexed as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        };

        let stats_text = format!(
            "Time: {:.1}s | Markets: {} | Conditions: {} | Tokens: {} | Duplicates: {} | Rate: {:.0}/s",
            elapsed.as_secs_f64(),
            progress.total_markets_indexed,
            progress.total_conditions,
            progress.total_tokens,
            progress.duplicates_skipped,
            rate
        );

        let stats = Paragraph::new(stats_text)
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(stats, chunks[3]);
    }

    fn render_file_list(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
        let items: Vec<ListItem> = self
            .chunk_files
            .iter()
            .enumerate()
            .map(|(i, file)| {
                let checkbox = if self.selected_files[i] { "☑" } else { "☐" };
                
                // Split the name to show dataset type and filename separately
                let parts: Vec<&str> = file.name.split('/').collect();
                let (dataset_type, filename) = if parts.len() >= 2 {
                    (parts[..parts.len()-1].join("/"), parts[parts.len()-1].to_string())
                } else {
                    ("unknown".to_string(), file.name.clone())
                };
                
                // Color code by dataset type
                let type_color = match dataset_type.as_str() {
                    s if s.contains("raw_markets") => Color::Cyan,
                    s if s.contains("bitcoin") => Color::Yellow,
                    _ => Color::Gray,
                };
                
                let line = Line::from(vec![
                    Span::raw(format!("{} ", checkbox)),
                    Span::styled(
                        format!("[{}] ", dataset_type),
                        Style::default().fg(type_color),
                    ),
                    Span::styled(
                        filename,
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(format!(" ({:.1} MB, ~{} markets)", file.size_mb, file.market_count_estimate)),
                ]);
                ListItem::new(line)
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(format!(
                "Market Data Files ({})",
                self.chunk_files.len()
            )))
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("► ");

        f.render_stateful_widget(list, area, &mut self.list_state);
    }

    fn render_help_popup(&self, f: &mut Frame) {
        let popup_area = centered_rect(60, 50, f.area());
        f.render_widget(Clear, popup_area);

        let help_text = vec![
            Line::from("🗄️ RocksDB Market Data Indexer Help"),
            Line::from(""),
            Line::from("This tool indexes raw market JSON chunks into a fast RocksDB database."),
            Line::from(""),
            Line::from("Navigation:"),
            Line::from("  ↑/↓ or j/k  - Move up/down in the file list"),
            Line::from("  Space       - Toggle selection of current file"),
            Line::from("  a           - Select all files"),
            Line::from("  n           - Select none (clear all selections)"),
            Line::from(""),
            Line::from("Actions:"),
            Line::from("  Enter       - Start indexing selected files"),
            Line::from("  r           - Refresh file list from disk"),
            Line::from("  h or F1     - Toggle this help"),
            Line::from("  q or Esc    - Quit without indexing"),
            Line::from(""),
            Line::from("The indexer will:"),
            Line::from("• Convert JSON markets to typed RocksDB entries"),
            Line::from("• Create search indices for fast queries"),
            Line::from("• Extract tokens and conditions for analysis"),
            Line::from("• Enable fast CLI queries with 'polybot markets --mode db'"),
        ];

        let help_popup = Paragraph::new(help_text)
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Help")
                    .style(Style::default().fg(Color::Cyan)),
            );

        f.render_widget(help_popup, popup_area);
    }

    fn previous_item(&mut self) {
        if self.chunk_files.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.chunk_files.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn next_item(&mut self) {
        if self.chunk_files.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.chunk_files.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn toggle_selection(&mut self) {
        if let Some(i) = self.list_state.selected() {
            if i < self.selected_files.len() {
                self.selected_files[i] = !self.selected_files[i];
            }
        }
    }

    fn select_all(&mut self) {
        for selected in &mut self.selected_files {
            *selected = true;
        }
        self.set_status_message("✅ All files selected".to_string());
    }

    fn select_none(&mut self) {
        for selected in &mut self.selected_files {
            *selected = false;
        }
        self.set_status_message("❌ All files deselected".to_string());
    }

    async fn start_indexing(&mut self) -> Result<()> {
        let selected_files: Vec<PathBuf> = self
            .chunk_files
            .iter()
            .enumerate()
            .filter_map(|(i, file)| {
                if self.selected_files.get(i).copied().unwrap_or(false) {
                    Some(file.path.clone())
                } else {
                    None
                }
            })
            .collect();

        if selected_files.is_empty() {
            self.set_status_message("⚠️ No files selected for indexing".to_string());
            return Ok(());
        }

        // Build the command arguments
        let chunk_files_str = selected_files
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(",");

        let args = IndexArgs {
            db_path: None, // Use default
            use_file_store: false, // Don't use file-based storage
            rocksdb: true, // Use RocksDB storage
            source_dir: None,
            chunk_files: Some(chunk_files_str),
            clear: false,
            skip_duplicates: true,
            batch_size: 1000,
            detailed: true,
            threads: 0, // Auto-detect optimal thread count
        };

        // Create progress channel
        let (progress_sender, progress_receiver) = crate::tui::create_progress_channel();
        
        // Reset progress state
        {
            let mut progress = self.indexing_progress.lock().unwrap();
            *progress = crate::tui::IndexingProgress::default();
        }
        
        // Set up progress receiver
        self.progress_receiver = Some(progress_receiver);
        self.is_indexing = true;
        self.set_status_message("🚀 Starting indexing process...".to_string());

        // Spawn the indexing task
        let command = IndexCommand::new(args).with_progress_sender(progress_sender);
        let data_paths = self.data_paths.clone();
        tokio::spawn(async move {
            let result = command.execute_internal(&data_paths).await;
            if let Err(e) = result {
                error!("Indexing error: {}", e);
            }
        });

        Ok(())
    }

    fn refresh_file_list(&mut self) -> Result<()> {
        let new_files = Self::discover_chunk_files(&self.data_paths)?;
        self.chunk_files = new_files;
        self.selected_files = vec![false; self.chunk_files.len()];
        
        // Reset selection if no files
        if self.chunk_files.is_empty() {
            self.list_state.select(None);
        } else if self.list_state.selected().is_none() {
            self.list_state.select(Some(0));
        }

        Ok(())
    }

    fn set_status_message(&mut self, message: String) {
        self.status_message = Some(message);
        self.last_status_time = Some(Instant::now());
    }

    fn handle_progress_update(&mut self, update: crate::tui::ProgressUpdate) {
        let (complete, error_msg) = {
            let mut progress = self.indexing_progress.lock().unwrap();
            match update {
                crate::tui::ProgressUpdate::FileStart { file_index, total_files, file_name, market_count } => {
                    progress.current_file = file_index;
                    progress.total_files = total_files;
                    progress.current_file_name = file_name;
                    progress.markets_in_file = market_count;
                    progress.markets_processed = 0;
                    (false, None)
                }
                crate::tui::ProgressUpdate::MarketProcessed { markets_in_batch } => {
                    progress.markets_processed += markets_in_batch;
                    progress.total_markets_indexed += markets_in_batch;
                    (false, None)
                }
                crate::tui::ProgressUpdate::FileComplete { duplicates } => {
                    progress.duplicates_skipped += duplicates;
                    (false, None)
                }
                crate::tui::ProgressUpdate::PhaseChange(phase) => {
                    progress.phase = phase;
                    (false, None)
                }
                crate::tui::ProgressUpdate::Event(msg) => {
                    progress.events.push(msg);
                    if progress.events.len() > 20 {
                        progress.events.remove(0);
                    }
                    (false, None)
                }
                crate::tui::ProgressUpdate::ConditionCount(count) => {
                    progress.total_conditions = count;
                    (false, None)
                }
                crate::tui::ProgressUpdate::TokenCount(count) => {
                    progress.total_tokens = count;
                    (false, None)
                }
                crate::tui::ProgressUpdate::Complete => {
                    (true, None)
                }
                crate::tui::ProgressUpdate::Error(err) => {
                    progress.phase = crate::tui::IndexingPhase::Failed(err.clone());
                    (false, Some(err))
                }
            }
        };
        
        if complete {
            self.is_indexing = false;
            self.set_status_message("✅ Indexing completed successfully!".to_string());
        } else if let Some(err) = error_msg {
            self.is_indexing = false;
            self.set_status_message(format!("❌ Indexing failed: {}", err));
        }
    }

    fn discover_chunk_files(data_paths: &DataPaths) -> Result<Vec<ChunkFileInfo>> {
        let datasets_dir = data_paths.datasets();
        if !datasets_dir.exists() {
            return Ok(Vec::new());
        }

        let mut files = Vec::new();

        // Recursively search for market data files
        Self::find_market_files(&datasets_dir, &mut files)?;

        // Sort by path for consistent ordering
        files.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(files)
    }

    fn find_market_files(dir: &PathBuf, files: &mut Vec<ChunkFileInfo>) -> Result<()> {
        // Skip the "runs" directory
        if dir.file_name().and_then(|n| n.to_str()) == Some("runs") {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Recursively search subdirectories
                Self::find_market_files(&path, files)?;
            } else if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                // Check for market data files (both chunk files and regular markets.json)
                let is_chunk_file = file_name.starts_with("markets_chunk_") && file_name.ends_with(".json");
                let is_markets_file = file_name == "markets.json";
                
                if is_chunk_file || is_markets_file {
                    let metadata = fs::metadata(&path)?;
                    let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
                    
                    // Estimate market count differently for different file types
                    let market_count_estimate = if is_markets_file {
                        // For markets.json files, estimate fewer markets (they're usually filtered)
                        (metadata.len() / 5000) as usize
                    } else {
                        // For chunk files, use original estimation
                        (metadata.len() / 1024) as usize
                    };

                    // Create a display name that includes the dataset type
                    let dataset_type = Self::extract_dataset_type(&path);
                    let display_name = format!("{}/{}", dataset_type, file_name);

                    files.push(ChunkFileInfo {
                        path: path.clone(),
                        name: display_name,
                        size_mb,
                        market_count_estimate,
                    });
                }
            }
        }

        Ok(())
    }

    fn extract_dataset_type(path: &PathBuf) -> String {
        // Extract the dataset type from the path
        let path_str = path.to_string_lossy();
        
        if path_str.contains("raw_markets") {
            if let Some(date) = Self::extract_date_from_path(&path_str) {
                return format!("raw_markets/{}", date);
            }
            return "raw_markets".to_string();
        } else if path_str.contains("bitcoin_price_bets") {
            if let Some(date) = Self::extract_date_from_path(&path_str) {
                return format!("bitcoin_price_bets/{}", date);
            }
            return "bitcoin_price_bets".to_string();
        } else if path_str.contains("bitcoin_bets_detailed") {
            if let Some(date) = Self::extract_date_from_path(&path_str) {
                return format!("bitcoin_bets_detailed/{}", date);
            }
            return "bitcoin_bets_detailed".to_string();
        } else if path_str.contains("bitcoin_bets_stats") {
            if let Some(date) = Self::extract_date_from_path(&path_str) {
                return format!("bitcoin_bets_stats/{}", date);
            }
            return "bitcoin_bets_stats".to_string();
        }
        
        "unknown".to_string()
    }

    fn extract_date_from_path(path_str: &str) -> Option<String> {
        // Look for date pattern YYYY-MM-DD in the path
        let parts: Vec<&str> = path_str.split('/').collect();
        for part in parts {
            if part.len() == 10 && part.chars().nth(4) == Some('-') && part.chars().nth(7) == Some('-') {
                return Some(part.to_string());
            }
        }
        None
    }
}

// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: ratatui::layout::Rect) -> ratatui::layout::Rect {
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