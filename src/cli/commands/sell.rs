use anyhow::Result;
use clap::Args;
use rust_decimal::Decimal;
use tracing::info;
use crate::data_paths::DataPaths;
use crate::portfolio::command_handlers::enhanced_sell_command;

#[derive(Args, Clone)]
pub struct SellArgs {
    /// Token ID
    pub token_id: String,
    
    /// Price in USDC (e.g., 0.52)
    #[arg(long)]
    pub price: Decimal,
    
    /// Size in USDC
    #[arg(long)]
    pub size: Decimal,
    
    /// Market ID (optional, will use token_id if not provided)
    #[arg(long)]
    pub market_id: Option<String>,
    
    /// Confirm order placement (required unless RUST_ENV=production)
    #[arg(long)]
    pub yes: bool,
}

pub struct SellCommand {
    args: SellArgs,
}

impl SellCommand {
    pub fn new(args: SellArgs) -> Self {
        Self { args }
    }

    pub async fn execute(&self, host: &str, data_paths: DataPaths) -> Result<()> {
        info!("Executing sell command for token: {}", self.args.token_id);
        
        // Use the enhanced sell command from portfolio system
        enhanced_sell_command(
            &self.args.token_id,
            self.args.price,
            self.args.size,
            self.args.market_id.clone(),
            self.args.yes,
            host,
            data_paths,
        ).await?;
        
        Ok(())
    }
}
