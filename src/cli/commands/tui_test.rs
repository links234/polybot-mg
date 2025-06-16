//! Simple TUI test command for debugging terminal issues

use anyhow::Result;
use clap::Args;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, KeyCode, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};
use std::io::{self, Stdout};
use std::time::{Duration, Instant};
use tracing::{info, error};

#[derive(Args, Clone)]
pub struct TuiTestArgs {
    /// Duration to run test in seconds
    #[arg(long, default_value = "10")]
    pub duration: u64,
}

pub struct TuiTestCommand {
    args: TuiTestArgs,
}

impl TuiTestCommand {
    pub fn new(args: TuiTestArgs) -> Self {
        Self { args }
    }

    pub async fn execute(&self) -> Result<()> {
        info!("Starting TUI diagnostic test");
        
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        
        let start_time = Instant::now();
        let duration = Duration::from_secs(self.args.duration);
        let mut events = Vec::new();
        let mut tick_count = 0u64;
        
        let result = loop {
            // Check if we should quit
            if start_time.elapsed() >= duration {
                break Ok(());
            }
            
            tick_count += 1;
            
            // Draw UI
            match terminal.draw(|f| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Min(10),
                        Constraint::Length(3),
                    ])
                    .split(f.area());
                
                // Title
                let title = Paragraph::new("TUI Diagnostic Test")
                    .style(Style::default().fg(Color::Cyan))
                    .alignment(Alignment::Center)
                    .block(Block::default().borders(Borders::ALL));
                f.render_widget(title, chunks[0]);
                
                // Status and events
                let status_text = format!(
                    "Running for {:.1}s / {}s | Ticks: {} | Events: {}\\nPress 'q' to quit early",
                    start_time.elapsed().as_secs_f32(),
                    self.args.duration,
                    tick_count,
                    events.len()
                );
                
                let event_list: Vec<ListItem> = events
                    .iter()
                    .rev()
                    .take(10)
                    .map(|e| ListItem::new(Line::from(Span::raw(e))))
                    .collect();
                
                let content = List::new(event_list)
                    .block(Block::default()
                        .borders(Borders::ALL)
                        .title(status_text));
                f.render_widget(content, chunks[1]);
                
                // Help
                let help = Paragraph::new("This test verifies TUI rendering and input handling")
                    .style(Style::default().fg(Color::Gray))
                    .alignment(Alignment::Center)
                    .block(Block::default().borders(Borders::ALL));
                f.render_widget(help, chunks[2]);
            }) {
                Ok(_) => {},
                Err(e) => {
                    error!("Drawing error: {}", e);
                    events.push(format!("ERROR: Drawing failed: {}", e));
                }
            }
            
            // Handle input with timeout
            if crossterm::event::poll(Duration::from_millis(100))? {
                match crossterm::event::read()? {
                    Event::Key(key) => {
                        let event_msg = format!("Key: {:?}", key);
                        events.push(event_msg);
                        
                        if let KeyCode::Char('q') = key.code {
                            info!("User requested quit");
                            break Ok(());
                        }
                    }
                    Event::Mouse(mouse) => {
                        events.push(format!("Mouse: {:?}", mouse));
                    }
                    Event::Resize(w, h) => {
                        events.push(format!("Resize: {}x{}", w, h));
                    }
                    _ => {
                        events.push("Other event".to_string());
                    }
                }
                
                // Keep only last 50 events
                if events.len() > 50 {
                    events.remove(0);
                }
            }
        };
        
        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;
        
        info!("TUI diagnostic test completed");
        println!("TUI test completed successfully!");
        println!("Processed {} ticks and {} events", tick_count, events.len());
        
        result
    }
}