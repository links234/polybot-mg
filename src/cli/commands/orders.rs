use anyhow::Result;
use clap::Args;
use crate::data_paths::DataPaths;

#[derive(Args, Clone)]
pub struct OrdersArgs {
    /// Filter by token ID
    #[arg(long)]
    pub token_id: Option<String>,
}

pub struct OrdersCommand {
    args: OrdersArgs,
}

impl OrdersCommand {
    pub fn new(args: OrdersArgs) -> Self {
        Self { args }
    }

    pub async fn execute(&self, host: &str, data_paths: DataPaths) -> Result<()> {
        let client = crate::auth::get_authenticated_client(host, &data_paths).await?;
        crate::orders::list_orders(client, self.args.token_id.clone()).await?;
        Ok(())
    }
} 