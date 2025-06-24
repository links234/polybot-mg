//! Robust interactive TUI for portfolio display

use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, TableState, Tabs, Wrap},
    Frame, Terminal,
};
use std::{
    io::{self, Stdout},
    time::Duration,
};

use crate::core::portfolio::api::orders::PolymarketOrder;
use crate::core::portfolio::Position;
use rust_decimal::Decimal;

#[allow(dead_code)]
pub struct App {
    pub user_address: String,
    pub orders: Vec<PolymarketOrder>,
    pub positions: Vec<Position>,
    pub current_tab: usize,
    pub orders_state: TableState,
    pub positions_state: TableState,
    pub _scroll: u16,
    pub should_quit: bool,
}

#[allow(dead_code)]
impl App {
    pub fn new(
        user_address: String,
        orders: Vec<PolymarketOrder>,
        positions: Vec<Position>,
    ) -> Self {
        let mut orders_state = TableState::default();
        if !orders.is_empty() {
            orders_state.select(Some(0));
        }

        let mut positions_state = TableState::default();
        if !positions.is_empty() {
            positions_state.select(Some(0));
        }

        Self {
            user_address,
            orders,
            positions,
            current_tab: 0,
            orders_state,
            positions_state,
            _scroll: 0,
            should_quit: false,
        }
    }

    pub fn next_tab(&mut self) {
        self.current_tab = (self.current_tab + 1) % 2;
    }

    pub fn previous_tab(&mut self) {
        if self.current_tab == 0 {
            self.current_tab = 1;
        } else {
            self.current_tab = 0;
        }
    }

    pub fn next_order(&mut self) {
        if self.orders.is_empty() {
            return;
        }

        let i = match self.orders_state.selected() {
            Some(i) => {
                if i >= self.orders.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.orders_state.select(Some(i));
    }

    pub fn previous_order(&mut self) {
        if self.orders.is_empty() {
            return;
        }

        let i = match self.orders_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.orders.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.orders_state.select(Some(i));
    }

    pub fn next_position(&mut self) {
        if self.positions.is_empty() {
            return;
        }
        let i = match self.positions_state.selected() {
            Some(i) => {
                if i >= self.positions.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.positions_state.select(Some(i));
    }

    pub fn previous_position(&mut self) {
        if self.positions.is_empty() {
            return;
        }
        let i = match self.positions_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.positions.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.positions_state.select(Some(i));
    }
}

#[allow(dead_code)]
pub async fn run_portfolio_tui(
    user_address: String,
    orders: Vec<PolymarketOrder>,
    positions: Vec<Position>,
) -> Result<()> {
    // Create app
    let mut app = App::new(user_address, orders, positions);

    // Setup terminal
    let mut terminal = setup_terminal().context("Failed to setup terminal")?;

    // Run app
    let res = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    restore_terminal(&mut terminal).context("Failed to restore terminal")?;

    res
}

#[allow(dead_code)]
fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

#[allow(dead_code)]
fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

#[allow(dead_code)]
async fn run_app(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => app.should_quit = true,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.should_quit = true;
                    }
                    KeyCode::Tab => app.next_tab(),
                    KeyCode::BackTab => app.previous_tab(),
                    KeyCode::Down | KeyCode::Char('j') => {
                        if app.current_tab == 0 {
                            app.next_order();
                        } else if app.current_tab == 1 {
                            app.next_position();
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if app.current_tab == 0 {
                            app.previous_order();
                        } else if app.current_tab == 1 {
                            app.previous_position();
                        }
                    }
                    _ => {}
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

#[allow(dead_code)]
fn ui(f: &mut Frame, app: &App) {
    let size = f.area();

    // Create main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),  // Header
            Constraint::Length(3),  // Tabs
            Constraint::Min(10),    // Content
            Constraint::Length(10), // Order details (increased)
            Constraint::Length(2),  // Footer
        ])
        .split(size);

    // Render header
    render_header(f, chunks[0], app);

    // Render tabs
    render_tabs(f, chunks[1], app);

    // Render content based on selected tab
    match app.current_tab {
        0 => render_orders(f, chunks[2], app),
        1 => render_positions(f, chunks[2], app),
        _ => {}
    }

    // Render order details if an order is selected
    if app.current_tab == 0 {
        render_order_details(f, chunks[3], app);
    } else {
        // Empty block for positions tab
        let empty_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Details ");
        f.render_widget(empty_block, chunks[3]);
    }

    // Render footer
    render_footer(f, chunks[4]);
}

#[allow(dead_code)]
fn render_header(f: &mut Frame, area: Rect, app: &App) {
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // User info
    let user_text = vec![
        Line::from(vec![
            Span::raw("👤 Address: ").bold(),
            Span::raw(&app.user_address).cyan(),
        ]),
        Line::from(vec![
            Span::raw("🔗 Profile: ").bold(),
            Span::raw(format!("polymarket.com/profile/{}", &app.user_address[..8]))
                .blue()
                .underlined(),
        ]),
    ];

    let user_block = Block::default()
        .title(" Portfolio Overview ")
        .title_style(Style::default().bold())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    let user_paragraph = Paragraph::new(user_text)
        .block(user_block)
        .wrap(Wrap { trim: true });

    f.render_widget(user_paragraph, header_chunks[0]);

    // Summary
    let total_value: Decimal = app.orders.iter().map(|o| o.price * o.size_structured).sum();

    let open_count = app
        .orders
        .iter()
        .filter(|o| o.status == "LIVE" || o.status == "OPEN")
        .count();

    let summary_text = vec![
        Line::from(vec![
            Span::raw("📊 Orders: ").bold(),
            Span::raw(open_count.to_string()).yellow(),
            Span::raw(" active"),
        ]),
        Line::from(vec![
            Span::raw("💰 Value: ").bold(),
            Span::raw("$").green(),
            Span::raw(format!("{:.2}", total_value)).green().bold(),
            Span::raw(" USDC").green(),
        ]),
    ];

    let summary_block = Block::default()
        .title(" Account Summary ")
        .title_style(Style::default().bold())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    let summary_paragraph = Paragraph::new(summary_text)
        .block(summary_block)
        .wrap(Wrap { trim: true });

    f.render_widget(summary_paragraph, header_chunks[1]);
}

#[allow(dead_code)]
fn render_tabs(f: &mut Frame, area: Rect, app: &App) {
    let titles = vec!["Orders", "Positions"];
    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .select(app.current_tab)
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(tabs, area);
}

#[allow(dead_code)]
fn render_orders(f: &mut Frame, area: Rect, app: &App) {
    let orders = &app.orders;

    // Create table
    let header_cells = [
        "ID", "Market", "Side", "Price", "Size", "Filled", "Status", "Outcome",
    ]
    .iter()
    .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow).bold()));

    let header = Row::new(header_cells)
        .style(Style::default().bg(Color::DarkGray))
        .height(1)
        .bottom_margin(1);

    let rows = orders.iter().map(|order| {
        // Show full order ID
        let id_display = order.id.clone();

        let market_short = if order.market.len() > 20 {
            format!("{}...", &order.market[..20])
        } else {
            order.market.clone()
        };

        let side_style = match order.side.as_str() {
            "BUY" => Style::default().fg(Color::Green),
            "SELL" => Style::default().fg(Color::Red),
            _ => Style::default(),
        };

        let status_style = match order.status.as_str() {
            "LIVE" | "OPEN" => Style::default().fg(Color::Green),
            "FILLED" => Style::default().fg(Color::Blue),
            "CANCELLED" => Style::default().fg(Color::Red),
            _ => Style::default(),
        };

        let filled = order.size_matched.parse::<Decimal>().unwrap_or_default();

        let cells = vec![
            Cell::from(id_display).style(Style::default().fg(Color::Cyan)),
            Cell::from(market_short),
            Cell::from(order.side.clone()).style(side_style),
            Cell::from(format!("${:.4}", order.price)),
            Cell::from(format!("{:.2}", order.size_structured)),
            Cell::from(format!("{:.2}", filled)),
            Cell::from(order.status.clone()).style(status_style),
            Cell::from(order.outcome.clone()),
        ];

        Row::new(cells).height(1).bottom_margin(0)
    });

    let t = Table::new(
        rows,
        [
            Constraint::Min(66),    // Full order ID (66 chars)
            Constraint::Length(22), // Market
            Constraint::Length(6),  // Side
            Constraint::Length(10), // Price
            Constraint::Length(10), // Size
            Constraint::Length(10), // Filled
            Constraint::Length(12), // Status
            Constraint::Length(10), // Outcome
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(format!(" Orders ({}) ", orders.len())),
    )
    .row_highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("▶ ");

    f.render_stateful_widget(t, area, &mut app.orders_state.clone());
}

#[allow(dead_code)]
fn render_positions(f: &mut Frame, area: Rect, app: &App) {
    use crate::core::portfolio::{PositionSide, PositionStatus};

    if app.positions.is_empty() {
        let text = vec![
            Line::from(""),
            Line::from("No positions found.".dark_gray().italic()),
            Line::from(""),
            Line::from("Positions will appear here once your orders are filled.".dark_gray()),
        ];

        let paragraph = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(" Positions "),
            )
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });

        f.render_widget(paragraph, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from("Market").style(Style::default().fg(Color::Gray).bold()),
        Cell::from("Outcome").style(Style::default().fg(Color::Gray).bold()),
        Cell::from("Side").style(Style::default().fg(Color::Gray).bold()),
        Cell::from("Size").style(Style::default().fg(Color::Gray).bold()),
        Cell::from("Avg Price").style(Style::default().fg(Color::Gray).bold()),
        Cell::from("P&L").style(Style::default().fg(Color::Gray).bold()),
        Cell::from("Status").style(Style::default().fg(Color::Gray).bold()),
    ])
    .height(1);

    let rows: Vec<Row> = app
        .positions
        .iter()
        .enumerate()
        .map(|(i, position)| {
            let market_short = if position.market_id.len() > 20 {
                format!("{}...", &position.market_id[..20])
            } else {
                position.market_id.clone()
            };

            let side_cell = match position.side {
                PositionSide::Long => Cell::from("LONG").style(Style::default().fg(Color::Green)),
                PositionSide::Short => Cell::from("SHORT").style(Style::default().fg(Color::Red)),
            };

            let pnl = position.total_pnl();
            let pnl_cell = if pnl >= Decimal::ZERO {
                Cell::from(format!("+${:.2}", pnl)).style(Style::default().fg(Color::Green))
            } else {
                Cell::from(format!("-${:.2}", pnl.abs())).style(Style::default().fg(Color::Red))
            };

            let status_cell = match position.status {
                PositionStatus::Open => Cell::from("OPEN").style(Style::default().fg(Color::Green)),
                PositionStatus::Closed => {
                    Cell::from("CLOSED").style(Style::default().fg(Color::Blue))
                }
                PositionStatus::Liquidated => {
                    Cell::from("LIQUIDATED").style(Style::default().fg(Color::Red))
                }
            };

            let row = Row::new(vec![
                Cell::from(market_short),
                Cell::from(position.outcome.clone()),
                side_cell,
                Cell::from(format!("{:.2}", position.size)),
                Cell::from(format!("${:.4}", position.average_price)),
                pnl_cell,
                status_cell,
            ])
            .height(1);

            if app.current_tab == 1 && app.positions_state.selected() == Some(i) {
                row.style(Style::default().bg(Color::DarkGray))
            } else {
                row
            }
        })
        .collect();

    let widths = [
        Constraint::Length(22),
        Constraint::Length(10),
        Constraint::Length(6),
        Constraint::Length(10),
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Length(10),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(format!(" Positions ({}) ", app.positions.len())),
        )
        .row_highlight_style(Style::default().add_modifier(Modifier::BOLD));

    f.render_stateful_widget(table, area, &mut app.positions_state.clone());
}

#[allow(dead_code)]
fn render_order_details(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" Selected Order Details ");

    if let Some(selected) = app.orders_state.selected() {
        if let Some(order) = app.orders.get(selected) {
            let mut details = vec![
                Line::from(vec![
                    Span::raw("Order ID: ").bold(),
                    Span::raw(&order.id).cyan(),
                ]),
                Line::from(vec![
                    Span::raw("Market ID: ").bold(),
                    Span::raw(&order.market).yellow(),
                ]),
                Line::from(vec![
                    Span::raw("Asset ID: ").bold(),
                    Span::raw(&order.asset_id).green(),
                ]),
            ];

            // Add optional fields if present
            if let Some(condition_id) = &order.condition_id {
                details.push(Line::from(vec![
                    Span::raw("Condition ID: ").bold(),
                    Span::raw(condition_id).magenta(),
                ]));
            }

            if let Some(token_id) = &order.token_id {
                details.push(Line::from(vec![
                    Span::raw("Token ID: ").bold(),
                    Span::raw(token_id).blue(),
                ]));
            }

            if let Some(question_id) = &order.question_id {
                details.push(Line::from(vec![
                    Span::raw("Question ID: ").bold(),
                    Span::raw(question_id),
                ]));
            }

            // Add remaining details
            details.extend(vec![
                Line::from(vec![
                    Span::raw("Side: ").bold(),
                    match order.side.as_str() {
                        "BUY" => Span::raw(&order.side).green().bold(),
                        "SELL" => Span::raw(&order.side).red().bold(),
                        _ => Span::raw(&order.side),
                    },
                    Span::raw(" | Price: ").bold(),
                    Span::raw(format!("${:.4}", order.price)).yellow(),
                    Span::raw(" | Size: ").bold(),
                    Span::raw(format!("{:.2}", order.size_structured)),
                    Span::raw(" | Outcome: ").bold(),
                    Span::raw(&order.outcome).cyan(),
                ]),
                Line::from(vec![
                    Span::raw("Maker Address: ").bold(),
                    Span::raw(&order.maker_address).dark_gray(),
                ]),
                Line::from(vec![
                    Span::raw("Created: ").bold(),
                    Span::raw(format!("Timestamp: {}", order.created_at)),
                    Span::raw(" | Type: ").bold(),
                    Span::raw(&order.order_type),
                    Span::raw(" | Expiration: ").bold(),
                    Span::raw(&order.expiration),
                ]),
            ]);

            let paragraph = Paragraph::new(details)
                .block(block)
                .wrap(Wrap { trim: true });

            f.render_widget(paragraph, area);
        } else {
            let paragraph = Paragraph::new("No order selected")
                .block(block)
                .alignment(Alignment::Center);
            f.render_widget(paragraph, area);
        }
    } else {
        let paragraph = Paragraph::new("Select an order to view details")
            .block(block)
            .alignment(Alignment::Center);
        f.render_widget(paragraph, area);
    }
}

#[allow(dead_code)]
fn render_footer(f: &mut Frame, area: Rect) {
    let footer_text = vec![
        Span::raw("Navigate: "),
        Span::styled("↑↓/jk", Style::default().fg(Color::Yellow).bold()),
        Span::raw(" │ Switch Tabs: "),
        Span::styled("Tab", Style::default().fg(Color::Yellow).bold()),
        Span::raw(" │ Quit: "),
        Span::styled("q", Style::default().fg(Color::Yellow).bold()),
    ];

    let footer = Paragraph::new(Line::from(footer_text))
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::TOP));

    f.render_widget(footer, area);
}
