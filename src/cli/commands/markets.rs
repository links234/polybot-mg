use anyhow::Result;
use clap::{Args, ValueEnum};
use owo_colors::OwoColorize;
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

#[derive(Args)]
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

pub async fn execute(host: &str, data_paths: DataPaths, args: MarketsArgs) -> Result<()> {
    let client = crate::auth::get_authenticated_client(host, &data_paths).await?;
    
    match args.mode {
        MarketMode::List => {
            // Basic list with optional filter
            crate::markets::list_markets(client, args.query, args.limit).await?;
        }
        MarketMode::Volume => {
            // Volume-sorted markets with cache
            crate::markets::list_filtered_markets(
                client, 
                args.limit, 
                args.refresh, 
                args.min_volume, 
                args.detailed, 
                args.min_price, 
                args.max_price
            ).await?;
        }
        MarketMode::Active => {
            // Actively traded markets with orderbook data
            crate::markets::list_active_markets(
                client, 
                args.limit, 
                args.min_price, 
                args.max_price, 
                args.min_spread, 
                args.max_spread, 
                args.detailed
            ).await?;
        }
        MarketMode::Search => {
            // Search by keyword
            if let Some(keyword) = args.query {
                crate::markets::search_markets(client, &keyword, args.detailed, args.limit).await?;
            } else {
                println!("{}", "❌ Search mode requires a query term".bright_red());
                println!("{}", "Usage: polybot markets <search_term> --mode search".bright_cyan());
            }
        }
        MarketMode::Details => {
            // Get specific market details
            if let Some(identifier) = args.query {
                crate::markets::get_market_details(client, &identifier).await?;
            } else {
                println!("{}", "❌ Details mode requires a market identifier".bright_red());
                println!("{}", "Usage: polybot markets <condition_id_or_slug> --mode details".bright_cyan());
            }
        }
        MarketMode::Url => {
            // Get market from URL
            if let Some(url) = args.query {
                crate::markets::get_market_from_url(&url).await?;
            } else {
                println!("{}", "❌ URL mode requires a Polymarket URL".bright_red());
                println!("{}", "Usage: polybot markets <polymarket_url> --mode url".bright_cyan());
            }
        }
    }
    
    Ok(())
} 