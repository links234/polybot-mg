use crate::data_paths::DataPaths;
use anyhow::{Result, anyhow};
use clap::Args;
use tracing::{info, warn};
use crate::portfolio::command_handlers::enhanced_cancel_command;

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
            return Err(anyhow!("Cancellation confirmation required"));
        }
        
        info!("Executing cancel command for order: {}", self.args.order_id);
        
        // Use the enhanced cancel command from portfolio system
        enhanced_cancel_command(
            &self.args.order_id,
            host,
            data_paths,
        ).await?;
        
        Ok(())
    }
}
