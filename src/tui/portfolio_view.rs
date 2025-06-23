//! Interactive TUI view for portfolio command showing orders and positions

use anyhow::Result;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Tabs},
    Frame, Terminal,
};
use std::io;
use std::time::Duration;

use crate::portfolio::orders_api::PolymarketOrder;
use rust_decimal::Decimal;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PortfolioViewState {
    pub user_address: String,
    pub profile_url: String,
    pub orders: Vec<PolymarketOrder>,
    pub positions: Vec<PositionInfo>,
    pub selected_tab: usize,
    pub selected_row: usize,
    pub should_quit: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PositionInfo {
    pub market_id: String,
    pub outcome: String,
    pub size: Decimal,
    pub entry_price: Decimal,
    pub current_price: Decimal,
    pub pnl: Decimal,
    pub pnl_percent: Decimal,
}

#[allow(dead_code)]
impl PortfolioViewState {
    pub fn new(user_address: String, orders: Vec<PolymarketOrder>) -> Self {
        let profile_url = format!("https://polymarket.com/profile/{}", user_address);

        // For now, derive positions from filled orders
        let positions = Vec::new(); // TODO: Implement position fetching

        Self {
            user_address,
            profile_url,
            orders,
            positions,
            selected_tab: 0,
            selected_row: 0,
            should_quit: false,
        }
    }

    pub fn next_tab(&mut self) {
        self.selected_tab = (self.selected_tab + 1) % 2;
        self.selected_row = 0;
    }

    pub fn previous_tab(&mut self) {
        if self.selected_tab == 0 {
            self.selected_tab = 1;
        } else {
            self.selected_tab -= 1;
        }
        self.selected_row = 0;
    }

    pub fn next_row(&mut self) {
        let max_rows = if self.selected_tab == 0 {
            self.orders.len()
        } else {
            self.positions.len()
        };

        if max_rows > 0 && self.selected_row < max_rows - 1 {
            self.selected_row += 1;
        }
    }

    pub fn previous_row(&mut self) {
        if self.selected_row > 0 {
            self.selected_row -= 1;
        }
    }
}

#[allow(dead_code)]
pub async fn run_portfolio_tui(state: PortfolioViewState) -> Result<()> {
    // Setup terminal with better error handling
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, crossterm::cursor::Show);
        original_hook(panic);
    }));

    let result = run_tui_loop(state).await;

    // Always restore terminal
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, cursor::Show);

    result
}

async fn run_tui_loop(mut state: PortfolioViewState) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Clear the terminal
    terminal.clear()?;

    // Main loop
    loop {
        terminal.draw(|f| draw_ui(f, &state))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            break;
                        }
                        KeyCode::Tab | KeyCode::Right => state.next_tab(),
                        KeyCode::BackTab | KeyCode::Left => state.previous_tab(),
                        KeyCode::Down | KeyCode::Char('j') => state.next_row(),
                        KeyCode::Up | KeyCode::Char('k') => state.previous_row(),
                        _ => {}
                    }
                }
            }
        }
    }

    Ok(())
}

fn draw_ui(f: &mut Frame, state: &PortfolioViewState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Header
            Constraint::Length(3), // Tabs
            Constraint::Min(10),   // Content
            Constraint::Length(3), // Footer
        ])
        .split(f.area());

    // Header with user info
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);

    let user_info = vec![
        Line::from(vec![
            Span::raw("👤 User: "),
            Span::styled(&state.user_address, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::raw("🔗 Profile: "),
            Span::styled(
                &state.profile_url,
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::UNDERLINED),
            ),
        ]),
    ];

    let header_left = Paragraph::new(user_info).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Portfolio Overview"),
    );
    f.render_widget(header_left, header_chunks[0]);

    // Account summary
    let total_order_value: Decimal = state
        .orders
        .iter()
        .map(|o| o.price * o.size_structured)
        .sum();

    let open_orders = state
        .orders
        .iter()
        .filter(|o| o.status == "LIVE" || o.status == "OPEN")
        .count();

    let summary = vec![
        Line::from(vec![
            Span::raw("📊 Open Orders: "),
            Span::styled(open_orders.to_string(), Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::raw("💰 Order Value: $"),
            Span::styled(
                format!("{:.2}", total_order_value),
                Style::default().fg(Color::Green),
            ),
            Span::raw(" USDC"),
        ]),
    ];

    let header_right = Paragraph::new(summary).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Account Summary"),
    );
    f.render_widget(header_right, header_chunks[1]);

    // Tabs
    let tab_titles = vec!["Orders", "Positions"];
    let tabs = Tabs::new(tab_titles)
        .block(Block::default().borders(Borders::ALL))
        .select(state.selected_tab)
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(tabs, chunks[1]);

    // Content area
    match state.selected_tab {
        0 => draw_orders_table(f, chunks[2], state),
        1 => draw_positions_table(f, chunks[2], state),
        _ => {}
    }

    // Footer with help
    let help_text = vec![Line::from(vec![
        Span::raw("Navigate: "),
        Span::styled("↑↓/jk", Style::default().fg(Color::Yellow)),
        Span::raw(" | Tab: "),
        Span::styled("→←/Tab", Style::default().fg(Color::Yellow)),
        Span::raw(" | Quit: "),
        Span::styled("q/Esc", Style::default().fg(Color::Yellow)),
    ])];

    let footer = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[3]);
}

fn draw_orders_table(f: &mut Frame, area: Rect, state: &PortfolioViewState) {
    let header = Row::new(vec![
        Cell::from("Order ID").style(Style::default().fg(Color::Gray)),
        Cell::from("Market").style(Style::default().fg(Color::Gray)),
        Cell::from("Side").style(Style::default().fg(Color::Gray)),
        Cell::from("Price").style(Style::default().fg(Color::Gray)),
        Cell::from("Size").style(Style::default().fg(Color::Gray)),
        Cell::from("Filled").style(Style::default().fg(Color::Gray)),
        Cell::from("Status").style(Style::default().fg(Color::Gray)),
        Cell::from("Outcome").style(Style::default().fg(Color::Gray)),
    ])
    .height(1);

    let rows: Vec<Row> = state
        .orders
        .iter()
        .enumerate()
        .map(|(i, order)| {
            let id_short = if order.id.len() > 12 {
                format!("{}...", &order.id[..12])
            } else {
                order.id.clone()
            };

            let market_short = if order.market.len() > 20 {
                format!("{}...", &order.market[..20])
            } else {
                order.market.clone()
            };

            let side_cell = match order.side.as_str() {
                "BUY" => Cell::from("BUY").style(Style::default().fg(Color::Green)),
                "SELL" => Cell::from("SELL").style(Style::default().fg(Color::Red)),
                _ => Cell::from(order.side.as_str()),
            };

            let status_cell = match order.status.as_str() {
                "LIVE" | "OPEN" => Cell::from("LIVE").style(Style::default().fg(Color::Green)),
                "FILLED" => Cell::from("FILLED").style(Style::default().fg(Color::Blue)),
                "CANCELLED" => Cell::from("CANCELLED").style(Style::default().fg(Color::Red)),
                _ => Cell::from(order.status.as_str()),
            };

            let filled_decimal = order.size_matched.parse::<Decimal>().unwrap_or_default();

            let row = Row::new(vec![
                Cell::from(id_short),
                Cell::from(market_short),
                side_cell,
                Cell::from(format!("${:.4}", order.price)),
                Cell::from(format!("{:.2}", order.size_structured)),
                Cell::from(format!("{:.2}", filled_decimal)),
                status_cell,
                Cell::from(order.outcome.as_str()),
            ]);

            if state.selected_tab == 0 && i == state.selected_row {
                row.style(Style::default().bg(Color::DarkGray))
            } else {
                row
            }
        })
        .collect();

    let widths = [
        Constraint::Length(15),
        Constraint::Length(22),
        Constraint::Length(6),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(12),
        Constraint::Length(10),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Orders ({} total)", state.orders.len())),
        )
        .row_highlight_style(Style::default().add_modifier(Modifier::BOLD));

    f.render_widget(table, area);
}

fn draw_positions_table(f: &mut Frame, area: Rect, state: &PortfolioViewState) {
    if state.positions.is_empty() {
        let empty_msg = Paragraph::new(
            "No positions found. Positions will appear here once orders are filled.",
        )
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title("Positions"));
        f.render_widget(empty_msg, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from("Market").style(Style::default().fg(Color::Gray)),
        Cell::from("Outcome").style(Style::default().fg(Color::Gray)),
        Cell::from("Size").style(Style::default().fg(Color::Gray)),
        Cell::from("Entry Price").style(Style::default().fg(Color::Gray)),
        Cell::from("Current Price").style(Style::default().fg(Color::Gray)),
        Cell::from("P&L").style(Style::default().fg(Color::Gray)),
        Cell::from("P&L %").style(Style::default().fg(Color::Gray)),
    ])
    .height(1);

    let rows: Vec<Row> = state
        .positions
        .iter()
        .enumerate()
        .map(|(i, pos)| {
            let market_short = if pos.market_id.len() > 25 {
                format!("{}...", &pos.market_id[..25])
            } else {
                pos.market_id.clone()
            };

            let pnl_cell = if pos.pnl >= Decimal::ZERO {
                Cell::from(format!("+${:.2}", pos.pnl)).style(Style::default().fg(Color::Green))
            } else {
                Cell::from(format!("-${:.2}", pos.pnl.abs())).style(Style::default().fg(Color::Red))
            };

            let pnl_percent_cell = if pos.pnl_percent >= Decimal::ZERO {
                Cell::from(format!("+{:.2}%", pos.pnl_percent))
                    .style(Style::default().fg(Color::Green))
            } else {
                Cell::from(format!("{:.2}%", pos.pnl_percent))
                    .style(Style::default().fg(Color::Red))
            };

            let row = Row::new(vec![
                Cell::from(market_short),
                Cell::from(pos.outcome.as_str()),
                Cell::from(format!("{:.2}", pos.size)),
                Cell::from(format!("${:.4}", pos.entry_price)),
                Cell::from(format!("${:.4}", pos.current_price)),
                pnl_cell,
                pnl_percent_cell,
            ]);

            if state.selected_tab == 1 && i == state.selected_row {
                row.style(Style::default().bg(Color::DarkGray))
            } else {
                row
            }
        })
        .collect();

    let widths = [
        Constraint::Length(27),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(12),
        Constraint::Length(14),
        Constraint::Length(12),
        Constraint::Length(10),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Positions ({} total)", state.positions.len())),
        )
        .row_highlight_style(Style::default().add_modifier(Modifier::BOLD));

    f.render_widget(table, area);
}
