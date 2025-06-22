//! Portfolio CLI command for displaying orders and positions

use anyhow::Result;
use clap::Args;
use crate::data_paths::DataPaths;

#[derive(Args, Debug)]
pub struct PortfolioArgs {
    /// Show only orders for specific market
    #[arg(short, long)]
    market: Option<String>,
    
    /// Show only orders for specific asset
    #[arg(short, long)]
    asset: Option<String>,
    
    /// Use simple text output instead of interactive TUI
    #[arg(long, short = 't')]
    text: bool,
}

pub async fn portfolio(args: PortfolioArgs, host: &str, data_paths: DataPaths) -> Result<()> {
    // Use the enhanced portfolio command with the new portfolio system
    use crate::portfolio::command_handlers::enhanced_portfolio_command;
    
    enhanced_portfolio_command(
        args.market,
        args.asset,
        args.text,
        host,
        data_paths,
    ).await
}