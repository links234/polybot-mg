use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::tui::navigation::Page;
use crate::tui::pages::Page as PageTrait;
use crate::tui::App;

pub fn draw(frame: &mut Frame<'_>, app: &mut App) {
    // Update clipboard notification state
    app.update_clipboard_notification();
    
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(frame.area());

    // Render navigation at the top
    app.navigation.render(frame, chunks[0]);

    // Render the current page
    let current_page = app.navigation.current_page.clone();
    match current_page {
        Page::Stream => app.stream_page.render(frame, chunks[1], app),
        Page::Orders => app.orders_page.render(frame, chunks[1], app),
        Page::Tokens => app.tokens_page.render(frame, chunks[1], app),
        Page::Markets => app.markets_page.render(frame, chunks[1], app),
        Page::Portfolio => app.portfolio_page.render(frame, chunks[1], app),
    }
    
    // Render clipboard notification overlay if present
    if let Some((message, _)) = &app.clipboard_notification {
        render_notification(frame, message);
    }
}

/// Render a notification overlay at the bottom center of the screen
fn render_notification(frame: &mut Frame<'_>, message: &str) {
    let area = frame.area();
    
    // Calculate notification area - centered at bottom
    let width = (message.len() + 4).min(60) as u16;
    let height = 3;
    let x = (area.width.saturating_sub(width)) / 2;
    let y = area.height.saturating_sub(height + 1);
    
    let notification_area = Rect::new(x, y, width, height);
    
    let notification = Paragraph::new(message)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green))
            .style(Style::default().bg(Color::Black)))
        .style(Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center);
    
    frame.render_widget(notification, notification_area);
}

