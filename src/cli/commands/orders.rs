use anyhow::Result;
use clap::Args;
use crate::data_paths::DataPaths;

#[derive(Args)]
pub struct OrdersArgs {
    /// Filter by token ID
    #[arg(long)]
    pub token_id: Option<String>,
}

pub async fn execute(host: &str, data_paths: DataPaths, args: OrdersArgs) -> Result<()> {
    let client = crate::auth::get_authenticated_client(host, &data_paths).await?;
    crate::orders::list_orders(client, args.token_id).await?;
    Ok(())
} 