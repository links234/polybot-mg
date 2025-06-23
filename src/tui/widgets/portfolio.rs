//! Portfolio TUI widget for displaying positions and orders

use chrono::Utc;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Widget},
};
use rust_decimal::Decimal;

use crate::portfolio::{
    ActiveOrder, MarketPositionSummary, OrderSide, OrderStatus, PortfolioStats, Position,
    PositionStatus,
};

/// Portfolio widget state
pub struct PortfolioWidget<'a> {
    positions: &'a [Position],
    orders: &'a [ActiveOrder],
    stats: &'a PortfolioStats,
    market_summaries: &'a [MarketPositionSummary],
    selected_tab: PortfolioTab,
    _selected_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PortfolioTab {
    Overview,
    Positions,
    Orders,
    Markets,
}

impl<'a> PortfolioWidget<'a> {


}

impl<'a> Widget for PortfolioWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Create main layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Content
            ])
            .split(area);

        // Render header with tabs
        self.render_header(chunks[0], buf);

        // Render content based on selected tab
        match self.selected_tab {
            PortfolioTab::Overview => self.render_overview(chunks[1], buf),
            PortfolioTab::Positions => self.render_positions(chunks[1], buf),
            PortfolioTab::Orders => self.render_orders(chunks[1], buf),
            PortfolioTab::Markets => self.render_markets(chunks[1], buf),
        }
    }
}

impl<'a> PortfolioWidget<'a> {
    fn render_header(&self, area: Rect, buf: &mut Buffer) {
        let tabs = vec![
            ("Overview", PortfolioTab::Overview),
            ("Positions", PortfolioTab::Positions),
            ("Orders", PortfolioTab::Orders),
            ("Markets", PortfolioTab::Markets),
        ];

        let tab_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(25); 4])
            .split(area);

        for (i, (name, tab)) in tabs.iter().enumerate() {
            let style = if *tab == self.selected_tab {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                Style::default().fg(Color::Gray)
            };

            let block = Block::default().borders(Borders::ALL).style(style);

            let text = Paragraph::new(Span::styled(*name, style))
                .block(block)
                .alignment(Alignment::Center);

            text.render(tab_chunks[i], buf);
        }
    }

    fn render_overview(&self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(10), // Stats summary
                Constraint::Min(0),     // Recent activity
            ])
            .split(area);

        // Stats summary
        let stats_block = Block::default()
            .title(" Portfolio Summary ")
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Cyan));

        let portfolio_value = self.stats.total_portfolio_value();
        let total_pnl = self.stats.total_pnl();
        let pnl_color = if total_pnl >= Decimal::ZERO {
            Color::Green
        } else {
            Color::Red
        };

        let stats_text = vec![
            Line::from(vec![
                Span::raw("Total Value: "),
                Span::styled(
                    format!("${:.2}", portfolio_value),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::raw("Available: "),
                Span::styled(
                    format!("${:.2}", self.stats.available_balance),
                    Style::default().fg(Color::Green),
                ),
                Span::raw(" | Locked: "),
                Span::styled(
                    format!("${:.2}", self.stats.locked_balance),
                    Style::default().fg(Color::Gray),
                ),
            ]),
            Line::from(vec![
                Span::raw("Total P&L: "),
                Span::styled(
                    format!(
                        "${:.2} ({:+.2}%)",
                        total_pnl,
                        self.stats
                            .pnl_percentage(Decimal::from(10000))
                            .unwrap_or(Decimal::ZERO)
                    ),
                    Style::default().fg(pnl_color).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::raw("Realized: "),
                Span::styled(
                    format!("${:.2}", self.stats.total_realized_pnl),
                    Style::default().fg(Color::White),
                ),
                Span::raw(" | Unrealized: "),
                Span::styled(
                    format!("${:.2}", self.stats.total_unrealized_pnl),
                    Style::default().fg(Color::Gray),
                ),
            ]),
            Line::from(vec![
                Span::raw("Positions: "),
                Span::styled(
                    format!(
                        "{}/{}",
                        self.stats.open_positions, self.stats.total_positions
                    ),
                    Style::default().fg(Color::White),
                ),
                Span::raw(" | Win Rate: "),
                Span::styled(
                    self.stats
                        .win_rate
                        .map(|w| format!("{:.1}%", w))
                        .unwrap_or_else(|| "N/A".to_string()),
                    Style::default().fg(Color::Green),
                ),
            ]),
            Line::from(vec![
                Span::raw("Avg Win: "),
                Span::styled(
                    self.stats
                        .average_win
                        .map(|w| format!("${:.2}", w))
                        .unwrap_or_else(|| "N/A".to_string()),
                    Style::default().fg(Color::Green),
                ),
                Span::raw(" | Avg Loss: "),
                Span::styled(
                    self.stats
                        .average_loss
                        .map(|l| format!("${:.2}", l))
                        .unwrap_or_else(|| "N/A".to_string()),
                    Style::default().fg(Color::Red),
                ),
            ]),
        ];

        let stats_paragraph = Paragraph::new(stats_text)
            .block(stats_block)
            .style(Style::default().fg(Color::White));

        stats_paragraph.render(chunks[0], buf);

        // Recent activity
        let activity_block = Block::default()
            .title(" Recent Activity ")
            .borders(Borders::ALL);

        // Show recent positions and orders
        let mut activity_rows = Vec::new();

        // Add recent positions
        for position in self.positions.iter().take(5) {
            let pnl = position.total_pnl();
            let pnl_color = if pnl >= Decimal::ZERO {
                Color::Green
            } else {
                Color::Red
            };

            activity_rows.push(Row::new(vec![
                Cell::from("Position"),
                Cell::from(format!("{:?}", position.side)),
                Cell::from(format!("{:.8}", position.token_id)),
                Cell::from(format!("{:.2}", position.size)),
                Cell::from(format!("${:.4}", position.average_price)),
                Cell::from(format!("${:.2}", pnl)).style(Style::default().fg(pnl_color)),
            ]));
        }

        // Add recent orders
        for order in self.orders.iter().take(5) {
            let status_color = match order.status {
                OrderStatus::Open => Color::Yellow,
                OrderStatus::PartiallyFilled => Color::Blue,
                OrderStatus::Filled => Color::Green,
                OrderStatus::Cancelled => Color::Gray,
                OrderStatus::Rejected => Color::Red,
                _ => Color::White,
            };

            activity_rows.push(Row::new(vec![
                Cell::from("Order"),
                Cell::from(format!("{:?}", order.side)),
                Cell::from(format!("{:.8}", order.token_id)),
                Cell::from(format!("{:.2}", order.size)),
                Cell::from(format!("${:.4}", order.price)),
                Cell::from(format!("{:?}", order.status)).style(Style::default().fg(status_color)),
            ]));
        }

        let activity_table = Table::new(
            activity_rows,
            vec![
                Constraint::Length(8),
                Constraint::Length(6),
                Constraint::Length(12),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(12),
            ],
        )
        .header(Row::new(vec![
            "Type",
            "Side",
            "Token",
            "Size",
            "Price",
            "P&L/Status",
        ]))
        .block(activity_block)
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        activity_table.render(chunks[1], buf);
    }

    fn render_positions(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title(" Open Positions ")
            .borders(Borders::ALL);

        let header = Row::new(vec![
            "Token",
            "Side",
            "Size",
            "Avg Price",
            "Current",
            "P&L",
            "P&L %",
            "Status",
        ])
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

        let rows: Vec<Row> = self
            .positions
            .iter()
            .map(|position| {
                let pnl = position.total_pnl();
                let pnl_pct = position.pnl_percentage().unwrap_or(Decimal::ZERO);
                let pnl_color = if pnl >= Decimal::ZERO {
                    Color::Green
                } else {
                    Color::Red
                };

                let status_color = match position.status {
                    PositionStatus::Open => Color::Green,
                    PositionStatus::Closed => Color::Gray,
                    PositionStatus::Liquidated => Color::Red,
                };

                Row::new(vec![
                    Cell::from(format!("{:.8}", position.token_id)),
                    Cell::from(format!("{:?}", position.side)),
                    Cell::from(format!("{:.2}", position.size)),
                    Cell::from(format!("${:.4}", position.average_price)),
                    Cell::from(
                        position
                            .current_price
                            .map(|p| format!("${:.4}", p))
                            .unwrap_or_else(|| "N/A".to_string()),
                    ),
                    Cell::from(format!("${:.2}", pnl)).style(Style::default().fg(pnl_color)),
                    Cell::from(format!("{:+.2}%", pnl_pct)).style(Style::default().fg(pnl_color)),
                    Cell::from(format!("{:?}", position.status))
                        .style(Style::default().fg(status_color)),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            vec![
                Constraint::Length(12),
                Constraint::Length(6),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(8),
                Constraint::Length(10),
            ],
        )
        .header(header)
        .block(block)
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        table.render(area, buf);
    }

    fn render_orders(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title(" Active Orders ")
            .borders(Borders::ALL);

        let header = Row::new(vec![
            "Order ID", "Token", "Side", "Type", "Price", "Size", "Filled", "Status", "Age",
        ])
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

        let rows: Vec<Row> = self
            .orders
            .iter()
            .map(|order| {
                let status_color = match order.status {
                    OrderStatus::Open => Color::Yellow,
                    OrderStatus::PartiallyFilled => Color::Blue,
                    OrderStatus::Filled => Color::Green,
                    OrderStatus::Cancelled => Color::Gray,
                    OrderStatus::Rejected => Color::Red,
                    _ => Color::White,
                };

                let side_color = match order.side {
                    OrderSide::Buy => Color::Green,
                    OrderSide::Sell => Color::Red,
                };

                let age = Utc::now().signed_duration_since(order.created_at);
                let age_str = if age.num_hours() > 0 {
                    format!("{}h", age.num_hours())
                } else if age.num_minutes() > 0 {
                    format!("{}m", age.num_minutes())
                } else {
                    format!("{}s", age.num_seconds())
                };

                Row::new(vec![
                    Cell::from(format!("{:.8}", order.order_id)),
                    Cell::from(format!("{:.8}", order.token_id)),
                    Cell::from(format!("{:?}", order.side)).style(Style::default().fg(side_color)),
                    Cell::from(format!("{:?}", order.order_type)),
                    Cell::from(format!("${:.4}", order.price)),
                    Cell::from(format!("{:.2}", order.size)),
                    Cell::from(format!("{:.2}", order.filled_size)),
                    Cell::from(format!("{:?}", order.status))
                        .style(Style::default().fg(status_color)),
                    Cell::from(age_str),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            vec![
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(6),
                Constraint::Length(8),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(12),
                Constraint::Length(6),
            ],
        )
        .header(header)
        .block(block)
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        table.render(area, buf);
    }

    fn render_markets(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title(" Market Positions ")
            .borders(Borders::ALL);

        let header = Row::new(vec![
            "Market",
            "Positions",
            "Net Pos",
            "Exposure",
            "Total P&L",
            "Orders",
        ])
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

        let rows: Vec<Row> = self
            .market_summaries
            .iter()
            .map(|summary| {
                let pnl_color = if summary.total_pnl >= Decimal::ZERO {
                    Color::Green
                } else {
                    Color::Red
                };
                let net_color = if summary.net_position >= Decimal::ZERO {
                    Color::Green
                } else {
                    Color::Red
                };

                Row::new(vec![
                    Cell::from(format!("{:.40}", summary.market_question)),
                    Cell::from(format!("{}", summary.positions.len())),
                    Cell::from(format!("{:+.2}", summary.net_position))
                        .style(Style::default().fg(net_color)),
                    Cell::from(format!("${:.2}", summary.total_exposure)),
                    Cell::from(format!("${:.2}", summary.total_pnl))
                        .style(Style::default().fg(pnl_color)),
                    Cell::from(format!("{}", summary.open_order_count)),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            vec![
                Constraint::Percentage(40),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(8),
            ],
        )
        .header(header)
        .block(block)
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        table.render(area, buf);
    }
}
