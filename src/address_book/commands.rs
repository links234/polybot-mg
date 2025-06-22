//! CLI commands for address book management

use anyhow::Result;
use clap::{Parser, Subcommand};
use tokio::sync::oneshot;
// use tracing::{info, warn}; // Not needed for now

use crate::address_book::*;
use crate::address_book::types::{AddressType, AddressSortField, AddressQuery};
use crate::data::DataPaths;

/// Address book management commands
#[derive(Debug, Parser)]
pub struct AddressCommand {
    #[command(subcommand)]
    pub command: AddressSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum AddressSubcommand {
    /// Add a new address to the book
    Add(AddAddressArgs),
    
    /// List all addresses
    List(ListAddressesArgs),
    
    /// Get details of a specific address
    Get(GetAddressArgs),
    
    /// Update an existing address
    Update(UpdateAddressArgs),
    
    /// Edit an address by ID (interactive)
    Edit(EditAddressArgs),
    
    /// Remove an address from the book
    Remove(RemoveAddressArgs),
    
    /// Query addresses with filters
    Query(QueryAddressArgs),
    
    /// Set the current active address
    SetCurrent(SetCurrentArgs),
    
    /// Get the current active address
    Current,
    
    /// Sync address statistics with portfolio
    Sync(SyncAddressArgs),
    
    /// Sync all addresses with portfolio
    SyncAll(SyncAllAddressArgs),
    
    /// Import addresses from CSV
    Import(ImportAddressArgs),
    
    /// Export addresses to CSV
    Export(ExportAddressArgs),
    
    /// Show address book statistics
    Stats,
    
    /// List addresses with a quick view
    Quick(QuickViewArgs),
    
    /// Toggle active status for addresses
    Toggle(ToggleAddressArgs),
    
    /// Add tags to multiple addresses
    Tag(TagAddressesArgs),
}

#[derive(Debug, Parser)]
pub struct AddAddressArgs {
    /// Ethereum address
    pub address: String,
    
    /// Human-readable label/name
    #[arg(short, long)]
    pub label: Option<String>,
    
    /// Description
    #[arg(short, long)]
    pub description: Option<String>,
    
    /// Address type (own, watched, contract, exchange, market-maker, other)
    #[arg(short = 't', long, default_value = "watched")]
    pub address_type: String,
    
    /// Tags (comma-separated)
    #[arg(long)]
    pub tags: Option<String>,
}

#[derive(Debug, Parser)]
pub struct ListAddressesArgs {
    /// Maximum number of addresses to show
    #[arg(short, long)]
    pub limit: Option<usize>,
    
    /// Show only own addresses
    #[arg(long)]
    pub own: bool,
    
    /// Show only watched addresses
    #[arg(long)]
    pub watched: bool,
    
    /// Show detailed information
    #[arg(short, long)]
    pub detailed: bool,
    
    /// Display results in table format
    #[arg(long)]
    pub table: bool,
}

#[derive(Debug, Parser)]
pub struct GetAddressArgs {
    /// Address or label to look up
    pub address_or_label: String,
}

#[derive(Debug, Parser)]
pub struct UpdateAddressArgs {
    /// Address to update
    pub address: String,
    
    /// New label
    #[arg(short, long)]
    pub label: Option<String>,
    
    /// New description
    #[arg(short, long)]
    pub description: Option<String>,
    
    /// New tags (comma-separated)
    #[arg(long)]
    pub tags: Option<String>,
    
    /// Set active status
    #[arg(long)]
    pub active: Option<bool>,
    
    /// Additional notes
    #[arg(long)]
    pub notes: Option<String>,
}

#[derive(Debug, Parser)]
pub struct EditAddressArgs {
    /// ID of the address to edit (from list command)
    pub id: usize,
    
    /// New label
    #[arg(short, long)]
    pub label: Option<String>,
    
    /// New description
    #[arg(short, long)]
    pub description: Option<String>,
    
    /// New tags (comma-separated)
    #[arg(long)]
    pub tags: Option<String>,
    
    /// Add tags (comma-separated)
    #[arg(long)]
    pub add_tags: Option<String>,
    
    /// Remove tags (comma-separated)
    #[arg(long)]
    pub remove_tags: Option<String>,
    
    /// Set active status
    #[arg(long)]
    pub active: Option<bool>,
    
    /// Additional notes
    #[arg(long)]
    pub notes: Option<String>,
    
    /// Clear field (label, description, notes)
    #[arg(long)]
    pub clear: Option<String>,
}

#[derive(Debug, Parser)]
pub struct RemoveAddressArgs {
    /// Address to remove
    pub address: String,
    
    /// Skip confirmation
    #[arg(short, long)]
    pub yes: bool,
}

#[derive(Debug, Parser)]
pub struct QueryAddressArgs {
    /// Search term
    pub search: Option<String>,
    
    /// Filter by type
    #[arg(short = 't', long)]
    pub address_type: Option<String>,
    
    /// Filter by tags (comma-separated)
    #[arg(long)]
    pub tags: Option<String>,
    
    /// Show only active addresses
    #[arg(long)]
    pub active_only: bool,
    
    /// Sort by field (label, address, added, updated, value, queries)
    #[arg(long, default_value = "label")]
    pub sort: String,
    
    /// Sort ascending
    #[arg(long)]
    pub ascending: bool,
    
    /// Limit results
    #[arg(short, long)]
    pub limit: Option<usize>,
}

#[derive(Debug, Parser)]
pub struct SetCurrentArgs {
    /// Address to set as current
    pub address: String,
}

#[derive(Debug, Parser)]
pub struct SyncAddressArgs {
    /// Address to sync (or current if not specified)
    pub address: Option<String>,
    
    /// Display results in table format
    #[arg(long)]
    pub table: bool,
}

#[derive(Debug, Parser)]
pub struct ImportAddressArgs {
    /// CSV file path
    pub path: String,
}

#[derive(Debug, Parser)]
pub struct ExportAddressArgs {
    /// Output CSV file path
    pub path: String,
}

#[derive(Debug, Parser)]
pub struct SyncAllAddressArgs {
    /// Only sync active addresses
    #[arg(long)]
    pub active_only: bool,
    
    /// Maximum number of addresses to sync
    #[arg(short, long)]
    pub limit: Option<usize>,
    
    /// Display results in table format
    #[arg(long)]
    pub table: bool,
}

#[derive(Debug, Parser)]
pub struct QuickViewArgs {
    /// Show only own addresses
    #[arg(long)]
    pub own: bool,
    
    /// Show only watched addresses
    #[arg(long)]
    pub watched: bool,
    
    /// Show portfolio value
    #[arg(long)]
    pub with_value: bool,
    
    /// Display results in table format
    #[arg(long)]
    pub table: bool,
}

#[derive(Debug, Parser)]
pub struct ToggleAddressArgs {
    /// Address or label to toggle
    pub address_or_label: String,
}

#[derive(Debug, Parser)]
pub struct TagAddressesArgs {
    /// Tags to add (comma-separated)
    pub tags: String,
    
    /// Apply to all addresses matching pattern
    #[arg(long)]
    pub pattern: Option<String>,
    
    /// Apply to addresses of specific type
    #[arg(long)]
    pub address_type: Option<String>,
}

impl AddressCommand {
    pub async fn execute(self, _host: &str, data_paths: DataPaths) -> Result<()> {
        // Get address service (Gamma client will be created internally)
        let address_service = service::get_address_book_service(
            data_paths.clone(),
            None, // Don't pass portfolio service initially 
            None, // Placeholder for gamma tracker
        ).await?;
        
        // Ensure user's address is in the book (this is idempotent)
        ensure_user_address(&address_service, &data_paths).await?;
        
        match self.command {
            AddressSubcommand::Add(args) => {
                add_address(args, &address_service).await?;
            }
            AddressSubcommand::List(args) => {
                list_addresses(args, &address_service).await?;
            }
            AddressSubcommand::Get(args) => {
                get_address(args, &address_service).await?;
            }
            AddressSubcommand::Update(args) => {
                update_address(args, &address_service).await?;
            }
            AddressSubcommand::Edit(args) => {
                edit_address(args, &address_service).await?;
            }
            AddressSubcommand::Remove(args) => {
                remove_address(args, &address_service).await?;
            }
            AddressSubcommand::Query(args) => {
                query_addresses(args, &address_service).await?;
            }
            AddressSubcommand::SetCurrent(args) => {
                set_current_address(args, &address_service).await?;
            }
            AddressSubcommand::Current => {
                get_current_address(&address_service).await?;
            }
            AddressSubcommand::Sync(args) => {
                sync_address(args, &address_service).await?;
            }
            AddressSubcommand::SyncAll(args) => {
                sync_all_addresses(args, &address_service).await?;
            }
            AddressSubcommand::Import(args) => {
                import_addresses(args, &address_service).await?;
            }
            AddressSubcommand::Export(args) => {
                export_addresses(args, &address_service).await?;
            }
            AddressSubcommand::Stats => {
                show_stats(&address_service).await?;
            }
            AddressSubcommand::Quick(args) => {
                quick_view(args, &address_service).await?;
            }
            AddressSubcommand::Toggle(args) => {
                toggle_address(args, &address_service).await?;
            }
            AddressSubcommand::Tag(args) => {
                tag_addresses(args, &address_service).await?;
            }
        }
        
        Ok(())
    }
}

/// Add a new address
async fn add_address(args: AddAddressArgs, service: &AddressBookServiceHandle) -> Result<()> {
    let address_type = parse_address_type(&args.address_type)?;
    let tags = parse_tags(args.tags.as_deref());
    
    let (tx, rx) = oneshot::channel();
    service.send(AddressBookCommand::AddAddress {
        address: args.address,
        label: args.label,
        description: args.description,
        address_type,
        tags,
        response: tx,
    }).await?;
    
    let entry = rx.await??;
    
    println!("✅ Added address to book:");
    print_address_entry(&entry, true);
    
    Ok(())
}

/// List addresses
async fn list_addresses(args: ListAddressesArgs, service: &AddressBookServiceHandle) -> Result<()> {
    let (tx, rx) = oneshot::channel();
    service.send(AddressBookCommand::ListAddresses {
        limit: args.limit,
        response: tx,
    }).await?;
    
    let mut entries = rx.await??;
    
    // Apply filters
    if args.own {
        entries.retain(|e| e.address_type == AddressType::Own);
    } else if args.watched {
        entries.retain(|e| e.address_type == AddressType::Watched);
    }
    
    if entries.is_empty() {
        println!("No addresses found");
        return Ok(());
    }
    
    if args.table {
        print_addresses_table(&entries);
    } else {
        println!("📖 Address Book ({} entries)", entries.len());
        
        if args.detailed {
            for (id, entry) in entries.iter().enumerate() {
                println!("ID: {}", id);
                print_address_entry(entry, true);
                println!();
            }
        } else {
            // Print header
            println!("{:<4} {:<30} {:<15} {:<20} {:<8} {:<12}", 
                "ID", "Label/Address", "Type", "Tags", "Active", "Last Synced");
            println!("{}", "-".repeat(89));
            
            for (id, entry) in entries.iter().enumerate() {
                let display_name = entry.display_name();
                let tags = entry.tags.join(", ");
                let active = if entry.is_active { "✓" } else { "✗" };
                let last_synced = entry.metadata.last_synced
                    .map(|t| t.format("%Y-%m-%d").to_string())
                    .unwrap_or_else(|| "Never".to_string());
                
                println!("{:<4} {:<30} {:<15} {:<20} {:<8} {:<12}",
                    id,
                    display_name,
                    entry.address_type.to_string(),
                    tags,
                    active,
                    last_synced
                );
            }
        }
    }
    
    if !args.detailed && entries.len() > 0 {
        println!("\n💡 Tip: Use --detailed for full info or 'address quick --with-value' for portfolio values");
        println!("    Example: address quick --with-value");
    }
    
    Ok(())
}

/// Get address details
async fn get_address(args: GetAddressArgs, service: &AddressBookServiceHandle) -> Result<()> {
    let (tx, rx) = oneshot::channel();
    service.send(AddressBookCommand::GetAddress {
        address_or_label: args.address_or_label,
        response: tx,
    }).await?;
    
    match rx.await?? {
        Some(entry) => {
            print_address_entry(&entry, true);
        }
        None => {
            println!("Address not found");
        }
    }
    
    Ok(())
}

/// Update address
async fn update_address(args: UpdateAddressArgs, service: &AddressBookServiceHandle) -> Result<()> {
    let tags = args.tags.map(|t| parse_tags(Some(&t)));
    
    let (tx, rx) = oneshot::channel();
    service.send(AddressBookCommand::UpdateAddress {
        address: args.address,
        label: args.label,
        description: args.description,
        tags,
        is_active: args.active,
        notes: args.notes,
        response: tx,
    }).await?;
    
    let entry = rx.await??;
    
    println!("✅ Updated address:");
    print_address_entry(&entry, true);
    
    Ok(())
}

/// Edit address by ID
async fn edit_address(args: EditAddressArgs, service: &AddressBookServiceHandle) -> Result<()> {
    // Get all addresses to find the one at the given ID
    let (tx, rx) = oneshot::channel();
    service.send(AddressBookCommand::ListAddresses {
        limit: None,
        response: tx,
    }).await?;
    
    let entries = rx.await??;
    
    // Check if ID is valid
    if args.id >= entries.len() {
        println!("❌ Invalid ID: {}. Valid range is 0-{}", args.id, entries.len() - 1);
        return Ok(());
    }
    
    let entry = &entries[args.id];
    let address = entry.address.clone();
    
    // Handle clear operations
    let (label, description, notes) = if let Some(clear_field) = &args.clear {
        match clear_field.to_lowercase().as_str() {
            "label" => (Some("".to_string()), args.description, args.notes),
            "description" => (args.label, Some("".to_string()), args.notes),
            "notes" => (args.label, args.description, Some("".to_string())),
            _ => {
                println!("❌ Invalid clear field: {}. Valid options: label, description, notes", clear_field);
                return Ok(());
            }
        }
    } else {
        (args.label, args.description, args.notes)
    };
    
    // Handle tags
    let tags = if let Some(new_tags) = args.tags {
        // Replace all tags
        Some(parse_tags(Some(&new_tags)))
    } else if args.add_tags.is_some() || args.remove_tags.is_some() {
        // Add/remove specific tags
        let mut current_tags = entry.tags.clone();
        
        if let Some(add) = args.add_tags {
            for tag in parse_tags(Some(&add)) {
                if !current_tags.contains(&tag) {
                    current_tags.push(tag);
                }
            }
        }
        
        if let Some(remove) = args.remove_tags {
            let remove_tags = parse_tags(Some(&remove));
            current_tags.retain(|t| !remove_tags.contains(t));
        }
        
        Some(current_tags)
    } else {
        None
    };
    
    // Update the address
    let (tx, rx) = oneshot::channel();
    service.send(AddressBookCommand::UpdateAddress {
        address,
        label,
        description,
        tags,
        is_active: args.active,
        notes,
        response: tx,
    }).await?;
    
    let updated_entry = rx.await??;
    
    println!("✅ Updated address [ID: {}]:", args.id);
    print_address_entry(&updated_entry, true);
    
    Ok(())
}

/// Remove address
async fn remove_address(args: RemoveAddressArgs, service: &AddressBookServiceHandle) -> Result<()> {
    if !args.yes {
        print!("Remove address {}? [y/N] ", args.address);
        use std::io::{self, Write};
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled");
            return Ok(());
        }
    }
    
    let (tx, rx) = oneshot::channel();
    service.send(AddressBookCommand::RemoveAddress {
        address: args.address.clone(),
        response: tx,
    }).await?;
    
    rx.await??;
    
    println!("✅ Removed address {}", args.address);
    
    Ok(())
}

/// Query addresses
async fn query_addresses(args: QueryAddressArgs, service: &AddressBookServiceHandle) -> Result<()> {
    let address_type = args.address_type
        .as_deref()
        .map(parse_address_type)
        .transpose()?;
    
    let tags = parse_tags(args.tags.as_deref());
    
    let sort_by = match args.sort.as_str() {
        "label" => AddressSortField::Label,
        "address" => AddressSortField::Address,
        "added" => AddressSortField::AddedAt,
        "updated" => AddressSortField::UpdatedAt,
        "value" => AddressSortField::TotalValue,
        "queries" => AddressSortField::QueryCount,
        _ => AddressSortField::Label,
    };
    
    let query = AddressQuery {
        search: args.search,
        address_type,
        tags,
        active_only: args.active_only,
        sort_by,
        ascending: args.ascending,
        limit: args.limit,
    };
    
    let (tx, rx) = oneshot::channel();
    service.send(AddressBookCommand::QueryAddresses {
        query,
        response: tx,
    }).await?;
    
    let entries = rx.await??;
    
    if entries.is_empty() {
        println!("No addresses match the query");
        return Ok(());
    }
    
    println!("📖 Query Results ({} matches)", entries.len());
    
    // Print header
    println!("{:<4} {:<30} {:<15} {:<10} {:<10} {:<8} {:<8}", 
        "ID", "Label/Address", "Type", "Value", "Positions", "Orders", "Queries");
    println!("{}", "-".repeat(85));
    
    // Get all addresses to maintain consistent IDs
    let (tx, rx) = oneshot::channel();
    service.send(AddressBookCommand::ListAddresses {
        limit: None,
        response: tx,
    }).await?;
    let all_entries = rx.await??;
    
    // Create a map of address to ID
    let mut address_to_id = std::collections::HashMap::new();
    for (id, entry) in all_entries.iter().enumerate() {
        address_to_id.insert(&entry.address, id);
    }
    
    for entry in &entries {
        let id = address_to_id.get(&entry.address).unwrap_or(&999);
        let display_name = entry.display_name();
        let value = entry.stats.as_ref()
            .map(|s| format!("${:.2}", s.total_value))
            .unwrap_or_else(|| "-".to_string());
        let positions = entry.stats.as_ref()
            .map(|s| s.active_positions.to_string())
            .unwrap_or_else(|| "-".to_string());
        let orders = entry.stats.as_ref()
            .map(|s| s.active_orders.to_string())
            .unwrap_or_else(|| "-".to_string());
        
        println!("{:<4} {:<30} {:<15} {:<10} {:<10} {:<8} {:<8}",
            id,
            display_name,
            entry.address_type.to_string(),
            value,
            positions,
            orders,
            entry.metadata.query_count
        );
    }
    
    Ok(())
}

/// Set current address
async fn set_current_address(args: SetCurrentArgs, service: &AddressBookServiceHandle) -> Result<()> {
    let (tx, rx) = oneshot::channel();
    service.send(AddressBookCommand::SetCurrentAddress {
        address: args.address.clone(),
        response: tx,
    }).await?;
    
    rx.await??;
    
    println!("✅ Set current address to {}", args.address);
    
    Ok(())
}

/// Get current address
async fn get_current_address(service: &AddressBookServiceHandle) -> Result<()> {
    let (tx, rx) = oneshot::channel();
    service.send(AddressBookCommand::GetCurrentAddress {
        response: tx,
    }).await?;
    
    match rx.await?? {
        Some(entry) => {
            println!("Current address:");
            print_address_entry(&entry, true);
        }
        None => {
            println!("No current address set");
        }
    }
    
    Ok(())
}

/// Sync address stats
async fn sync_address(args: SyncAddressArgs, service: &AddressBookServiceHandle) -> Result<()> {
    let address = if let Some(addr) = args.address {
        addr
    } else {
        // Get current address
        let (tx, rx) = oneshot::channel();
        service.send(AddressBookCommand::GetCurrentAddress {
            response: tx,
        }).await?;
        
        match rx.await?? {
            Some(entry) => entry.address,
            None => {
                println!("No current address set. Please specify an address.");
                return Ok(());
            }
        }
    };
    
    println!("🔄 Syncing portfolio stats for {}...", address);
    
    let (tx, rx) = oneshot::channel();
    service.send(AddressBookCommand::SyncAddressStats {
        address: address.clone(),
        response: tx,
    }).await?;
    
    let stats = rx.await??;
    
    if args.table {
        // Table format output
        println!("✅ Sync Complete");
        println!();
        println!("┌{:─<48}┬{:─<20}┐", "", "");
        println!("│ {:46} │ {:>18} │", "Metric", "Value");
        println!("├{:─<48}┼{:─<20}┤", "", "");
        println!("│ {:46} │ {:>18} │", "Address", format!("{}...{}", &address[..6], &address[address.len()-4..]));
        println!("│ {:46} │ {:>18} │", "Total Portfolio Value", format!("${:.2}", stats.total_value));
        println!("│ {:46} │ {:>18} │", "Active Positions", format!("{}", stats.active_positions));
        println!("│ {:46} │ {:>18} │", "Active Orders", format!("{}", stats.active_orders));
        println!("│ {:46} │ {:>18} │", "Realized P&L", format!("${:.2}", stats.total_realized_pnl));
        println!("│ {:46} │ {:>18} │", "Unrealized P&L", format!("${:.2}", stats.total_unrealized_pnl));
        println!("│ {:46} │ {:>18} │", "Total P&L", format!("${:.2}", stats.total_realized_pnl + stats.total_unrealized_pnl));
        println!("│ {:46} │ {:>18} │", "Total Trades", format!("{}", stats.total_trades));
        println!("│ {:46} │ {:>18} │", "Total Volume", format!("${:.2}", stats.total_volume));
        println!("│ {:46} │ {:>18} │", "Buy Volume", format!("${:.2}", stats.buy_volume));
        println!("│ {:46} │ {:>18} │", "Sell Volume", format!("${:.2}", stats.sell_volume));
        if let Some(win_rate) = stats.win_rate {
            println!("│ {:46} │ {:>18} │", "Win Rate", format!("{:.1}%", win_rate));
        }
        if let Some(last_trade) = stats.last_trade {
            println!("│ {:46} │ {:>18} │", "Last Trade", last_trade.format("%Y-%m-%d").to_string());
        }
        println!("│ {:46} │ {:>18} │", "Last Updated", stats.updated_at.format("%Y-%m-%d %H:%M").to_string());
        println!("└{:─<48}┴{:─<20}┘", "", "");
        println!();
        println!("🌐 Profile: https://polymarket.com/profile/{}", address);
    } else {
        // Original detailed format
        println!("✅ Synced stats for {}", address);
        println!("   Total Value: ${:.2}", stats.total_value);
        println!("   Active Positions: {}", stats.active_positions);
        println!("   Active Orders: {}", stats.active_orders);
        println!("   Total P&L: ${:.2}", stats.total_realized_pnl);
        println!("     - Matched Trades: ${:.2}", stats.trading_pnl);
        println!("     - Unmatched Wins: ${:.2}", stats.unmatched_wins_pnl);
        println!("   Total Volume: ${:.2}", stats.total_volume);
        
        println!("\n🌐 Polymarket Profile:");
        println!("   https://polymarket.com/profile/{}", address);
        
        println!("\n📋 View more details:");
        println!("   • address get {}           - Full address details", address);
        println!("   • address quick --with-value       - Compare with other addresses");
    }
    
    Ok(())
}

/// Import addresses from CSV
async fn import_addresses(args: ImportAddressArgs, service: &AddressBookServiceHandle) -> Result<()> {
    println!("📥 Importing addresses from {}...", args.path);
    
    let (tx, rx) = oneshot::channel();
    service.send(AddressBookCommand::ImportCsv {
        path: args.path,
        response: tx,
    }).await?;
    
    let count = rx.await??;
    
    println!("✅ Imported {} addresses", count);
    
    Ok(())
}

/// Export addresses to CSV
async fn export_addresses(args: ExportAddressArgs, service: &AddressBookServiceHandle) -> Result<()> {
    println!("📤 Exporting addresses to {}...", args.path);
    
    let (tx, rx) = oneshot::channel();
    service.send(AddressBookCommand::ExportCsv {
        path: args.path.clone(),
        response: tx,
    }).await?;
    
    rx.await??;
    
    println!("✅ Exported addresses to {}", args.path);
    
    Ok(())
}

/// Show address book statistics
async fn show_stats(service: &AddressBookServiceHandle) -> Result<()> {
    let (tx, rx) = oneshot::channel();
    service.send(AddressBookCommand::GetStats {
        response: tx,
    }).await?;
    
    let stats = rx.await??;
    
    println!("📊 Address Book Statistics");
    println!("   Created: {}", stats.created_at.format("%Y-%m-%d %H:%M:%S"));
    println!("   Updated: {}", stats.updated_at.format("%Y-%m-%d %H:%M:%S"));
    println!("   Total Addresses: {}", stats.total_addresses);
    println!("   Own Addresses: {}", stats.own_addresses);
    println!("   Watched Addresses: {}", stats.watched_addresses);
    
    Ok(())
}

/// Print address entry details
fn print_address_entry(entry: &AddressEntry, detailed: bool) {
    println!("🏷️  {}", entry.display_name());
    println!("   Address: {}", entry.address);
    println!("   Type: {}", entry.address_type);
    
    if let Some(desc) = &entry.description {
        println!("   Description: {}", desc);
    }
    
    if !entry.tags.is_empty() {
        println!("   Tags: {}", entry.tags.join(", "));
    }
    
    if detailed {
        println!("   Active: {}", if entry.is_active { "Yes" } else { "No" });
        println!("   Added: {}", entry.metadata.added_at.format("%Y-%m-%d %H:%M:%S"));
        println!("   Queries: {}", entry.metadata.query_count);
        
        if let Some(ens) = &entry.metadata.ens_name {
            println!("   ENS: {}", ens);
        }
        
        if let Some(stats) = &entry.stats {
            println!("   📈 Portfolio Stats:");
            println!("      Total Value: ${:.2}", stats.total_value);
            println!("      Positions: {} active", stats.active_positions);
            println!("      Orders: {} active", stats.active_orders);
            println!("      P&L: ${:.2} realized, ${:.2} unrealized", 
                stats.total_realized_pnl, stats.total_unrealized_pnl);
            println!("        - Matched Trades: ${:.2}", stats.trading_pnl);
            println!("        - Unmatched Wins: ${:.2}", stats.unmatched_wins_pnl);
            if let Some(win_rate) = stats.win_rate {
                println!("      Win Rate: {:.1}%", win_rate);
            }
            println!("      Total Trades: {}", stats.total_trades);
            println!("      Total Volume: ${:.2}", stats.total_volume);
            println!("      Buy Volume: ${:.2} | Sell Volume: ${:.2}", 
                stats.buy_volume, stats.sell_volume);
        }
        
        if let Some(notes) = &entry.notes {
            println!("   📝 Notes: {}", notes);
        }
        
        // Always show Polymarket profile link
        println!("   🌐 Polymarket: https://polymarket.com/profile/{}", entry.address);
    }
}

/// Parse address type from string
fn parse_address_type(s: &str) -> Result<AddressType> {
    match s.to_lowercase().as_str() {
        "own" => Ok(AddressType::Own),
        "watched" => Ok(AddressType::Watched),
        "contract" => Ok(AddressType::Contract),
        "exchange" => Ok(AddressType::Exchange),
        "market-maker" | "mm" => Ok(AddressType::MarketMaker),
        "other" => Ok(AddressType::Other),
        _ => Err(anyhow::anyhow!("Invalid address type: {}", s)),
    }
}

/// Parse comma-separated tags
fn parse_tags(s: Option<&str>) -> Vec<String> {
    s.map(|s| s.split(',').map(|t| t.trim().to_string()).collect())
        .unwrap_or_default()
}

/// Ensure user's address is in the address book
async fn ensure_user_address(service: &AddressBookServiceHandle, data_paths: &DataPaths) -> Result<()> {
    use crate::config;
    use crate::ethereum_utils::derive_address_from_private_key;
    
    // Try to load private key
    if let Ok(private_key) = config::load_private_key(data_paths).await {
        // Derive address from private key
        if let Ok(address) = derive_address_from_private_key(&private_key) {
            // Check if address exists
            let (tx, rx) = oneshot::channel();
            service.send(AddressBookCommand::GetAddress {
                address_or_label: address.clone(),
                response: tx,
            }).await?;
            
            if rx.await??.is_none() {
                // Address doesn't exist, add it
                let (tx, rx) = oneshot::channel();
                service.send(AddressBookCommand::AddAddress {
                    address: address.clone(),
                    label: Some("my-wallet".to_string()),
                    description: Some("Primary trading wallet (auto-added)".to_string()),
                    address_type: AddressType::Own,
                    tags: vec!["primary".to_string(), "auto-added".to_string()],
                    response: tx,
                }).await?;
                
                match rx.await? {
                    Ok(_) => {
                        // Set as current
                        let (tx, rx) = oneshot::channel();
                        service.send(AddressBookCommand::SetCurrentAddress {
                            address,
                            response: tx,
                        }).await?;
                        let _ = rx.await?;
                    }
                    Err(_) => {
                        // Address might already exist, ignore error
                    }
                }
            }
        }
    }
    
    Ok(())
}

/// Sync all addresses
async fn sync_all_addresses(args: SyncAllAddressArgs, service: &AddressBookServiceHandle) -> Result<()> {
    // Get all addresses
    let (tx, rx) = oneshot::channel();
    service.send(AddressBookCommand::ListAddresses {
        limit: args.limit,
        response: tx,
    }).await?;
    
    let mut addresses = rx.await??;
    
    // Filter if needed
    if args.active_only {
        addresses.retain(|e| e.is_active);
    }
    
    if addresses.is_empty() {
        println!("No addresses to sync");
        return Ok(());
    }
    
    println!("🔄 Syncing {} addresses...", addresses.len());
    
    let mut success_count = 0;
    let mut error_count = 0;
    let mut synced_results = Vec::new();
    
    for (i, entry) in addresses.iter().enumerate() {
        println!("[{}/{}] Syncing {}...", i + 1, addresses.len(), entry.display_name());
        
        let (tx, rx) = oneshot::channel();
        service.send(AddressBookCommand::SyncAddressStats {
            address: entry.address.clone(),
            response: tx,
        }).await?;
        
        match rx.await? {
            Ok(stats) => {
                success_count += 1;
                println!("  ✅ Synced successfully");
                if !args.table {
                    println!("  🌐 https://polymarket.com/profile/{}", entry.address);
                }
                synced_results.push((entry.clone(), stats));
            }
            Err(e) => {
                error_count += 1;
                println!("  ❌ Error: {}", e);
            }
        }
    }
    
    if args.table && !synced_results.is_empty() {
        println!("\n✅ Sync Results");
        println!();
        print_sync_results_table(&synced_results);
    } else {
        println!("\n✅ Sync complete: {} successful, {} errors", success_count, error_count);
    }
    
    if success_count > 0 {
        println!("\n📋 View synced data:");
        println!("   • address quick --with-value   - Quick view with portfolio values");
        println!("   • address list --detailed      - Detailed view of all addresses");
        println!("   • address get <address>        - View detailed info for specific address");  
        println!("   • address quick --own          - Quick view of your own addresses");
        println!("   • address stats                - View address book statistics");
    }
    
    Ok(())
}

/// Quick view of addresses
async fn quick_view(args: QuickViewArgs, service: &AddressBookServiceHandle) -> Result<()> {
    let (tx, rx) = oneshot::channel();
    service.send(AddressBookCommand::ListAddresses {
        limit: None,
        response: tx,
    }).await?;
    
    let mut entries = rx.await??;
    
    // Apply filters
    if args.own {
        entries.retain(|e| e.address_type == AddressType::Own);
    } else if args.watched {
        entries.retain(|e| e.address_type == AddressType::Watched);
    }
    
    if entries.is_empty() {
        println!("No addresses found");
        return Ok(());
    }
    
    if args.table {
        print_addresses_table(&entries);
    } else {
        println!("📖 Address Book Quick View\n");
        
        for (id, entry) in entries.iter().enumerate() {
            let active_indicator = if entry.is_active { "●" } else { "○" };
            let address_short = format!("{}...{}", &entry.address[..6], &entry.address[entry.address.len()-4..]);
            
            print!("[{}] {} {} ", id, active_indicator, entry.display_name());
            
            if entry.label.is_some() {
                print!("({})", address_short);
            }
            
            print!(" - {}", entry.address_type);
            
            if args.with_value {
                if let Some(stats) = &entry.stats {
                    print!(" - ${:.2}", stats.total_value);
                }
            }
            
            if !entry.tags.is_empty() {
                print!(" [{}]", entry.tags.join(", "));
            }
            
            println!();
        }
        
        println!("\nTotal: {} addresses ({} active)", 
            entries.len(), 
            entries.iter().filter(|e| e.is_active).count()
        );
    }
    
    Ok(())
}

/// Toggle address active status
async fn toggle_address(args: ToggleAddressArgs, service: &AddressBookServiceHandle) -> Result<()> {
    // Get current address
    let (tx, rx) = oneshot::channel();
    service.send(AddressBookCommand::GetAddress {
        address_or_label: args.address_or_label.clone(),
        response: tx,
    }).await?;
    
    match rx.await?? {
        Some(entry) => {
            let new_status = !entry.is_active;
            
            let (tx, rx) = oneshot::channel();
            service.send(AddressBookCommand::UpdateAddress {
                address: entry.address.clone(),
                label: None,
                description: None,
                tags: None,
                is_active: Some(new_status),
                notes: None,
                response: tx,
            }).await?;
            
            rx.await??;
            
            println!("✅ {} is now {}", 
                entry.display_name(), 
                if new_status { "active" } else { "inactive" }
            );
        }
        None => {
            println!("Address not found");
        }
    }
    
    Ok(())
}

/// Tag multiple addresses
async fn tag_addresses(args: TagAddressesArgs, service: &AddressBookServiceHandle) -> Result<()> {
    let new_tags = parse_tags(Some(&args.tags));
    
    if new_tags.is_empty() {
        println!("No tags provided");
        return Ok(());
    }
    
    // Get addresses to tag
    let (tx, rx) = oneshot::channel();
    
    if let Some(pattern) = args.pattern {
        service.send(AddressBookCommand::QueryAddresses {
            query: AddressQuery {
                search: Some(pattern),
                address_type: args.address_type.as_deref().map(parse_address_type).transpose()?,
                ..Default::default()
            },
            response: tx,
        }).await?;
    } else if let Some(addr_type_str) = args.address_type {
        let addr_type = parse_address_type(&addr_type_str)?;
        service.send(AddressBookCommand::QueryAddresses {
            query: AddressQuery {
                address_type: Some(addr_type),
                ..Default::default()
            },
            response: tx,
        }).await?;
    } else {
        println!("Please specify --pattern or --address-type");
        return Ok(());
    }
    
    let entries = rx.await??;
    
    if entries.is_empty() {
        println!("No matching addresses found");
        return Ok(());
    }
    
    println!("🏷️  Adding tags [{}] to {} addresses...", args.tags, entries.len());
    
    let mut success_count = 0;
    
    for entry in entries {
        // Merge tags
        let mut all_tags = entry.tags.clone();
        for tag in &new_tags {
            if !all_tags.contains(tag) {
                all_tags.push(tag.clone());
            }
        }
        
        let (tx, rx) = oneshot::channel();
        service.send(AddressBookCommand::UpdateAddress {
            address: entry.address.clone(),
            label: None,
            description: None,
            tags: Some(all_tags),
            is_active: None,
            notes: None,
            response: tx,
        }).await?;
        
        if rx.await?.is_ok() {
            success_count += 1;
        }
    }
    
    println!("✅ Tagged {} addresses", success_count);
    
    Ok(())
}

// Re-export command types
// pub use AddAddressArgs as AddAddressCommand;
// pub use ListAddressesArgs as ListAddressesCommand;
// pub use RemoveAddressArgs as RemoveAddressCommand;
// pub use UpdateAddressArgs as UpdateAddressCommand;
// pub use QueryAddressArgs as QueryAddressCommand;

/// Print addresses in table format
fn print_addresses_table(entries: &[AddressEntry]) {
    if entries.is_empty() {
        println!("No addresses found");
        return;
    }
    
    println!("┌────┬─────────────────────┬──────────────┬─────────────┬─────────────────────┬────────┬────────────┬───────────┬──────────┐");
    println!("│ ID │ Label               │ Address      │ Type        │ Tags                │ Active │ Value      │ Positions │ Orders   │");
    println!("├────┼─────────────────────┼──────────────┼─────────────┼─────────────────────┼────────┼────────────┼───────────┼──────────┤");
    
    for (id, entry) in entries.iter().enumerate() {
        let label = entry.label.as_deref().unwrap_or("-");
        let label_display = if label.len() > 19 {
            format!("{}...", &label[..16])
        } else {
            format!("{:<19}", label)
        };
        
        let address_short = format!("{}...{}", &entry.address[..6], &entry.address[entry.address.len()-4..]);
        
        let tags = if entry.tags.is_empty() { 
            "-".to_string() 
        } else { 
            entry.tags.join(", ")
        };
        let tags_display = if tags.len() > 19 {
            format!("{}...", &tags[..16])
        } else {
            format!("{:<19}", tags)
        };
        
        let active = if entry.is_active { "✓" } else { "✗" };
        
        let address_type = entry.address_type.to_string();
        let type_display = if address_type.len() > 11 {
            format!("{}...", &address_type[..8])
        } else {
            format!("{:<11}", address_type)
        };
        
        let (value, positions, orders) = if let Some(stats) = &entry.stats {
            (
                format!("${:.2}", stats.total_value),
                stats.active_positions.to_string(),
                stats.active_orders.to_string(),
            )
        } else {
            ("-".to_string(), "-".to_string(), "-".to_string())
        };
        
        println!("│ {:<2} │ {} │ {} │ {} │ {} │ {:<6} │ {:>10} │ {:>9} │ {:>8} │",
            id,
            label_display,
            address_short,
            type_display,
            tags_display,
            active,
            value,
            positions,
            orders
        );
    }
    
    println!("└────┴─────────────────────┴──────────────┴─────────────┴─────────────────────┴────────┴────────────┴───────────┴──────────┘");
}

/// Print sync results in table format
fn print_sync_results_table(results: &[(AddressEntry, AddressStats)]) {
    if results.is_empty() {
        println!("No sync results");
        return;
    }
    
    println!("┌─────────────────────┬──────────────┬─────────────┬────────────┬───────────┬──────────┬──────────────┬────────────────┬────────────┬──────────┐");
    println!("│ Label               │ Address      │ Type        │ Total Value│ Positions │ Orders   │ Realized P&L │ Unrealized P&L │ Total P&L  │ Win Rate │");
    println!("├─────────────────────┼──────────────┼─────────────┼────────────┼───────────┼──────────┼──────────────┼────────────────┼────────────┼──────────┤");
    
    for (entry, stats) in results {
        let label = entry.label.as_deref().unwrap_or("-");
        let label_display = if label.len() > 19 {
            format!("{}...", &label[..16])
        } else {
            format!("{:<19}", label)
        };
        
        let address_short = format!("{}...{}", &entry.address[..6], &entry.address[entry.address.len()-4..]);
        
        let address_type = entry.address_type.to_string();
        let type_display = if address_type.len() > 11 {
            format!("{}...", &address_type[..8])
        } else {
            format!("{:<11}", address_type)
        };
        
        let total_pnl = stats.total_realized_pnl + stats.total_unrealized_pnl;
        let win_rate = stats.win_rate
            .map(|wr| format!("{:.1}%", wr))
            .unwrap_or_else(|| "-".to_string());
        
        println!("│ {} │ {} │ {} │ {:>10.2} │ {:>9} │ {:>8} │ {:>12.2} │ {:>14.2} │ {:>10.2} │ {:>8} │",
            label_display,
            address_short,
            type_display,
            stats.total_value,
            stats.active_positions,
            stats.active_orders,
            stats.total_realized_pnl,
            stats.total_unrealized_pnl,
            total_pnl,
            win_rate
        );
    }
    
    println!("└─────────────────────┴──────────────┴─────────────┴────────────┴───────────┴──────────┴──────────────┴────────────────┴────────────┴──────────┘");
}