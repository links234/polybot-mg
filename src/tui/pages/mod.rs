use crate::tui::App;
use crossterm::event::KeyEvent;
use ratatui::prelude::*;

pub mod markets;
pub mod orders;
pub mod portfolio;
pub mod stream;
pub mod tokens;

pub use markets::MarketsPage;
pub use orders::OrdersPage;
pub use portfolio::PortfolioPage;
pub use stream::StreamPage;
pub use tokens::TokensPage;

pub trait Page {
    fn render(&self, frame: &mut Frame, area: Rect, app: &App);
    fn handle_key(&mut self, key: KeyEvent, app: &mut App) -> bool;
}
