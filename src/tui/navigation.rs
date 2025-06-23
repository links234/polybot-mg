use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Tabs},
};

#[derive(Debug, Clone, PartialEq)]
pub enum Page {
    Stream,
    Orders,
    Tokens,
    Markets,
    Portfolio,
}

impl Page {
    pub fn all() -> Vec<Page> {
        vec![
            Page::Stream,
            Page::Orders,
            Page::Tokens,
            Page::Markets,
            Page::Portfolio,
        ]
    }

    pub fn title(&self) -> &'static str {
        match self {
            Page::Stream => "Stream",
            Page::Orders => "Orders",
            Page::Tokens => "Tokens",
            Page::Markets => "Markets",
            Page::Portfolio => "Portfolio",
        }
    }

    pub fn next(&self) -> Page {
        let pages = Self::all();
        let current_index = pages.iter().position(|p| p == self).unwrap_or(0);
        let next_index = (current_index + 1) % pages.len();
        pages[next_index].clone()
    }

    pub fn previous(&self) -> Page {
        let pages = Self::all();
        let current_index = pages.iter().position(|p| p == self).unwrap_or(0);
        let prev_index = if current_index == 0 {
            pages.len() - 1
        } else {
            current_index - 1
        };
        pages[prev_index].clone()
    }
}

pub struct Navigation {
    pub current_page: Page,
}

impl Navigation {
    pub fn new() -> Self {
        Self {
            current_page: Page::Stream,
        }
    }

    pub fn go_to_page(&mut self, page: Page) {
        self.current_page = page;
    }

    pub fn next_page(&mut self) {
        self.current_page = self.current_page.next();
    }

    pub fn previous_page(&mut self) {
        self.current_page = self.current_page.previous();
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let pages = Page::all();
        let titles: Vec<Line> = pages
            .iter()
            .map(|page| {
                let title = page.title();
                Line::from(title)
            })
            .collect();

        let current_index = pages
            .iter()
            .position(|p| p == &self.current_page)
            .unwrap_or(0);

        let tabs = Tabs::new(titles)
            .block(Block::default().borders(Borders::ALL).title("Navigation"))
            .style(Style::default().fg(Color::White))
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .select(current_index);

        frame.render_widget(tabs, area);
    }
}

impl Default for Navigation {
    fn default() -> Self {
        Self::new()
    }
}
