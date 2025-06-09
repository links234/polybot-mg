use anyhow::Result;
use clap::Args;
use owo_colors::OwoColorize;
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

pub async fn execute(host: &str, data_paths: DataPaths, args: InitArgs) -> Result<()> {
    println!("{}", "🔐 Initializing Polymarket authentication...".bright_blue());
    crate::auth::init_auth(host, &data_paths, &args.private_key, args.nonce).await?;
    println!("{}", "✅ Authentication successful! Credentials saved.".bright_green());
    Ok(())
} 