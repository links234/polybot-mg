// Command modules
mod init;
mod markets;
mod fetch_all_markets;
mod analyze;
mod enrich;
mod book;
mod buy;
mod sell;
mod cancel;
mod orders;

// Re-export command argument structs
pub use init::InitArgs;
pub use markets::{MarketsArgs, MarketMode};
pub use fetch_all_markets::FetchAllMarketsArgs;
pub use analyze::AnalyzeArgs;
pub use enrich::EnrichArgs;
pub use book::BookArgs;
pub use buy::BuyArgs;
pub use sell::SellArgs;
pub use cancel::CancelArgs;
pub use orders::OrdersArgs;

// Re-export command execution functions
pub use init::execute as execute_init;
pub use markets::execute as execute_markets;
pub use fetch_all_markets::execute as execute_fetch_all_markets;
pub use analyze::execute as execute_analyze;
pub use enrich::execute as execute_enrich;
pub use book::execute as execute_book;
pub use buy::execute as execute_buy;
pub use sell::execute as execute_sell;
pub use cancel::execute as execute_cancel;
pub use orders::execute as execute_orders; 