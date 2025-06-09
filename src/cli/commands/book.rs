use anyhow::Result;
use clap::Args;
use crate::data_paths::DataPaths;

#[derive(Args)]
pub struct BookArgs {
    /// Token ID
    pub token_id: String,
    
    /// Number of levels to show
    #[arg(long, default_value = "5")]
    pub depth: usize,
}

pub async fn execute(host: &str, data_paths: DataPaths, args: BookArgs) -> Result<()> {
    let client = crate::auth::get_authenticated_client(host, &data_paths).await?;
    crate::markets::show_orderbook(client, &args.token_id, args.depth).await?;
    Ok(())
} 