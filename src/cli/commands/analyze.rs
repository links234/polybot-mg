use anyhow::Result;
use clap::Args;
use crate::data_paths::DataPaths;

#[derive(Args, Clone)]
pub struct AnalyzeArgs {
    /// Name for the filtered dataset
    pub dataset_name: String,
    
    /// Source dataset path or name to analyze
    #[arg(long)]
    pub source_dataset: String,
    
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
    
    /// Filter: title must contain this text (case-insensitive)
    #[arg(long)]
    pub title_contains: Option<String>,
    
    /// Filter: title must match this regex pattern
    #[arg(long)]
    pub title_regex: Option<String>,
    
    /// Filter: title must contain ANY of these keywords (comma-separated)
    #[arg(long)]
    pub title_contains_any: Option<String>,
    
    /// Filter: title must contain ALL of these keywords (comma-separated)
    #[arg(long)]
    pub title_contains_all: Option<String>,
    
    /// Filter: description must contain this text (case-insensitive)
    #[arg(long)]
    pub description_contains: Option<String>,
    
    /// Filter: fuzzy search in title and description (0.0-1.0 threshold)
    #[arg(long)]
    pub fuzzy_search: Option<String>,
    
    /// Filter: fuzzy search threshold (0.0-1.0, default 0.7)
    #[arg(long, default_value = "0.7")]
    pub fuzzy_threshold: f64,
    
    /// Filter: search in all text fields (title, description, tags)
    #[arg(long)]
    pub text_search: Option<String>,
    
    /// Include detailed analysis in output
    #[arg(long)]
    pub detailed: bool,
    
    /// Show analysis summary
    #[arg(long)]
    pub summary: bool,
}

pub struct AnalyzeCommand {
    args: AnalyzeArgs,
}

impl AnalyzeCommand {
    pub fn new(args: AnalyzeArgs) -> Self {
        Self { args }
    }

    pub async fn execute(&self, _host: &str, data_paths: DataPaths) -> Result<()> {
        crate::markets::analyze_markets(data_paths, self.args.clone()).await
    }
} 