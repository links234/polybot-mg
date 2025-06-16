use anyhow::Result;
use clap::Args;
use crate::data_paths::DataPaths;

#[derive(Args, Clone)]
pub struct EnrichArgs {
    /// Name of the source dataset to enrich
    pub source_dataset: String,
    
    /// Name for the enriched dataset
    pub output_dataset: String,
    
    /// Include orderbook data (best bid/ask, spread, depth)
    #[arg(long, default_value = "true")]
    pub include_orderbook: bool,
    
    /// Include liquidity metrics (total bid/ask size)
    #[arg(long, default_value = "true")]
    pub include_liquidity: bool,
    
    /// Include volume data from Gamma API
    #[arg(long)]
    pub include_volume: bool,
    
    /// Maximum orderbook depth to fetch (levels on each side)
    #[arg(long, default_value = "10")]
    pub max_depth: usize,
    
    /// Number of markets to process in parallel
    #[arg(long, default_value = "5")]
    pub parallel: usize,
    
    /// Delay between API calls in milliseconds
    #[arg(long, default_value = "100")]
    pub delay_ms: u64,
    
    /// Show progress while enriching
    #[arg(long)]
    pub progress: bool,
    
    /// Continue from a specific market index (for resuming)
    #[arg(long)]
    pub start_from: Option<usize>,
}

pub struct EnrichCommand {
    args: EnrichArgs,
}

impl EnrichCommand {
    pub fn new(args: EnrichArgs) -> Self {
        Self { args }
    }

    pub async fn execute(&self, host: &str, data_paths: DataPaths) -> Result<()> {
        crate::markets::enrich_markets(host, data_paths, self.args.clone()).await
    }
} 