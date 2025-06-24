use crate::data_paths::DataPaths;
use anyhow::Result;
use clap::Args;
use crate::core::portfolio::cli::PortfolioCommandHandlers;
use crate::core::portfolio::display::{OrdersFormatter, DashboardFormatter};
use tracing::info;

#[derive(Args, Clone)]
pub struct OrdersArgs {
    /// Filter by token ID
    #[arg(long)]
    pub token_id: Option<String>,
    
    /// Show dashboard view with portfolio stats
    #[arg(long, short = 'd')]
    pub dashboard: bool,
}

pub struct OrdersCommand {
    args: OrdersArgs,
}

impl OrdersCommand {
    pub fn new(args: OrdersArgs) -> Self {
        Self { args }
    }

    pub async fn execute(&self, host: &str, data_paths: DataPaths) -> Result<()> {
        info!("Executing orders command");
        
        // Create portfolio command handlers
        let handlers = PortfolioCommandHandlers::new(host.to_string(), data_paths).await?;
        
        // Refresh data first
        if let Err(e) = handlers.refresh_data().await {
            tracing::warn!("Failed to refresh portfolio data: {}", e);
        }
        
        // Get active orders
        let mut orders = handlers.get_active_orders().await?;
        
        // Filter by token ID if provided
        if let Some(token_id) = &self.args.token_id {
            orders.retain(|o| o.token_id.contains(token_id));
        }
        
        if self.args.dashboard {
            // Show full dashboard
            let portfolio_state = handlers.get_portfolio_state().await?;
            let dashboard = DashboardFormatter::new(
                &portfolio_state,
                handlers.get_address(),
                handlers.get_host(),
            );
            println!("{}", dashboard.format_dashboard());
        } else {
            // Show just orders
            println!("\n📋 Active Orders\n");
            println!("👤 Account: {}", handlers.get_address());
            println!("🌐 Host: {}\n", handlers.get_host());
            
            let formatter = OrdersFormatter::new(&orders);
            println!("{}", formatter.format_table());
            
            if orders.is_empty() {
                println!("💡 No active orders found");
                if self.args.token_id.is_some() {
                    println!("💡 Try removing the --token-id filter to see all orders");
                }
                println!("💡 Use 'polybot buy' or 'polybot sell' to place new orders");
            } else {
                println!("💡 Use 'polybot cancel <order-id>' to cancel an order");
                println!("💡 Use 'polybot orders --dashboard' for full portfolio view");
            }
        }
        
        Ok(())
    }
}
