use anyhow::Result;
use clap::Args;
use owo_colors::OwoColorize;
use crate::data_paths::DataPaths;

#[derive(Args)]
pub struct FetchAllMarketsArgs {
    /// Show progress updates
    #[arg(long)]
    pub verbose: bool,
    
    /// Clear previous state and start fresh
    #[arg(long)]
    pub clear_state: bool,
    
    /// Maximum file size in MB for each chunk (default: 100)
    #[arg(long, default_value = "100")]
    pub chunk_size_mb: f64,
    
    /// Use Gamma API instead of CLOB API (different data structure)
    #[arg(long)]
    pub use_gamma: bool,
}

pub async fn execute(host: &str, data_paths: DataPaths, args: FetchAllMarketsArgs) -> Result<()> {
    if args.use_gamma {
        println!("{}", "🌐 Using Gamma API for market data...".bright_cyan());
        let output_dir = data_paths.markets_gamma();
        crate::markets::fetch_all_markets_gamma(
            output_dir.to_str().unwrap(), 
            args.verbose, 
            args.clear_state, 
            args.chunk_size_mb
        ).await?;
    } else {
        let client = crate::auth::get_authenticated_client(host, &data_paths).await?;
        let output_dir = data_paths.markets_clob();
        crate::markets::fetch_all_markets(
            client, 
            output_dir.to_str().unwrap(), 
            args.verbose, 
            args.clear_state, 
            args.chunk_size_mb
        ).await?;
    }
    Ok(())
} 