use crate::data_paths::DataPaths;
use anyhow::Result;
use clap::Args;

#[derive(Args, Clone)]
pub struct BookArgs {
    /// Token ID
    pub token_id: String,

    /// Number of levels to show
    #[arg(long, default_value = "5")]
    pub depth: usize,
}

pub struct BookCommand {
    args: BookArgs,
}

impl BookCommand {
    pub fn new(args: BookArgs) -> Self {
        Self { args }
    }

    pub async fn execute(&self, host: &str, data_paths: DataPaths) -> Result<()> {
        let client = crate::auth::get_authenticated_client(host, &data_paths).await?;
        crate::markets::show_orderbook(client, &self.args.token_id, self.args.depth).await?;
        Ok(())
    }
}
