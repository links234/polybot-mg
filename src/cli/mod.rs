use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod args;
mod commands;

pub use args::parse_percentage;
pub use commands::*;
use crate::data_paths::{DataPaths, DEFAULT_DATA_DIR};

#[derive(Parser)]
#[command(name = "polybot")]
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
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize authentication and save credentials
    Init(commands::InitArgs),
    
    /// Browse and search Polymarket markets
    Markets(commands::MarketsArgs),
    
    /// Fetch all markets and save to JSON file
    FetchAllMarkets(commands::FetchAllMarketsArgs),
    
    /// Analyze and filter fetched markets
    Analyze(commands::AnalyzeArgs),
    
    /// Enrich market data with real-time information
    Enrich(commands::EnrichArgs),
    
    /// Show orderbook for a token
    Book(commands::BookArgs),
    
    /// Place a buy order
    Buy(commands::BuyArgs),
    
    /// Place a sell order
    Sell(commands::SellArgs),
    
    /// Cancel an order
    Cancel(commands::CancelArgs),
    
    /// List open orders
    Orders(commands::OrdersArgs),
}

impl Cli {
    /// Get the host URL based on sandbox flag
    pub fn get_host(&self) -> &'static str {
        if self.sandbox || cfg!(feature = "sandbox") {
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
            Commands::Init(args) => commands::execute_init(host, data_paths, args).await,
            Commands::Markets(args) => commands::execute_markets(host, data_paths, args).await,
            Commands::FetchAllMarkets(args) => commands::execute_fetch_all_markets(host, data_paths, args).await,
            Commands::Analyze(args) => commands::execute_analyze(host, data_paths, args).await,
            Commands::Enrich(args) => commands::execute_enrich(host, data_paths, args).await,
            Commands::Book(args) => commands::execute_book(host, data_paths, args).await,
            Commands::Buy(args) => commands::execute_buy(host, data_paths, args).await,
            Commands::Sell(args) => commands::execute_sell(host, data_paths, args).await,
            Commands::Cancel(args) => commands::execute_cancel(host, data_paths, args).await,
            Commands::Orders(args) => commands::execute_orders(host, data_paths, args).await,
        }
    }
} 