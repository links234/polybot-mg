use anyhow::Result;
use clap::Parser;

mod auth;
mod config;
mod markets;
mod orders;
mod poland_election_markets;
mod cli;
mod data_paths;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenvy::dotenv().ok();
    
    // Parse CLI and execute
    let cli = cli::Cli::parse();
    cli.execute().await
}
