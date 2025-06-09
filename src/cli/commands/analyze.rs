use anyhow::Result;
use clap::Args;
use crate::data_paths::DataPaths;

#[derive(Args)]
pub struct AnalyzeArgs {
    /// Name for the filtered dataset
    pub dataset_name: String,
    
    /// Source of markets to analyze (clob or gamma)
    #[arg(long, default_value = "clob")]
    pub source: String,
    
    /// Filter: minimum price for YES outcome (0-100)
    #[arg(long, value_parser = crate::cli::parse_percentage)]
    pub min_price: Option<f64>,
    
    /// Filter: maximum price for YES outcome (0-100)
    #[arg(long, value_parser = crate::cli::parse_percentage)]
    pub max_price: Option<f64>,
    
    /// Filter: only active markets
    #[arg(long)]
    pub active_only: bool,
    
    /// Filter: only markets accepting orders
    #[arg(long)]
    pub accepting_orders_only: bool,
    
    /// Filter: only open markets (not closed)
    #[arg(long)]
    pub open_only: bool,
    
    /// Filter: exclude archived markets
    #[arg(long)]
    pub no_archived: bool,
    
    /// Filter: markets by category (comma-separated)
    #[arg(long)]
    pub categories: Option<String>,
    
    /// Filter: markets by tags (comma-separated)
    #[arg(long)]
    pub tags: Option<String>,
    
    /// Filter: minimum order size
    #[arg(long)]
    pub min_order_size: Option<f64>,
    
    /// Filter: created after date (ISO format)
    #[arg(long)]
    pub created_after: Option<String>,
    
    /// Filter: ending before date (ISO format)
    #[arg(long)]
    pub ending_before: Option<String>,
    
    /// Include detailed analysis in output
    #[arg(long)]
    pub detailed: bool,
    
    /// Show analysis summary
    #[arg(long)]
    pub summary: bool,
}

pub async fn execute(_host: &str, data_paths: DataPaths, args: AnalyzeArgs) -> Result<()> {
    crate::markets::analyze_markets(data_paths, args).await
} 