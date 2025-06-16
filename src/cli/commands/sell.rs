use anyhow::Result;
use clap::Args;
use rust_decimal::Decimal;
use tracing::warn;
use crate::data_paths::DataPaths;

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
        // Check confirmation in non-production environments
        if !self.args.yes && std::env::var("RUST_ENV").unwrap_or_default() != "production" {
            warn!("⚠️  Order confirmation required. Use --yes to confirm.");
            return Ok(());
        }
        
        let mut client = crate::auth::get_authenticated_client(host, &data_paths).await?;
        crate::orders::place_sell_order(&mut client, &self.args.token_id, self.args.price, self.args.size).await?;
        Ok(())
    }
}
