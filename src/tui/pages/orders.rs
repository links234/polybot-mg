use crate::core::portfolio::types::{ActiveOrder, OrderSide, OrderStatus, OrderType};
use crate::tui::App;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Cell, List, ListItem, Paragraph, Row, Table},
};

pub struct OrdersPage {
    pub selected_order: usize,
}

impl OrdersPage {
    pub fn new() -> Self {
        Self { selected_order: 0 }
    }

    fn render_orders_list(&self, frame: &mut Frame, area: Rect, app: &App) {
        // Get real orders from portfolio manager
        let orders = if let Ok(orders_lock) = app.portfolio_manager.active_orders().try_read() {
            orders_lock.values().cloned().collect::<Vec<ActiveOrder>>()
        } else {
            Vec::new()
        };

        let header = Row::new(vec!["Order ID", "Side", "Size", "Price", "Status"]).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

        let title = if app.is_fetching_orders {
            format!("Active Orders (Fetching...) - Use ↑↓ to select, R to refresh")
        } else if orders.is_empty() && app.last_orders_fetch.is_none() {
            "Active Orders (Not loaded) - Use R to fetch orders".to_string()
        } else {
            format!(
                "Active Orders ({}) - Use ↑↓ to select, R to refresh",
                orders.len()
            )
        };

        let rows: Vec<Row> = if orders.is_empty() && app.is_fetching_orders {
            vec![Row::new(vec![
                Cell::from("Fetching orders..."),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
            ])
            .style(Style::default().fg(Color::Yellow))]
        } else if orders.is_empty() {
            vec![Row::new(vec![
                Cell::from("No orders found"),
                Cell::from("Use R to refresh"),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
            ])
            .style(Style::default().fg(Color::Gray))]
        } else {
            orders
                .iter()
                .enumerate()
                .map(|(i, order)| {
                    let style = if i == self.selected_order {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };

                    let side_str = match order.side {
                        OrderSide::Buy => "BUY",
                        OrderSide::Sell => "SELL",
                    };

                    let status_str = match order.status {
                        OrderStatus::Pending => "PENDING",
                        OrderStatus::Open => "OPEN",
                        OrderStatus::PartiallyFilled => "PARTIAL",
                        OrderStatus::Filled => "FILLED",
                        OrderStatus::Cancelled => "CANCELLED",
                        OrderStatus::Rejected => "REJECTED",
                        OrderStatus::Expired => "EXPIRED",
                    };

                    Row::new(vec![
                        Cell::from(&order.order_id[..8]), // Show first 8 chars
                        Cell::from(side_str),
                        Cell::from(format!("{:.2}", order.remaining_size)),
                        Cell::from(format!("${:.4}", order.price)),
                        Cell::from(status_str),
                    ])
                    .style(style)
                })
                .collect()
        };

        let table = Table::new(
            rows,
            &[
                Constraint::Percentage(20),
                Constraint::Percentage(15),
                Constraint::Percentage(15),
                Constraint::Percentage(15),
                Constraint::Percentage(35),
            ],
        )
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(title));

        frame.render_widget(table, area);
    }

    fn render_order_details(&self, frame: &mut Frame, area: Rect, app: &App) {
        let content = if let Ok(orders_lock) = app.portfolio_manager.active_orders().try_read() {
            let orders: Vec<_> = orders_lock.values().collect();
            if let Some(order) = orders.get(self.selected_order) {
                let side_str = match order.side {
                    OrderSide::Buy => "BUY",
                    OrderSide::Sell => "SELL",
                };

                let order_type_str = match order.order_type {
                    OrderType::Limit => "LIMIT",
                    OrderType::Market => "MARKET",
                };

                let status_str = match order.status {
                    OrderStatus::Pending => "PENDING",
                    OrderStatus::Open => "OPEN",
                    OrderStatus::PartiallyFilled => "PARTIALLY FILLED",
                    OrderStatus::Filled => "FILLED",
                    OrderStatus::Cancelled => "CANCELLED",
                    OrderStatus::Rejected => "REJECTED",
                    OrderStatus::Expired => "EXPIRED",
                };

                format!(
                    "Order Details:\n\nOrder ID: {}\nToken: {}\nOutcome: {}\nSide: {}\nType: {}\nSize: {:.2}\nPrice: ${:.4}\nFilled: {:.2}\nRemaining: {:.2}\nStatus: {}\nCreated: {}\nPost Only: {}\nReduce Only: {}\n\nControls:\nC - Cancel Order\nM - Modify Order\nR - Refresh",
                    order.order_id,
                    &order.token_id[..16],
                    order.outcome,
                    side_str,
                    order_type_str,
                    order.size,
                    order.price,
                    order.filled_size,
                    order.remaining_size,
                    status_str,
                    order.created_at.format("%Y-%m-%d %H:%M:%S"),
                    order.post_only,
                    order.reduce_only
                )
            } else {
                "No order selected".to_string()
            }
        } else {
            "Unable to load order details".to_string()
        };

        let paragraph = Paragraph::new(content).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Order Details"),
        );

        frame.render_widget(paragraph, area);
    }

    fn render_order_actions(&self, frame: &mut Frame, area: Rect, _app: &App) {
        let actions = vec![
            "N - New Order",
            "C - Cancel Selected",
            "M - Modify Selected",
            "R - Refresh Orders",
            "F - Filter Orders",
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

impl super::Page for OrdersPage {
    fn render(&self, frame: &mut Frame, area: Rect, app: &App) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area);

        // Left side: Orders list
        self.render_orders_list(frame, chunks[0], app);

        // Right side: Order details and actions
        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(chunks[1]);

        self.render_order_details(frame, right_chunks[0], app);
        self.render_order_actions(frame, right_chunks[1], app);
    }

    fn handle_key(&mut self, key: KeyEvent, app: &mut App) -> bool {
        match key.code {
            KeyCode::Up => {
                self.selected_order = self.selected_order.saturating_sub(1);
                true
            }
            KeyCode::Down => {
                if let Ok(orders_lock) = app.portfolio_manager.active_orders().try_read() {
                    let orders_count = orders_lock.len();
                    if orders_count > 0 {
                        self.selected_order = (self.selected_order + 1).min(orders_count - 1);
                    }
                }
                true
            }
            KeyCode::Char('c') | KeyCode::Char('C') => {
                // Handle cancel order
                true
            }
            KeyCode::Char('m') | KeyCode::Char('M') => {
                // Handle modify order
                true
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                // Handle new order
                true
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                // Request orders refresh
                app.request_orders_refresh();
                true
            }
            _ => false,
        }
    }
}

impl Default for OrdersPage {
    fn default() -> Self {
        Self::new()
    }
}
