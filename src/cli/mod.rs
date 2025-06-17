//! CLI module for Polybot
//!
//! This module provides the command-line interface for the Polybot trading bot.
//! It uses clap for argument parsing and provides a structured command pattern
//! for all trading operations.
//!
//! See README.md for detailed documentation of the CLI architecture and usage patterns.

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod args;
pub mod commands;

pub use args::parse_percentage;
use crate::data_paths::{DataPaths, DEFAULT_DATA_DIR};

// Import all command args and commands
use commands::init::{InitArgs, InitCommand};
use commands::markets::{MarketsArgs, MarketsCommand};
use commands::fetch_all_markets::{FetchAllMarketsArgs, FetchAllMarketsCommand};
use commands::analyze::{AnalyzeArgs, AnalyzeCommand};
use commands::enrich::{EnrichArgs, EnrichCommand};
use commands::book::{BookArgs, BookCommand};
use commands::buy::{BuyArgs, BuyCommand};
use commands::sell::{SellArgs, SellCommand};
use commands::cancel::{CancelArgs, CancelCommand};
use commands::orders::{OrdersArgs, OrdersCommand};
use commands::stream::{StreamArgs, StreamCommand};
use commands::daemon::{DaemonArgs, DaemonCommand};
use commands::pipeline::{PipelineArgs, PipelineCommand};
use commands::datasets::{DatasetsArgs, DatasetsCommand};
use commands::install::{InstallArgs, InstallCommand};
use commands::version::{VersionArgs, VersionCommand};
use commands::index::{IndexArgs, IndexCommand};

#[derive(Parser)]
#[command(name = "polybot")]
#[command(version)]
#[command(about = "Rust CLI trading bot for Polymarket CLOB", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
    
    /// Use sandbox environment (Mumbai testnet)
    #[arg(long, global = true)]
    pub sandbox: bool,
    
    /// Data directory path (default: ./data)
    #[arg(long, global = true, default_value = DEFAULT_DATA_DIR)]
    pub data_dir: PathBuf,
    
    /// Verbose logging
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize authentication and save credentials
    Init(InitArgs),
    
    /// Browse and search Polymarket markets
    Markets(MarketsArgs),
    
    /// Fetch all markets and save to JSON file
    FetchAllMarkets(FetchAllMarketsArgs),
    
    /// Analyze and filter fetched markets
    Analyze(AnalyzeArgs),
    
    /// Enrich market data with real-time information
    Enrich(EnrichArgs),
    
    /// Show orderbook for a token
    Book(BookArgs),
    
    /// Place a buy order
    Buy(BuyArgs),
    
    /// Place a sell order
    Sell(SellArgs),
    
    /// Cancel an order
    Cancel(CancelArgs),
    
    /// List open orders
    Orders(OrdersArgs),
    
    /// Stream real-time market data via WebSocket
    Stream(StreamArgs),
    
    /// Run streaming daemon with sample strategy
    Daemon(DaemonArgs),
    
    /// Run pipeline workflows from YAML configurations
    Pipeline(PipelineArgs),
    
    /// Manage datasets and pipeline outputs
    Datasets(DatasetsArgs),
    
    /// Install polybot system-wide for easy access
    Install(InstallArgs),
    
    /// Show version information
    Version(VersionArgs),
    
    /// Index raw market data into RocksDB for fast queries
    Index(IndexArgs),
}

impl Cli {
    /// Get the host URL based on sandbox flag
    pub fn get_host(&self) -> &'static str {
        if self.sandbox {
            "https://clob-mumbai.polymarket.com"  // Mumbai testnet
        } else {
            "https://clob.polymarket.com"
        }
    }
    
    /// Execute the CLI command
    pub async fn execute(self) -> Result<()> {
        let host = self.get_host();
        let data_paths = DataPaths::new(&self.data_dir);
        
        // Ensure all directories exist
        data_paths.ensure_directories()?;
        
        match self.command {
            Commands::Init(args) => InitCommand::new(args).execute(host, data_paths).await,
            Commands::Markets(args) => MarketsCommand::new(args).execute(host, data_paths).await,
            Commands::FetchAllMarkets(args) => FetchAllMarketsCommand::new(args).execute(host, data_paths, self.verbose > 0).await,
            Commands::Analyze(args) => AnalyzeCommand::new(args).execute(host, data_paths).await,
            Commands::Enrich(args) => EnrichCommand::new(args).execute(host, data_paths).await,
            Commands::Book(args) => BookCommand::new(args).execute(host, data_paths).await,
            Commands::Buy(args) => BuyCommand::new(args).execute(host, data_paths).await,
            Commands::Sell(args) => SellCommand::new(args).execute(host, data_paths).await,
            Commands::Cancel(args) => CancelCommand::new(args).execute(host, data_paths).await,
            Commands::Orders(args) => OrdersCommand::new(args).execute(host, data_paths).await,
            Commands::Stream(args) => StreamCommand::new(args).execute(host, data_paths).await,
            Commands::Daemon(args) => DaemonCommand::new(args).execute(host, data_paths).await,
            Commands::Pipeline(args) => PipelineCommand::new(args).execute(host, data_paths).await,
            Commands::Datasets(args) => DatasetsCommand::new(args).execute(host, data_paths).await,
            Commands::Install(args) => InstallCommand::new(args).execute(host, data_paths).await,
            Commands::Version(args) => VersionCommand::new(args).execute(host, data_paths).await,
            Commands::Index(args) => IndexCommand::new(args).execute(host, data_paths).await,
        }
    }
} 