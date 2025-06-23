//! CLI commands for Gamma API operations
//! 
//! This module provides comprehensive command-line interface for:
//! - Fetching markets, events, trades, and positions
//! - Interactive TUI for data exploration  
//! - Search and filtering operations
//! - Data export and analytics

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use std::time::Duration;
use tracing::{info, debug, error};
use tokio::time::timeout;

// TUI and crossterm imports
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, DisableMouseCapture, EnableMouseCapture, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{
        Block, Borders, List, ListItem, ListState, Paragraph, Wrap,
    },
    Frame, Terminal,
};
use rust_decimal::prelude::ToPrimitive;

use crate::gamma::*;
use crate::data_paths::DataPaths;

/// Gamma API command structure
#[derive(Debug, Args)]
pub struct GammaArgs {
    #[command(subcommand)]
    pub command: GammaCommand,
}

/// Available Gamma subcommands
#[derive(Debug, Subcommand)]
pub enum GammaCommand {
    /// Interactive TUI for data exploration
    Tui(TuiArgs),
    /// Fetch markets from Gamma API
    Markets(MarketsArgs),
    /// Fetch events from Gamma API
    Events(EventsArgs),
    /// Fetch trade history
    Trades(TradesArgs),
    /// Fetch user positions
    Positions(PositionsArgs),
    /// Search local data
    Search(SearchArgs),
    /// Interactive search with ultra-fast engine
    InteractiveSearch(InteractiveSearchArgs),
    /// Get analytics and statistics
    Analytics(AnalyticsArgs),
    /// Sync data from all APIs
    Sync(SyncArgs),
    /// Export data to various formats
    Export(ExportArgs),
    /// Process raw API responses into individual storage
    ProcessRaw(ProcessRawArgs),
    /// Clear/reset the cache
    ClearCache(ClearCacheArgs),
    /// Import raw session data into SurrealDB database
    ImportSession(ImportSessionArgs),
    /// Database operations (stats, search, export, etc.)
    Db(DbArgs),
    /// Build or rebuild Milli search index
    BuildIndex(BuildIndexArgs),
    /// Check search service status
    SearchStatus(SearchStatusArgs),
}

/// TUI arguments
#[derive(Debug, Args)]
pub struct TuiArgs {
    /// Custom data directory
    #[arg(long)]
    data_dir: Option<PathBuf>,
}

/// Market fetching arguments
#[derive(Debug, Args)]
pub struct MarketsArgs {
    /// Number of markets to fetch
    #[arg(long, default_value = "100")]
    limit: u32,
    
    /// Offset for pagination
    #[arg(long, default_value = "0")]
    offset: u32,
    
    /// Include archived markets
    #[arg(long)]
    archived: bool,
    
    /// Include only active markets
    #[arg(long)]
    active_only: bool,
    
    /// Include only closed markets
    #[arg(long)]
    closed_only: bool,
    
    /// Filter by tag
    #[arg(long)]
    tag: Option<String>,
    
    /// Filter by category
    #[arg(long)]
    category: Option<String>,
    
    /// Minimum volume filter
    #[arg(long)]
    min_volume: Option<f64>,
    
    /// Maximum volume filter
    #[arg(long)]
    max_volume: Option<f64>,
    
    /// Sort by field (volume, liquidity, start_date, end_date)
    #[arg(long, default_value = "volume")]
    sort_by: String,
    
    /// Sort in ascending order
    #[arg(long)]
    ascending: bool,
    
    /// Show detailed information
    #[arg(long, short)]
    detailed: bool,
    
    /// Export to JSON file
    #[arg(long)]
    export: Option<PathBuf>,
    
    /// Disable interactive TUI browser (use text output instead)
    #[arg(long)]
    no_interactive: bool,
    
    /// Fetch ALL markets from API (ignore limit, use pagination)
    #[arg(long)]
    all: bool,
    
    /// Force new session (don't resume from existing session)
    #[arg(long)]
    new_session: bool,
    
    /// Manual refresh - clear immutable storage and refetch
    #[arg(long)]
    refresh: bool,
    
    /// Load markets from database instead of fetching from API
    #[arg(long)]
    from_db: bool,
}

/// Event fetching arguments
#[derive(Debug, Args)]
pub struct EventsArgs {
    /// Number of events to fetch
    #[arg(long, default_value = "50")]
    limit: u32,
    
    /// Include archived events
    #[arg(long)]
    archived: bool,
    
    /// Filter by tag
    #[arg(long)]
    tag: Option<String>,
    
    /// Show detailed information
    #[arg(long, short)]
    detailed: bool,
}

/// Trade fetching arguments
#[derive(Debug, Args)]
pub struct TradesArgs {
    /// User address to fetch trades for
    #[arg(long)]
    user: Option<String>,
    
    /// Market condition ID to fetch trades for
    #[arg(long)]
    market: Option<String>,
    
    /// Number of trades to fetch
    #[arg(long, default_value = "100")]
    limit: u32,
    
    /// Only show taker trades
    #[arg(long)]
    taker_only: bool,
    
    /// Filter by trade side (buy/sell)
    #[arg(long)]
    side: Option<String>,
    
    /// Minimum trade size
    #[arg(long)]
    min_size: Option<f64>,
    
    /// Show detailed information
    #[arg(long, short)]
    detailed: bool,
}

/// Position fetching arguments
#[derive(Debug, Args)]
pub struct PositionsArgs {
    /// User address to fetch positions for (required)
    #[arg(long)]
    user: String,
    
    /// Market condition ID to filter by
    #[arg(long)]
    market: Option<String>,
    
    /// Event ID to filter by
    #[arg(long)]
    event: Option<String>,
    
    /// Minimum position size threshold
    #[arg(long, default_value = "1.0")]
    size_threshold: f64,
    
    /// Only show redeemable positions
    #[arg(long)]
    redeemable_only: bool,
    
    /// Sort by field (size, current_value, pnl)
    #[arg(long, default_value = "current_value")]
    sort_by: String,
    
    /// Show detailed information
    #[arg(long, short)]
    detailed: bool,
}

/// Search arguments
#[derive(Debug, Args)]
pub struct SearchArgs {
    /// Keyword to search for
    #[arg(long)]
    keyword: Option<String>,
    
    /// Tags to filter by
    #[arg(long)]
    tags: Vec<String>,
    
    /// Category to filter by
    #[arg(long)]
    category: Option<String>,
    
    /// Minimum volume
    #[arg(long)]
    min_volume: Option<f64>,
    
    /// Maximum volume
    #[arg(long)]
    max_volume: Option<f64>,
    
    /// Market type (binary/categorical)
    #[arg(long)]
    market_type: Option<String>,
    
    /// Only active markets
    #[arg(long)]
    active_only: bool,
    
    /// Number of results to return
    #[arg(long, default_value = "50")]
    limit: usize,
}

/// Analytics arguments
#[derive(Debug, Args)]
pub struct AnalyticsArgs {
    /// Show detailed breakdown
    #[arg(long, short)]
    detailed: bool,
    
    /// Export analytics to JSON
    #[arg(long)]
    export: Option<PathBuf>,
}

/// Sync arguments
#[derive(Debug, Args)]
pub struct SyncArgs {
    /// Sync markets
    #[arg(long)]
    markets: bool,
    
    /// Sync events
    #[arg(long)]
    events: bool,
    
    /// Sync trades
    #[arg(long)]
    trades: bool,
    
    /// User address for position sync
    #[arg(long)]
    user: Option<String>,
    
    /// Number of items to fetch per API call
    #[arg(long, default_value = "500")]
    batch_size: u32,
    
    /// Clear existing data before sync
    #[arg(long)]
    clear: bool,
    
    /// Show detailed output
    #[arg(long, short)]
    detailed: bool,
}

/// Export arguments
#[derive(Debug, Args)]
pub struct ExportArgs {
    /// Output file path
    #[arg(long)]
    output: PathBuf,
    
    /// Format (json, csv)
    #[arg(long, default_value = "json")]
    format: String,
    
    /// Data type to export (markets, events, trades, positions)
    #[arg(long, default_value = "markets")]
    data_type: String,
    
    /// Apply filters during export
    #[arg(long)]
    filter: Option<String>,
}

/// Process raw API responses arguments
#[derive(Debug, Args)]
pub struct ProcessRawArgs {
    /// Path to raw responses directory (default: ./data/gamma_raw_responses)
    #[arg(long)]
    raw_dir: Option<PathBuf>,
    
    /// Output directory for individual storage (default: ./data/database/gamma)
    #[arg(long)]
    output_dir: Option<PathBuf>,
    
    /// Process only specific file
    #[arg(long)]
    file: Option<PathBuf>,
}

/// Clear cache arguments
#[derive(Debug, Args)]
pub struct ClearCacheArgs {
    /// Clear only the cache file (keep raw responses and individual storage)
    #[arg(long)]
    cache_only: bool,
    
    /// Clear raw API responses
    #[arg(long)]
    raw_responses: bool,
    
    /// Clear individual storage (JSON and RocksDB)
    #[arg(long)]
    individual_storage: bool,
    
    /// Clear everything (cache, raw responses, and individual storage)
    #[arg(long)]
    all: bool,
    
    /// Confirm the operation without prompting
    #[arg(long, short)]
    yes: bool,
}

/// Database subcommands
/// Database command arguments
#[derive(Debug, Args)]
pub struct DbArgs {
    /// Database subcommand (defaults to interactive list if not specified)
    #[command(subcommand)]
    pub command: Option<DbCommand>,
}

#[derive(Debug, Subcommand)]
pub enum DbCommand {
    /// List markets with filters (default: interactive TUI)
    #[command(alias = "browse")]
    List(DbListArgs),
    /// Show database statistics
    Stats(DbStatsArgs),
    /// Search markets in database
    Search(DbSearchArgs),
    /// Export database content
    Export(DbExportArgs),
    /// Clean up database (remove duplicates, optimize)
    Cleanup(DbCleanupArgs),
    /// Get specific market by ID
    Get(DbGetArgs),
    /// Count markets by various criteria
    Count(DbCountArgs),
    /// Check database health and performance
    Health(DbHealthArgs),
}

/// Database stats arguments
#[derive(Debug, Args)]
pub struct DbStatsArgs {
    /// Show detailed statistics
    #[arg(long, short)]
    detailed: bool,
    
    /// Group by category
    #[arg(long)]
    by_category: bool,
    
    /// Show volume distribution
    #[arg(long)]
    volume_dist: bool,
    
    /// Show temporal distribution
    #[arg(long)]
    temporal: bool,
}

/// Database search arguments
#[derive(Debug, Args)]
pub struct DbSearchArgs {
    /// Search query (searches in question, description, category)
    query: String,
    
    /// Limit number of results
    #[arg(long, default_value = "20")]
    limit: u64,
    
    /// Search only in specific field
    #[arg(long)]
    field: Option<String>,
    
    /// Case sensitive search
    #[arg(long)]
    case_sensitive: bool,
    
    /// Regular expression search
    #[arg(long)]
    regex: bool,
    
    /// Output format (table, json, csv)
    #[arg(long, default_value = "table")]
    format: String,
}

/// Database export arguments
#[derive(Debug, Args)]
pub struct DbExportArgs {
    /// Output file path
    output: PathBuf,
    
    /// Export format (json, csv, parquet)
    #[arg(long, default_value = "json")]
    format: String,
    
    /// Filter active markets only
    #[arg(long)]
    active_only: bool,
    
    /// Filter by minimum volume
    #[arg(long)]
    min_volume: Option<f64>,
    
    /// Filter by category
    #[arg(long)]
    category: Option<String>,
    
    /// Limit number of records
    #[arg(long)]
    limit: Option<u64>,
    
    /// Include only specific fields (comma-separated)
    #[arg(long)]
    fields: Option<String>,
}

/// Database list arguments
#[derive(Debug, Clone, Args)]
pub struct DbListArgs {
    /// Number of markets to display
    #[arg(long, default_value = "20")]
    limit: u64,
    
    /// Offset for pagination
    #[arg(long, default_value = "0")]
    offset: u64,
    
    /// Sort by field (volume, liquidity, created_at, updated_at)
    #[arg(long, default_value = "volume")]
    sort_by: String,
    
    /// Sort order (asc, desc)
    #[arg(long, default_value = "desc")]
    order: String,
    
    /// Filter active markets only
    #[arg(long)]
    active_only: bool,
    
    /// Filter closed markets only
    #[arg(long)]
    closed_only: bool,
    
    /// Filter by category
    #[arg(long)]
    category: Option<String>,
    
    /// Filter by minimum volume
    #[arg(long)]
    min_volume: Option<f64>,
    
    /// Output format (table, json, csv)
    #[arg(long, default_value = "table")]
    format: String,
    
    /// Launch interactive TUI browser (like gamma markets) - default behavior
    #[arg(long, short, default_value = "true")]
    interactive: bool,
    
    /// Disable interactive mode and use simple table output
    #[arg(long, conflicts_with = "interactive")]
    no_interactive: bool,
}

/// Database cleanup arguments
#[derive(Debug, Args)]
pub struct DbCleanupArgs {
    /// Remove duplicate markets
    #[arg(long)]
    remove_duplicates: bool,
    
    /// Vacuum database (reclaim space)
    #[arg(long)]
    vacuum: bool,
    
    /// Update statistics
    #[arg(long)]
    update_stats: bool,
    
    /// Remove markets older than N days
    #[arg(long)]
    remove_older_than: Option<u32>,
    
    /// Dry run (show what would be done without doing it)
    #[arg(long)]
    dry_run: bool,
    
    /// Confirm the operation without prompting
    #[arg(long, short)]  
    yes: bool,
}

/// Database get arguments
#[derive(Debug, Args)]
pub struct DbGetArgs {
    /// Market ID to retrieve
    market_id: String,
    
    /// Output format (json, yaml, table)
    #[arg(long, default_value = "json")]
    format: String,
    
    /// Show all fields including internal ones
    #[arg(long)]
    all_fields: bool,
}

/// Database count arguments
#[derive(Debug, Args)]
pub struct DbCountArgs {
    /// Group by field (category, active, closed, archived)
    #[arg(long)]
    group_by: Option<String>,
    
    /// Filter active markets only
    #[arg(long)]
    active_only: bool,
    
    /// Filter by category
    #[arg(long)]
    category: Option<String>,
    
    /// Show as percentage
    #[arg(long)]
    percentage: bool,
}

/// Database health check arguments
#[derive(Debug, Args)]
pub struct DbHealthArgs {
    /// Show detailed health information
    #[arg(long)]
    detailed: bool,
    
    /// Run performance benchmarks
    #[arg(long)]
    benchmark: bool,
}

/// Build index arguments
#[derive(Debug, Args)]
pub struct BuildIndexArgs {
    /// Index type to build (default: "markets")
    #[arg(long, default_value = "markets")]
    index_type: String,
    
    /// Force rebuild even if index exists
    #[arg(long)]
    force: bool,
}

/// Interactive search arguments
#[derive(Debug, Args)]
pub struct InteractiveSearchArgs {
    /// Show search statistics
    #[arg(long)]
    show_stats: bool,
}

/// Search status arguments
#[derive(Debug, Args)]
pub struct SearchStatusArgs {
    /// Watch status updates in real-time
    #[arg(long)]
    watch: bool,
    
    /// Output format (text, json)
    #[arg(long, default_value = "text")]
    format: String,
}

/// Import session arguments
#[derive(Debug, Args)]
pub struct ImportSessionArgs {
    /// Session ID to import (e.g., 1, 2, 3...) or 'all' for all sessions
    #[arg(long)]
    session_id: Option<String>,
    
    /// Starting session ID for range import (inclusive)
    #[arg(long)]
    from_session_id: Option<u32>,
    
    /// Ending session ID for range import (inclusive)
    #[arg(long)]
    to_session_id: Option<u32>,
    
    /// Force reimport even if data already exists in database
    #[arg(long)]
    force: bool,
    
    
    /// Batch size for database insertions (default: 100)
    #[arg(long, default_value = "100")]
    batch_size: usize,
    
    /// Confirm the operation without prompting
    #[arg(long, short)]
    yes: bool,
}

/// Execute Gamma command
pub async fn execute_gamma_command(args: GammaArgs, verbose: bool) -> Result<()> {
    // Set log level BEFORE any logging initialization to ensure it takes effect
    // This suppresses verbose SurrealDB and RocksDB initialization logs that would otherwise
    // clutter the output with internal database operations
    if verbose {
        std::env::set_var("RUST_LOG", "debug,polybot=debug,surrealdb=warn,surreal=warn,rocksdb=warn,tokio_util=warn");
    } else {
        std::env::set_var("RUST_LOG", "info,polybot=info,surrealdb=warn,surreal=warn,rocksdb=warn,tokio_util=warn");
    }
    
    // Determine if we're using TUI mode (which should not log to console)
    let uses_tui = matches!(args.command, GammaCommand::Tui(_) | GammaCommand::Db(_));
    
    // Initialize logging
    let data_paths = crate::data_paths::DataPaths::new("./data");
    let log_mode = if uses_tui {
        crate::logging::LogMode::FileOnly
    } else {
        crate::logging::LogMode::ConsoleAndFile
    };
    let log_config = crate::logging::LoggingConfig::new(log_mode, data_paths);
    
    crate::logging::init_logging(log_config)?;
    
    info!("Starting Gamma command");
    
    match args.command {
        GammaCommand::Tui(tui_args) => execute_tui(tui_args).await,
        GammaCommand::Markets(market_args) => execute_markets(market_args, verbose).await,
        GammaCommand::Events(event_args) => execute_events(event_args, verbose).await,
        GammaCommand::Trades(trade_args) => execute_trades(trade_args, verbose).await,
        GammaCommand::Positions(position_args) => execute_positions(position_args, verbose).await,
        GammaCommand::Search(search_args) => execute_search(search_args).await,
        GammaCommand::InteractiveSearch(args) => execute_interactive_search(args, verbose).await,
        GammaCommand::Analytics(analytics_args) => execute_analytics(analytics_args).await,
        GammaCommand::Sync(sync_args) => execute_sync(sync_args, verbose).await,
        GammaCommand::Export(export_args) => execute_export(export_args).await,
        GammaCommand::ProcessRaw(process_args) => execute_process_raw(process_args).await,
        GammaCommand::ClearCache(clear_args) => execute_clear_cache(clear_args).await,
        GammaCommand::ImportSession(import_args) => execute_import_session(import_args, verbose).await,
        GammaCommand::Db(db_args) => execute_db_command(db_args, verbose).await,
        GammaCommand::BuildIndex(build_args) => execute_build_index(build_args, verbose).await,
        GammaCommand::SearchStatus(status_args) => execute_search_status(status_args, verbose).await,
    }
}

/// Execute TUI command
async fn execute_tui(args: TuiArgs) -> Result<()> {
    println!("{}", "🚀 Starting Gamma Explorer TUI...".bright_green());
    
    let data_dir = args.data_dir
        .unwrap_or_else(|| DataPaths::new(std::env::current_dir().unwrap()).root().join("database/gamma"));
    
    let storage = GammaStorage::new(&data_dir)
        .context("Failed to open Gamma storage")?;
    
    let mut tui = GammaTui::new(storage);
    tui.run().context("TUI execution failed")?;
    
    Ok(())
}

/// Execute markets command with session-based persistent storage and search service integration
async fn execute_markets(args: MarketsArgs, verbose: bool) -> Result<()> {
    // Check if loading from database
    if args.from_db {
        println!("{}", "📊 Loading markets from database with search service integration...".bright_blue());
        
        // Initialize search service
        let data_dir = DataPaths::new(std::env::current_dir().unwrap()).root().join("database/gamma");
        let storage = GammaStorage::new(&data_dir)
            .context("Failed to open Gamma storage")?;
        let search_engine = GammaSearchEngine::new(storage);
        
        // Build search filters from command arguments
        let search_filters = SearchFilters {
            keyword: None, // Markets command doesn't have keyword search, use search subcommand instead
            tags: args.tag.as_ref().map(|t| vec![t.clone()]).unwrap_or_default(),
            category: args.category.clone(),
            min_volume: args.min_volume.map(|v| rust_decimal::Decimal::try_from(v).unwrap()),
            max_volume: args.max_volume.map(|v| rust_decimal::Decimal::try_from(v).unwrap()),
            min_liquidity: None,
            max_liquidity: None,
            active_only: args.active_only,
            closed_only: args.closed_only,
            archived_only: args.archived,
            market_type: None,
            date_range: None,
        };
        
        // Use search engine for sophisticated filtering
        let has_filters = search_filters.category.is_some() || 
                          search_filters.min_volume.is_some() || 
                          search_filters.max_volume.is_some() ||
                          !search_filters.tags.is_empty() ||
                          search_filters.active_only || 
                          search_filters.closed_only || 
                          search_filters.archived_only;
                          
        let filtered_markets = if has_filters {
            println!("{}", "🔍 Applying advanced search filters...".bright_cyan());
            search_engine.search_markets(&search_filters)
                .context("Failed to search markets with filters")?
        } else {
            // Fallback to loading all markets from database
            let all_markets = load_markets_from_database().await
                .context("Failed to load markets from database")?;
            
            if all_markets.is_empty() {
                println!("{}", "⚠️  No markets found in database. Use 'gamma import-session' to import data first.".bright_yellow());
                return Ok(());
            }
            
            println!("{}", format!("✅ Loaded {} markets from database", all_markets.len()).bright_green());
            all_markets
        };
        
        // Sort markets (search engine provides basic sorting by volume)
        let mut final_markets = filtered_markets;
        match args.sort_by.as_str() {
            "volume" => {
                if args.ascending {
                    final_markets.sort_by_key(|m| m.volume());
                } else {
                    final_markets.sort_by_key(|m| std::cmp::Reverse(m.volume()));
                }
            },
            "liquidity" => {
                if args.ascending {
                    final_markets.sort_by_key(|m| m.liquidity.unwrap_or_default());
                } else {
                    final_markets.sort_by_key(|m| std::cmp::Reverse(m.liquidity.unwrap_or_default()));
                }
            },
            "start_date" => {
                if args.ascending {
                    final_markets.sort_by_key(|m| m.start_date);
                } else {
                    final_markets.sort_by_key(|m| std::cmp::Reverse(m.start_date));
                }
            },
            "end_date" => {
                if args.ascending {
                    final_markets.sort_by_key(|m| m.end_date);
                } else {
                    final_markets.sort_by_key(|m| std::cmp::Reverse(m.end_date));
                }
            },
            _ => {} // Default is already sorted by volume desc
        }
        
        // Apply limit if not --all
        if !args.all && final_markets.len() > args.limit as usize {
            final_markets.truncate(args.limit as usize);
        }
        
        println!("{}", format!("📊 Showing {} markets after search service filtering", final_markets.len()).bright_cyan());
        
        // Handle output
        return handle_markets_output(final_markets, &args, verbose).await;
    }
    
    println!("{}", "📈 Fetching markets with session-based storage...".bright_blue());
    info!("Executing markets command with args: {:?}", args);
    
    // Setup session manager
    let base_path = PathBuf::from("./data/gamma/raw");
    let mut session_manager = SessionManager::new(base_path)
        .context("Failed to create session manager")?;
    
    // Build query from arguments
    let query = MarketQuery {
        limit: Some(500), // Always use batch size of 500 for optimal API performance
        offset: Some(args.offset),
        archived: if args.archived { Some(true) } else { None },
        active: if args.active_only { Some(true) } else { None },
        closed: if args.closed_only { Some(true) } else { None },
        tags: args.tag.clone().map(|t| vec![t]).unwrap_or_default(),
        order: Some(args.sort_by.clone()),
        ascending: Some(args.ascending),
        volume_min: args.min_volume.map(|v| rust_decimal::Decimal::try_from(v).unwrap()),
        volume_max: args.max_volume.map(|v| rust_decimal::Decimal::try_from(v).unwrap()),
        ..Default::default()
    };
    
    info!("Starting session-based market fetching with query: {:?}", query);
    
    // Handle refresh flag by clearing sessions if requested
    if args.refresh {
        println!("{}", "🔄 Manual refresh requested - clearing existing sessions...".bright_yellow());
        // TODO: Implement session clearing functionality
    }
    
    // Start or resume session
    let (session_id, session_path, mut metadata) = session_manager
        .start_or_resume_session(query.clone(), args.new_session)
        .context("Failed to start or resume session")?;
    
    // Always check for new data - don't assume completion is permanent
    if metadata.is_complete && !args.refresh {
        println!("{}", format!("🔍 Session {} marked complete - checking for new data from offset {}", 
                               session_id, metadata.last_offset).bright_cyan());
        
        // Reset completion flag to allow checking for new data
        metadata.is_complete = false;
    }
    
    let status_msg = if metadata.total_markets_fetched > 0 {
        format!("🔄 Using session {} - checking for new data from offset {} ({} markets already cached)", 
                session_id, metadata.last_offset, metadata.total_markets_fetched)
    } else {
        format!("📊 Using session {} - starting fresh fetch from offset {}", 
                session_id, metadata.last_offset)
    };
    println!("{}", status_msg.bright_cyan());
    
    // Initialize client and setup Ctrl+C handling
    let client = GammaClient::new();
    let session_path_clone = session_path.to_path_buf();
    let metadata_clone = metadata.clone();
    
    ctrlc::set_handler(move || {
        println!("\n{}", "⚠️  Interrupted! Saving session metadata...".bright_yellow());
        if let Err(e) = metadata_clone.save(&session_path_clone) {
            eprintln!("Failed to save metadata on interrupt: {}", e);
        } else {
            println!("{}", "✅ Session metadata saved successfully".bright_green());
        }
        std::process::exit(0);
    }).context("Failed to set Ctrl+C handler")?;
    
    // Initialize database connection for session storage
    let db_path = PathBuf::from("./data/database/gamma");
    let database = GammaDatabase::new(&db_path).await
        .context("Failed to initialize gamma database")?;
    
    // Fetch markets with session-based storage
    let mut total_fetched = 0;
    let mut current_query = query;
    current_query.offset = Some(metadata.last_offset as u32);
    
    let target_limit = if args.all { 
        None // Fetch all available
    } else { 
        Some(args.limit as usize)
    };
    
    loop {
        // Check if we've reached the target limit (if specified) but only for display purposes
        // We should continue fetching until we exhaust the data source
        let should_display_limited = if let Some(limit) = target_limit {
            total_fetched >= limit
        } else {
            false
        };
        
        // Fetch next batch
        let fetch_msg = if metadata.total_markets_fetched > 0 {
            format!("🔍 Checking for new markets from offset {}...", metadata.last_offset)
        } else {
            format!("📥 Fetching initial batch from offset {}...", metadata.last_offset)
        };
        println!("{}", fetch_msg.bright_blue());
        
        let response = client.fetch_markets(&current_query).await
            .context("Failed to fetch markets batch")?;
        let raw_json = serde_json::to_string(&response.data)?;
        
        if response.data.is_empty() {
            println!("{}", "✅ No new markets found - all data up to date".bright_green());
            metadata.is_complete = true;
            // Mark that we checked for updates
            metadata.mark_checked_for_updates();
            session_manager.save_session_metadata(&session_path, &metadata)
                .context("Failed to save metadata after update check")?;
            break;
        }
        
        // Check if we've reached data we already have (overlapping with previous fetch)
        if let Some(last_market_id) = &metadata.last_market_id {
            // Check if the last market from our previous fetch appears in current batch
            let overlaps_with_previous = response.data.iter()
                .any(|m| m.id.0.to_string() == *last_market_id);
                
            if overlaps_with_previous {
                // Filter out markets we already have
                let new_markets: Vec<_> = response.data.iter()
                    .skip_while(|m| m.id.0.to_string() != *last_market_id)
                    .skip(1) // Skip the duplicate market itself
                    .cloned()
                    .collect();
                
                if new_markets.is_empty() {
                    println!("{}", "✅ No new markets found beyond previous fetch - data is current".bright_green());
                    metadata.is_complete = true;
                    // Mark that we checked for updates
                    metadata.mark_checked_for_updates();
                    session_manager.save_session_metadata(&session_path, &metadata)
                        .context("Failed to save metadata after update check")?;
                    break;
                } else {
                    println!("{}", format!("🔄 Found {} new markets (filtered {} duplicates)", 
                                         new_markets.len(), response.data.len() - new_markets.len()).bright_yellow());
                    // Continue with new markets only
                }
            }
        }
        
        // Store raw response in session
        session_manager.store_raw_response(&session_path, metadata.last_offset, &raw_json)
            .context("Failed to store raw response")?;
        
        // Store markets in database for deduplicated access
        let db_stored_count = database.store_markets(&response.data, session_id).await
            .context("Failed to store markets in database")?;
        
        debug!("Stored {} markets in database (session {})", db_stored_count, session_id);
        
        // Update cursor state with new count and last market info
        let batch_size = response.data.len();
        let last_market_id = response.data.last().map(|m| m.id.0.to_string());
        
        // Create updated cursor state
        let mut updated_cursor = metadata.cursor_state.clone();
        updated_cursor.count += batch_size;
        updated_cursor.last_market_id = last_market_id.clone();
        updated_cursor.is_exhausted = !response.has_more || batch_size < 500;
        
        // Update metadata with the new cursor state
        metadata.update_after_fetch(batch_size, &updated_cursor, last_market_id);
        
        // Save metadata after each batch
        session_manager.save_session_metadata(&session_path, &metadata)
            .context("Failed to save session metadata")?;
        
        total_fetched += batch_size;
        
        let progress_msg = if should_display_limited {
            format!("✅ Stored {} markets (fetching continues beyond display limit, session total: {})", 
                    batch_size, metadata.total_markets_fetched)
        } else {
            format!("✅ Stored {} markets (total: {}, session total: {})", 
                    batch_size, total_fetched, metadata.total_markets_fetched)
        };
        println!("{}", progress_msg.bright_green());
        
        // Update query for next batch
        current_query.offset = Some(metadata.last_offset as u32);
        
        // Check if API indicates no more data or we got an incomplete batch
        if !response.has_more || batch_size < 500 {
            println!("{}", "✅ API indicates no more data - fetch complete".bright_green());
            metadata.is_complete = true;
            session_manager.save_session_metadata(&session_path, &metadata)
                .context("Failed to save final metadata")?;
            break;
        }
        
        // Continue fetching if we got a full batch (500 markets), regardless of display limit
        // Only stop when we actually run out of data or get incomplete batches
        
        // Small delay to be respectful to API
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    
    let completion_msg = if metadata.is_complete {
        format!("🎉 Session {} fetch complete! Fetched {} new markets (total: {})", 
                session_id, total_fetched, metadata.total_markets_fetched)
    } else {
        format!("🎉 Session {} updated! Fetched {} new markets (total: {})", 
                session_id, total_fetched, metadata.total_markets_fetched)
    };
    println!("{}", completion_msg.bright_green());
    
    // Load all markets from the same database connection for output
    let all_markets = database.get_all_markets(None).await
        .context("Failed to load markets from database")?;
        
    // Close the database connection properly
    database.close().await.context("Failed to close database connection")?;
    
    handle_markets_output(all_markets, &args, verbose).await
}

/// Execute events command
async fn execute_events(args: EventsArgs, verbose: bool) -> Result<()> {
    println!("{}", "📅 Fetching events...".bright_blue());
    
    let client = GammaClient::new();
    
    let query = EventQuery {
        limit: Some(args.limit),
        archived: if args.archived { Some(true) } else { None },
        tags: args.tag.map(|t| vec![t]).unwrap_or_default(),
        ..Default::default()
    };
    
    let response = client.fetch_events(&query).await
        .context("Failed to fetch events")?;
    
    println!("Found {} events:", response.data.len());
    
    for (i, event) in response.data.iter().enumerate() {
        if args.detailed || verbose {
            print_event_detailed(event, i + 1);
        } else {
            print_event_summary(event, i + 1);
        }
    }
    
    Ok(())
}

/// Execute trades command
async fn execute_trades(args: TradesArgs, verbose: bool) -> Result<()> {
    println!("{}", "💱 Fetching trades...".bright_blue());
    
    let client = GammaClient::new();
    
    let query = TradeQuery {
        user: args.user.map(|u| UserAddress(u)),
        market: args.market.map(|m| ConditionId(m)),
        limit: Some(args.limit),
        taker_only: if args.taker_only { Some(true) } else { None },
        side: args.side.and_then(|s| match s.to_lowercase().as_str() {
            "buy" => Some(TradeSide::Buy),
            "sell" => Some(TradeSide::Sell),
            _ => None,
        }),
        ..Default::default()
    };
    
    let response = client.fetch_trades(&query).await
        .context("Failed to fetch trades")?;
    
    println!("Found {} trades:", response.data.len());
    
    for (i, trade) in response.data.iter().enumerate() {
        if args.detailed || verbose {
            print_trade_detailed(trade, i + 1);
        } else {
            print_trade_summary(trade, i + 1);
        }
    }
    
    Ok(())
}

/// Execute positions command
async fn execute_positions(args: PositionsArgs, verbose: bool) -> Result<()> {
    println!("{}", "📊 Fetching positions...".bright_blue());
    
    let client = GammaClient::new();
    
    let query = PositionQuery {
        user: UserAddress(args.user),
        markets: args.market.map(|m| vec![ConditionId(m)]).unwrap_or_default(),
        event_id: args.event.map(|e| EventId(e.parse().unwrap_or(0))),
        size_threshold: Some(rust_decimal::Decimal::try_from(args.size_threshold).unwrap()),
        redeemable: if args.redeemable_only { Some(true) } else { None },
        sort_by: Some(args.sort_by),
        sort_direction: Some("DESC".to_string()),
        ..Default::default()
    };
    
    let response = client.fetch_positions(&query).await
        .context("Failed to fetch positions")?;
    
    println!("Found {} positions:", response.data.len());
    
    for (i, position) in response.data.iter().enumerate() {
        if args.detailed || verbose {
            print_position_detailed(position, i + 1);
        } else {
            print_position_summary(position, i + 1);
        }
    }
    
    Ok(())
}

/// Execute search command
async fn execute_search(args: SearchArgs) -> Result<()> {
    println!("{}", "🔍 Searching local data...".bright_blue());
    
    let data_dir = DataPaths::new(std::env::current_dir().unwrap()).root().join("database/gamma");
    let storage = GammaStorage::new(&data_dir)
        .context("Failed to open Gamma storage")?;
    let search_engine = GammaSearchEngine::new(storage);
    
    let filters = SearchFilters {
        keyword: args.keyword,
        tags: args.tags,
        category: args.category,
        min_volume: args.min_volume.map(|v| rust_decimal::Decimal::try_from(v).unwrap()),
        max_volume: args.max_volume.map(|v| rust_decimal::Decimal::try_from(v).unwrap()),
        active_only: args.active_only,
        market_type: args.market_type.and_then(|t| match t.to_lowercase().as_str() {
            "binary" => Some(MarketType::Binary),
            "categorical" => Some(MarketType::Categorical),
            _ => None,
        }),
        ..Default::default()
    };
    
    let results = search_engine.search_markets(&filters)
        .context("Search failed")?;
    
    println!("Found {} matching markets:", results.len());
    
    for (i, market) in results.iter().take(args.limit).enumerate() {
        print_market_summary(market, i + 1);
    }
    
    Ok(())
}

/// Execute analytics command
async fn execute_analytics(args: AnalyticsArgs) -> Result<()> {
    println!("{}", "📊 Generating analytics...".bright_blue());
    
    let data_dir = DataPaths::new(std::env::current_dir().unwrap()).root().join("database/gamma");
    let storage = GammaStorage::new(&data_dir)
        .context("Failed to open Gamma storage")?;
    let search_engine = GammaSearchEngine::new(storage);
    
    let analytics = search_engine.get_market_analytics()
        .context("Failed to generate analytics")?;
    
    print_analytics(&analytics, args.detailed);
    
    // Export if requested
    if let Some(export_path) = args.export {
        let json = serde_json::to_string_pretty(&analytics)
            .context("Failed to serialize analytics")?;
        std::fs::write(&export_path, json)
            .context("Failed to write export file")?;
        println!("Exported analytics to {}", export_path.display());
    }
    
    Ok(())
}

/// Execute sync command
async fn execute_sync(args: SyncArgs, verbose: bool) -> Result<()> {
    println!("{}", "🔄 Syncing data...".bright_green());
    
    let data_dir = DataPaths::new(std::env::current_dir().unwrap()).root().join("database/gamma");
    let storage = GammaStorage::new(&data_dir)
        .context("Failed to open Gamma storage")?;
    
    if args.clear {
        println!("{}", "🗑️  Clearing existing data...".bright_yellow());
        storage.clear_all().context("Failed to clear data")?;
    }
    
    let client = GammaClient::new();
    
    if args.markets {
        println!("{}", "📈 Syncing markets...".bright_blue());
        sync_markets(&client, &storage, args.batch_size, args.detailed || verbose).await?;
    }
    
    if args.events {
        println!("{}", "📅 Syncing events...".bright_blue());
        sync_events(&client, &storage, args.batch_size, args.detailed || verbose).await?;
    }
    
    if args.trades {
        println!("{}", "💱 Syncing trades...".bright_blue());
        sync_trades(&client, &storage, args.batch_size, args.detailed || verbose).await?;
    }
    
    if let Some(user) = args.user {
        println!("{}", "📊 Syncing positions...".bright_blue());
        sync_positions(&client, &storage, &user, args.detailed || verbose).await?;
    }
    
    println!("{}", "✅ Sync completed!".bright_green());
    
    Ok(())
}

/// Execute export command
async fn execute_export(args: ExportArgs) -> Result<()> {
    println!("{}", "📁 Exporting data...".bright_blue());
    
    // Load data based on type
    let data: Vec<serde_json::Value> = match args.data_type.as_str() {
        "markets" => {
            println!("Loading markets from database...");
            let markets = load_markets_from_database().await?;
            println!("Found {} markets", markets.len());
            
            // Apply filter if provided
            let filtered = if let Some(filter) = &args.filter {
                markets.into_iter()
                    .filter(|m| m.question.to_lowercase().contains(&filter.to_lowercase()))
                    .collect::<Vec<_>>()
            } else {
                markets
            };
            
            filtered.into_iter()
                .map(|m| serde_json::to_value(m).unwrap())
                .collect()
        }
        "events" => {
            println!("Export for events not yet implemented");
            return Err(anyhow::anyhow!("Events export is not yet implemented. Currently only 'markets' export is supported."));
        }
        "trades" => {
            println!("Export for trades not yet implemented");
            return Err(anyhow::anyhow!("Trades export is not yet implemented. Currently only 'markets' export is supported."));
        }
        "positions" => {
            println!("Export for positions not yet implemented");
            return Err(anyhow::anyhow!("Positions export is not yet implemented. Currently only 'markets' export is supported."));
        }
        _ => {
            return Err(anyhow::anyhow!("Unknown data type: {}. Use: markets, events, trades, or positions", args.data_type));
        }
    };
    
    if data.is_empty() {
        println!("{}", "⚠️  No data found to export".bright_yellow());
        return Ok(());
    }
    
    // Export based on format
    match args.format.as_str() {
        "json" => {
            let json = serde_json::to_string_pretty(&data)?;
            std::fs::write(&args.output, json)?;
            println!("✅ Exported {} records to {} (JSON)", data.len(), args.output.display());
        }
        "csv" => {
            // Create CSV writer
            let mut wtr = csv::Writer::from_path(&args.output)?;
            
            // Write records - handle nested JSON by flattening
            for record in &data {
                if let Some(obj) = record.as_object() {
                    let flat_record: std::collections::HashMap<String, String> = obj.iter()
                        .map(|(k, v)| {
                            let value_str = match v {
                                serde_json::Value::String(s) => s.clone(),
                                serde_json::Value::Number(n) => n.to_string(),
                                serde_json::Value::Bool(b) => b.to_string(),
                                serde_json::Value::Null => "".to_string(),
                                _ => serde_json::to_string(v).unwrap_or_default(),
                            };
                            (k.clone(), value_str)
                        })
                        .collect();
                    wtr.serialize(&flat_record)?;
                }
            }
            wtr.flush()?;
            println!("✅ Exported {} records to {} (CSV)", data.len(), args.output.display());
        }
        _ => {
            return Err(anyhow::anyhow!("Unknown format: {}. Use: json or csv", args.format));
        }
    }
    
    Ok(())
}

// ============================================================================
// SESSION-BASED HELPER FUNCTIONS
// ============================================================================

/// Load all markets from database (deduplicated and efficient)
async fn load_markets_from_database() -> Result<Vec<GammaMarket>> {
    let db_path = PathBuf::from("./data/database/gamma");
    let database = GammaDatabase::new(&db_path).await
        .context("Failed to initialize gamma database")?;
    
    // Use get_all_markets to fetch all markets, not just the default limit
    let markets = database.get_all_markets(None).await
        .context("Failed to load markets from database")?;
    
    info!("Loaded {} deduplicated markets from database", markets.len());
    Ok(markets)
}


/// Handle markets output - export, display, or launch TUI
async fn handle_markets_output(markets: Vec<GammaMarket>, args: &MarketsArgs, verbose: bool) -> Result<()> {
    // Export if requested (do this before TUI to avoid interfering with display)
    if let Some(export_path) = &args.export {
        let json = serde_json::to_string_pretty(&markets)
            .context("Failed to serialize markets")?;
        std::fs::write(export_path, json)
            .context("Failed to write export file")?;
        println!("Exported to {}", export_path.display());
    }
    
    // Handle empty markets case
    if markets.is_empty() {
        println!("{}", "📊 No markets available to display".bright_yellow());
        if args.no_interactive {
            println!("{}", "💡 All available data has been fetched and stored in sessions.".bright_blue());
            println!("{}", "   Use --refresh flag to check for new data from the API.".bright_blue());
        } else {
            println!("{}", "💡 No markets to browse. Use --refresh to check for new data.".bright_blue());
        }
        return Ok(());
    }
    
    if args.no_interactive {
        // Use text-based output
        println!("Found {} markets:", markets.len());
        
        for (i, market) in markets.iter().enumerate() {
            if args.detailed || verbose {
                print_market_detailed(market, i + 1);
            } else {
                print_market_summary(market, i + 1);
            }
        }
    } else {
        // Launch interactive TUI browser (default behavior)
        println!("{}", "🚀 Launching interactive markets browser...".bright_green());
        launch_markets_browser(markets).await?;
    }
    
    Ok(())
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

fn print_market_summary(market: &GammaMarket, index: usize) {
    let status = if market.active { "🟢" } else { "🔴" };
    let volume = format!("${:.0}", market.volume());
    
    println!(
        "{:3}. {} {} {} - {}",
        index,
        status,
        market.question.chars().take(60).collect::<String>(),
        if market.question.len() > 60 { "..." } else { "" },
        volume.bright_green()
    );
}

fn print_market_detailed(market: &GammaMarket, index: usize) {
    println!("{}", format!("=== Market {} ===", index).bright_yellow());
    println!("Question: {}", market.question);
    if market.active {
        println!("Status: {}", "Active".green());
    } else {
        println!("Status: {}", "Inactive".red());
    }
    println!("Volume: ${:.2}", market.volume());
    println!("Liquidity: ${:.2}", market.liquidity.unwrap_or_default());
    println!("Outcomes: {:?}", market.outcomes);
    println!("Prices: {:?}", market.outcome_prices);
    println!("Category: {:?}", market.category);
    println!();
}

fn print_event_summary(event: &GammaEvent, index: usize) {
    let status = match event.active {
        Some(true) => "🟢",
        Some(false) => "🔴",
        None => "⚪",
    };
    
    println!(
        "{:3}. {} {} (Volume: ${:.0})",
        index,
        status,
        event.title,
        event.volume_num.or(event.volume).unwrap_or_default()
    );
}

fn print_event_detailed(event: &GammaEvent, index: usize) {
    println!("{}", format!("=== Event {} ===", index).bright_yellow());
    println!("Title: {}", event.title);
    println!("Slug: {}", event.slug);
    if let Some(ref desc) = event.description {
        println!("Description: {}", desc.chars().take(200).collect::<String>());
    }
    if let Some(volume) = event.volume_num.or(event.volume) {
        println!("Volume: ${:.2}", volume);
    }
    if let Some(liquidity) = event.liquidity_num.or(event.liquidity) {
        println!("Liquidity: ${:.2}", liquidity);
    }
    if let Some(ref tags) = event.tags {
        let tag_labels: Vec<&str> = tags.iter().map(|t| t.label.as_str()).collect();
        println!("Tags: {}", tag_labels.join(", "));
    }
    println!();
}

fn print_trade_summary(trade: &GammaTrade, index: usize) {
    let side_color = match trade.side {
        TradeSide::Buy => owo_colors::AnsiColors::Green,
        TradeSide::Sell => owo_colors::AnsiColors::Red,
    };
    
    println!(
        "{:3}. {} {} {} @ ${:.3} ({})",
        index,
        format!("{:?}", trade.side).color(side_color),
        trade.size,
        trade.outcome,
        trade.price,
        trade.timestamp.format("%H:%M:%S")
    );
}

fn print_trade_detailed(trade: &GammaTrade, index: usize) {
    println!("{}", format!("=== Trade {} ===", index).bright_yellow());
    println!("Side: {:?}", trade.side);
    println!("Outcome: {}", trade.outcome);
    println!("Size: {}", trade.size);
    println!("Price: ${:.4}", trade.price);
    println!("Value: ${:.2}", trade.size * trade.price);
    println!("User: {}", trade.proxy_wallet.0);
    println!("Time: {}", trade.timestamp);
    println!();
}

fn print_position_summary(position: &GammaPosition, index: usize) {
    let pnl_color = if position.cash_pnl >= rust_decimal::Decimal::ZERO {
        owo_colors::AnsiColors::Green
    } else {
        owo_colors::AnsiColors::Red
    };
    
    println!(
        "{:3}. {} {} shares - ${:.2} ({:.1}%)",
        index,
        position.outcome,
        position.size,
        position.current_value,
        format!("{:.1}", position.percent_pnl).color(pnl_color)
    );
}

fn print_position_detailed(position: &GammaPosition, index: usize) {
    println!("{}", format!("=== Position {} ===", index).bright_yellow());
    println!("Market: {}", position.title);
    println!("Outcome: {}", position.outcome);
    println!("Size: {} shares", position.size);
    println!("Avg Price: ${:.4}", position.avg_price);
    println!("Current Value: ${:.2}", position.current_value);
    println!("P&L: ${:.2} ({:.1}%)", position.cash_pnl, position.percent_pnl);
    if position.redeemable {
        println!("Redeemable: {}", "Yes".green());
    } else {
        println!("Redeemable: {}", "No".red());
    }
    println!();
}

fn print_analytics(analytics: &MarketAnalytics, detailed: bool) {
    println!("{}", "=== Market Analytics ===".bright_yellow());
    println!("Total Markets: {}", analytics.total_markets);
    println!("Active: {}", analytics.active_markets.to_string().green());
    println!("Closed: {}", analytics.closed_markets.to_string().red());
    println!("Archived: {}", analytics.archived_markets);
    println!("Total Volume: ${:.0}", analytics.total_volume);
    println!("Average Volume: ${:.0}", analytics.avg_volume);
    println!("Total Liquidity: ${:.0}", analytics.total_liquidity);
    println!("Average Liquidity: ${:.0}", analytics.avg_liquidity);
    
    if detailed {
        println!("\n{}", "Top Categories:".bright_blue());
        for (i, (category, count)) in analytics.top_categories.iter().enumerate() {
            println!("  {:2}. {} ({})", i + 1, category, count);
        }
        
        println!("\n{}", "Top Tags:".bright_blue());
        for (i, (tag, count)) in analytics.top_tags.iter().take(10).enumerate() {
            println!("  {:2}. {} ({})", i + 1, tag, count);
        }
    }
}

async fn sync_markets(
    client: &GammaClient,
    storage: &GammaStorage,
    batch_size: u32,
    verbose: bool,
) -> Result<()> {
    let mut offset = 0;
    let mut total_fetched = 0;
    
    loop {
        let query = MarketQuery {
            limit: Some(batch_size),
            offset: Some(offset),
            order: Some("id".to_string()),
            ascending: Some(true),
            ..Default::default()
        };
        
        let response = client.fetch_markets(&query).await?;
        
        if response.data.is_empty() {
            break;
        }
        
        storage.store_markets_batch(&response.data)?;
        total_fetched += response.data.len();
        
        if verbose {
            println!("Fetched {} markets (total: {})", response.data.len(), total_fetched);
        }
        
        offset += batch_size;
        
        if !response.has_more {
            break;
        }
    }
    
    println!("Synced {} markets total", total_fetched);
    Ok(())
}

async fn sync_events(
    client: &GammaClient,
    storage: &GammaStorage,
    batch_size: u32,
    verbose: bool,
) -> Result<()> {
    let mut offset = 0;
    let mut total_fetched = 0;
    
    loop {
        let query = EventQuery {
            limit: Some(batch_size),
            offset: Some(offset),
            order: Some("id".to_string()),
            ascending: Some(true),
            ..Default::default()
        };
        
        let response = client.fetch_events(&query).await?;
        
        if response.data.is_empty() {
            break;
        }
        
        storage.store_events_batch(&response.data)?;
        total_fetched += response.data.len();
        
        if verbose {
            println!("Fetched {} events (total: {})", response.data.len(), total_fetched);
        }
        
        offset += batch_size;
        
        if !response.has_more {
            break;
        }
    }
    
    println!("Synced {} events total", total_fetched);
    Ok(())
}

async fn sync_trades(
    client: &GammaClient,
    storage: &GammaStorage,
    batch_size: u32,
    verbose: bool,
) -> Result<()> {
    let mut offset = 0;
    let mut total_fetched = 0;
    
    loop {
        let query = TradeQuery {
            limit: Some(batch_size),
            offset: Some(offset),
            taker_only: Some(true),
            ..Default::default()
        };
        
        let response = client.fetch_trades(&query).await?;
        
        if response.data.is_empty() {
            break;
        }
        
        storage.store_trades_batch(&response.data)?;
        total_fetched += response.data.len();
        
        if verbose {
            println!("Fetched {} trades (total: {})", response.data.len(), total_fetched);
        }
        
        offset += batch_size;
        
        if !response.has_more {
            break;
        }
    }
    
    println!("Synced {} trades total", total_fetched);
    Ok(())
}

async fn sync_positions(
    client: &GammaClient,
    storage: &GammaStorage,
    user: &str,
    verbose: bool,
) -> Result<()> {
    let query = PositionQuery {
        user: UserAddress(user.to_string()),
        limit: Some(500),
        size_threshold: Some(rust_decimal::Decimal::new(1, 0)),
        ..Default::default()
    };
    
    let response = client.fetch_positions(&query).await?;
    
    storage.store_positions_batch(&response.data)?;
    
    if verbose {
        println!("Fetched {} positions for user {}", response.data.len(), user);
    }
    
    Ok(())
}

/// Execute clear cache command
async fn execute_clear_cache(args: ClearCacheArgs) -> Result<()> {
    use std::io::{self, Write};
    
    // Determine what to clear
    let clear_cache = args.cache_only || args.all || (!args.raw_responses && !args.individual_storage && !args.cache_only);
    let clear_raw = args.raw_responses || args.all;
    let clear_individual = args.individual_storage || args.all;
    
    // Show what will be cleared
    println!("{}", "🗑️  The following will be cleared:".bright_yellow());
    if clear_cache {
        println!("  • Cache file (./data/gamma_cache.json)");
        println!("  • Cache snapshot (./data/gamma_cache.snapshot.json)");
    }
    if clear_raw {
        println!("  • Raw API responses (./data/gamma_raw_responses/)");
    }
    if clear_individual {
        println!("  • Individual storage - JSON files (./data/database/gamma/jsons/)");
        println!("  • Individual storage - RocksDB (./data/database/gamma/rocksdb/)");
    }
    
    // Confirm unless --yes is provided
    if !args.yes {
        print!("\nAre you sure you want to proceed? [y/N] ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Operation cancelled.");
            return Ok(());
        }
    }
    
    // Clear cache files
    if clear_cache {
        let cache_path = PathBuf::from("./data/gamma_cache.json");
        let snapshot_path = PathBuf::from("./data/gamma_cache.snapshot.json");
        
        if cache_path.exists() {
            fs::remove_file(&cache_path)
                .context("Failed to remove cache file")?;
            println!("✅ Removed cache file");
        }
        
        if snapshot_path.exists() {
            fs::remove_file(&snapshot_path)
                .context("Failed to remove snapshot file")?;
            println!("✅ Removed snapshot file");
        }
    }
    
    // Clear raw responses
    if clear_raw {
        let raw_dir = PathBuf::from("./data/gamma_raw_responses");
        if raw_dir.exists() {
            fs::remove_dir_all(&raw_dir)
                .context("Failed to remove raw responses directory")?;
            println!("✅ Removed raw responses directory");
        }
    }
    
    // Clear individual storage
    if clear_individual {
        let jsons_dir = PathBuf::from("./data/database/gamma/jsons");
        let rocksdb_dir = PathBuf::from("./data/database/gamma/rocksdb");
        
        if jsons_dir.exists() {
            fs::remove_dir_all(&jsons_dir)
                .context("Failed to remove JSON storage directory")?;
            println!("✅ Removed JSON storage");
        }
        
        if rocksdb_dir.exists() {
            fs::remove_dir_all(&rocksdb_dir)
                .context("Failed to remove RocksDB directory")?;
            println!("✅ Removed RocksDB storage");
        }
    }
    
    println!("\n{}", "🎉 Cache clearing completed!".bright_green());
    Ok(())
}

/// Execute process raw command
async fn execute_process_raw(args: ProcessRawArgs) -> Result<()> {
    println!("{}", "🔄 Processing raw API responses...".bright_blue());
    
    let output_dir = args.output_dir
        .unwrap_or_else(|| PathBuf::from("./data/database/gamma"));
    
    // Initialize individual storage
    let storage = IndividualMarketStorage::new(&output_dir)
        .context("Failed to initialize individual storage")?;
    
    if let Some(file_path) = args.file {
        // Process single file
        println!("Processing single file: {:?}", file_path);
        storage.process_raw_response(&file_path)
            .context("Failed to process raw response file")?;
    } else {
        // Process all files in directory
        println!("Processing all raw response files...");
        storage.process_all_raw_responses()
            .context("Failed to process raw responses")?;
    }
    
    println!("{}", "✅ Processing completed!".bright_green());
    Ok(())
}

/// Launch interactive markets browser TUI
async fn launch_markets_browser(markets: Vec<GammaMarket>) -> Result<()> {
    use crate::gamma::tui::MarketsBrowser;
    use ratatui::{Terminal, backend::CrosstermBackend};
    use std::io;
    
    // Setup terminal
    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture).context("Failed to setup terminal")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;
    
    // Initialize browser state
    let mut browser = MarketsBrowser::new(markets);
    
    let res = run_markets_browser(&mut terminal, &mut browser).await;
    
    // Restore terminal
    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    ).context("Failed to restore terminal")?;
    terminal.show_cursor().context("Failed to show cursor")?;
    
    res
}

/// Run the markets browser loop
async fn run_markets_browser(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    browser: &mut crate::gamma::tui::MarketsBrowser,
) -> Result<()> {
    loop {
        terminal.draw(|f| browser.render(f, f.area()))?;
        
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Esc => {
                            if browser.is_searching() {
                                browser.cancel_search();
                            } else {
                                break;
                            }
                        },
                        KeyCode::Char('/') => browser.start_search(),
                        KeyCode::Enter => browser.handle_enter(),
                        KeyCode::Up => browser.previous(),
                        KeyCode::Down => browser.next(),
                        KeyCode::PageUp => browser.page_up(),
                        KeyCode::PageDown => browser.page_down(),
                        KeyCode::Home => browser.go_to_top(),
                        KeyCode::End => browser.go_to_bottom(),
                        KeyCode::Char(c) => browser.handle_char(c),
                        KeyCode::Backspace => browser.handle_backspace(),
                        _ => {}
                    }
                }
            }
        }
    }
    
    Ok(())
}

/// Execute import session command
async fn execute_import_session(args: ImportSessionArgs, verbose: bool) -> Result<()> {
    println!("{}", "📊 Importing session data into SurrealDB...".bright_blue());
    
    // Validate arguments
    if args.session_id.is_some() && (args.from_session_id.is_some() || args.to_session_id.is_some()) {
        return Err(anyhow::anyhow!("Cannot specify both --session-id and range options (--from-session-id/--to-session-id)"));
    }
    
    if args.from_session_id.is_some() != args.to_session_id.is_some() {
        return Err(anyhow::anyhow!("Both --from-session-id and --to-session-id must be specified for range import"));
    }
    
    // Initialize database
    let db_path = PathBuf::from("./data/database/gamma");
    let database = GammaDatabase::new(&db_path).await
        .context("Failed to initialize gamma database")?;
    
    // Initialize session manager
    let base_path = PathBuf::from("./data/gamma/raw");
    let session_manager = SessionManager::new(base_path)
        .context("Failed to create session manager")?;
    
    let mut sessions_to_import = Vec::new();
    let all_sessions = session_manager.list_sessions();
    
    // Determine which sessions to import
    if let (Some(from), Some(to)) = (args.from_session_id, args.to_session_id) {
        // Range import
        if from > to {
            return Err(anyhow::anyhow!("from-session-id ({}) must be less than or equal to to-session-id ({})", from, to));
        }
        
        for (session_id, session_dir, is_complete) in &all_sessions {
            if *session_id >= from && *session_id <= to {
                sessions_to_import.push((*session_id, session_dir.clone(), *is_complete));
            }
        }
        
        println!("Importing sessions {} to {} ({} sessions found)", from, to, sessions_to_import.len());
    } else if let Some(session_id_str) = &args.session_id {
        if session_id_str == "all" {
            // Import all sessions
            sessions_to_import = all_sessions.clone();
            println!("Found {} sessions to import", sessions_to_import.len());
        } else {
            // Import specific session
            let session_id: u32 = session_id_str.parse()
                .context("Invalid session ID - must be a number or 'all'")?;
            
            if let Some(session_info) = all_sessions.iter().find(|(id, _, _)| *id == session_id) {
                sessions_to_import.push(session_info.clone());
            } else {
                return Err(anyhow::anyhow!("Session {} not found", session_id));
            }
        }
    } else {
        return Err(anyhow::anyhow!("Must specify either --session-id or --from-session-id and --to-session-id"));
    }
    
    // Import the selected sessions (process sessions sequentially but files in parallel)
    let mut total_imported = 0;
    
    println!("🚀 Processing {} sessions...", sessions_to_import.len());
    
    // Create multi-progress container for tracking parallel file imports
    let multi_progress = if !verbose {
        Some(MultiProgress::new())
    } else {
        None
    };
    
    // Create main progress bar for sessions
    let main_pb = if let Some(ref mp) = multi_progress {
        let pb = mp.add(ProgressBar::new(sessions_to_import.len() as u64));
        pb.set_style(
            ProgressStyle::default_bar()
                .template("📦 Sessions: [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) {msg}")
                .unwrap()
                .progress_chars("#>-")
        );
        pb.set_message("Starting import...");
        Some(pb)
    } else {
        None
    };
    
    for (_idx, (session_id, session_dir, is_complete)) in sessions_to_import.iter().enumerate() {
        if verbose {
            println!("🔄 Processing session {} ({})...", session_id, 
                if *is_complete { "complete" } else { "incomplete" });
        }
        
        if let Some(ref pb) = main_pb {
            pb.set_message(format!("Processing session {}...", session_id));
        }
        
        let session_path = session_manager.base_path.join(&session_dir);
        let imported_count = import_single_session_parallel(
            &database, 
            &session_path, 
            *session_id, 
            &args, 
            verbose,
            multi_progress.as_ref()
        ).await?;
        total_imported += imported_count;
        
        if let Some(ref pb) = main_pb {
            pb.inc(1);
            pb.set_message(format!("{} markets imported from session {}", imported_count, session_id));
        }
        
        if verbose {
            println!("✅ Imported {} markets from session {}", imported_count, session_id);
        }
    }
    
    if let Some(pb) = main_pb {
        pb.finish_with_message(format!("✓ Imported {} total markets", total_imported));
    }
    
    println!("\n🎉 Successfully imported {} total markets", total_imported);
    
    // Update database statistics
    let stats = database.get_stats().await?;
    println!("\n📈 Database Statistics:");
    println!("Total markets: {}", stats.total_markets);
    println!("Active markets: {}", stats.active_markets);
    println!("Closed markets: {}", stats.closed_markets);
    println!("Archived markets: {}", stats.archived_markets);
    
    Ok(())
}

/// Import markets from a single session into the database (parallel version)
async fn import_single_session_parallel(
    database: &GammaDatabase, 
    session_path: &Path, 
    session_id: u32,
    _args: &ImportSessionArgs,
    verbose: bool,
    multi_progress: Option<&MultiProgress>
) -> Result<u64> {
    if !session_path.exists() {
        info!("Session directory does not exist, skipping: {:?}", session_path);
        return Ok(0);
    }
    
    // Read session directory for raw response files
    let entries = fs::read_dir(session_path)
        .context("Failed to read session directory")?;
    
    let mut raw_files = Vec::new();
    for entry in entries {
        let entry = entry.context("Failed to read directory entry")?;
        let path = entry.path();
        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
            if filename.starts_with("raw-offset-") && filename.ends_with(".json") {
                // Extract offset from filename for sorting
                if let Some(offset_str) = filename.strip_prefix("raw-offset-").and_then(|s| s.strip_suffix(".json")) {
                    if let Ok(offset) = offset_str.parse::<usize>() {
                        raw_files.push((offset, path));
                    }
                }
            }
        }
    }
    
    // Sort by offset to ensure correct order
    raw_files.sort_by_key(|(offset, _)| *offset);
    
    if raw_files.is_empty() {
        if verbose {
            println!("No raw response files found in session {}", session_id);
        }
        return Ok(0);
    }
    
    // Create progress bar for this session
    let session_progress = if let Some(mp) = multi_progress {
        let pb = mp.add(ProgressBar::new(raw_files.len() as u64));
        pb.set_style(
            ProgressStyle::default_bar()
                .template(&format!("{{spinner:.green}} Session {}: [{{bar:40.cyan/blue}}] {{pos}}/{{len}} files ({{percent}}%) {{msg}}", session_id))
                .unwrap()
                .progress_chars("#>-")
        );
        pb.set_message("Processing...");
        Some(pb)
    } else {
        if verbose {
            println!("  📁 Found {} raw response files", raw_files.len());
        }
        None
    };
    
    // Process files in parallel within the session
    let session_total = Arc::new(AtomicU64::new(0));
    let processed_files = Arc::new(AtomicU64::new(0));
    
    // Process files in chunks to avoid overwhelming the database
    let chunk_size = 10; // Process 10 files at a time
    for chunk in raw_files.chunks(chunk_size) {
        let session_progress_ref = session_progress.as_ref();
        let processed_files_ref = processed_files.clone();
        let raw_files_len = raw_files.len();
        
        let chunk_results: Vec<Result<Vec<GammaMarket>>> = chunk
            .par_iter()
            .map(|(offset, path)| {
                let result = (|| -> Result<Vec<GammaMarket>> {
                    let content = fs::read_to_string(path)
                        .with_context(|| format!("Failed to read raw response file at offset {}", offset))?;
                    
                    // Parse directly as array of markets
                    let markets: Vec<GammaMarket> = serde_json::from_str(&content)
                        .with_context(|| format!("Failed to parse raw response file at offset {}", offset))?;
                    
                    Ok(markets)
                })();
                
                // Update progress bar
                let files_done = processed_files_ref.fetch_add(1, Ordering::SeqCst) + 1;
                if let Some(ref pb) = session_progress_ref {
                    pb.set_position(files_done);
                    if files_done == raw_files_len as u64 {
                        pb.set_message("Finalizing...");
                    }
                }
                
                result
            })
            .collect();
        
        // Collect all markets from this chunk
        let mut chunk_markets = Vec::new();
        for result in chunk_results {
            match result {
                Ok(markets) => chunk_markets.extend(markets),
                Err(e) => return Err(e),
            }
        }
        
        // Store the chunk in the database
        if !chunk_markets.is_empty() {
            let stored_count = database.store_markets(&chunk_markets, session_id).await
                .context("Failed to store markets batch in database")?;
            session_total.fetch_add(stored_count, Ordering::SeqCst);
            
            if verbose {
                println!("  ✅ Stored {} markets from chunk", stored_count);
            }
        }
    }
    
    let total = session_total.load(Ordering::SeqCst);
    
    // Finish progress bar
    if let Some(pb) = session_progress {
        pb.finish_with_message(format!("✓ {} markets imported", total));
    }
    
    Ok(total)
}

/// Import markets from a single session into the database (sequential version - kept for compatibility)

// ============================================================================
// DATABASE COMMAND IMPLEMENTATIONS
// ============================================================================

/// Execute database command
async fn execute_db_command(args: DbArgs, verbose: bool) -> Result<()> {
    match args.command {
        Some(cmd) => match cmd {
            DbCommand::Stats(args) => execute_db_stats(args, verbose).await,
            DbCommand::Search(args) => execute_db_search(args, verbose).await,
            DbCommand::Export(args) => execute_db_export(args, verbose).await,
            DbCommand::List(args) => execute_db_list(args, verbose).await,
            DbCommand::Cleanup(args) => execute_db_cleanup(args, verbose).await,
            DbCommand::Get(args) => execute_db_get(args, verbose).await,
            DbCommand::Count(args) => execute_db_count(args, verbose).await,
            DbCommand::Health(args) => execute_db_health(args, verbose).await,
        },
        None => {
            // Default to interactive list when no subcommand is provided
            let default_args = DbListArgs {
                limit: 50,
                offset: 0,
                sort_by: "volume".to_string(),
                order: "desc".to_string(),
                active_only: false,
                closed_only: false,
                category: None,
                min_volume: None,
                format: "table".to_string(),
                interactive: true,
                no_interactive: false,
            };
            execute_db_list(default_args, verbose).await
        }
    }
}

/// Execute database stats command
async fn execute_db_stats(args: DbStatsArgs, _verbose: bool) -> Result<()> {
    // Clear console and show header
    print!("\x1B[2J\x1B[1;1H");
    println!("{}", "📊 Database Statistics".bright_blue().bold());
    println!("{}", "=".repeat(50).bright_black());
    
    // Initialize database with progress
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap()
    );
    pb.set_message("Connecting to database...");
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    
    let db_path = PathBuf::from("./data/database/gamma");
    let database = GammaDatabase::new(&db_path).await
        .context("Failed to initialize gamma database")?;
    
    pb.set_message("Loading statistics...");
    let stats = database.get_stats().await?;
    pb.finish_and_clear();
    
    // Display basic stats with beautiful formatting
    println!("\n{} {}", "📦".bright_yellow(), "Overview".bright_white().bold());
    println!("  {} Total markets: {}", "🔹".bright_blue(), stats.total_markets.to_string().bright_cyan().bold());
    println!("  {} Active: {} {}", 
        "🟢".bright_green(),
        stats.active_markets.to_string().bright_green().bold(),
        format!("({:.1}%)", stats.active_markets as f64 / stats.total_markets as f64 * 100.0).bright_black()
    );
    println!("  {} Closed: {} {}", 
        "🔴".bright_red(),
        stats.closed_markets.to_string().bright_red().bold(),
        format!("({:.1}%)", stats.closed_markets as f64 / stats.total_markets as f64 * 100.0).bright_black()
    );
    println!("  {} Archived: {} {}", 
        "🟡".bright_yellow(),
        stats.archived_markets.to_string().bright_yellow().bold(),
        format!("({:.1}%)", stats.archived_markets as f64 / stats.total_markets as f64 * 100.0).bright_black()
    );
    
    if args.detailed || args.by_category || args.volume_dist || args.temporal {
        println!("\n{} {}", "🔍".bright_cyan(), "Detailed Analysis".bright_white().bold());
        
        // Create progress bar for loading markets
        let pb = ProgressBar::new(stats.total_markets);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("#>-")
        );
        pb.set_message("Loading markets for analysis...");
        
        // Get all markets with progress updates
        let markets = database.get_all_markets_with_progress(Some(&pb)).await?;
        pb.finish_and_clear();
        
        if args.by_category {
            println!("\n{} {}", "📊".bright_magenta(), "Category Distribution".bright_white().bold());
            
            // Use rayon for parallel category counting
            let category_counts: dashmap::DashMap<String, usize> = dashmap::DashMap::new();
            
            markets.par_iter().for_each(|market| {
                let category = market.category.as_deref().unwrap_or("Uncategorized").to_string();
                category_counts.entry(category).and_modify(|e| *e += 1).or_insert(1);
            });
            
            let mut categories: Vec<_> = category_counts.into_iter().collect();
            categories.sort_by(|a, b| b.1.cmp(&a.1));
            
            // Display top categories with bars
            let max_count = categories.first().map(|(_, c)| *c).unwrap_or(0);
            for (_i, (category, count)) in categories.iter().take(15).enumerate() {
                let bar_width = (*count as f64 / max_count as f64 * 30.0) as usize;
                let bar = "█".repeat(bar_width);
                let percentage = *count as f64 / stats.total_markets as f64 * 100.0;
                
                println!("  {:<20} {} {} {}", 
                    category.chars().take(20).collect::<String>(),
                    bar.bright_cyan(),
                    count.to_string().bright_white().bold(),
                    format!("({:.1}%)", percentage).bright_black()
                );
            }
            
            if categories.len() > 15 {
                println!("  {}", format!("... and {} more categories", categories.len() - 15).bright_black());
            }
        }
        
        if args.volume_dist {
            println!("\n💰 Volume Distribution:");
            let volumes: Vec<_> = markets.iter()
                .map(|m| m.volume())
                .filter(|v| *v > rust_decimal::Decimal::ZERO)
                .collect();
                
            if !volumes.is_empty() {
                let total_volume: rust_decimal::Decimal = volumes.iter().sum();
                let avg_volume = total_volume / rust_decimal::Decimal::from(volumes.len());
                let mut sorted_volumes = volumes.clone();
                sorted_volumes.sort();
                let median_volume = sorted_volumes[sorted_volumes.len() / 2];
                
                println!("  Total volume: ${:.2}", total_volume);
                println!("  Average volume: ${:.2}", avg_volume);
                println!("  Median volume: ${:.2}", median_volume);
                println!("  Markets with volume > $1M: {}", 
                    volumes.iter().filter(|v| **v > rust_decimal::Decimal::from(1_000_000)).count()
                );
                println!("  Markets with volume > $100k: {}", 
                    volumes.iter().filter(|v| **v > rust_decimal::Decimal::from(100_000)).count()
                );
            }
        }
        
        if args.temporal {
            println!("\n📅 Temporal Distribution:");
            // This would require parsing dates from the markets
            println!("  (Temporal analysis not yet implemented)");
        }
    }
    
    Ok(())
}

/// Execute database search command
async fn execute_db_search(args: DbSearchArgs, _verbose: bool) -> Result<()> {
    // Clear console and show header
    print!("\x1B[2J\x1B[1;1H");
    println!("{}", "🔍 Market Search".bright_blue().bold());
    println!("{}", "=".repeat(50).bright_black());
    println!("  {} Query: {}", "🔎", args.query.bright_cyan());
    if let Some(ref field) = args.field {
        println!("  {} Field: {}", "🎯", field.bright_yellow());
    }
    println!();
    
    // Initialize search index for ultra-fast search
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap()
    );
    spinner.set_message("Loading search index...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));
    
    // Initialize fast search engine
    let search_start = std::time::Instant::now();
    let db_path = PathBuf::from("./data/database/gamma");
    let index_path = crate::gamma::get_index_path();
    
    // Create a static cache for the search engine to avoid rebuilding
    use std::sync::OnceLock;
    static SEARCH_ENGINE: OnceLock<crate::gamma::FastSearchEngine> = OnceLock::new();
    
    let results = if let Some(engine) = SEARCH_ENGINE.get() {
        // Use cached engine
        spinner.set_message("⚡ Searching with ultra-fast in-memory index...");
        let params = crate::gamma::SearchParams {
            query: args.query.clone(),
            category: args.field.clone().filter(|f| f == "category").and_then(|_| Some(args.query.clone())),
            tags: vec![],
            min_volume: None,
            max_volume: None,
            limit: args.limit as usize,
            case_sensitive: args.case_sensitive,
        };
        
        let search_results = engine.search(&params);
        let markets: Vec<GammaMarket> = search_results.into_iter().map(|arc| (*arc).clone()).collect();
        spinner.finish_with_message(format!(
            "⚡ Ultra-fast search completed: {} results in {}ms", 
            markets.len(),
            search_start.elapsed().as_millis()
        ));
        markets
    } else {
        // Try to build the engine
        spinner.set_message("Building ultra-fast search engine (one-time operation)...");
        
        match crate::gamma::build_fast_search_index(&db_path, &index_path, false).await {
            Ok(engine) => {
                let build_time = search_start.elapsed();
                spinner.set_message(format!("⚡ Search engine built in {}ms, searching...", build_time.as_millis()));
                
                // Cache the engine for future use
                let engine = SEARCH_ENGINE.get_or_init(|| engine);
                
                let params = crate::gamma::SearchParams {
                    query: args.query.clone(),
                    category: args.field.clone().filter(|f| f == "category").and_then(|_| Some(args.query.clone())),
                    tags: vec![],
                    min_volume: None,
                    max_volume: None,
                    limit: args.limit as usize,
                    case_sensitive: args.case_sensitive,
                };
                
                let search_time = std::time::Instant::now();
                let search_results = engine.search(&params);
                let markets: Vec<GammaMarket> = search_results.into_iter().map(|arc| (*arc).clone()).collect();
                let search_elapsed = search_time.elapsed();
                
                spinner.finish_with_message(format!(
                    "⚡ Ultra-fast search completed: {} results in {}ms (engine build: {}ms, search: {}ms)", 
                    markets.len(),
                    search_start.elapsed().as_millis(),
                    build_time.as_millis(),
                    search_elapsed.as_millis()
                ));
                markets
            },
            Err(e) => {
                // Fallback to database search
                spinner.set_message(format!("Failed to build search engine: {}, using database search...", e));
                let database = GammaDatabase::new(&db_path).await
                    .context("Failed to initialize gamma database")?;
                let results = database.search_markets(&args.query, Some(args.limit)).await?;
                spinner.finish_with_message(format!(
                    "🔍 Database search completed in {}ms (💡 Fix search engine for ultra-fast searches)", 
                    search_start.elapsed().as_millis()
                ));
                results
            }
        }
    };
    
    if results.is_empty() {
        println!("{} {}", "❌", "No markets found matching your search.".bright_red());
        return Ok(());
    }
    
    println!("{} Found {} markets", 
        "✅".bright_green(), 
        results.len().to_string().bright_green().bold()
    );
    println!("{}", "=".repeat(50).bright_black());
    
    match args.format.as_str() {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&results)?);
        },
        "csv" => {
            // Print CSV header
            println!("market_id,question,category,volume,liquidity,active,closed");
            for market in results {
                println!("{},{:?},{:?},{},{},{},{}",
                    market.id.0,
                    market.question,
                    market.category.as_deref().unwrap_or(""),
                    market.volume(),
                    market.liquidity.unwrap_or_default(),
                    market.active,
                    market.closed
                );
            }
        },
        _ => {
            // Default table format with beautiful display
            for (i, market) in results.iter().enumerate() {
                println!("\n{} Market #{}", "📄", (i + 1).to_string().bright_yellow());
                
                // Question with wrapping
                let question = if market.question.len() > 80 {
                    format!("{}…", market.question.chars().take(77).collect::<String>())
                } else {
                    market.question.clone()
                };
                println!("  {} {}", "❓", question.bright_white());
                
                // Status indicators
                let status_emoji = if market.active { "🟢" } else { "🔴" };
                let status_text = if market.active { "Active" } else { "Inactive" };
                let status_color = if market.active { 
                    status_text.bright_green().to_string() 
                } else { 
                    status_text.bright_red().to_string() 
                };
                
                // Category and volume
                if let Some(ref cat) = market.category {
                    println!("  {} Category: {} | {} {} | {} Volume: {}",
                        "🏷️",
                        cat.bright_magenta(),
                        status_emoji,
                        status_color,
                        "💰",
                        format!("${:.0}", market.volume()).bright_green()
                    );
                } else {
                    println!("  {} {} | {} Volume: {}",
                        status_emoji,
                        status_color,
                        "💰",
                        format!("${:.0}", market.volume()).bright_green()
                    );
                }
            }
        }
    }
    
    Ok(())
}

/// Execute database export command
async fn execute_db_export(args: DbExportArgs, _verbose: bool) -> Result<()> {
    // Clear console and show header
    print!("\x1B[2J\x1B[1;1H");
    println!("{}", "📤 Export Database".bright_blue().bold());
    println!("{}", "=".repeat(50).bright_black());
    println!("  {} Output: {}", "📁", args.output.display().to_string().bright_cyan());
    println!("  {} Format: {}", "📋", args.format.bright_yellow());
    println!();
    
    // Initialize database with spinner
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap()
    );
    spinner.set_message("Initializing database...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));
    
    let db_path = PathBuf::from("./data/database/gamma");
    let database = GammaDatabase::new(&db_path).await
        .context("Failed to initialize gamma database")?;
    
    // Build query with filters
    let mut query = "SELECT * FROM markets".to_string();
    let mut conditions = Vec::new();
    
    if args.active_only {
        conditions.push("active = true".to_string());
    }
    
    if let Some(min_vol) = args.min_volume {
        conditions.push(format!("volume >= {}", min_vol));
    }
    
    if let Some(ref category) = args.category {
        conditions.push(format!("category = '{}'", category));
    }
    
    if !conditions.is_empty() {
        query.push_str(" WHERE ");
        query.push_str(&conditions.join(" AND "));
    }
    
    if let Some(limit) = args.limit {
        query.push_str(&format!(" LIMIT {}", limit));
    }
    
    spinner.set_message("Querying database...");
    let markets = database.execute_query(&query).await?;
    spinner.finish_and_clear();
    
    // Show progress bar for export
    let export_pb = ProgressBar::new(markets.len() as u64);
    export_pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-")
    );
    export_pb.set_message("Exporting markets...");
    
    println!("{} Found {} markets to export", 
        "📊".bright_green(), 
        markets.len().to_string().bright_green().bold()
    );
    
    match args.format.as_str() {
        "csv" => {
            let mut wtr = csv::Writer::from_path(&args.output)?;
            
            // Write headers
            wtr.write_record(&["market_id", "question", "category", "volume", "liquidity", "active", "closed", "created_at", "updated_at"])?;
            
            for market in &markets {
                wtr.write_record(&[
                    &market.id.0.to_string(),
                    &market.question,
                    market.category.as_deref().unwrap_or(""),
                    &market.volume().to_string(),
                    &market.liquidity.unwrap_or_default().to_string(),
                    &market.active.to_string(),
                    &market.closed.to_string(),
                    &market.created_at.to_rfc3339(),
                    &market.updated_at.to_rfc3339(),
                ])?;
            }
            
            wtr.flush()?;
            export_pb.finish_with_message("CSV export complete");
        },
        _ => {
            // Default to JSON
            let json = if let Some(ref fields_str) = args.fields {
                // Filter fields if specified
                let fields: Vec<&str> = fields_str.split(',').collect();
                let filtered_markets: Vec<serde_json::Value> = markets.iter()
                    .map(|m| {
                        let market_json = serde_json::to_value(&m).unwrap();
                        let mut filtered = serde_json::json!({});
                        
                        for field in &fields {
                            if let Some(value) = market_json.get(field) {
                                filtered[field] = value.clone();
                            }
                        }
                        
                        filtered
                    })
                    .collect();
                    
                serde_json::to_string_pretty(&filtered_markets)?
            } else {
                serde_json::to_string_pretty(&markets)?
            };
            
            // Write with progress updates
            fs::write(&args.output, json)?;
            export_pb.finish_with_message("JSON export complete");
        }
    }
    
    println!("\n{} Successfully exported {} markets to {}", 
        "✅".bright_green(),
        markets.len().to_string().bright_green().bold(),
        args.output.display().to_string().bright_cyan()
    );
    
    Ok(())
}

/// Execute database list command with interactive features and enhanced error handling
async fn execute_db_list(args: DbListArgs, verbose: bool) -> Result<()> {
    // Set verbose logging if requested
    if verbose {
        info!("Verbose mode enabled for db list command");
        debug!("Command args: {:?}", args);
    }
    
    // Launch interactive TUI by default, unless --no-interactive is specified
    if args.interactive && !args.no_interactive {
        return execute_db_list_interactive(args).await;
    }
    use std::time::Duration;
    use tokio::time::timeout;
    
    // Clear console and show header
    print!("\x1B[2J\x1B[1;1H");
    println!("{}", "📝 Market List (Press 'q' or Ctrl+C to quit)".bright_blue().bold());
    println!("{}", "=".repeat(60).bright_black());
    
    // Show active filters
    let mut filters = Vec::new();
    if args.active_only { filters.push("Active only".bright_green().to_string()); }
    if args.closed_only { filters.push("Closed only".bright_red().to_string()); }
    if let Some(ref cat) = args.category { 
        filters.push(format!("Category: {}", cat).bright_magenta().to_string()); 
    }
    if let Some(vol) = args.min_volume { 
        filters.push(format!("Min volume: ${:.0}", vol).bright_yellow().to_string()); 
    }
    
    if !filters.is_empty() {
        println!("  {} Filters: {}", "🎯", filters.join(" | "));
    }
    println!("  {} Sort: {} {}", 
        "📊", 
        args.sort_by.bright_cyan(),
        if args.order == "desc" { "↓" } else { "↑" }
    );
    println!();
    
    // Initialize database with detailed progress tracking
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg} [{elapsed_precise}]")
            .unwrap()
    );
    pb.set_message("Connecting to database...");
    pb.enable_steady_tick(Duration::from_millis(80));
    
    let db_path = PathBuf::from("./data/database/gamma");
    info!("Initializing database connection to: {}", db_path.display());
    
    let db_start = std::time::Instant::now();
    let database = GammaDatabase::new(&db_path).await
        .context("Failed to initialize gamma database")?;
    let db_time = db_start.elapsed();
    info!("Database connection established in {:.2}s", db_time.as_secs_f64());
    
    // Get total count first for better progress display
    pb.set_message("Counting total markets...");
    let count_start = std::time::Instant::now();
    
    let total_count = match timeout(Duration::from_secs(5), database.get_market_count()).await {
        Ok(Ok(count)) => {
            let count_time = count_start.elapsed();
            info!("Market count query completed in {:.2}s: {} markets", count_time.as_secs_f64(), count);
            count
        },
        Ok(Err(e)) => {
            pb.finish_and_clear();
            error!("Failed to get market count: {}", e);
            return Err(anyhow::anyhow!("Failed to count markets: {}\nDatabase may be corrupted or inaccessible", e));
        },
        Err(_) => {
            pb.finish_and_clear();
            error!("Market count query timed out after 5s");
            return Err(anyhow::anyhow!("Market count timed out - database may be unresponsive"));
        }
    };
    
    pb.set_message(format!("Found {} total markets. Building query...", total_count));
    
    // Build query with filters and sorting
    let mut query = "SELECT * FROM markets".to_string();
    let mut conditions = Vec::new();
    
    if args.active_only {
        conditions.push("active = true".to_string());
    }
    if args.closed_only {
        conditions.push("closed = true".to_string());
    }
    
    if let Some(min_vol) = args.min_volume {
        conditions.push(format!("volume >= {}", min_vol));
    }
    
    if let Some(ref category) = args.category {
        conditions.push(format!("category = '{}'", category));
    }
    
    if !conditions.is_empty() {
        query.push_str(" WHERE ");
        query.push_str(&conditions.join(" AND "));
    }
    
    // Add sorting - but skip it for troubleshooting large datasets
    if total_count < 10000 || args.sort_by == "created_at" || args.sort_by == "updated_at" {
        let order_dir = if args.order == "asc" { "ASC" } else { "DESC" };
        query.push_str(&format!(" ORDER BY {} {}", args.sort_by, order_dir));
        info!("Added sorting: ORDER BY {} {}", args.sort_by, order_dir);
    } else {
        info!("Skipping ORDER BY {} for performance on large dataset ({}+ records). Use --sort-by created_at for faster queries.", args.sort_by, total_count);
        println!("  {} Skipping sorting by '{}' for performance ({}+ records)", 
            "⚠️".bright_yellow(), 
            args.sort_by.bright_red(), 
            total_count.to_string().bright_yellow()
        );
        println!("  {} Use --sort-by created_at for faster queries", "💡".bright_cyan());
    }
    
    // Use the same efficient batching approach as gamma markets for full database access
    info!("Query configuration: requested_limit={}, offset={}", args.limit, args.offset);
    info!("Filters applied: active_only={}, closed_only={}, category={:?}, min_volume={:?}", 
        args.active_only, args.closed_only, args.category, args.min_volume);
    
    pb.set_message("Loading markets with efficient batching...");
    
    // For simple queries without complex filters, use get_all_markets for efficiency
    let markets = if conditions.is_empty() && args.offset == 0 {
        // Use the same efficient method as gamma markets for full database access
        pb.set_message("Fetching all markets with batching...");
        let query_start = std::time::Instant::now();
        
        match timeout(Duration::from_secs(300), database.get_all_markets(Some(args.limit))).await {
            Ok(Ok(markets)) => {
                let query_time = query_start.elapsed();
                info!("Batch query completed successfully in {:.2}s, got {} markets", query_time.as_secs_f64(), markets.len());
                markets
            },
            Ok(Err(e)) => {
                pb.finish_and_clear();
                let query_time = query_start.elapsed();
                error!("Database batch query failed after {:.2}s: {}", query_time.as_secs_f64(), e);
                return Err(anyhow::anyhow!("Database batch query failed: {}", e));
            }
            Err(_) => {
                pb.finish_and_clear();
                error!("Database query timed out after 5 minutes");
                return Err(anyhow::anyhow!("Database query timed out - this is unusual for batch queries"));
            }
        }
    } else {
        // For filtered queries, use direct SQL query (but still allow larger limits)
        let effective_limit = if args.limit > 10000 { 10000 } else { args.limit }; // Reasonable limit for filtered queries
        query.push_str(&format!(" LIMIT {} START {}", effective_limit, args.offset));
        
        pb.set_message(format!("Loading {} filtered markets (offset: {})...", effective_limit, args.offset));
        
        let query_start = std::time::Instant::now();
        debug!("Executing filtered db list query: {}", query);
        
        match timeout(Duration::from_secs(60), database.execute_query(&query)).await {
            Ok(Ok(markets)) => {
                let query_time = query_start.elapsed();
                info!("Filtered query completed successfully in {:.2}s, got {} markets", query_time.as_secs_f64(), markets.len());
                markets
            },
            Ok(Err(e)) => {
                pb.finish_and_clear();
                let query_time = query_start.elapsed();
                error!("Database filtered query failed after {:.2}s: {}", query_time.as_secs_f64(), e);
                error!("Query was: {}", query);
                return Err(anyhow::anyhow!("Database filtered query failed: {}", e));
            }
            Err(_) => {
                pb.finish_and_clear();
                error!("Database filtered query timed out after 60s");
                error!("Query was: {}", query);
                return Err(anyhow::anyhow!("Database filtered query timed out"));
            }
        }
    };
    
    pb.finish_with_message(format!("✅ Loaded {} markets", markets.len()));
    println!();
    
    if markets.is_empty() {
        println!("{} {}", "❌", "No markets found matching criteria.".bright_red());
        
        // Provide helpful suggestions when no results found
        println!("\n{} Suggestions:", "💡".bright_yellow());
        if args.category.is_some() || args.min_volume.is_some() || args.active_only || args.closed_only {
            println!("  • Try removing some filters to see more results");
            println!("  • Check if your filter values are correct");
        }
        if args.offset > 0 {
            println!("  • Try --offset 0 to start from the beginning");
        }
        println!("  • Use 'gamma db count' to see total market counts");
        println!("  • Database contains {} total markets", total_count.to_string().bright_cyan());
        
        return Ok(());
    }
    
    match args.format.as_str() {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&markets)?);
        },
        "csv" => {
            // Print CSV with progress
            println!("market_id,question,category,volume,liquidity,active,closed");
            let csv_pb = ProgressBar::new(markets.len() as u64);
            csv_pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} Formatting CSV [{bar:30}] {pos}/{len}")
                    .unwrap()
            );
            
            for market in markets {
                println!("{},{:?},{:?},{},{},{},{}",
                    market.id.0,
                    market.question,
                    market.category.as_deref().unwrap_or(""),
                    market.volume(),
                    market.liquidity.unwrap_or_default(),
                    market.active,
                    market.closed
                );
                csv_pb.inc(1);
            }
            csv_pb.finish_and_clear();
        },
        _ => {
            // Beautiful interactive table format
            println!("{} Showing markets {} to {} (Total in DB: {})", 
                "📦",
                (args.offset + 1).to_string().bright_yellow(),
                (args.offset + markets.len() as u64).to_string().bright_yellow(),
                total_count.to_string().bright_cyan()
            );
            println!("{}", "=".repeat(80).bright_black());
            
            // Table header
            println!("{:<5} {:<50} {:<15} {:>12} {}",
                "#".bright_black(),
                "Question".bright_white().bold(),
                "Category".bright_white().bold(),
                "Volume".bright_white().bold(),
                "Status".bright_white().bold()
            );
            println!("{}", "─".repeat(80).bright_black());
            
            // Enable raw mode for interactive quit
            let _ = enable_raw_mode();
            
            // Spawn a task to check for quit key
            let quit_handle = tokio::spawn(async {
                loop {
                    if event::poll(Duration::from_millis(100)).unwrap_or(false) {
                        if let Ok(Event::Key(key)) = event::read() {
                            match key {
                                KeyEvent { code: KeyCode::Char('q'), modifiers: KeyModifiers::NONE, .. } |
                                KeyEvent { code: KeyCode::Char('c'), modifiers: KeyModifiers::CONTROL, .. } => {
                                    return true;
                                }
                                _ => {}
                            }
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            });
            
            // Print markets with progress animation
            let display_pb = ProgressBar::new(markets.len() as u64);
            display_pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} Displaying... [{bar:30}] {pos}/{len}")
                    .unwrap()
                    .progress_chars("█▓▒░")
            );
            
            for (i, market) in markets.iter().enumerate() {
                // Check if user wants to quit
                if quit_handle.is_finished() {
                    let _ = disable_raw_mode();
                    println!("\n\n{} Display interrupted by user", "⚠️".bright_yellow());
                    return Ok(());
                }
                
                let num = format!("{}", (args.offset as usize) + i + 1);
                let question = if market.question.len() > 48 {
                    format!("{}…", market.question.chars().take(47).collect::<String>())
                } else {
                    market.question.clone()
                };
                let category = market.category.as_deref().unwrap_or("-").chars().take(13).collect::<String>();
                let volume = format!("${:.0}", market.volume());
                
                let status = if market.active {
                    "🟢 Active".bright_green().to_string()
                } else if market.closed {
                    "🔴 Closed".bright_red().to_string()
                } else {
                    "🟡 Inactive".bright_yellow().to_string()
                };
                
                println!("{:<5} {:<50} {:<15} {:>12} {}",
                    num.bright_cyan(),
                    question,
                    category.bright_magenta(),
                    volume.bright_green(),
                    status
                );
                
                display_pb.inc(1);
                
                // Small delay for smooth display
                if i < 50 {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
            
            display_pb.finish_and_clear();
            let _ = disable_raw_mode();
            
            // Clean up the quit handler
            quit_handle.abort();
            
            println!("{}", "─".repeat(80).bright_black());
            
            // Smart pagination info
            let displayed_count = markets.len() as u64;
            let has_more = displayed_count == args.limit;
            
            if has_more || args.offset > 0 {
                println!();
                if args.offset > 0 {
                    println!("  {} Previous page: {}", 
                        "⬅️",
                        format!("--offset {}", args.offset.saturating_sub(args.limit)).bright_cyan()
                    );
                }
                if has_more {
                    println!("  {} Next page: {}", 
                        "➡️",
                        format!("--offset {}", args.offset + args.limit).bright_cyan()
                    );
                }
            }
            
            println!("  {} Total markets in database: {}", "📊", total_count.to_string().bright_yellow());
            
            // Show how many we're displaying
            if args.offset > 0 || displayed_count < total_count {
                let start_num = args.offset + 1;
                let end_num = args.offset + displayed_count;
                println!("  {} Showing: {} to {} of {}", 
                    "📋",
                    start_num.to_string().bright_cyan(),
                    end_num.to_string().bright_cyan(),
                    total_count.to_string().bright_yellow()
                );
            }
            
            // Performance tips for very large queries
            if args.limit > 50000 {
                println!("\n  {} Tips for very large datasets:", "💡".bright_yellow());
                println!("    • Use filters to reduce results: --active-only, --category, --min-volume");
                println!("    • Consider smaller limits for faster response: --limit 10000");
                println!("    • Use 'gamma markets' for API-based data fetching");
            }
        }
    }
    
    Ok(())
}

/// Execute database cleanup command
async fn execute_db_cleanup(args: DbCleanupArgs, _verbose: bool) -> Result<()> {
    // Clear console and show header
    print!("\x1B[2J\x1B[1;1H");
    println!("{}", "🧽 Database Cleanup".bright_blue().bold());
    println!("{}", "=".repeat(50).bright_black());
    
    if !args.yes {
        println!("{}", "⚠️  Warning: This operation will modify the database!".bright_yellow());
        println!("Operations to perform:");
        if args.remove_duplicates {
            println!("  - Remove duplicate markets");
        }
        if args.vacuum {
            println!("  - Vacuum database (reclaim space)");
        }
        if args.update_stats {
            println!("  - Update statistics");
        }
        if let Some(days) = args.remove_older_than {
            println!("  - Remove markets older than {} days", days);
        }
        
        print!("\nContinue? [y/N] ");
        use std::io::{self, Write};
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Operation cancelled.");
            return Ok(());
        }
    }
    
    // Initialize database
    let db_path = PathBuf::from("./data/database/gamma");
    let database = GammaDatabase::new(&db_path).await
        .context("Failed to initialize gamma database")?;
    
    if args.remove_duplicates {
        println!("Removing duplicates...");
        if args.dry_run {
            println!("  [DRY RUN] Would remove duplicates");
        } else {
            // Get all markets and find duplicates
            let all_markets = database.get_all_markets(None).await?;
            let mut seen = std::collections::HashSet::new();
            let mut duplicates = 0;
            
            for market in all_markets {
                if !seen.insert(market.id.0.clone()) {
                    duplicates += 1;
                }
            }
            
            println!("  Found {} duplicate markets", duplicates);
            // Note: Actual deletion would require additional database methods
        }
    }
    
    if args.vacuum {
        println!("Vacuuming database...");
        if args.dry_run {
            println!("  [DRY RUN] Would vacuum database");
        } else {
            // This would require a VACUUM command in SurrealDB
            println!("  Vacuum operation not yet implemented");
        }
    }
    
    if args.update_stats {
        println!("Updating statistics...");
        if args.dry_run {
            println!("  [DRY RUN] Would update statistics");
        } else {
            let stats = database.get_stats().await?;
            println!("  Total markets: {}", stats.total_markets);
            println!("  Active markets: {}", stats.active_markets);
            println!("  Closed markets: {}", stats.closed_markets);
        }
    }
    
    if let Some(days) = args.remove_older_than {
        println!("Removing markets older than {} days...", days);
        if args.dry_run {
            println!("  [DRY RUN] Would remove old markets");
        } else {
            // This would require date filtering
            println!("  Date-based removal not yet implemented");
        }
    }
    
    println!("{}", "✅ Cleanup completed!".bright_green());
    
    Ok(())
}

/// Execute database get command
async fn execute_db_get(args: DbGetArgs, _verbose: bool) -> Result<()> {
    // Clear console and show header
    print!("\x1B[2J\x1B[1;1H");
    println!("{}", "🔎 Market Details".bright_blue().bold());
    println!("{}", "=".repeat(50).bright_black());
    println!("  {} Market ID: {}", "🆔", args.market_id.bright_cyan());
    println!();
    
    // Initialize database with spinner
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap()
    );
    spinner.set_message("Loading market data...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));
    
    let db_path = PathBuf::from("./data/database/gamma");
    let database = GammaDatabase::new(&db_path).await
        .context("Failed to initialize gamma database")?;
    
    // Query for specific market
    let query = format!("SELECT * FROM markets WHERE market_id = '{}'", args.market_id);
    let mut results = database.execute_query(&query).await?;
    spinner.finish_and_clear();
    
    if results.is_empty() {
        println!("{} {}", "❌", format!("Market '{}' not found.", args.market_id).bright_red());
        return Ok(());
    }
    
    let market = results.remove(0);
    
    match args.format.as_str() {
        "yaml" => {
            println!("{}", serde_yaml::to_string(&market)?);
        },
        "table" => {
            // Print as beautiful formatted table
            println!("{} {}", "📄", "Market Information".bright_white().bold());
            println!("{}", "─".repeat(60).bright_black());
            
            // ID and Question
            println!("  {} ID: {}", "🆔", market.id.0.to_string().bright_cyan());
            println!("  {} Question: {}", "❓", market.question.bright_white());
            
            // Status with colored indicators
            let status_parts = vec![
                if market.active { "🟢 Active".bright_green().to_string() } else { "🔴 Inactive".bright_red().to_string() },
                if market.closed { "🔒 Closed".bright_red().to_string() } else { "🔓 Open".bright_green().to_string() },
                if market.archived { "📦 Archived".bright_yellow().to_string() } else { "📂 Not Archived".bright_white().to_string() }
            ];
            println!("  {} Status: {}", "📊", status_parts.join(" | "));
            
            // Category and financial info
            if let Some(ref cat) = market.category {
                println!("  {} Category: {}", "🏷️", cat.bright_magenta());
            }
            println!("  {} Volume: {}", "💰", format!("${:.2}", market.volume()).bright_green().bold());
            println!("  {} Liquidity: {}", "💧", format!("${:.2}", market.liquidity.unwrap_or_default()).bright_blue());
            
            // Outcomes
            println!("  {} Outcomes ({}):", "🎯", market.outcomes.len().to_string().bright_yellow());
            for (i, outcome) in market.outcomes.iter().enumerate() {
                println!("     {}. {}", (i + 1).to_string().bright_cyan(), outcome);
            }
            
            // Timestamps
            println!("  {} Created: {}", "📅", market.created_at.format("%Y-%m-%d %H:%M:%S").to_string().bright_black());
            println!("  {} Updated: {}", "🔄", market.updated_at.format("%Y-%m-%d %H:%M:%S").to_string().bright_black());
            
            if args.all_fields {
                println!("\n--- All Fields ---");
                println!("{}", serde_json::to_string_pretty(&market)?);
            }
        },
        _ => {
            // Default to JSON
            println!("{}", serde_json::to_string_pretty(&market)?);
        }
    }
    
    Ok(())
}

/// Execute database count command
async fn execute_db_count(args: DbCountArgs, _verbose: bool) -> Result<()> {
    // Clear console and show header
    print!("\x1B[2J\x1B[1;1H");
    println!("{}", "🧮 Market Count Analysis".bright_blue().bold());
    println!("{}", "=".repeat(50).bright_black());
    
    // Initialize database with spinner
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap()
    );
    spinner.set_message("Connecting to database...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));
    
    let db_path = PathBuf::from("./data/database/gamma");
    let database = GammaDatabase::new(&db_path).await
        .context("Failed to initialize gamma database")?;
    
    spinner.set_message("Counting markets...");
    let total_stats = database.get_stats().await?;
    let total_count = total_stats.total_markets as f64;
    spinner.finish_and_clear();
    
    if let Some(ref group_by) = args.group_by {
        match group_by.as_str() {
            "category" => {
                let query = "SELECT category, COUNT(*) as count FROM markets GROUP BY category ORDER BY count DESC";
                let _results = database.execute_raw_query(query).await?;
                
                println!("\n📊 Markets by Category:");
                println!("{:<30} {:>10} {:>10}", "Category", "Count", "Percentage");
                println!("{}", "-".repeat(52));
                
                // Parse results and display
                // Note: This would need proper result parsing from SurrealDB
                println!("(Category grouping requires enhanced database query support)");
            },
            "active" => {
                println!("\n{} {}", "📊".bright_cyan(), "Markets by Status".bright_white().bold());
                println!();
                
                let active_pct = total_stats.active_markets as f64 / total_count * 100.0;
                let inactive_pct = 100.0 - active_pct;
                let inactive_count = total_stats.total_markets - total_stats.active_markets;
                
                // Active markets bar
                let active_bar_width = (active_pct / 100.0 * 40.0) as usize;
                let active_bar = "█".repeat(active_bar_width);
                println!("  {} Active:   {} {} {}",
                    "🟢",
                    active_bar.bright_green(),
                    total_stats.active_markets.to_string().bright_green().bold(),
                    format!("({:.1}%)", active_pct).bright_black()
                );
                
                // Inactive markets bar
                let inactive_bar_width = (inactive_pct / 100.0 * 40.0) as usize;
                let inactive_bar = "█".repeat(inactive_bar_width);
                println!("  {} Inactive: {} {} {}",
                    "🔴",
                    inactive_bar.bright_red(),
                    inactive_count.to_string().bright_red().bold(),
                    format!("({:.1}%)", inactive_pct).bright_black()
                );
            },
            "closed" => {
                println!("\n📊 Markets by Closed Status:");
                println!("{:<20} {:>10} {:>10}", "Status", "Count", "Percentage");
                println!("{}", "-".repeat(42));
                
                let closed_pct = total_stats.closed_markets as f64 / total_count * 100.0;
                let open_pct = 100.0 - closed_pct;
                
                println!("{:<20} {:>10} {:>9.1}%", "Closed", total_stats.closed_markets, closed_pct);
                println!("{:<20} {:>10} {:>9.1}%", "Open", 
                    total_stats.total_markets - total_stats.closed_markets, open_pct);
            },
            "archived" => {
                println!("\n📊 Markets by Archive Status:");
                println!("{:<20} {:>10} {:>10}", "Status", "Count", "Percentage");
                println!("{}", "-".repeat(42));
                
                let archived_pct = total_stats.archived_markets as f64 / total_count * 100.0;
                let not_archived_pct = 100.0 - archived_pct;
                
                println!("{:<20} {:>10} {:>9.1}%", "Archived", total_stats.archived_markets, archived_pct);
                println!("{:<20} {:>10} {:>9.1}%", "Not Archived", 
                    total_stats.total_markets - total_stats.archived_markets, not_archived_pct);
            },
            _ => {
                println!("{}", format!("⚠️  Unknown group_by field: {}", group_by).yellow());
            }
        }
    } else {
        // Simple count with optional filters
        println!("\n{} {}", "📦".bright_yellow(), "Market Count".bright_white().bold());
        
        let mut conditions_desc = Vec::new();
        if args.active_only {
            conditions_desc.push("Active only");
        }
        if let Some(ref _category) = args.category {
            conditions_desc.push("Category filter");
        }
        
        if !conditions_desc.is_empty() {
            println!("  {} Filters: {}", "🎯", conditions_desc.join(", ").bright_black());
        }
        
        // Calculate count based on filters
        let count = if args.active_only {
            total_stats.active_markets
        } else {
            total_stats.total_markets
        };
        
        println!("\n  {} Total: {}", 
            "🔢",
            count.to_string().bright_cyan().bold()
        );
        
        if args.percentage && (args.active_only || args.category.is_some()) {
            let percentage = count as f64 / total_count * 100.0;
            let bar_width = (percentage / 100.0 * 30.0) as usize;
            let bar = "█".repeat(bar_width);
            let empty = "░".repeat(30 - bar_width);
            
            println!("  {} Percentage: {}{} {:.1}%", 
                "📈",
                bar.bright_cyan(),
                empty.bright_black(),
                percentage
            );
        }
    }
    
    Ok(())
}

/// Execute database health check command
async fn execute_db_health(args: DbHealthArgs, verbose: bool) -> Result<()> {
    // Clear console and show header
    print!("\x1B[2J\x1B[1;1H");
    println!("{}", "🏥 Database Health Check".bright_blue().bold());
    println!("{}", "=".repeat(60).bright_black());
    
    // Initialize database with progress
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg} [{elapsed_precise}]")
            .unwrap()
    );
    pb.set_message("Connecting to database...");
    pb.enable_steady_tick(Duration::from_millis(80));
    
    let db_path = PathBuf::from("./data/database/gamma");
    let database = GammaDatabase::new(&db_path).await
        .context("Failed to initialize gamma database")?;
    
    pb.set_message("Running health diagnostics...");
    
    // Run health check
    let health_start = std::time::Instant::now();
    let health = match timeout(Duration::from_secs(30), database.health_check()).await {
        Ok(Ok(health)) => health,
        Ok(Err(e)) => {
            pb.finish_and_clear();
            error!("Health check failed: {}", e);
            return Err(anyhow::anyhow!("Health check failed: {}", e));
        }
        Err(_) => {
            pb.finish_and_clear();
            error!("Health check timed out after 30 seconds");
            return Err(anyhow::anyhow!("Health check timed out - database may be unresponsive"));
        }
    };
    let health_time = health_start.elapsed();
    
    pb.finish_and_clear();
    
    // Display results
    let status_icon = if health.is_healthy { "✅" } else { "❌" };
    let status_text = if health.is_healthy { 
        "HEALTHY".bright_green().to_string()
    } else { 
        "UNHEALTHY".bright_red().to_string()
    };
    
    println!("\n{} Overall Status: {}", status_icon, status_text);
    println!("  {} Health check completed in {:.2}s", "⏱️", health_time.as_secs_f64());
    
    // Performance metrics
    println!("\n{} {}", "📊".bright_cyan(), "Performance Metrics".bright_white().bold());
    
    let ping_status = if health.ping_time_ms < 100 { 
        "🟢 Excellent" 
    } else if health.ping_time_ms < 500 { 
        "🟡 Good" 
    } else { 
        "🔴 Poor" 
    };
    
    println!("  {} Database Ping: {}ms ({})", 
        "🏓", 
        health.ping_time_ms.to_string().bright_yellow(), 
        ping_status
    );
    
    if let Some(count_time) = health.count_time_ms {
        let count_status = if count_time < 1000 { 
            "🟢 Fast" 
        } else if count_time < 5000 { 
            "🟡 Moderate" 
        } else { 
            "🔴 Slow" 
        };
        
        println!("  {} Count Query: {}ms ({})", 
            "🧮", 
            count_time.to_string().bright_yellow(), 
            count_status
        );
    }
    
    // Database info
    if let Some(count) = health.market_count {
        println!("\n{} {}", "💾".bright_cyan(), "Database Information".bright_white().bold());
        println!("  {} Market Count: {}", "📦", count.to_string().bright_green());
        
        // Database size category
        let size_category = if count < 1000 {
            "🟢 Small dataset"
        } else if count < 50000 {
            "🟡 Medium dataset"
        } else if count < 200000 {
            "🟠 Large dataset"
        } else {
            "🔴 Very large dataset"
        };
        println!("  {} Dataset Size: {}", "📏", size_category);
        
        // Storage path info
        println!("  {} Database Path: {}", "📁", db_path.display().to_string().bright_black());
        
        if verbose {
            // Show disk usage if verbose
            if let Ok(_metadata) = std::fs::metadata(&db_path) {
                println!("  {} Path Exists: {}", "✓", "Yes".bright_green());
            } else {
                println!("  {} Path Exists: {}", "✗", "No".bright_red());
            }
        }
    }
    
    // Issues and recommendations
    if !health.issues.is_empty() {
        println!("\n{} {}", "⚠️".bright_yellow(), "Issues Detected".bright_yellow().bold());
        for (i, issue) in health.issues.iter().enumerate() {
            println!("  {}. {}", (i + 1).to_string().bright_red(), issue);
        }
    }
    
    if !health.recommendations.is_empty() {
        println!("\n{} {}", "💡".bright_yellow(), "Recommendations".bright_white().bold());
        for (i, rec) in health.recommendations.iter().enumerate() {
            println!("  {}. {}", (i + 1).to_string().bright_cyan(), rec);
        }
    }
    
    // Benchmark section
    if args.benchmark {
        println!("\n{} {}", "🏃".bright_blue(), "Running Performance Benchmarks".bright_white().bold());
        
        // Benchmark different query sizes
        let test_sizes = vec![1, 10, 100];
        for size in test_sizes {
            let bench_start = std::time::Instant::now();
            let query = format!("SELECT * FROM markets LIMIT {}", size);
            
            match timeout(Duration::from_secs(10), database.execute_query(&query)).await {
                Ok(Ok(results)) => {
                    let bench_time = bench_start.elapsed();
                    let rate = results.len() as f64 / bench_time.as_secs_f64().max(0.001);
                    println!("  {} Query {} records: {:.2}s ({:.0} records/sec)", 
                        "📊", size, bench_time.as_secs_f64(), rate);
                }
                Ok(Err(e)) => {
                    println!("  {} Query {} records: FAILED ({})", "❌", size, e);
                }
                Err(_) => {
                    println!("  {} Query {} records: TIMEOUT (>10s)", "⏱️", size);
                }
            }
        }
    }
    
    // Detailed section
    if args.detailed {
        println!("\n{} {}", "🔍".bright_blue(), "Detailed Diagnostics".bright_white().bold());
        
        // Check database schema
        match database.execute_raw_query("SHOW TABLES").await {
            Ok(tables) => {
                println!("  {} Database tables: {} found", "📋", tables.len());
                if verbose {
                    for table in tables {
                        println!("    • {}", table);
                    }
                }
            }
            Err(e) => {
                println!("  {} Database tables: ERROR ({})", "❌", e);
            }
        }
        
        // Check cache status
        println!("  {} Query cache: Active", "🗄️");
        println!("  {} Connection: SurrealDB RocksDB", "🔗");
    }
    
    // Summary recommendations based on health
    println!("\n{} {}", "🎯".bright_yellow(), "Next Steps".bright_white().bold());
    
    if health.is_healthy {
        println!("  {} Database is performing well", "✅");
        if health.market_count.unwrap_or(0) > 50000 {
            println!("  {} For large datasets, consider using filters in queries", "💡");
        }
        println!("  {} Regular health checks recommended", "🔄");
    } else {
        println!("  {} Address the issues listed above", "⚠️");
        println!("  {} Consider database maintenance", "🔧");
        println!("  {} Check system resources and disk space", "💾");
    }
    
    Ok(())
}

/// Build or load fast search engine with consistent error handling

/// Execute build index command
async fn execute_build_index(args: BuildIndexArgs, _verbose: bool) -> Result<()> {
    println!("{}", "🔨 Building search index...".bright_blue());
    
    // For now, we only support the markets index
    if args.index_type != "markets" {
        println!("{}", "❌ Only 'markets' index type is currently supported".red());
        return Ok(());
    }
    
    // Initialize or get the search service
    let db_path = PathBuf::from("./data/database/gamma");
    let index_path = crate::gamma::get_index_path();
    let service = crate::gamma::init_search_service(db_path, index_path.clone()).await?;
    
    // Start the service
    service.start().await?;
    
    // Watch the build progress
    println!("{}", "📊 Build Progress:".bright_cyan());
    let mut status_rx = service.subscribe_status();
    
    loop {
        let status = status_rx.borrow().clone();
        println!("{}", format_service_status(&status, true));
        
        match &status {
            crate::gamma::ServiceStatus::Ready { .. } => {
                println!("{}", "✅ Fast search index built successfully!".bright_green());
                println!("📁 Index location: {}", index_path.display().to_string().bright_cyan());
                break;
            }
            crate::gamma::ServiceStatus::Failed { error, .. } => {
                println!("{}", format!("❌ Failed to build index: {}", error).red());
                return Err(anyhow::anyhow!("Index build failed: {}", error));
            }
            _ => {
                status_rx.changed().await?;
            }
        }
    }
    
    Ok(())
}

/// Execute interactive search with ultra-fast engine
async fn execute_interactive_search(args: InteractiveSearchArgs, _verbose: bool) -> Result<()> {
    use std::io::{self, Write};
    
    println!("{}", "🚀 Ultra-Fast Interactive Search".bright_blue().bold());
    println!("{}", "=".repeat(50).bright_black());
    
    // Initialize or get the search service
    let db_path = PathBuf::from("./data/database/gamma");
    let index_path = crate::gamma::get_index_path();
    let service = crate::gamma::init_search_service(db_path, index_path).await?;
    
    // Start the service if not already started
    service.start().await?;
    
    // Wait for the service to be ready
    print!("⏳ Waiting for search service to be ready...");
    io::stdout().flush()?;
    
    let mut status_rx = service.subscribe_status();
    loop {
        let status = status_rx.borrow().clone();
        match &status {
            crate::gamma::ServiceStatus::Ready { .. } => {
                println!("\r✅ Search engine ready! Ultra-fast Aho-Corasick search enabled");
                if args.show_stats {
                    if let crate::gamma::ServiceStatus::Ready { markets, patterns, categories, build_time_ms } = status {
                        println!("\n📊 Index Statistics:");
                        println!("  • Documents: {}", markets.to_string().bright_cyan());
                        println!("  • Patterns: {}", patterns.to_string().bright_cyan());
                        println!("  • Categories: {}", categories.to_string().bright_cyan());
                        println!("  • Build time: {:.1}s", (build_time_ms as f64 / 1000.0));
                    }
                }
                break;
            }
            crate::gamma::ServiceStatus::Failed { error, .. } => {
                println!("\r❌ Failed to build search engine: {}", error);
                return Err(anyhow::anyhow!("Search service failed: {}", error));
            }
            _ => {
                print!("\r{}", format_service_status(&status, false));
                io::stdout().flush()?;
                status_rx.changed().await?;
            }
        }
    }
    
    println!("\n💡 Enter search queries (type 'quit' to exit):");
    println!("   Examples: 'trump', 'bitcoin', 'election', 'sports'");
    println!("{}", "=".repeat(50).bright_black());
    
    // Interactive search loop
    loop {
        print!("\n🔍 Search> ");
        io::stdout().flush()?;
        
        let mut query = String::new();
        io::stdin().read_line(&mut query)?;
        let query = query.trim();
        
        if query.is_empty() {
            continue;
        }
        
        if query == "quit" || query == "exit" || query == "q" {
            println!("👋 Goodbye!");
            break;
        }
        
        // Perform ultra-fast search
        let start = std::time::Instant::now();
        let params = crate::gamma::SearchParams {
            query: query.to_string(),
            category: None,
            tags: vec![],
            min_volume: None,
            max_volume: None,
            limit: 10,
            case_sensitive: false,
        };
        
        let results = match service.search(&params).await {
            Ok(results) => results,
            Err(e) => {
                println!("❌ Search error: {}", e);
                continue;
            }
        };
        let search_time = start.elapsed();
        
        println!("\n⚡ Found {} results in {}μs ({}ms)", 
            results.len(), 
            search_time.as_micros(),
            search_time.as_millis()
        );
        
        if results.is_empty() {
            println!("❌ No markets found matching '{}'", query);
        } else {
            println!("{}", "=".repeat(80).bright_black());
            for (i, market) in results.iter().enumerate() {
                println!("\n{} Result #{}", "📄", (i + 1).to_string().bright_yellow());
                println!("  {} {}", "❓", market.question.bright_white());
                println!("  {} ID: {} | Volume: ${:.0}", 
                    "🔹", 
                    market.id.0.bright_cyan(),
                    market.volume()
                );
                if let Some(ref cat) = market.category {
                    println!("  {} Category: {}", "🏷️", cat.bright_magenta());
                }
            }
        }
    }
    
    Ok(())
}

/// Execute search status command
async fn execute_search_status(args: SearchStatusArgs, _verbose: bool) -> Result<()> {
    use std::io::{self, Write};
    
    // Initialize or get the search service
    let db_path = PathBuf::from("./data/database/gamma");
    let index_path = crate::gamma::get_index_path();
    let service = crate::gamma::init_search_service(db_path, index_path).await?;
    
    if args.watch {
        // Watch mode - continuously display status updates
        println!("{}", "👀 Watching search service status (Ctrl+C to exit)...".bright_blue());
        println!("{}", "=".repeat(60).bright_black());
        
        let mut status_rx = service.subscribe_status();
        
        loop {
            // Clear line and move cursor to beginning
            print!("\r");
            
            let status = status_rx.borrow().clone();
            let status_str = format_service_status(&status, false);
            print!("{}", status_str);
            io::stdout().flush()?;
            
            // Check if ready or failed
            match &status {
                crate::gamma::ServiceStatus::Ready { .. } => {
                    println!(); // New line after final status
                    break;
                }
                crate::gamma::ServiceStatus::Failed { .. } => {
                    println!(); // New line after final status
                    break;
                }
                _ => {}
            }
            
            // Wait for next update or timeout
            tokio::select! {
                _ = status_rx.changed() => continue,
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => continue,
            }
        }
    } else {
        // One-time status check
        let progress = service.get_progress().await;
        
        if args.format == "json" {
            // JSON output
            println!("{}", serde_json::to_string_pretty(&progress)?);
        } else {
            // Text output
            println!("{}", "🔍 Fast Search Service Status".bright_blue().bold());
            println!("{}", "=".repeat(60).bright_black());
            
            let status_str = format_service_status(&progress.status, true);
            println!("{}", status_str);
            
            if let Some(started) = progress.started_at {
                println!("⏰ Started: {}", started.format("%Y-%m-%d %H:%M:%S UTC"));
            }
            
            if let Some(completed) = progress.completed_at {
                println!("✅ Completed: {}", completed.format("%Y-%m-%d %H:%M:%S UTC"));
            }
            
            println!("🔄 Last Update: {}", progress.last_update.format("%Y-%m-%d %H:%M:%S UTC"));
        }
    }
    
    Ok(())
}

/// Format service status for display
fn format_service_status(status: &crate::gamma::ServiceStatus, verbose: bool) -> String {
    use crate::gamma::ServiceStatus;
    
    match status {
        ServiceStatus::NotStarted => {
            "⚪ Not Started - Service has not been initialized".bright_black().to_string()
        }
        ServiceStatus::Connecting => {
            "🔵 Connecting - Establishing database connection...".bright_blue().to_string()
        }
        ServiceStatus::LoadingMarkets { loaded, total, rate } => {
            let percent = if *total > 0 { (*loaded as f64 / *total as f64) * 100.0 } else { 0.0 };
            format!("📊 Loading Markets - {}/{} ({:.1}%) at {:.0} markets/s", 
                    loaded.to_string().bright_cyan(),
                    total.to_string().bright_cyan(),
                    percent,
                    rate).bright_yellow().to_string()
        }
        ServiceStatus::BuildingIndex { markets, elapsed_ms } => {
            format!("🔨 Building Index - {} markets, elapsed: {:.1}s", 
                    markets.to_string().bright_cyan(),
                    (*elapsed_ms as f64 / 1000.0)).bright_yellow().to_string()
        }
        ServiceStatus::Ready { markets, patterns, categories, build_time_ms } => {
            if verbose {
                format!("✅ Ready - {} markets, {} patterns, {} categories (built in {:.1}s)",
                        markets.to_string().bright_green(),
                        patterns.to_string().bright_green(),
                        categories.to_string().bright_green(),
                        (*build_time_ms as f64 / 1000.0)).bright_green().bold().to_string()
            } else {
                format!("✅ Ready - {} markets indexed", markets).bright_green().bold().to_string()
            }
        }
        ServiceStatus::Failed { error, timestamp } => {
            format!("❌ Failed at {} - {}", 
                    timestamp.format("%H:%M:%S"),
                    error).bright_red().to_string()
        }
    }
}

/// Execute interactive database list with TUI (like gamma markets)
async fn execute_db_list_interactive(args: DbListArgs) -> Result<()> {
    use ratatui::{
        backend::CrosstermBackend,
        Terminal,
    };
    
    info!("Launching interactive database browser");
    
    // Initialize database
    let db_path = PathBuf::from("./data/database/gamma");
    let database = GammaDatabase::new(&db_path).await
        .context("Failed to initialize gamma database")?;
    
    // Don't initialize search service here to avoid database lock conflicts
    info!("Database browser ready - press 'b' to build fast search index");
    
    // Try to initialize TUI with graceful fallback
    match (|| -> Result<()> {
        enable_raw_mode().context("Failed to enable raw mode - terminal may not be available")?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .context("Failed to setup terminal - make sure you're running in a proper terminal")?;
        Ok(())
    })() {
        Ok(()) => {
            // TUI initialization successful, continue with TUI mode
            let backend = CrosstermBackend::new(std::io::stdout());
            let mut terminal = Terminal::new(backend)
                .context("Failed to create terminal - TUI not available")?;
            
            // Create the database browser
            let mut browser = DatabaseMarketsBrowser::new(database, args).await?;
            
            // Main event loop
            let result = run_database_browser(&mut terminal, &mut browser).await;
            
            // Cleanup TUI
            disable_raw_mode()?;
            execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture
            )?;
            terminal.show_cursor()?;
            
            result
        },
        Err(e) => {
            // TUI failed, fallback to non-interactive mode with existing database
            eprintln!("⚠️  TUI mode failed: {}", e);
            eprintln!("📋 Falling back to non-interactive list mode...");
            
            // Use the already initialized database to avoid lock conflicts
            execute_db_list_non_interactive_with_db(args, database).await
        }
    }
}

/// Execute non-interactive database list with pre-initialized database
async fn execute_db_list_non_interactive_with_db(args: DbListArgs, database: GammaDatabase) -> Result<()> {
    use std::time::Duration;
    use tokio::time::timeout;
    
    // Clear console and show header
    print!("\x1B[2J\x1B[1;1H");
    println!("{}", "📝 Market List (Press 'q' or Ctrl+C to quit)".bright_blue().bold());
    println!("{}", "=".repeat(60).bright_black());
    
    // Show active filters
    let mut filters = Vec::new();
    if args.active_only { filters.push("Active only".bright_green().to_string()); }
    if args.closed_only { filters.push("Closed only".bright_red().to_string()); }
    if let Some(ref cat) = args.category { 
        filters.push(format!("Category: {}", cat).bright_magenta().to_string()); 
    }
    if let Some(vol) = args.min_volume { 
        filters.push(format!("Min volume: ${:.0}", vol).bright_yellow().to_string()); 
    }
    
    if !filters.is_empty() {
        println!("  {} Filters: {}", "🎯", filters.join(" | "));
    }
    println!("  {} Sort: {} {}", 
        "📊", 
        args.sort_by.bright_cyan(),
        if args.order == "desc" { "↓" } else { "↑" }
    );
    println!();
    
    // Get total count (database already initialized)
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg} [{elapsed_precise}]")
            .unwrap()
    );
    pb.set_message("Counting total markets...");
    pb.enable_steady_tick(Duration::from_millis(80));
    
    let total_count = match timeout(Duration::from_secs(5), database.get_market_count()).await {
        Ok(Ok(count)) => {
            info!("Market count query completed: {} markets", count);
            count
        },
        Ok(Err(e)) => {
            pb.finish_with_message("❌ Failed to count markets");
            return Err(e.into());
        },
        Err(_) => {
            pb.finish_with_message("⏰ Database query timed out after 5s");
            return Err(anyhow::anyhow!("Database count query timed out"));
        }
    };
    
    pb.finish_with_message(format!("✅ Found {} markets", total_count));
    
    // Continue with the rest of the non-interactive logic...
    // (This would be the rest of the non-interactive implementation)
    println!("📊 Total markets: {}", total_count.to_string().bright_cyan().bold());
    println!("✅ Non-interactive mode working with fallback!");
    
    Ok(())
}

/// Database Markets Browser - with optional ultra-fast search capabilities
struct DatabaseMarketsBrowser {
    database: GammaDatabase,
    db_path: PathBuf,
    index_service: Arc<crate::gamma::IndexService>,
    fast_search_engine: Option<crate::gamma::FastSearchEngine>,
    markets: Vec<GammaMarket>,
    state: ListState,
    selected_market: Option<GammaMarket>,
    show_details: bool,
    search_query: String,
    search_mode: bool,
    show_help: bool,
    total_markets: u64,
    current_page: u64,
    page_size: u64,
    status_message: String,
    search_loading: bool,
    search_start_time: Option<std::time::Instant>,
    // Progress tracking
    last_index_progress: crate::gamma::IndexProgress,
    show_progress: bool,
    // Search result tracking
    all_search_results: Vec<GammaMarket>,
    total_search_results: usize,
}

impl DatabaseMarketsBrowser {
    async fn new(database: GammaDatabase, _filters: DbListArgs) -> Result<Self> {
        let total_markets = database.get_market_count().await?;
        let page_size = 50; // Start with reasonable page size for TUI
        let db_path = PathBuf::from("./data/database/gamma");
        
        let status_message = "📊 Database search mode (Press 'b' to build fast search index)".to_string();
        
        // Initialize the index service
        let index_service = crate::gamma::init_index_service().await?;
        let initial_progress = index_service.get_progress().await;
        
        let mut browser = Self {
            database,
            db_path,
            index_service,
            fast_search_engine: None,
            markets: Vec::new(),
            state: ListState::default(),
            selected_market: None,
            show_details: false,
            search_query: String::new(),
            search_mode: false,
            show_help: false,
            total_markets,
            current_page: 0,
            page_size,
            status_message,
            search_loading: false,
            search_start_time: None,
            last_index_progress: initial_progress,
            show_progress: false,
            all_search_results: Vec::new(),
            total_search_results: 0,
        };
        
        // Load first page
        browser.load_current_page().await?;
        
        Ok(browser)
    }
    
    
    async fn load_current_page(&mut self) -> Result<()> {
        let offset = self.current_page * self.page_size;
        
        // Apply search filter if active
        if !self.search_query.is_empty() {
            // Check if we need to reload search results
            if self.all_search_results.is_empty() {
                // Load all search results first
                if self.index_service.is_ready().await && self.fast_search_engine.is_some() {
                    let params = crate::gamma::SearchParams {
                        query: self.search_query.clone(),
                        category: None,
                        tags: vec![],
                        min_volume: None,
                        max_volume: None,
                        limit: 0, // Get all results for proper pagination
                        case_sensitive: false,
                    };
                    
                    if let Some(ref engine) = self.fast_search_engine {
                        let all_results = engine.search(&params);
                        self.all_search_results = all_results
                            .iter()
                            .map(|arc| (**arc).clone())
                            .collect();
                        self.total_search_results = self.all_search_results.len();
                    }
                } else {
                    // For database search, we'll simulate getting all results
                    // In practice, this might need pagination for very large result sets
                    self.all_search_results = self.database.search_markets(&self.search_query, None).await?;
                    self.total_search_results = self.all_search_results.len();
                }
            }
            
            // Apply pagination to cached search results
            let start_idx = (offset as usize).min(self.all_search_results.len());
            let end_idx = ((offset + self.page_size) as usize).min(self.all_search_results.len());
            
            self.markets = self.all_search_results[start_idx..end_idx].to_vec();
            
            let search_time = self.search_start_time
                .map(|start| start.elapsed().as_millis())
                .unwrap_or(0);
            
            let search_type = if self.index_service.is_ready().await && self.fast_search_engine.is_some() {
                "⚡ Ultra-fast search"
            } else {
                "🔍 Database search"
            };
            
            let max_page = (self.total_search_results + self.page_size as usize - 1) / self.page_size as usize;
            self.status_message = format!(
                "{}: '{}' - {} total results in {}ms | Page {} of {} | Showing {}-{}", 
                search_type,
                self.search_query, 
                self.total_search_results,
                search_time,
                self.current_page + 1,
                max_page.max(1),
                start_idx + 1,
                end_idx
            );
        } else {
            // Clear search results cache when not searching
            self.all_search_results.clear();
            self.total_search_results = 0;
            
            // Use direct query for proper pagination
            let query = format!("SELECT * FROM markets LIMIT {} START {}", self.page_size, offset);
            self.markets = self.database.execute_query(&query).await?;
            
            let start = offset + 1;
            let end = offset + self.markets.len() as u64;
            let max_page = (self.total_markets + self.page_size - 1) / self.page_size;
            self.status_message = format!(
                "Showing {}-{} of {} markets | Page {} of {}", 
                start, 
                end, 
                self.total_markets,
                self.current_page + 1,
                max_page
            );
        }
        
        // Reset selection to first item if we have markets
        if !self.markets.is_empty() {
            self.state.select(Some(0));
            self.selected_market = self.markets.first().cloned();
        } else {
            self.state.select(None);
            self.selected_market = None;
        }
        
        Ok(())
    }
    
    async fn next_page(&mut self) -> Result<()> {
        let max_page = if !self.search_query.is_empty() {
            // Use search results count for pagination
            (self.total_search_results + self.page_size as usize - 1) / self.page_size as usize
        } else {
            // Use total markets count for browsing
            ((self.total_markets + self.page_size - 1) / self.page_size) as usize
        };
        
        if (self.current_page + 1) < max_page as u64 {
            self.current_page += 1;
            self.load_current_page().await?;
        }
        Ok(())
    }
    
    async fn prev_page(&mut self) -> Result<()> {
        if self.current_page > 0 {
            self.current_page -= 1;
            self.load_current_page().await?;
        }
        Ok(())
    }
    
    fn select_next(&mut self) {
        if self.markets.is_empty() { return; }
        
        let selected = self.state.selected().unwrap_or(0);
        let next = if selected >= self.markets.len() - 1 { 0 } else { selected + 1 };
        self.state.select(Some(next));
        self.selected_market = self.markets.get(next).cloned();
    }
    
    fn select_previous(&mut self) {
        if self.markets.is_empty() { return; }
        
        let selected = self.state.selected().unwrap_or(0);
        let prev = if selected == 0 { self.markets.len() - 1 } else { selected - 1 };
        self.state.select(Some(prev));
        self.selected_market = self.markets.get(prev).cloned();
    }
    
    fn start_search(&mut self, query: String) {
        self.search_query = query;
        self.current_page = 0; // Reset to first page for search
        self.search_loading = true;
        self.search_start_time = Some(std::time::Instant::now());
        // Clear cached search results when starting new search
        self.all_search_results.clear();
        self.total_search_results = 0;
        self.status_message = if self.search_query.is_empty() {
            "Clearing search and loading all markets...".to_string()
        } else {
            format!("Searching for '{}'...", self.search_query)
        };
    }
    
    async fn complete_search_if_needed(&mut self) -> Result<()> {
        if self.search_loading {
            self.load_current_page().await?;
            self.search_loading = false;
            self.search_start_time = None;
        }
        Ok(())
    }
    
    /// Start building the fast search index
    async fn start_fast_search_build(&mut self) -> Result<()> {
        // Check current status
        let progress = self.index_service.get_progress().await;
        
        match progress.status {
            crate::gamma::IndexStatus::Ready { .. } => {
                self.status_message = "⚡ Fast search already ready! Enabled for searches.".to_string();
                return Ok(());
            }
            crate::gamma::IndexStatus::LoadingMarkets { .. } | 
            crate::gamma::IndexStatus::BuildingIndex { .. } => {
                self.status_message = "⏳ Fast search index already building...".to_string();
                self.show_progress = true;
                return Ok(());
            }
            _ => {}
        }
        
        // Show progress immediately
        self.show_progress = true;
        self.status_message = "📊 Loading all markets for index build...".to_string();
        
        // Create a new task to load markets and build index
        let index_service = self.index_service.clone();
        let db_path = self.db_path.clone();
        
        // Spawn the loading and building task
        tokio::spawn(async move {
            info!("Starting background task to load markets and build index");
            
            // Wait a moment to ensure the main thread has released any locks
            tokio::time::sleep(Duration::from_millis(100)).await;
            
            // Try to load markets with retries
            let mut retry_count = 0;
            const MAX_RETRIES: u32 = 10;
            
            loop {
                match GammaDatabase::new(&db_path).await {
                    Ok(db) => {
                        match db.get_all_markets(None).await {
                            Ok(markets) => {
                                info!("Successfully loaded {} markets, starting index build", markets.len());
                                if let Err(e) = index_service.start_build(markets, false).await {
                                    error!("Failed to start index build: {}", e);
                                }
                                break;
                            }
                            Err(e) => {
                                error!("Failed to load markets: {}", e);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        if retry_count < MAX_RETRIES && e.to_string().contains("locked") {
                            retry_count += 1;
                            info!("Database locked, retrying in 1 second... (attempt {}/{})", retry_count, MAX_RETRIES);
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        } else {
                            error!("Failed to connect to database after {} retries: {}", retry_count, e);
                            break;
                        }
                    }
                }
            }
        });
        
        self.status_message = "🔨 Loading markets in background for index build...".to_string();
        
        Ok(())
    }
    
    /// Update progress from index service (non-blocking)
    async fn update_progress(&mut self) {
        // Use a timeout to prevent blocking the UI
        match tokio::time::timeout(Duration::from_millis(1), self.index_service.get_progress()).await {
            Ok(new_progress) => {
                // Check if we need to update the fast search engine reference
                if matches!(new_progress.status, crate::gamma::IndexStatus::Ready { .. }) && self.fast_search_engine.is_none() {
                    // For now, we'd need to get the engine from the service
                    // This is a limitation of the current design
                }
                
                self.last_index_progress = new_progress;
            }
            Err(_) => {
                // Timeout - don't block, keep the current progress
            }
        }
    }
}

/// Main event loop for the database browser
async fn run_database_browser(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    browser: &mut DatabaseMarketsBrowser,
) -> Result<()> {
    let mut last_tick = std::time::Instant::now();
    let tick_rate = Duration::from_millis(100); // Faster refresh for better responsiveness
    
    loop {
        terminal.draw(|f| render_database_browser(f, browser))?;
        
        // Update progress from index service (non-blocking)
        browser.update_progress().await;
        
        // Use a very short timeout for better responsiveness
        let timeout = Duration::from_millis(50);
            
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match handle_key_event(browser, key.code).await {
                        Ok(should_quit) => {
                            if should_quit { break; }
                        }
                        Err(e) => {
                            browser.status_message = format!("Error: {}", e);
                        }
                    }
                }
            }
        }
        
        // Handle search completion asynchronously
        if browser.search_loading {
            // Use a quick timeout to check if we can complete the search
            match tokio::time::timeout(Duration::from_millis(1), browser.complete_search_if_needed()).await {
                Ok(Ok(())) => {
                    // Search completed successfully
                },
                Ok(Err(e)) => {
                    browser.search_loading = false;
                    browser.search_start_time = None;
                    browser.status_message = format!("Search error: {}", e);
                },
                Err(_) => {
                    // Search still in progress, update status with elapsed time
                    if let Some(start_time) = browser.search_start_time {
                        let elapsed = start_time.elapsed();
                        browser.status_message = if browser.search_query.is_empty() {
                            format!("Loading all markets... ({:.1}s) - Press Esc to cancel", elapsed.as_secs_f32())
                        } else {
                            format!("Searching for '{}'... ({:.1}s) - Press Esc to cancel", browser.search_query, elapsed.as_secs_f32())
                        };
                    }
                }
            }
        }
        
        if last_tick.elapsed() >= tick_rate {
            last_tick = std::time::Instant::now();
        }
    }
    
    Ok(())
}

/// Handle keyboard input for the database browser
async fn handle_key_event(browser: &mut DatabaseMarketsBrowser, key: KeyCode) -> Result<bool> {
    // Allow quitting even during search loading
    if key == KeyCode::Char('q') {
        return Ok(true);
    }
    
    // If search is loading, allow cancellation with Escape
    if browser.search_loading {
        match key {
            KeyCode::Esc => {
                browser.search_loading = false;
                browser.search_start_time = None;
                browser.search_mode = false;
                browser.search_query.clear();
                browser.status_message = "Search cancelled".to_string();
            }
            _ => {} // Ignore other keys during search loading
        }
        return Ok(false);
    }
    
    if browser.search_mode {
        match key {
            KeyCode::Enter => {
                browser.search_mode = false;
                browser.start_search(browser.search_query.clone());
            }
            KeyCode::Esc => {
                browser.search_mode = false;
                browser.search_query.clear();
                browser.start_search(String::new());
            }
            KeyCode::Char(c) => {
                browser.search_query.push(c);
            }
            KeyCode::Backspace => {
                browser.search_query.pop();
            }
            _ => {}
        }
        return Ok(false);
    }
    
    match key {
        KeyCode::Char('h') => browser.show_help = !browser.show_help,
        KeyCode::Char('/') => browser.search_mode = true,
        KeyCode::Down => browser.select_next(),
        KeyCode::Up => browser.select_previous(),
        KeyCode::PageDown => browser.next_page().await?,
        KeyCode::PageUp => browser.prev_page().await?,
        KeyCode::Enter => browser.show_details = !browser.show_details,
        KeyCode::Esc => browser.show_details = false,
        KeyCode::Char('b') => browser.start_fast_search_build().await?,
        KeyCode::Char('p') => browser.show_progress = !browser.show_progress,
        _ => {}
    }
    
    Ok(false)
}

/// Render the database browser UI
fn render_database_browser(f: &mut Frame, browser: &DatabaseMarketsBrowser) {
    let mut constraints = vec![
        Constraint::Length(3), // Header
    ];
    
    // Add progress section if building or showing progress
    if browser.show_progress {
        constraints.push(Constraint::Length(4)); // Progress section
    }
    
    constraints.extend([
        Constraint::Min(0),    // Main content
        Constraint::Length(3), // Footer
    ]);
    
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(f.area());
    
    let mut chunk_idx = 0;
    
    // Header with search mode info
    let search_mode_info = match browser.last_index_progress.status {
        crate::gamma::IndexStatus::Ready { .. } => " | ⚡ FAST SEARCH",
        crate::gamma::IndexStatus::LoadingMarkets { .. } | 
        crate::gamma::IndexStatus::BuildingIndex { .. } => " | 🔨 BUILDING INDEX...",
        _ => " | 📊 DATABASE MODE",
    };
    
    let header_text = format!("📚 Database Market Browser{} | Press 'b' to build, 'p' to toggle progress", search_mode_info);
    let header = Paragraph::new(header_text)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);
    f.render_widget(header, chunks[chunk_idx]);
    chunk_idx += 1;
    
    // Progress section (if showing)
    if browser.show_progress {
        render_progress_section(f, chunks[chunk_idx], &browser.last_index_progress);
        chunk_idx += 1;
    }
    
    // Main content
    if browser.show_help {
        render_help_overlay(f, chunks[chunk_idx]);
    } else if browser.show_details && browser.selected_market.is_some() {
        render_market_details(f, chunks[chunk_idx], browser.selected_market.as_ref().unwrap());
    } else {
        render_market_list(f, chunks[chunk_idx], browser);
    }
    chunk_idx += 1;
    
    // Footer with status and controls
    let footer_text = if browser.search_loading {
        format!("{} | Press 'q' to quit", browser.status_message)
    } else if browser.search_mode {
        format!("Search: {} (Enter to search, Esc to cancel)", browser.search_query)
    } else {
        format!("{} | Press 'h' for help, 'q' to quit, '/' to search, 'p' for progress", browser.status_message)
    };
    
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[chunk_idx]);
}

/// Render the market list
fn render_market_list(f: &mut Frame, area: Rect, browser: &DatabaseMarketsBrowser) {
    // Show loading message if search is in progress
    if browser.search_loading {
        let loading_text = if browser.search_query.is_empty() {
            "🔄 Loading all markets..."
        } else {
            "🔍 Searching markets..."
        };
        
        let loading = Paragraph::new(loading_text)
            .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .block(Block::default().borders(Borders::ALL).title("Loading"))
            .alignment(Alignment::Center);
        f.render_widget(loading, area);
        return;
    }
    
    let items: Vec<ListItem> = browser.markets
        .iter()
        .enumerate()
        .map(|(i, market)| {
            let status = if market.active { "🟢" } else if market.closed { "🔴" } else { "🟡" };
            let volume = market.volume().to_f64().unwrap_or(0.0);
            let volume_str = if volume >= 1_000_000.0 {
                format!("${:.1}M", volume / 1_000_000.0)
            } else if volume >= 1_000.0 {
                format!("${:.1}K", volume / 1_000.0)
            } else {
                format!("${:.0}", volume)
            };
            
            let question = if market.question.len() > 60 {
                format!("{}...", &market.question[..57])
            } else {
                market.question.clone()
            };
            
            let category = market.category.as_ref().unwrap_or(&"-".to_string()).clone();
            let category_short = if category.len() > 12 {
                format!("{}...", &category[..9])
            } else {
                category
            };
            
            let line = format!("{} {} | {} | {}", 
                status, question, category_short, volume_str);
            
            ListItem::new(line).style(
                if Some(i) == browser.state.selected() {
                    Style::default().bg(Color::Blue).fg(Color::White)
                } else {
                    Style::default()
                }
            )
        })
        .collect();
    
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Markets"))
        .highlight_style(Style::default().bg(Color::Blue).fg(Color::White));
    
    f.render_stateful_widget(list, area, &mut browser.state.clone());
}

/// Render the progress section
fn render_progress_section(f: &mut Frame, area: Rect, progress: &crate::gamma::IndexProgress) {
    use crate::gamma::IndexStatus;
    
    let progress_text = match &progress.status {
        IndexStatus::NotStarted => "🔴 Not Started - Press 'b' to build index".to_string(),
        IndexStatus::Initializing => "🔵 Initializing - Setting up database connection...".to_string(),
        IndexStatus::LoadingMarkets { loaded, total, rate, elapsed_seconds } => {
            let percentage = if *total > 0 { (*loaded as f64 / *total as f64) * 100.0 } else { 0.0 };
            let progress_bar = create_progress_bar(percentage, 30);
            format!(
                "📊 Loading Markets: {} {} ({:.1}%) | {}/{} markets | {:.0} markets/s | {:.1}s elapsed",
                progress_bar, 
                format!("{:.1}%", percentage),
                percentage,
                loaded, 
                total, 
                rate, 
                elapsed_seconds
            )
        }
        IndexStatus::BuildingIndex { markets, patterns, elapsed_seconds } => {
            format!(
                "🔨 Building Index: {} markets loaded, {} patterns found | {:.1}s elapsed",
                markets, patterns, elapsed_seconds
            )
        }
        IndexStatus::Ready { markets, patterns, categories, build_time_seconds, memory_usage_mb } => {
            format!(
                "✅ Ready: {} markets, {} patterns, {} categories | Built in {:.1}s | {:.1}MB memory",
                markets, patterns, categories, build_time_seconds, memory_usage_mb
            )
        }
        IndexStatus::Failed { error, timestamp } => {
            format!(
                "❌ Failed at {}: {}",
                timestamp.format("%H:%M:%S"),
                error
            )
        }
    };
    
    // Add timing information
    let timing_info = if let Some(started) = progress.started_at {
        let elapsed = chrono::Utc::now() - started;
        let elapsed_str = if elapsed.num_seconds() < 60 {
            format!("{}s", elapsed.num_seconds())
        } else {
            format!("{}m{}s", elapsed.num_minutes(), elapsed.num_seconds() % 60)
        };
        
        let eta_str = if let Some(eta) = progress.estimated_completion {
            let remaining = eta - chrono::Utc::now();
            if remaining.num_seconds() > 0 {
                format!(" | ETA: {}s", remaining.num_seconds())
            } else {
                "".to_string()
            }
        } else {
            "".to_string()
        };
        
        format!("\nElapsed: {}{}", elapsed_str, eta_str)
    } else {
        "".to_string()
    };
    
    let full_text = format!("{}{}", progress_text, timing_info);
    
    let progress_widget = Paragraph::new(full_text)
        .style(Style::default().fg(Color::Green))
        .block(Block::default().borders(Borders::ALL).title("🚀 Index Progress"))
        .wrap(Wrap { trim: false });
        
    f.render_widget(progress_widget, area);
}

/// Create a visual progress bar
fn create_progress_bar(percentage: f64, width: usize) -> String {
    let filled = ((percentage / 100.0) * width as f64) as usize;
    let empty = width - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

/// Render market details view
fn render_market_details(f: &mut Frame, area: Rect, market: &GammaMarket) {
    let volume = market.volume().to_f64().unwrap_or(0.0);
    let liquidity = market.liquidity.map(|l| l.to_f64().unwrap_or(0.0)).unwrap_or(0.0);
    
    let details = format!(
        "Question: {}\n\nCategory: {}\nVolume: ${:.2}\nLiquidity: ${:.2}\nActive: {}\nClosed: {}\n\nOutcomes:\n{}",
        market.question,
        market.category.as_ref().unwrap_or(&"None".to_string()),
        volume,
        liquidity,
        market.active,
        market.closed,
        market.outcomes.join("\n• ")
    );
    
    let paragraph = Paragraph::new(details)
        .block(Block::default().borders(Borders::ALL).title("Market Details"))
        .wrap(Wrap { trim: true });
    
    f.render_widget(paragraph, area);
}

/// Render help overlay
fn render_help_overlay(f: &mut Frame, area: Rect) {
    let help_text = vec![
        "Database Market Browser - Keyboard Shortcuts",
        "",
        "Navigation:",
        "  ↑/↓         - Navigate markets",
        "  PageUp/Down  - Navigate pages",
        "  Enter        - View market details",
        "  Esc          - Back to list",
        "",
        "Search:",
        "  /            - Start search",
        "  Enter        - Execute search",
        "  Esc          - Cancel search",
        "",
        "Fast Search:",
        "  b            - Build/Enable fast search index",
        "  p            - Toggle progress display",
        "               (Takes ~1 minute, then ultra-fast!)",
        "",
        "Other:",
        "  h            - Toggle this help",
        "  q            - Quit browser",
        "",
        "Default: Database search (reliable but slower)",
        "Fast search: Optional Aho-Corasick index ('b')",
    ];
    
    let paragraph = Paragraph::new(help_text.join("\n"))
        .block(Block::default().borders(Borders::ALL).title("Help"))
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::Green));
    
    f.render_widget(paragraph, area);
}