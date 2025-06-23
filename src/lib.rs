pub mod auth;
pub mod auth_env;
pub mod cli;
pub mod config;
pub mod data_paths;
pub use data_paths as data;
pub mod datasets;
pub mod errors;
pub mod ethereum_utils;
pub mod execution;
pub mod gui;
pub mod logging;
pub mod markets;
pub mod pipeline;
pub mod portfolio;
pub mod services;
pub mod storage;
pub mod tui;
pub mod types;
pub mod ws;

// Re-export the GUI launcher function at the root level
pub use gui::launch_trading_canvas;
pub mod file_store;
pub mod address_book;
pub mod gamma_api;
pub mod typed_store;
