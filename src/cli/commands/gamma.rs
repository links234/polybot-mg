//! Gamma API CLI commands

use anyhow::Result;
use clap::{Args, Subcommand};
use tracing::{error, warn};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::data::DataPaths;
use crate::address_book::service::get_address_book_service;
use crate::gamma_api::tracker::{get_gamma_tracker, TrackerCommand, GammaTrackerHandle};
use crate::gamma_api::types::{PositionState, ActivityType};

/// Gamma API commands for historical data
#[derive(Debug, Args, Clone)]
pub struct GammaArgs {
    #[command(subcommand)]
    pub command: GammaSubcommand,
}

/// Gamma API subcommands
#[derive(Debug, Subcommand, Clone)]
pub enum GammaSubcommand {
    /// Track an address for Gamma data collection
    Track(TrackArgs),
    
    /// Untrack an address
    Untrack(UntrackArgs),
    
    /// Sync data for tracked addresses
    Sync(SyncArgs),
    
    /// List all tracked addresses
    List(ListArgs),
    
    /// Show detailed information for an address
    Show(ShowArgs),
    
    /// Show positions for an address
    Positions(PositionsArgs),
    
    /// Show activity for an address
    Activity(ActivityArgs),
    
    /// Show portfolio summary for an address
    Portfolio(PortfolioArgs),
    
    /// Sync all tracked addresses
    SyncAll(SyncAllArgs),
    
    /// Show storage statistics
    Stats(StatsArgs),
}

/// Track address arguments
#[derive(Debug, Args, Clone)]
pub struct TrackArgs {
    /// Ethereum address to track
    pub address: String,
    
    /// Whether this is your own address (uses only Gamma API, not auth API)
    #[arg(long)]
    pub own: bool,
}

/// Untrack address arguments
#[derive(Debug, Args, Clone)]
pub struct UntrackArgs {
    /// Ethereum address to untrack
    pub address: String,
}

/// Sync arguments
#[derive(Debug, Args, Clone)]
pub struct SyncArgs {
    /// Ethereum address to sync
    pub address: String,
    
    /// Sync positions data
    #[arg(long, default_value = "true")]
    pub positions: bool,
    
    /// Sync activity data
    #[arg(long, default_value = "true")]
    pub activity: bool,
}

/// List arguments
#[derive(Debug, Args, Clone)]
pub struct ListArgs {
    /// Show only owned addresses
    #[arg(long)]
    pub owned_only: bool,
    
    /// Show sync statistics
    #[arg(long)]
    pub with_stats: bool,
}

/// Show arguments
#[derive(Debug, Args, Clone)]
pub struct ShowArgs {
    /// Ethereum address to show
    pub address: String,
    
    /// Include recent activity
    #[arg(long)]
    pub with_activity: bool,
}

/// Positions arguments
#[derive(Debug, Args, Clone)]
pub struct PositionsArgs {
    /// Ethereum address
    pub address: String,
    
    /// Show only open positions
    #[arg(long)]
    pub open_only: bool,
    
    /// Show only closed positions
    #[arg(long)]
    pub closed_only: bool,
    
    /// Limit number of results
    #[arg(short, long)]
    pub limit: Option<usize>,
}

/// Activity arguments
#[derive(Debug, Args, Clone)]
pub struct ActivityArgs {
    /// Ethereum address
    pub address: String,
    
    /// Filter by activity type
    #[arg(long)]
    pub activity_type: Option<String>,
    
    /// Show only trades
    #[arg(long)]
    pub trades_only: bool,
    
    /// Limit number of results
    #[arg(short, long, default_value = "50")]
    pub limit: usize,
}

/// Portfolio arguments
#[derive(Debug, Args, Clone)]
pub struct PortfolioArgs {
    /// Ethereum address
    pub address: String,
}

/// Sync all arguments
#[derive(Debug, Args, Clone)]
pub struct SyncAllArgs {
    /// Sync positions for all addresses
    #[arg(long, default_value = "true")]
    pub positions: bool,
    
    /// Sync activity for all addresses
    #[arg(long, default_value = "true")]
    pub activity: bool,
}

/// Stats arguments
#[derive(Debug, Args, Clone)]
pub struct StatsArgs {
    /// Show detailed statistics
    #[arg(long)]
    pub detailed: bool,
}

/// Gamma API command implementation
#[derive(Debug)]
pub struct GammaCommand {
    args: GammaArgs,
}

impl GammaCommand {
    pub fn new(args: GammaArgs) -> Self {
        Self { args }
    }

    pub async fn execute(&self, _host: &str, data_paths: DataPaths) -> Result<()> {
        let address_book = get_address_book_service(data_paths.clone(), None, None).await?;
        let gamma_tracker = get_gamma_tracker(data_paths, Some(address_book)).await?;

        match &self.args.command {
            GammaSubcommand::Track(args) => self.track_address(gamma_tracker.as_ref(), args).await,
            GammaSubcommand::Untrack(args) => self.untrack_address(gamma_tracker.as_ref(), args).await,
            GammaSubcommand::Sync(args) => self.sync_address(gamma_tracker.as_ref(), args).await,
            GammaSubcommand::List(args) => self.list_addresses(gamma_tracker.as_ref(), args).await,
            GammaSubcommand::Show(args) => self.show_address(gamma_tracker.as_ref(), args).await,
            GammaSubcommand::Positions(args) => self.show_positions(gamma_tracker.as_ref(), args).await,
            GammaSubcommand::Activity(args) => self.show_activity(gamma_tracker.as_ref(), args).await,
            GammaSubcommand::Portfolio(args) => self.show_portfolio(gamma_tracker.as_ref(), args).await,
            GammaSubcommand::SyncAll(args) => self.sync_all(gamma_tracker.as_ref(), args).await,
            GammaSubcommand::Stats(args) => self.show_stats(gamma_tracker.as_ref(), args).await,
        }
    }

    async fn track_address(&self, tracker: &GammaTrackerHandle, args: &TrackArgs) -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        
        tracker.send(TrackerCommand::TrackAddress {
            address: args.address.clone(),
            is_own_address: args.own,
            response: tx,
        }).await?;

        match rx.await? {
            Ok(metadata) => {
                println!("✅ Started tracking address: {}", args.address);
                if args.own {
                    println!("   📍 Marked as own address (will use only Gamma API)");
                }
                if let Some(label) = metadata.label {
                    println!("   🏷️  Label: {}", label);
                }
                println!("   📁 Data will be stored in: data/gamma/tracker/raw/{}/", args.address);
            }
            Err(e) => {
                error!("Failed to track address: {}", e);
                return Err(e);
            }
        }
        
        Ok(())
    }

    async fn untrack_address(&self, tracker: &GammaTrackerHandle, args: &UntrackArgs) -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        
        tracker.send(TrackerCommand::UntrackAddress {
            address: args.address.clone(),
            response: tx,
        }).await?;

        match rx.await? {
            Ok(_) => {
                println!("✅ Stopped tracking address: {}", args.address);
                println!("   ℹ️  Historical data remains in data/gamma/tracker/raw/{}/", args.address);
            }
            Err(e) => {
                error!("Failed to untrack address: {}", e);
                return Err(e);
            }
        }
        
        Ok(())
    }

    async fn sync_address(&self, tracker: &GammaTrackerHandle, args: &SyncArgs) -> Result<()> {
        println!("🔄 Syncing data for address: {}", args.address);
        
        let (tx, rx) = tokio::sync::oneshot::channel();
        
        tracker.send(TrackerCommand::SyncAddress {
            address: args.address.clone(),
            sync_positions: args.positions,
            sync_activity: args.activity,
            response: tx,
        }).await?;

        match rx.await? {
            Ok(user_state) => {
                println!("✅ Sync completed for address: {}", args.address);
                
                if args.positions {
                    println!("   📈 Positions: {} total, {} active", 
                        user_state.positions.len(),
                        user_state.positions.iter().filter(|p| p.state == PositionState::Open).count()
                    );
                }
                
                if args.activity {
                    println!("   📊 Activities: {} recent transactions", user_state.recent_activity.len());
                }
                
                let summary = &user_state.portfolio_summary;
                println!("   💰 Portfolio Value: ${:.2}", summary.total_value);
                println!("   📊 Total PnL: ${:.2}", summary.total_realized_pnl + summary.total_unrealized_pnl);
                println!("   🏪 Unique Markets: {}", summary.unique_markets);
            }
            Err(e) => {
                error!("Failed to sync address: {}", e);
                return Err(e);
            }
        }
        
        Ok(())
    }

    async fn list_addresses(&self, tracker: &GammaTrackerHandle, args: &ListArgs) -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        
        tracker.send(TrackerCommand::ListTracked {
            response: tx,
        }).await?;

        match rx.await? {
            Ok(addresses) => {
                let filtered: Vec<_> = if args.owned_only {
                    addresses.into_iter().filter(|a| a.is_own_address).collect()
                } else {
                    addresses
                };

                if filtered.is_empty() {
                    println!("No tracked addresses found.");
                    return Ok(());
                }

                println!("📋 Tracked Addresses ({}):", filtered.len());
                println!("{}", "─".repeat(80));

                for metadata in filtered {
                    let owned_marker = if metadata.is_own_address { "👤" } else { "🔍" };
                    let label = metadata.label.as_deref().unwrap_or("(no label)");
                    
                    println!("{} {} - {}", owned_marker, metadata.address, label);
                    
                    if args.with_stats {
                        println!("   📈 Positions: {} total, {} active", 
                            metadata.total_positions, metadata.active_positions);
                        println!("   📊 Activities: {}", metadata.total_activities);
                        println!("   🏪 Markets: {}", metadata.total_unique_markets);
                        println!("   💰 Volume: ${:.2}, PnL: ${:.2}", 
                            metadata.total_volume, metadata.total_pnl);
                        
                        if let Some(last_sync) = metadata.last_positions_sync {
                            println!("   🔄 Last positions sync: {}", format_timestamp(last_sync));
                        }
                        if let Some(last_sync) = metadata.last_activity_sync {
                            println!("   🔄 Last activity sync: {}", format_timestamp(last_sync));
                        }
                        println!();
                    }
                }
            }
            Err(e) => {
                error!("Failed to list addresses: {}", e);
                return Err(e);
            }
        }
        
        Ok(())
    }

    async fn show_address(&self, tracker: &GammaTrackerHandle, args: &ShowArgs) -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        
        tracker.send(TrackerCommand::GetUserState {
            address: args.address.clone(),
            response: tx,
        }).await?;

        match rx.await? {
            Ok(Some(user_state)) => {
                let metadata = &user_state.metadata;
                let summary = &user_state.portfolio_summary;

                println!("📍 Address Details: {}", args.address);
                println!("{}", "─".repeat(80));
                
                let owned_status = if metadata.is_own_address { "Own Address (Gamma API only)" } else { "External Address" };
                println!("🔍 Status: {}", owned_status);
                
                if let Some(label) = &metadata.label {
                    println!("🏷️  Label: {}", label);
                }
                
                if let Some(addr_type) = &metadata.address_type {
                    println!("📋 Type: {}", addr_type);
                }

                println!("\n💰 Portfolio Summary:");
                println!("   Total Value: ${:.2}", summary.total_value);
                println!("   Realized PnL: ${:.2}", summary.total_realized_pnl);
                println!("   Unrealized PnL: ${:.2}", summary.total_unrealized_pnl);
                println!("   Total Volume: ${:.2}", summary.total_volume);
                
                if let Some(win_rate) = summary.win_rate {
                    println!("   Win Rate: {:.1}%", win_rate);
                }

                println!("\n📊 Statistics:");
                println!("   Active Positions: {}", summary.active_positions);
                println!("   Closed Positions: {}", summary.closed_positions);
                println!("   Total Trades: {}", summary.total_trades);
                println!("   Unique Markets: {}", summary.unique_markets);
                
                if let Some(last_trade) = summary.last_trade {
                    println!("   Last Trade: {}", format_timestamp(last_trade));
                }

                println!("\n🔄 Sync Status:");
                if let Some(last_sync) = metadata.last_positions_sync {
                    println!("   Positions: {}", format_timestamp(last_sync));
                }
                if let Some(last_sync) = metadata.last_activity_sync {
                    println!("   Activity: {}", format_timestamp(last_sync));
                }

                if args.with_activity && !user_state.recent_activity.is_empty() {
                    println!("\n📈 Recent Activity (last {}):", user_state.recent_activity.len().min(10));
                    for activity in user_state.recent_activity.iter().take(10) {
                        let activity_type = format!("{:?}", activity.activity_type);
                        println!("   {} {} {} {} @ ${:.4} - {}", 
                            format_timestamp(activity.timestamp),
                            activity_type,
                            activity.side,
                            activity.size,
                            activity.price,
                            activity.outcome.as_deref().unwrap_or("Unknown")
                        );
                    }
                }
            }
            Ok(None) => {
                println!("❌ Address not found or not tracked: {}", args.address);
                println!("   Use 'gamma track {}' to start tracking this address", args.address);
            }
            Err(e) => {
                error!("Failed to get address details: {}", e);
                return Err(e);
            }
        }
        
        Ok(())
    }

    async fn show_positions(&self, tracker: &GammaTrackerHandle, args: &PositionsArgs) -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        
        tracker.send(TrackerCommand::GetUserState {
            address: args.address.clone(),
            response: tx,
        }).await?;

        match rx.await? {
            Ok(Some(user_state)) => {
                let mut positions = user_state.positions;

                // Apply filters
                if args.open_only {
                    positions.retain(|p| p.state == PositionState::Open);
                } else if args.closed_only {
                    positions.retain(|p| p.state == PositionState::Closed);
                }

                // Apply limit
                if let Some(limit) = args.limit {
                    positions.truncate(limit);
                }

                if positions.is_empty() {
                    println!("No positions found for address: {}", args.address);
                    return Ok(());
                }

                println!("📈 Positions for {} ({}):", args.address, positions.len());
                println!("{}", "─".repeat(120));

                for position in positions {
                    let state_emoji = match position.state {
                        PositionState::Open => "🟢",
                        PositionState::Closed => "🔴",
                        PositionState::Expired => "⚫",
                    };
                    
                    println!("{} {} - {}", state_emoji, position.outcome, position.question);
                    println!("   💰 Size: {} @ ${:.4} (Avg) | Value: ${:.2}", 
                        position.size, position.average_price, position.value);
                    println!("   📊 PnL: Realized ${:.2} | Unrealized ${:.2}", 
                        position.realized_pnl, position.unrealized_pnl);
                    
                    if let Some(current_price) = position.current_price {
                        println!("   💲 Current Price: ${:.4}", current_price);
                    }
                    
                    if let Some(last_trade) = position.last_trade_time {
                        println!("   🕒 Last Trade: {}", format_timestamp(last_trade));
                    }
                    
                    println!("   🏪 Market: {}", position.market);
                    println!();
                }
            }
            Ok(None) => {
                println!("❌ Address not found or not tracked: {}", args.address);
            }
            Err(e) => {
                error!("Failed to get positions: {}", e);
                return Err(e);
            }
        }
        
        Ok(())
    }

    async fn show_activity(&self, tracker: &GammaTrackerHandle, args: &ActivityArgs) -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        
        tracker.send(TrackerCommand::GetUserState {
            address: args.address.clone(),
            response: tx,
        }).await?;

        match rx.await? {
            Ok(Some(user_state)) => {
                let mut activities = user_state.recent_activity;

                // Apply filters
                if args.trades_only {
                    activities.retain(|a| a.activity_type == ActivityType::Trade);
                }

                if let Some(activity_type_str) = &args.activity_type {
                    let filter_type = match activity_type_str.to_lowercase().as_str() {
                        "trade" => Some(ActivityType::Trade),
                        "mint" => Some(ActivityType::Mint),
                        "burn" => Some(ActivityType::Burn),
                        "transfer" => Some(ActivityType::Transfer),
                        "claim" => Some(ActivityType::Claim),
                        _ => {
                            warn!("Unknown activity type: {}", activity_type_str);
                            None
                        }
                    };
                    
                    if let Some(filter) = filter_type {
                        activities.retain(|a| a.activity_type == filter);
                    }
                }

                // Apply limit
                activities.truncate(args.limit);

                if activities.is_empty() {
                    println!("No activities found for address: {}", args.address);
                    return Ok(());
                }

                println!("📊 Activity for {} ({}):", args.address, activities.len());
                println!("{}", "─".repeat(120));

                for activity in activities {
                    let type_emoji = match activity.activity_type {
                        ActivityType::Trade => "💱",
                        ActivityType::Mint => "🔨",
                        ActivityType::Burn => "🔥",
                        ActivityType::Transfer => "📤",
                        ActivityType::Claim => "🎁",
                    };
                    
                    println!("{} {} {} {} {}", 
                        type_emoji,
                        format_timestamp(activity.timestamp),
                        format!("{:?}", activity.activity_type),
                        activity.side,
                        activity.outcome.as_deref().unwrap_or("Unknown")
                    );
                    
                    println!("   💰 Size: {} @ ${:.4} | Value: ${:.2}", 
                        activity.size, activity.price, activity.size * activity.price);
                    
                    if let Some(fee) = activity.fee {
                        println!("   💸 Fee: ${:.4}", fee);
                    }
                    
                    if let Some(tx_hash) = &activity.tx_hash {
                        println!("   🔗 Tx: {}", tx_hash);
                    }
                    
                    println!("   🏪 Market: {}", activity.market);
                    println!();
                }
            }
            Ok(None) => {
                println!("❌ Address not found or not tracked: {}", args.address);
            }
            Err(e) => {
                error!("Failed to get activity: {}", e);
                return Err(e);
            }
        }
        
        Ok(())
    }

    async fn show_portfolio(&self, tracker: &GammaTrackerHandle, args: &PortfolioArgs) -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        
        tracker.send(TrackerCommand::GetUserState {
            address: args.address.clone(),
            response: tx,
        }).await?;

        match rx.await? {
            Ok(Some(user_state)) => {
                let summary = &user_state.portfolio_summary;

                println!("💼 Portfolio Summary for {}", args.address);
                println!("{}", "═".repeat(80));

                println!("💰 Financial Overview:");
                println!("   Total Portfolio Value: ${:.2}", summary.total_value);
                println!("   Realized PnL:         ${:.2}", summary.total_realized_pnl);
                println!("   Unrealized PnL:       ${:.2}", summary.total_unrealized_pnl);
                println!("   Total PnL:            ${:.2}", summary.total_realized_pnl + summary.total_unrealized_pnl);
                println!("   Total Volume:         ${:.2}", summary.total_volume);

                println!("\n📊 Trading Statistics:");
                println!("   Active Positions:     {}", summary.active_positions);
                println!("   Closed Positions:     {}", summary.closed_positions);
                println!("   Total Positions:      {}", summary.active_positions + summary.closed_positions);
                println!("   Total Trades:         {}", summary.total_trades);
                println!("   Unique Markets:       {}", summary.unique_markets);

                if let Some(win_rate) = summary.win_rate {
                    println!("   Win Rate:             {:.1}%", win_rate);
                }

                if let Some(last_trade) = summary.last_trade {
                    println!("   Last Trade:           {}", format_timestamp(last_trade));
                }

                // Show top positions by value
                if !user_state.positions.is_empty() {
                    println!("\n🔝 Top Positions by Value:");
                    let mut positions = user_state.positions.clone();
                    positions.sort_by(|a, b| b.value.cmp(&a.value));
                    
                    for (i, position) in positions.iter().take(5).enumerate() {
                        let pnl_total = position.realized_pnl + position.unrealized_pnl;
                        let pnl_emoji = if pnl_total >= Decimal::ZERO { "📈" } else { "📉" };
                        
                        println!("   {}. {} {} - ${:.2} | PnL: {}{:.2}", 
                            i + 1,
                            match position.state {
                                PositionState::Open => "🟢",
                                PositionState::Closed => "🔴",
                                PositionState::Expired => "⚫",
                            },
                            position.outcome,
                            position.value,
                            pnl_emoji,
                            pnl_total
                        );
                    }
                }
            }
            Ok(None) => {
                println!("❌ Address not found or not tracked: {}", args.address);
            }
            Err(e) => {
                error!("Failed to get portfolio: {}", e);
                return Err(e);
            }
        }
        
        Ok(())
    }

    async fn sync_all(&self, tracker: &GammaTrackerHandle, _args: &SyncAllArgs) -> Result<()> {
        println!("🔄 Syncing all tracked addresses...");
        
        let (tx, rx) = tokio::sync::oneshot::channel();
        
        tracker.send(TrackerCommand::SyncAll {
            response: tx,
        }).await?;

        match rx.await? {
            Ok(synced_addresses) => {
                if synced_addresses.is_empty() {
                    println!("No addresses to sync.");
                } else {
                    println!("✅ Synced {} addresses:", synced_addresses.len());
                    for address in synced_addresses {
                        println!("   ✓ {}", address);
                    }
                }
            }
            Err(e) => {
                error!("Failed to sync all addresses: {}", e);
                return Err(e);
            }
        }
        
        Ok(())
    }

    async fn show_stats(&self, tracker: &GammaTrackerHandle, args: &StatsArgs) -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        
        tracker.send(TrackerCommand::GetStorageStats {
            response: tx,
        }).await?;

        match rx.await? {
            Ok(stats) => {
                if stats.is_empty() {
                    println!("No storage statistics available.");
                    return Ok(());
                }

                println!("📊 Storage Statistics ({} addresses):", stats.len());
                println!("{}", "─".repeat(80));

                let mut total_size = 0u64;
                
                for (address, user_stats) in stats {
                    total_size += user_stats.total_size_bytes;
                    
                    println!("📍 {}", address);
                    println!("   📂 Files: {}{}{}{}",
                        if user_stats.has_metadata { "metadata " } else { "" },
                        if user_stats.has_state { "state " } else { "" },
                        if user_stats.has_positions { "positions " } else { "" },
                        if user_stats.has_activity { "activity" } else { "" }
                    );
                    println!("   💾 Size: {}", format_bytes(user_stats.total_size_bytes));
                    
                    if let Some(last_modified) = user_stats.last_modified {
                        println!("   🕒 Last Modified: {}", format_timestamp(last_modified));
                    }
                    
                    if args.detailed {
                        println!();
                    }
                }

                println!("{}", "─".repeat(80));
                println!("📊 Total Storage: {}", format_bytes(total_size));
            }
            Err(e) => {
                error!("Failed to get storage stats: {}", e);
                return Err(e);
            }
        }
        
        Ok(())
    }
}

/// Format timestamp for display
fn format_timestamp(timestamp: DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(timestamp);
    
    if duration.num_days() > 0 {
        format!("{} days ago", duration.num_days())
    } else if duration.num_hours() > 0 {
        format!("{} hours ago", duration.num_hours())
    } else if duration.num_minutes() > 0 {
        format!("{} minutes ago", duration.num_minutes())
    } else {
        "Just now".to_string()
    }
}

/// Format bytes for display
fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}