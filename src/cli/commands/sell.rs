use anyhow::Result;
use clap::Args;
use rust_decimal::Decimal;
use owo_colors::OwoColorize;
use crate::data_paths::DataPaths;

#[derive(Args)]
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

pub async fn execute(host: &str, data_paths: DataPaths, args: SellArgs) -> Result<()> {
    // Check confirmation in non-production environments
    if !args.yes && std::env::var("RUST_ENV").unwrap_or_default() != "production" {
        println!("{}", "⚠️  Order confirmation required. Use --yes to confirm.".yellow());
        return Ok(());
    }
    
    let mut client = crate::auth::get_authenticated_client(host, &data_paths).await?;
    crate::orders::place_sell_order(&mut client, &args.token_id, args.price, args.size).await?;
    Ok(())
}
