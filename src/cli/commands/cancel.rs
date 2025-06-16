use anyhow::Result;
use clap::Args;
use tracing::warn;
use crate::data_paths::DataPaths;

#[derive(Args, Clone)]
pub struct CancelArgs {
    /// Order ID
    pub order_id: String,
    
    /// Confirm cancellation (required unless RUST_ENV=production)
    #[arg(long)]
    pub yes: bool,
}

pub struct CancelCommand {
    args: CancelArgs,
}

impl CancelCommand {
    pub fn new(args: CancelArgs) -> Self {
        Self { args }
    }

    pub async fn execute(&self, host: &str, data_paths: DataPaths) -> Result<()> {
        // Check confirmation in non-production environments
        if !self.args.yes && std::env::var("RUST_ENV").unwrap_or_default() != "production" {
            warn!("⚠️  Cancellation confirmation required. Use --yes to confirm.");
            return Ok(());
        }
        
        let mut client = crate::auth::get_authenticated_client(host, &data_paths).await?;
        crate::execution::orders::cancel_order(&mut client, &self.args.order_id).await?;
        Ok(())
    }
} 