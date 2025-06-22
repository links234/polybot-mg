//! Pane definitions for the trading interface

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Pane {
    /// Orders management pane
    Orders,
    /// Real-time market streams
    Streams,
    /// Portfolio overview
    Portfolio,
    /// Token information and selection
    Tokens,
    /// Market depth and order book for a specific token
    MarketDepth(Option<String>),
    /// Price charts and analysis
    Charts,
    /// Trade history
    TradeHistory,
    /// Account balances
    Balances,
    /// WebSocket connections manager
    WebSocketManager,
    /// Individual worker details and event stream
    WorkerDetails(usize),
}

impl Pane {
    pub fn title(&self) -> String {
        match self {
            Pane::Orders => format!("{} Orders", self.icon()),
            Pane::Streams => format!("{} Market Streams", self.icon()),
            Pane::Portfolio => format!("{} Portfolio", self.icon()),
            Pane::Tokens => format!("{} Tokens", self.icon()),
            Pane::MarketDepth(token_id) => {
                if let Some(id) = token_id {
                    // Truncate token ID for display
                    let display_id = if id.len() > 12 {
                        format!("{}...", &id[..12])
                    } else {
                        id.clone()
                    };
                    format!("{} Market Depth - {}", self.icon(), display_id)
                } else {
                    format!("{} Market Depth", self.icon())
                }
            }
            Pane::Charts => format!("{} Charts", self.icon()),
            Pane::TradeHistory => format!("{} Trade History", self.icon()),
            Pane::Balances => format!("{} Balances", self.icon()),
            Pane::WebSocketManager => format!("{} WebSocket Manager", self.icon()),
            Pane::WorkerDetails(worker_id) => {
                format!("{} Worker #{} Details", self.icon(), worker_id)
            }
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Pane::Orders => "📋",
            Pane::Streams => "📡",
            Pane::Portfolio => "💼",
            Pane::Tokens => "🪙",
            Pane::MarketDepth(_) => "📊",
            Pane::Charts => "📈",
            Pane::TradeHistory => "📜",
            Pane::Balances => "💰",
            Pane::WebSocketManager => "🔌",
            Pane::WorkerDetails(_) => "👷",
        }
    }

    pub fn tab_title(&self) -> String {
        match self {
            Pane::MarketDepth(Some(token_id)) => {
                // Truncate token ID for display
                let display_id = if token_id.len() > 12 {
                    format!("{}...", &token_id[..12])
                } else {
                    token_id.clone()
                };
                format!("{} Market Depth - {}", self.icon(), display_id)
            }
            Pane::WorkerDetails(worker_id) => {
                format!("{} Worker #{}", self.icon(), worker_id)
            }
            _ => format!("{} {}", self.icon(), self.title()),
        }
    }
}
