use anyhow::Result;
use owo_colors::OwoColorize;
use polymarket_rs_client::ClobClient;

use crate::markets::{
    fetcher::{FetcherConfig, MarketFetcher},
    providers::ClobProvider,
    storage::MarketStorage,
    types::FetchState,
};

/// Fetch all markets with pagination and save to JSON file
///
/// TODO: This is a placeholder. The full implementation needs to be
/// migrated from the old markets.rs file. It includes:
/// - State management for resumable fetching
/// - Chunked file storage
/// - Progress tracking
pub async fn fetch_all_markets(
    client: ClobClient,
    output_dir: &str,
    verbose: bool,
    clear_state: bool,
    chunk_size_mb: f64,
) -> Result<()> {
    // Create storage
    let storage = MarketStorage::new(output_dir, chunk_size_mb)?;

    // Handle clear state
    if clear_state {
        if verbose {
            println!(
                "{}",
                "🗑️  Clearing previous state and data...".bright_yellow()
            );
        }
        storage.clear_all()?;
    }

    // Create provider
    let provider = ClobProvider::new(client);

    // Create fetcher
    // Create fetcher with verbose config
    let config = if verbose {
        FetcherConfig {
            verbose: true,
            ..Default::default()
        }
    } else {
        Default::default()
    };
    let mut fetcher = MarketFetcher::with_config(provider, storage, config);

    // Fetch all markets
    fetcher
        .fetch_all::<FetchState>("fetch_state.json", "markets")
        .await?;

    Ok(())
}
