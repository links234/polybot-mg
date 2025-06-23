use anyhow::Result;
use clap::Parser;

mod auth;
mod auth_env;
mod cli;
mod config;
mod data_paths;
use data_paths as data;
mod datasets;
mod errors;
mod ethereum_utils;
mod execution;
mod file_store;
mod gui;
mod logging;
mod markets;
mod pipeline;
mod portfolio;
mod services;
mod storage;
mod tui;
mod typed_store;
mod types;
mod ws;
mod address_book;
mod gamma_api;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Parse CLI and execute (CLI will handle logging initialization)
    let cli = cli::Cli::parse();

    // Execute with error handling
    match cli.execute().await {
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
    }
}
