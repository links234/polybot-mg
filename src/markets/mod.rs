// Market-related functionality split into focused modules

mod active;
mod analyze;
mod cache;
mod display;
mod enrich;
mod fetch;
pub mod fetcher;
mod filtered;
mod list;
mod orderbook;
mod providers;
mod search;
mod storage;
mod types;
mod utils;

// Re-export public functions
pub use active::list_active_markets;
pub use analyze::analyze_markets;
pub use enrich::enrich_markets;
pub use fetch::{fetch_all_markets, fetch_all_markets_gamma};
pub use filtered::list_filtered_markets;
pub use list::list_markets;
pub use orderbook::show_orderbook;
pub use search::{get_market_details, get_market_from_url, search_markets};
