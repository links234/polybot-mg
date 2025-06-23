use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};

use crate::tui::navigation::Page;
use crate::tui::pages::Page as PageTrait;
use crate::tui::App;

pub fn draw(frame: &mut Frame<'_>, app: &mut App) {
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
}

