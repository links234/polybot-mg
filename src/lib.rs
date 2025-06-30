pub mod auth;
pub mod auth_env;
pub mod cli;
pub mod config;
pub mod core;
pub mod data_paths;
pub use data_paths as data;
pub mod errors;
pub mod ethereum_utils;
pub mod gui;
pub mod logging;
pub mod markets;
pub mod pipeline;
pub mod storage;
pub mod strategy;
pub mod tui;
pub mod types;
pub mod address_book;
pub mod typed_store;

// Re-export the GUI launcher function at the root level
pub use gui::launch_trading_canvas;
