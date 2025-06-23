//! Unified dataset and selection TUI interface

use crate::data_paths::DataPaths;
use crate::datasets::selection::{ImplicitSelection, SelectionManager, TokenSelection};
use crate::storage::{DatasetDiscovery, DiscoveredDataset};
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{
        self, disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use serde_json::Value;
use std::collections::HashSet;
use std::io::{self, IsTerminal, Stdout};
use tracing::{debug, error};

pub struct DatasetSelectorResult {
    pub selected_tokens: Vec<String>,
    pub cancelled: bool,
}

#[derive(Debug, Clone)]
enum SelectionItem {
    SavedSelection(TokenSelection),
    ImplicitSelection(ImplicitSelection),
}

#[derive(Debug, Clone)]
enum DatasetItem {
    Folder { name: String, datasets_count: usize },
    Dataset(DiscoveredDataset),
}

#[derive(Debug, Clone, PartialEq)]
enum PanelFocus {
    Selections,
    Datasets,
}

pub struct DatasetSelector {
    // Left panel: Selections
    selections: Vec<SelectionItem>,
    selections_state: ListState,

    // Right panel: Datasets
    datasets: Vec<DatasetItem>,
    datasets_state: ListState,

    // UI state
    current_panel: PanelFocus,
    _selected_tokens: HashSet<String>,
    show_help: bool,

    // Data paths for loading selections
    data_paths: DataPaths,
}

impl DatasetSelector {
    /// Check if we're running in a terminal environment
    fn is_terminal_available() -> bool {
        debug!("Checking if terminal is available...");

        // Check for explicit non-terminal indicators
        if std::env::var("CI").is_ok()
            || std::env::var("GITHUB_ACTIONS").is_ok()
            || std::env::var("BUILDKITE").is_ok()
        {
            debug!("CI environment detected, terminal not available");
            return false;
        }

        // Try to get terminal size as a more reliable indicator
        if let Ok((cols, rows)) = terminal::size() {
            if cols > 0 && rows > 0 {
                debug!(
                    "Terminal size available: {}x{}, assuming terminal is available",
                    cols, rows
                );
                return true;
            }
        }

        // Fallback to traditional check
        if io::stdout().is_terminal() {
            return true;
        }

        // Check if we're in a known terminal environment
        if let Ok(term) = std::env::var("TERM") {
            if !term.is_empty() && term != "dumb" {
                debug!("TERM env var set to '{}', attempting to use terminal", term);
                return true;
            }
        }

        false
    }

    /// Setup terminal with proper error handling
    fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
        enable_raw_mode()?;

        let mut stdout = io::stdout();
        stdout.execute(EnterAlternateScreen)?;

        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(terminal)
    }

    pub async fn new(datasets_path: &str) -> Result<Self> {
        debug!("Creating unified DatasetSelector");

        let data_paths = DataPaths::new(".");
        let selection_manager = SelectionManager::new(&data_paths.data());

        // Load saved selections
        debug!("Loading saved selections...");
        let mut selections = Vec::new();

        // Load explicit selections
        if let Ok(explicit_selections) = selection_manager.list_selections() {
            for selection_name in explicit_selections {
                if let Ok(selection) = selection_manager.load_selection(&selection_name) {
                    selections.push(SelectionItem::SavedSelection(selection));
                }
            }
        }

        // Load implicit selections (datasets as selections)
        if let Ok(implicit_selections) = selection_manager.discover_implicit_selections() {
            for implicit in implicit_selections {
                selections.push(SelectionItem::ImplicitSelection(implicit));
            }
        }

        debug!("Loaded {} total selections", selections.len());

        // Load datasets
        debug!("Loading datasets from: {}", datasets_path);
        let discovery = DatasetDiscovery::new(datasets_path);
        let discovered_datasets = discovery.discover_datasets().await?;

        // Organize datasets by folder
        let datasets = Self::organize_datasets(discovered_datasets);
        debug!("Organized {} dataset items", datasets.len());

        let mut selector = Self {
            selections,
            selections_state: ListState::default(),
            datasets,
            datasets_state: ListState::default(),
            current_panel: PanelFocus::Selections,
            _selected_tokens: HashSet::new(),
            show_help: false,
            data_paths,
        };

        // Set initial selection
        if !selector.selections.is_empty() {
            selector.selections_state.select(Some(0));
        } else if !selector.datasets.is_empty() {
            selector.current_panel = PanelFocus::Datasets;
            selector.datasets_state.select(Some(0));
        }

        Ok(selector)
    }

    fn organize_datasets(datasets: Vec<DiscoveredDataset>) -> Vec<DatasetItem> {
        let mut items = Vec::new();
        let mut folders: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        // Group by folder and count
        for dataset in &datasets {
            let folder_name = dataset
                .dataset_info
                .name
                .split('/')
                .next()
                .unwrap_or(&dataset.dataset_info.name)
                .to_string();
            *folders.entry(folder_name).or_insert(0) += 1;
        }

        // Add folder headers
        for (folder_name, count) in folders {
            items.push(DatasetItem::Folder {
                name: folder_name.clone(),
                datasets_count: count,
            });

            // Add datasets in this folder
            for dataset in &datasets {
                let dataset_folder = dataset
                    .dataset_info
                    .name
                    .split('/')
                    .next()
                    .unwrap_or(&dataset.dataset_info.name);
                if dataset_folder == folder_name {
                    items.push(DatasetItem::Dataset(dataset.clone()));
                }
            }
        }

        items
    }

    pub async fn run(datasets_path: &str) -> Result<DatasetSelectorResult> {
        debug!("Starting unified dataset selector");

        if !Self::is_terminal_available() {
            return Err(anyhow::anyhow!(
                "Interactive dataset selector requires a terminal environment.\n\n\
                The TUI interface is not available in this environment.\n\
                Please use one of these alternatives:\n\n\
                1. Run in a proper terminal (not in a pipe/redirect)\n\
                2. Specify assets directly: polybot stream --assets TOKEN1,TOKEN2\n\
                3. Use a saved selection: polybot stream --selection <name>\n\
                4. Load from file: polybot stream --markets-path <file>\n\n\
                To create selections for later use:\n\
                polybot selections create --name <name> --tokens TOKEN1,TOKEN2"
            ));
        }

        let mut selector = Self::new(datasets_path).await?;
        let mut terminal = Self::setup_terminal()?;

        debug!("Starting main UI loop");
        let result = selector.run_loop(&mut terminal).await;

        // Cleanup
        debug!("Cleaning up terminal");
        disable_raw_mode()?;
        terminal.backend_mut().execute(LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    async fn run_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<DatasetSelectorResult> {
        let mut iteration = 0;

        loop {
            iteration += 1;
            if iteration % 100 == 0 {
                debug!("UI loop iteration {}", iteration);
            }

            // Render
            if let Err(e) = terminal.draw(|f| self.render(f)) {
                error!("Rendering failed: {:?}", e);
                return Err(anyhow::anyhow!("Rendering failed"));
            }

            // Handle events
            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                debug!("User cancelled selection");
                                return Ok(DatasetSelectorResult {
                                    selected_tokens: vec![],
                                    cancelled: true,
                                });
                            }
                            KeyCode::Char('h') | KeyCode::F(1) => {
                                self.show_help = !self.show_help;
                            }
                            KeyCode::Tab => {
                                self.switch_panel();
                            }
                            KeyCode::Up => self.move_selection(-1),
                            KeyCode::Down => self.move_selection(1),
                            KeyCode::Enter => {
                                if let Some(tokens) = self.select_current_item().await? {
                                    debug!("Selected {} tokens", tokens.len());
                                    return Ok(DatasetSelectorResult {
                                        selected_tokens: tokens,
                                        cancelled: false,
                                    });
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    fn switch_panel(&mut self) {
        self.current_panel = match self.current_panel {
            PanelFocus::Selections => PanelFocus::Datasets,
            PanelFocus::Datasets => PanelFocus::Selections,
        };
        debug!("Switched to panel: {:?}", self.current_panel);
    }

    fn move_selection(&mut self, delta: i32) {
        match self.current_panel {
            PanelFocus::Selections => {
                let len = self.selections.len();
                if len == 0 {
                    return;
                }

                let current = self.selections_state.selected().unwrap_or(0);
                let new_index = if delta > 0 {
                    (current + 1) % len
                } else {
                    if current == 0 {
                        len - 1
                    } else {
                        current - 1
                    }
                };
                self.selections_state.select(Some(new_index));
            }
            PanelFocus::Datasets => {
                let len = self.datasets.len();
                if len == 0 {
                    return;
                }

                let current = self.datasets_state.selected().unwrap_or(0);
                let new_index = if delta > 0 {
                    (current + 1) % len
                } else {
                    if current == 0 {
                        len - 1
                    } else {
                        current - 1
                    }
                };
                self.datasets_state.select(Some(new_index));
            }
        }
    }

    async fn select_current_item(&self) -> Result<Option<Vec<String>>> {
        match self.current_panel {
            PanelFocus::Selections => {
                if let Some(index) = self.selections_state.selected() {
                    if let Some(selection_item) = self.selections.get(index) {
                        let tokens = match selection_item {
                            SelectionItem::SavedSelection(selection) => {
                                debug!("Selected saved selection: {}", selection.name);
                                selection
                                    .tokens
                                    .iter()
                                    .map(|t| t.token_id.clone())
                                    .collect()
                            }
                            SelectionItem::ImplicitSelection(implicit) => {
                                debug!("Selected implicit selection: {}", implicit.name);
                                // Load tokens from the implicit selection
                                let selection_manager =
                                    SelectionManager::new(&self.data_paths.data());
                                selection_manager.get_tokens(&implicit.name)?
                            }
                        };
                        return Ok(Some(tokens));
                    }
                }
            }
            PanelFocus::Datasets => {
                if let Some(index) = self.datasets_state.selected() {
                    if let Some(dataset_item) = self.datasets.get(index) {
                        match dataset_item {
                            DatasetItem::Folder { .. } => {
                                // Don't select folders, only datasets
                                return Ok(None);
                            }
                            DatasetItem::Dataset(dataset) => {
                                debug!("Selected dataset: {}", dataset.dataset_info.name);
                                // Extract tokens from the dataset
                                let tokens = self.extract_tokens_from_dataset(dataset)?;
                                return Ok(Some(tokens));
                            }
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    fn extract_tokens_from_dataset(&self, dataset: &DiscoveredDataset) -> Result<Vec<String>> {
        debug!(
            "Extracting tokens from dataset: {}",
            dataset.dataset_info.name
        );

        // Look for markets.json file in the dataset
        for file_info in &dataset.dataset_info.files {
            if file_info.name == "markets.json" {
                let markets_file_path = dataset.dataset_info.path.join(&file_info.relative_path);
                debug!("Found markets.json at: {:?}", markets_file_path);

                let content = std::fs::read_to_string(&markets_file_path)?;
                let markets: Vec<Value> = serde_json::from_str(&content)?;

                let mut tokens = Vec::new();
                for market in markets {
                    if let Some(market_tokens) = market.get("tokens").and_then(|v| v.as_array()) {
                        for token in market_tokens {
                            if let Some(token_id) = token.get("token_id").and_then(|v| v.as_str()) {
                                tokens.push(token_id.to_string());
                            }
                        }
                    }
                }

                debug!("Extracted {} tokens from dataset", tokens.len());
                return Ok(tokens);
            }
        }

        // Fallback: just return the token_id if no markets.json found
        debug!("No markets.json found, using dataset token_id");
        Ok(vec![dataset.token_id.clone()])
    }

    fn render(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(f.area());

        // Left panel: Selections
        self.render_selections_panel(f, chunks[0]);

        // Right panel: Datasets
        self.render_datasets_panel(f, chunks[1]);

        // Help overlay
        if self.show_help {
            self.render_help(f);
        }
    }

    fn render_selections_panel(&mut self, f: &mut Frame, area: ratatui::prelude::Rect) {
        let is_focused = self.current_panel == PanelFocus::Selections;

        let border_style = if is_focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };

        let items: Vec<ListItem> = self
            .selections
            .iter()
            .map(|selection| {
                let (name, description) = match selection {
                    SelectionItem::SavedSelection(sel) => (
                        sel.name.clone(),
                        format!("Saved - {} tokens", sel.tokens.len()),
                    ),
                    SelectionItem::ImplicitSelection(imp) => (
                        imp.name.clone(),
                        format!("Dataset - {} tokens", imp.tokens.len()),
                    ),
                };

                ListItem::new(vec![
                    Line::from(vec![Span::styled(name, Style::default().fg(Color::White))]),
                    Line::from(vec![Span::styled(
                        description,
                        Style::default().fg(Color::Gray),
                    )]),
                ])
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title("📁 Selections")
                    .borders(Borders::ALL)
                    .border_style(border_style),
            )
            .highlight_style(Style::default().bg(Color::Blue).fg(Color::White))
            .highlight_symbol("▶ ");

        f.render_stateful_widget(list, area, &mut self.selections_state);
    }

    fn render_datasets_panel(&mut self, f: &mut Frame, area: ratatui::prelude::Rect) {
        let is_focused = self.current_panel == PanelFocus::Datasets;

        let border_style = if is_focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };

        let items: Vec<ListItem> = self
            .datasets
            .iter()
            .map(|dataset| match dataset {
                DatasetItem::Folder {
                    name,
                    datasets_count,
                } => ListItem::new(vec![
                    Line::from(vec![Span::styled(
                        format!("📂 {}", name),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )]),
                    Line::from(vec![Span::styled(
                        format!("   {} datasets", datasets_count),
                        Style::default().fg(Color::Gray),
                    )]),
                ]),
                DatasetItem::Dataset(dataset) => {
                    let short_name = dataset
                        .dataset_info
                        .name
                        .split('/')
                        .last()
                        .unwrap_or(&dataset.dataset_info.name);
                    ListItem::new(vec![
                        Line::from(vec![Span::styled(
                            format!("  📄 {}", short_name),
                            Style::default().fg(Color::White),
                        )]),
                        Line::from(vec![Span::styled(
                            format!("     {} files", dataset.dataset_info.file_count),
                            Style::default().fg(Color::Gray),
                        )]),
                    ])
                }
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title("📊 Raw Datasets")
                    .borders(Borders::ALL)
                    .border_style(border_style),
            )
            .highlight_style(Style::default().bg(Color::Blue).fg(Color::White))
            .highlight_symbol("▶ ");

        f.render_stateful_widget(list, area, &mut self.datasets_state);
    }

    fn render_help(&self, f: &mut Frame) {
        let help_text = vec![
            Line::from("📖 Dataset Selector Help"),
            Line::from(""),
            Line::from("Navigation:"),
            Line::from("  ↑/↓     - Move selection up/down"),
            Line::from("  Tab     - Switch between panels"),
            Line::from("  Enter   - Select item"),
            Line::from("  h/F1    - Toggle this help"),
            Line::from("  q/Esc   - Cancel and exit"),
            Line::from(""),
            Line::from("Panels:"),
            Line::from("  Left    - Saved selections & dataset shortcuts"),
            Line::from("  Right   - Raw dataset files"),
        ];

        let paragraph = Paragraph::new(help_text)
            .block(
                Block::default()
                    .title("Help")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .alignment(Alignment::Left);

        let area = f.area();
        let popup_area = ratatui::layout::Rect {
            x: area.width / 4,
            y: area.height / 4,
            width: area.width / 2,
            height: area.height / 2,
        };

        f.render_widget(Clear, popup_area);
        f.render_widget(paragraph, popup_area);
    }
}
