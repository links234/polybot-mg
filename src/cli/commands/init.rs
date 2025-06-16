use anyhow::Result;
use clap::Args;
use tracing::info;
use crate::data_paths::DataPaths;

#[derive(Args)]
pub struct InitArgs {
    /// Private key in hex format (without 0x prefix)
    #[arg(long = "pk")]
    pub private_key: String,
    
    /// Nonce for API key derivation (default: 0)
    #[arg(long, default_value = "0")]
    pub nonce: u64,
}

pub struct InitCommand {
    args: InitArgs,
}

impl InitCommand {
    pub fn new(args: InitArgs) -> Self {
        Self { args }
    }

    pub async fn execute(&self, host: &str, data_paths: DataPaths) -> Result<()> {
        info!("🔐 Initializing Polymarket authentication...");
        crate::auth::init_auth(host, &data_paths, &self.args.private_key, self.args.nonce).await?;
        info!("✅ Authentication successful! Credentials saved.");
        Ok(())
    }
} 