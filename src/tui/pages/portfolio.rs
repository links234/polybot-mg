use crate::core::portfolio::types::{Position, PositionSide, PositionStatus};
use crate::tui::App;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Cell, List, ListItem, Paragraph, Row, Table},
};

pub struct PortfolioPage {
    pub selected_position: usize,
}

impl PortfolioPage {
    pub fn new() -> Self {
        Self {
            selected_position: 0,
        }
    }

    fn render_portfolio_summary(&self, frame: &mut Frame, area: Rect, app: &App) {
        let content = if let Ok(stats_lock) = app.portfolio_manager.stats().try_read() {
            let stats = &*stats_lock;
            format!(
                "Portfolio Summary:\n\nTotal Balance: ${:.2}\nAvailable: ${:.2}\nLocked: ${:.2}\n\nPositions: {} total, {} open\nRealized P&L: ${:.2}\nUnrealized P&L: ${:.2}\nTotal P&L: ${:.2}\n\nFees Paid: ${:.2}\nWin Rate: {}\nLast Updated: {}",
                stats.total_balance,
                stats.available_balance,
                stats.locked_balance,
                stats.total_positions,
                stats.open_positions,
                stats.total_realized_pnl,
                stats.total_unrealized_pnl,
                stats.total_pnl(),
                stats.total_fees_paid,
                stats.win_rate.map(|w| format!("{:.1}%", w)).unwrap_or_else(|| "N/A".to_string()),
                stats.last_updated.format("%Y-%m-%d %H:%M:%S")
            )
        } else {
            "Portfolio data loading...".to_string()
        };

        let paragraph = Paragraph::new(content).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Portfolio Summary"),
        );

        frame.render_widget(paragraph, area);
    }

    fn render_positions_list(&self, frame: &mut Frame, area: Rect, app: &App) {
        let positions = if let Ok(positions_lock) = app.portfolio_manager.positions().try_read() {
            positions_lock.values().cloned().collect::<Vec<Position>>()
        } else {
            Vec::new()
        };

        let header = Row::new(vec![
            "Token",
            "Side",
            "Size",
            "Avg Price",
            "Current",
            "P&L",
            "P&L %",
        ])
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

        let rows: Vec<Row> = positions
            .iter()
            .enumerate()
            .map(|(i, position)| {
                let style = if i == self.selected_position {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                let side_str = match position.side {
                    PositionSide::Long => "LONG",
                    PositionSide::Short => "SHORT",
                };

                let current_price_str = position
                    .current_price
                    .map(|p| format!("${:.4}", p))
                    .unwrap_or_else(|| "N/A".to_string());

                let total_pnl = position.total_pnl();
                let pnl_pct = position.pnl_percentage().unwrap_or_default();

                let pnl_style = if total_pnl >= rust_decimal::Decimal::ZERO {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Red)
                };

                let pnl_str = if total_pnl >= rust_decimal::Decimal::ZERO {
                    format!("+${:.2}", total_pnl)
                } else {
                    format!("-${:.2}", total_pnl.abs())
                };

                let pnl_pct_str = if pnl_pct >= rust_decimal::Decimal::ZERO {
                    format!("+{:.1}%", pnl_pct)
                } else {
                    format!("{:.1}%", pnl_pct)
                };

                Row::new(vec![
                    Cell::from(&position.token_id[..12]),
                    Cell::from(side_str),
                    Cell::from(format!("{:.2}", position.size)),
                    Cell::from(format!("${:.4}", position.average_price)),
                    Cell::from(current_price_str),
                    Cell::from(pnl_str).style(pnl_style),
                    Cell::from(pnl_pct_str).style(pnl_style),
                ])
                .style(style)
            })
            .collect();

        let table = Table::new(
            rows,
            &[
                Constraint::Percentage(20),
                Constraint::Percentage(10),
                Constraint::Percentage(12),
                Constraint::Percentage(12),
                Constraint::Percentage(12),
                Constraint::Percentage(12),
                Constraint::Percentage(12),
            ],
        )
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(format!(
            "Positions ({}) - Use ↑↓ to select, Enter for details",
            positions.len()
        )));

        frame.render_widget(table, area);
    }

    fn render_position_details(&self, frame: &mut Frame, area: Rect, app: &App) {
        let content = if let Ok(positions_lock) = app.portfolio_manager.positions().try_read() {
            let positions: Vec<_> = positions_lock.values().collect();
            if let Some(position) = positions.get(self.selected_position) {
                let side_str = match position.side {
                    PositionSide::Long => "LONG",
                    PositionSide::Short => "SHORT",
                };

                let status_str = match position.status {
                    PositionStatus::Open => "OPEN",
                    PositionStatus::Closed => "CLOSED",
                    PositionStatus::Liquidated => "LIQUIDATED",
                };

                let market_value = position
                    .current_price
                    .map(|p| position.size * p)
                    .unwrap_or_default();

                let cost_basis = position.size * position.average_price;

                format!(
                    "Position Details:\n\nToken: {}\nOutcome: {}\nMarket: {}\nSide: {}\nStatus: {}\nSize: {:.2}\nAverage Price: ${:.4}\nCurrent Price: {}\nCost Basis: ${:.2}\nMarket Value: ${:.2}\n\nP&L Analysis:\n• Realized P&L: ${:.2}\n• Unrealized P&L: {}\n• Total P&L: ${:.2}\n• P&L %: {}\n\nTiming:\n• Opened: {}\n• Updated: {}\n• Fees Paid: ${:.2}\n\nControls:\nS - Sell Position\nT - Trade More\nI - Position Info",
                    &position.token_id[..16],
                    position.outcome,
                    position.market_question.as_deref().unwrap_or("N/A"),
                    side_str,
                    status_str,
                    position.size,
                    position.average_price,
                    position.current_price.map(|p| format!("${:.4}", p)).unwrap_or_else(|| "N/A".to_string()),
                    cost_basis,
                    market_value,
                    position.realized_pnl,
                    position.unrealized_pnl.map(|p| format!("${:.2}", p)).unwrap_or_else(|| "N/A".to_string()),
                    position.total_pnl(),
                    position.pnl_percentage().map(|p| format!("{:.2}%", p)).unwrap_or_else(|| "N/A".to_string()),
                    position.opened_at.format("%Y-%m-%d %H:%M:%S"),
                    position.updated_at.format("%Y-%m-%d %H:%M:%S"),
                    position.fees_paid
                )
            } else {
                "No position selected".to_string()
            }
        } else {
            "Unable to load position details".to_string()
        };

        let paragraph = Paragraph::new(content).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Position Details"),
        );

        frame.render_widget(paragraph, area);
    }

    fn render_portfolio_actions(&self, frame: &mut Frame, area: Rect, _app: &App) {
        let actions = vec![
            "S - Sell Position",
            "T - Trade Token",
            "I - Position Info",
            "R - Refresh Portfolio",
            "E - Export Report",
        ];

        let items: Vec<ListItem> = actions
            .iter()
            .map(|action| ListItem::new(*action))
            .collect();

        let list = List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Available Actions"),
        );

        frame.render_widget(list, area);
    }
}

impl super::Page for PortfolioPage {
    fn render(&self, frame: &mut Frame, area: Rect, app: &App) {
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(area);

        // Top: Portfolio summary
        self.render_portfolio_summary(frame, main_chunks[0], app);

        // Bottom: Positions and details
        let bottom_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(main_chunks[1]);

        // Left: Positions list
        self.render_positions_list(frame, bottom_chunks[0], app);

        // Right: Position details and actions
        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(bottom_chunks[1]);

        self.render_position_details(frame, right_chunks[0], app);
        self.render_portfolio_actions(frame, right_chunks[1], app);
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut App) -> bool {
        match key.code {
            KeyCode::Up => {
                self.selected_position = self.selected_position.saturating_sub(1);
                true
            }
            KeyCode::Down => {
                if let Ok(positions_lock) = app.portfolio_manager.positions().try_read() {
                    let positions_count = positions_lock.len();
                    if positions_count > 0 {
                        self.selected_position =
                            (self.selected_position + 1).min(positions_count - 1);
                    }
                }
                true
            }
            KeyCode::Enter => {
                // Handle view position details
                true
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                // Handle sell position
                true
            }
            KeyCode::Char('t') | KeyCode::Char('T') => {
                // Handle trade more
                true
            }
            KeyCode::Char('i') | KeyCode::Char('I') => {
                // Handle position info
                true
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                // Handle refresh
                true
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                // Handle export report
                true
            }
            _ => false,
        }
    }
}

impl Default for PortfolioPage {
    fn default() -> Self {
        Self::new()
    }
}
