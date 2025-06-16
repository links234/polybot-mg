use anyhow::Result;
use clap::{Args, ValueEnum};
use tracing::{info, error};
use crate::data_paths::DataPaths;

#[derive(Debug, Clone, ValueEnum)]
pub enum MarketMode {
    /// List all markets (default)
    List,
    /// Show markets sorted by volume (cached data)
    Volume,
    /// Show actively traded markets (live orderbook data)
    Active,
    /// Search markets by keyword
    Search,
    /// Get details for a specific market
    Details,
    /// Get market from Polymarket URL
    Url,
}

#[derive(Args, Clone)]
pub struct MarketsArgs {
    /// Search term, market ID, or Polymarket URL (optional)
    pub query: Option<String>,
    
    /// Mode of operation
    #[arg(long, short = 'm', value_enum, default_value = "list")]
    pub mode: MarketMode,
    
    /// Maximum number of markets to display
    #[arg(long, short = 'n', default_value = "20")]
    pub limit: usize,
    
    /// Show detailed information
    #[arg(long, short = 'd')]
    pub detailed: bool,
    
    /// Force refresh cache (for volume mode)
    #[arg(long)]
    pub refresh: bool,
    
    /// Minimum volume filter in USD (for volume/active modes)
    #[arg(long)]
    pub min_volume: Option<f64>,
    
    /// Minimum price for YES outcome (0-100)
    #[arg(long, value_parser = crate::cli::parse_percentage)]
    pub min_price: Option<f64>,
    
    /// Maximum price for YES outcome (0-100)
    #[arg(long, value_parser = crate::cli::parse_percentage)]
    pub max_price: Option<f64>,
    
    /// Minimum spread between bid and ask (0-100) (for active mode)
    #[arg(long, value_parser = crate::cli::parse_percentage)]
    pub min_spread: Option<f64>,
    
    /// Maximum spread between bid and ask (0-100) (for active mode)
    #[arg(long, value_parser = crate::cli::parse_percentage)]
    pub max_spread: Option<f64>,
}

pub struct MarketsCommand {
    args: MarketsArgs,
}

impl MarketsCommand {
    pub fn new(args: MarketsArgs) -> Self {
        Self { args }
    }

    pub async fn execute(&self, host: &str, data_paths: DataPaths) -> Result<()> {
        let client = crate::auth::get_authenticated_client(host, &data_paths).await?;
        
        match self.args.mode {
            MarketMode::List => {
                // Basic list with optional filter
                crate::markets::list_markets(client, self.args.query.clone(), self.args.limit).await?;
            }
            MarketMode::Volume => {
                // Volume-sorted markets with cache
                crate::markets::list_filtered_markets(
                    client, 
                    self.args.limit, 
                    self.args.refresh, 
                    self.args.min_volume, 
                    self.args.detailed, 
                    self.args.min_price, 
                    self.args.max_price
                ).await?;
            }
            MarketMode::Active => {
                // Actively traded markets with orderbook data
                crate::markets::list_active_markets(
                    client, 
                    self.args.limit, 
                    self.args.min_price, 
                    self.args.max_price, 
                    self.args.min_spread, 
                    self.args.max_spread, 
                    self.args.detailed
                ).await?;
            }
            MarketMode::Search => {
                // Search by keyword
                if let Some(keyword) = &self.args.query {
                    crate::markets::search_markets(client, keyword, self.args.detailed, self.args.limit).await?;
                } else {
                    error!("❌ Search mode requires a query term");
                    info!("Usage: polybot markets <search_term> --mode search");
                }
            }
            MarketMode::Details => {
                // Get specific market details
                if let Some(identifier) = &self.args.query {
                    crate::markets::get_market_details(client, identifier).await?;
                } else {
                    error!("❌ Details mode requires a market identifier");
                    info!("Usage: polybot markets <condition_id_or_slug> --mode details");
                }
            }
            MarketMode::Url => {
                // Get market from URL
                if let Some(url) = &self.args.query {
                    crate::markets::get_market_from_url(url).await?;
                } else {
                    error!("❌ URL mode requires a Polymarket URL");
                    info!("Usage: polybot markets <polymarket_url> --mode url");
                }
            }
        }
        
        Ok(())
    }
} 