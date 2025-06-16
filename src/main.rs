use anyhow::Result;
use clap::Parser;

mod auth;
mod config;
mod markets;
mod orders;
mod cli;
mod data_paths;
mod ws;
mod services;
mod pipeline;
mod datasets;
mod errors;
mod logging;
mod tui;
mod types;

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
