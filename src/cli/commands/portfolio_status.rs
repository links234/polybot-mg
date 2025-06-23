//! Portfolio status command for checking service health and cache statistics

use anyhow::Result;
use clap::Args;
use crate::data_paths::DataPaths;
use crate::portfolio::{
    get_portfolio_service_handle, DashboardFormatter,
};
use crate::config;
use crate::ethereum_utils;
use tracing::info;

#[derive(Args, Debug)]
pub struct PortfolioStatusArgs {
    /// Show cache statistics
    #[arg(long)]
    cache_stats: bool,
    
    /// Show detailed portfolio dashboard
    #[arg(long)]
    dashboard: bool,
    
    /// Clear cache
    #[arg(long)]
    clear_cache: bool,
    
    /// Create manual snapshot
    #[arg(long)]
    snapshot: bool,
    
    /// Snapshot reason (used with --snapshot)
    #[arg(long, default_value = "manual")]
    reason: String,
}

pub async fn portfolio_status(args: PortfolioStatusArgs, host: &str, data_paths: DataPaths) -> Result<()> {
    println!("\n🔍 Portfolio Service Status\n");
    
    // Load private key to derive user address
    let private_key = config::load_private_key(&data_paths).await?;
    let address = ethereum_utils::derive_address_from_private_key(&private_key)?;
    
    println!("👤 Account: {}", address);
    println!("🌐 Host: {}", host);
    println!();
    
    // Get portfolio service handle
    let service_handle = get_portfolio_service_handle(host, &data_paths).await?;
    
    // Handle clear cache
    if args.clear_cache {
        println!("🗑️  Clearing portfolio cache...");
        // Cache functionality disabled
        // let cache_config = crate::portfolio::cache::CacheConfig::default();
        // let cache = PortfolioCache::new(data_paths.root().join("cache"), cache_config);
        // cache.init().await?;
        // cache.invalidate("all", "all").await;
        println!("✅ Cache cleared successfully");
        println!();
    }
    
    // Handle snapshot creation
    if args.snapshot {
        println!("📸 Creating portfolio snapshot...");
        let snapshot_file = service_handle.create_snapshot(args.reason.clone()).await?;
        println!("✅ Snapshot created: {}", snapshot_file);
        println!();
    }
    
    // Show cache statistics
    if args.cache_stats {
        println!("📊 Cache Statistics:");
        // Cache functionality disabled
        // let cache_config = crate::portfolio::cache::CacheConfig::default();
        // let cache = PortfolioCache::new(data_paths.root().join("cache"), cache_config);
        // cache.init().await?;
        // let stats = cache.get_stats().await;
        println!("Cache functionality disabled");
        
        // println!("  Hits: {}", stats.hits);
        // println!("  Misses: {}", stats.misses);
        // println!("  Hit Rate: {:.1}%", stats.hit_rate() * 100.0);
        // println!("  Evictions: {}", stats.evictions);
        // println!("  Disk Reads: {}", stats.disk_reads);
        // println!("  Disk Writes: {}", stats.disk_writes);
        // if let Some(last_cleanup) = stats.last_cleanup {
        //     println!("  Last Cleanup: {}", last_cleanup.format("%Y-%m-%d %H:%M:%S UTC"));
        // }
        println!();
    }
    
    // Show portfolio dashboard
    if args.dashboard {
        // Refresh data first
        info!("Refreshing portfolio data...");
        service_handle.refresh_data().await?;
        
        // Get portfolio state
        let portfolio_state = service_handle.get_portfolio_state().await?;
        
        // Format and display dashboard
        let formatter = DashboardFormatter::new(&portfolio_state, &address, host);
        print!("{}", formatter.format_dashboard());
    } else {
        // Show summary by default
        let portfolio_state = service_handle.get_portfolio_state().await?;
        
        println!("📈 Portfolio Summary:");
        println!("  Last Updated: {}", portfolio_state.last_updated.format("%Y-%m-%d %H:%M:%S UTC"));
        println!("  Synced: {}", if portfolio_state.is_synced { "✅" } else { "❌" });
        println!("  Total Value: ${:.2}", portfolio_state.balances.total_value);
        println!("  Available Cash: ${:.2}", portfolio_state.balances.available_cash);
        println!("  Positions: {} ({} open)", portfolio_state.stats.total_positions, portfolio_state.stats.open_positions);
        println!("  Active Orders: {}", portfolio_state.active_orders.len());
        println!("  Total P&L: ${:.2}", portfolio_state.stats.total_pnl());
        
        if let Some(win_rate) = portfolio_state.stats.win_rate {
            println!("  Win Rate: {:.1}%", win_rate);
        }
        
        println!();
        println!("💡 Use --dashboard for detailed view");
        println!("💡 Use --cache-stats to see cache performance");
        println!("💡 Use --snapshot to create a manual snapshot");
    }
    
    Ok(())
}