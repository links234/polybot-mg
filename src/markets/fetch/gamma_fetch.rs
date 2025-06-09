use anyhow::Result;
use owo_colors::OwoColorize;

use crate::markets::{
    storage::MarketStorage,
    providers::GammaProvider,
    fetcher::MarketFetcher,
    types::GammaFetchState,
};

/// Fetch all markets from Gamma API with pagination and save to JSON file
/// 
/// TODO: This is a placeholder. The full implementation needs to be
/// migrated from the old markets.rs file. It includes:
/// - Offset-based pagination
/// - State management for resumable fetching
/// - Chunked file storage
pub async fn fetch_all_markets_gamma(
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
            println!("{}", "🗑️  Clearing previous state and data...".bright_yellow());
        }
        storage.clear_all()?;
    }
    
    // Create provider
    let provider = GammaProvider::new();
    
    // Create fetcher
    let mut fetcher = MarketFetcher::new(provider, storage, verbose);
    
    // Fetch all markets
    fetcher.fetch_all::<GammaFetchState>("gamma_fetch_state.json", "gamma_markets").await?;
    
    Ok(())
} 