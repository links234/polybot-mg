use anyhow::Result;
use clap::Parser;

mod auth;
mod auth_env;
mod cli;
mod config;
mod core;
mod data_paths;
use data_paths as data;
mod errors;
mod ethereum_utils;
mod gui;
mod logging;
mod markets;
mod pipeline;
mod storage;
mod strategy;
mod tui;
mod typed_store;
mod types;
mod address_book;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Parse CLI and execute (CLI will handle logging initialization)
    let cli = cli::Cli::parse();

    // Execute with error handling
    let result = match cli.execute().await {
        Ok(()) => {
            logging::log_session_end();
            Ok(())
        }
        Err(e) => {
            // Log the error using tracing (will respect logging configuration)
            tracing::error!("Application error: {}", e);

            // Log error chain if available
            let mut source = e.source();
            while let Some(err) = source {
                tracing::error!("   Caused by: {}", err);
                source = err.source();
            }

            logging::log_session_end();
            Err(e)
        }
    };
    
    // Give a final moment for async tasks to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    
    result
}
