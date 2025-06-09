use anyhow::Result;
use clap::Args;
use owo_colors::OwoColorize;
use crate::data_paths::DataPaths;

#[derive(Args)]
pub struct CancelArgs {
    /// Order ID
    pub order_id: String,
    
    /// Confirm cancellation (required unless RUST_ENV=production)
    #[arg(long)]
    pub yes: bool,
}

pub async fn execute(host: &str, data_paths: DataPaths, args: CancelArgs) -> Result<()> {
    // Check confirmation in non-production environments
    if !args.yes && std::env::var("RUST_ENV").unwrap_or_default() != "production" {
        println!("{}", "⚠️  Cancellation confirmation required. Use --yes to confirm.".yellow());
        return Ok(());
    }
    
    let mut client = crate::auth::get_authenticated_client(host, &data_paths).await?;
    crate::orders::cancel_order(&mut client, &args.order_id).await?;
    Ok(())
} 