// Fetch functionality is split into separate modules for better organization
mod clob_fetch;
mod gamma_fetch;

pub use clob_fetch::fetch_all_markets;
pub use gamma_fetch::fetch_all_markets_gamma;
