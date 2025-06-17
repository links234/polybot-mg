// Market-related functionality split into focused modules

mod types;
mod list;
mod fetch;
mod orderbook;
mod search;
mod active;
mod filtered;
mod display;
mod utils;
mod cache;
mod storage;
mod providers;
pub mod fetcher;
mod analyze;
mod enrich;

// Re-export public functions
pub use list::list_markets;
pub use fetch::{fetch_all_markets, fetch_all_markets_gamma};
pub use orderbook::show_orderbook;
pub use search::{search_markets, get_market_details, get_market_from_url};
pub use active::list_active_markets;
pub use filtered::list_filtered_markets;
pub use analyze::analyze_markets;
pub use enrich::enrich_markets; 