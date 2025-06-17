use anyhow::Result;
use clap::{Args, ValueEnum};
use tracing::{info, error};
use crate::data_paths::DataPaths;
use crate::typed_store::{
    TypedStore, TypedDbContext,
    models::{MarketTable, MarketIndexTable, RocksDbMarket, MarketCf, MarketIndex, MarketIndexCf, ALL_COLUMN_FAMILIES}
};
use comfy_table::{Table, Cell, Color, Attribute, ContentArrangement};

#[derive(Debug, Clone, ValueEnum, PartialEq)]
pub enum MarketMode {
    /// List all markets (default)
    List,
    /// Show markets sorted by volume (cached data)
    Volume,
    /// Show actively traded markets (live orderbook data)
    Active,
    /// Search markets by keyword
    Search,
    /// Get details for a specific market
    Details,
    /// Get market from Polymarket URL
    Url,
    /// Query markets from local RocksDB database (fast)
    Db,
}

#[derive(Args, Clone)]
pub struct MarketsArgs {
    /// Search term, market ID, or Polymarket URL (optional)
    pub query: Option<String>,
    
    /// Mode of operation
    #[arg(long, short = 'm', value_enum, default_value = "list")]
    pub mode: MarketMode,
    
    /// Maximum number of markets to display
    #[arg(long, short = 'n', default_value = "20")]
    pub limit: usize,
    
    /// Show detailed information
    #[arg(long, short = 'd')]
    pub detailed: bool,
    
    /// Force refresh cache (for volume mode)
    #[arg(long)]
    pub refresh: bool,
    
    /// Minimum volume filter in USD (for volume/active modes)
    #[arg(long)]
    pub min_volume: Option<f64>,
    
    /// Minimum price for YES outcome (0-100)
    #[arg(long, value_parser = crate::cli::parse_percentage)]
    pub min_price: Option<f64>,
    
    /// Maximum price for YES outcome (0-100)
    #[arg(long, value_parser = crate::cli::parse_percentage)]
    pub max_price: Option<f64>,
    
    /// Minimum spread between bid and ask (0-100) (for active mode)
    #[arg(long, value_parser = crate::cli::parse_percentage)]
    pub min_spread: Option<f64>,
    
    /// Maximum spread between bid and ask (0-100) (for active mode)
    #[arg(long, value_parser = crate::cli::parse_percentage)]
    pub max_spread: Option<f64>,
    
    /// Path to RocksDB database (for db mode)
    #[arg(long)]
    pub db_path: Option<std::path::PathBuf>,
    
    /// Filter by category (for db mode)
    #[arg(long)]
    pub category: Option<String>,
    
    /// Only show active markets (for db mode)
    #[arg(long)]
    pub active_only: bool,
    
    /// Only show closed markets (for db mode)
    #[arg(long)]
    pub closed_only: bool,
}

pub struct MarketsCommand {
    args: MarketsArgs,
}

impl MarketsCommand {
    pub fn new(args: MarketsArgs) -> Self {
        Self { args }
    }

    pub async fn execute(&self, host: &str, data_paths: DataPaths) -> Result<()> {
        // If no mode or query provided, show interactive TUI
        if self.args.mode == MarketMode::List && self.args.query.is_none() {
            let mut tui = crate::tui::MarketsTui::new(data_paths)?;
            return tui.run().await;
        }

        let client = crate::auth::get_authenticated_client(host, &data_paths).await?;
        
        match self.args.mode {
            MarketMode::List => {
                // Basic list with optional filter
                crate::markets::list_markets(client, self.args.query.clone(), self.args.limit).await?;
            }
            MarketMode::Volume => {
                // Volume-sorted markets with cache
                crate::markets::list_filtered_markets(
                    client, 
                    self.args.limit, 
                    self.args.refresh, 
                    self.args.min_volume, 
                    self.args.detailed, 
                    self.args.min_price, 
                    self.args.max_price
                ).await?;
            }
            MarketMode::Active => {
                // Actively traded markets with orderbook data
                crate::markets::list_active_markets(
                    client, 
                    self.args.limit, 
                    self.args.min_price, 
                    self.args.max_price, 
                    self.args.min_spread, 
                    self.args.max_spread, 
                    self.args.detailed
                ).await?;
            }
            MarketMode::Search => {
                // Search by keyword
                if let Some(keyword) = &self.args.query {
                    crate::markets::search_markets(client, keyword, self.args.detailed, self.args.limit).await?;
                } else {
                    error!("❌ Search mode requires a query term");
                    info!("Usage: polybot markets <search_term> --mode search");
                }
            }
            MarketMode::Details => {
                // Get specific market details
                if let Some(identifier) = &self.args.query {
                    crate::markets::get_market_details(client, identifier).await?;
                } else {
                    error!("❌ Details mode requires a market identifier");
                    info!("Usage: polybot markets <condition_id_or_slug> --mode details");
                }
            }
            MarketMode::Url => {
                // Get market from URL
                if let Some(url) = &self.args.query {
                    crate::markets::get_market_from_url(url).await?;
                } else {
                    error!("❌ URL mode requires a Polymarket URL");
                    info!("Usage: polybot markets <polymarket_url> --mode url");
                }
            }
            MarketMode::Db => {
                // Query from RocksDB database
                self.query_from_database(&data_paths).await?;
            }
        }
        
        Ok(())
    }

    async fn query_from_database(&self, data_paths: &DataPaths) -> Result<()> {
        // Check for new RocksDB location first
        let rocksdb_path = std::path::PathBuf::from("./data/database/rocksdb");
        let file_db_path = std::path::PathBuf::from("./data/database/markets");
        let legacy_db_path = data_paths.datasets().join("markets.db");
        
        info!("🔍 Checking database paths:");
        info!("   - RocksDB path: {} (exists: {})", rocksdb_path.display(), rocksdb_path.exists());
        info!("   - File path: {} (exists: {})", file_db_path.display(), file_db_path.exists());
        info!("   - Legacy path: {} (exists: {})", legacy_db_path.display(), legacy_db_path.exists());
        
        // Try new RocksDB with TypedDbContext first
        if rocksdb_path.exists() {
            info!("🗄️ Querying markets from RocksDB (TypedDbContext): {}", rocksdb_path.display());
            return self.query_from_rocksdb_typed(&rocksdb_path).await;
        }
        
        // Try file-based storage
        if file_db_path.exists() {
            info!("📁 Querying markets from file-based storage: {}", file_db_path.display());
            return self.query_from_file_store(&file_db_path).await;
        }
        
        // Try legacy RocksDB
        if legacy_db_path.exists() {
            info!("🗄️ Querying markets from legacy RocksDB: {}", legacy_db_path.display());
            let store = TypedStore::open(&legacy_db_path)?;
            if let Some(query) = &self.args.query {
                self.search_markets_in_db(&store, query).await?;
            } else {
                self.list_markets_from_db(&store).await?;
            }
            return Ok(());
        }
        
        // No database found
        error!("❌ No market database found!");
        info!("💡 Checked locations:");
        info!("   - RocksDB: {}", rocksdb_path.display());
        info!("   - File storage: {}", file_db_path.display());
        info!("   - Legacy: {}", legacy_db_path.display());
        info!("");
        info!("💡 Run 'polybot index --rocksdb' to create the database from market data");
        
        Ok(())
    }
    
    async fn query_from_rocksdb_typed(&self, db_path: &std::path::Path) -> Result<()> {
        let db = TypedDbContext::open(db_path, ALL_COLUMN_FAMILIES.to_vec())?;
        
        if let Some(query) = &self.args.query {
            // Search mode
            info!("🔍 Searching markets containing: '{}'", query);
            
            // First try to get by market ID
            if let Some(market) = db.get::<MarketCf>(&query.to_string())? {
                info!("🎯 Found market by ID: {}", query);
                self.display_market_details(&market);
                return Ok(());
            }
            
            // Then search by index
            let mut matching_markets = Vec::new();
            let query_lower = query.to_lowercase();
            
            // Get all market indices
            let indices: Vec<(String, MarketIndex)> = db.scan::<MarketIndexCf>()?;
            
            for (_market_id, index) in indices {
                if index.question_lower.contains(&query_lower) ||
                   index.category_lower.as_ref().map_or(false, |c| c.contains(&query_lower)) ||
                   index.tags_lower.as_ref().map_or(false, |tags| {
                       tags.iter().any(|tag| tag.contains(&query_lower))
                   }) {
                    if let Some(market) = db.get::<MarketCf>(&index.market_id)? {
                        matching_markets.push(market);
                    }
                }
            }
            
            if matching_markets.is_empty() {
                info!("❌ No markets found matching: '{}'", query);
                return Ok(());
            }
            
            info!("✅ Found {} matching markets", matching_markets.len());
            self.display_markets_table(&matching_markets[..self.args.limit.min(matching_markets.len())]);
        } else {
            // List mode
            info!("📋 Listing markets from database...");
            
            let mut markets = Vec::new();
            let all_markets: Vec<(String, RocksDbMarket)> = db.scan::<MarketCf>()?;
            info!("📊 Database scan returned {} markets", all_markets.len());
            
            for (_market_id, market) in all_markets {
                // Apply filters
                if self.args.active_only && !market.active {
                    continue;
                }
                if self.args.closed_only && !market.closed {
                    continue;
                }
                if let Some(ref category_filter) = self.args.category {
                    if !market.category.as_ref().map_or(false, |c| c.to_lowercase().contains(&category_filter.to_lowercase())) {
                        continue;
                    }
                }
                if let Some(min_vol) = self.args.min_volume {
                    if market.volume.unwrap_or(0.0) < min_vol {
                        continue;
                    }
                }
                
                markets.push(market);
            }
            
            // Sort by volume (descending) if available
            markets.sort_by(|a, b| {
                b.volume.unwrap_or(0.0).partial_cmp(&a.volume.unwrap_or(0.0)).unwrap_or(std::cmp::Ordering::Equal)
            });
            
            let limited_markets = &markets[..self.args.limit.min(markets.len())];
            info!("✅ Found {} markets (showing {})", markets.len(), limited_markets.len());
            
            self.display_markets_table(limited_markets);
        }
        
        Ok(())
    }
    
    async fn query_from_file_store(&self, _db_path: &std::path::Path) -> Result<()> {
        // FileStore is for writing, not reading - redirect to RocksDB
        error!("❌ File storage is write-only. Cannot query from file storage.");
        info!("💡 Run 'polybot index --rocksdb' to create a queryable database");
        Ok(())
    }

    async fn search_markets_in_db(&self, store: &TypedStore, query: &str) -> Result<()> {
        // First try to get by market ID
        if let Ok(Some(market)) = store.get::<MarketTable>(&query.to_string()) {
            info!("🎯 Found market by ID: {}", query);
            self.display_market_details(&market);
            return Ok(());
        }

        // Then search by question/category in index
        info!("🔍 Searching markets containing: '{}'", query);
        let indices = store.scan::<MarketIndexTable>()?;
        
        let mut matching_markets = Vec::new();
        let query_lower = query.to_lowercase();
        
        for (_market_id, index) in indices {
            if index.question_lower.contains(&query_lower) ||
               index.category_lower.as_ref().map_or(false, |c| c.contains(&query_lower)) ||
               index.tags_lower.as_ref().map_or(false, |tags| {
                   tags.iter().any(|tag| tag.contains(&query_lower))
               }) {
                if let Ok(Some(market)) = store.get::<MarketTable>(&index.market_id) {
                    matching_markets.push(market);
                }
            }
        }

        if matching_markets.is_empty() {
            info!("❌ No markets found matching: '{}'", query);
            return Ok(());
        }

        info!("✅ Found {} matching markets", matching_markets.len());
        self.display_markets_table(&matching_markets[..self.args.limit.min(matching_markets.len())]);
        
        Ok(())
    }

    async fn list_markets_from_db(&self, store: &TypedStore) -> Result<()> {
        info!("📋 Listing markets from database...");
        
        let mut markets = Vec::new();
        let all_markets = store.scan::<MarketTable>()?;
        
        for (_market_id, market) in all_markets {
            // Apply filters
            if self.args.active_only && !market.active {
                continue;
            }
            if self.args.closed_only && !market.closed {
                continue;
            }
            if let Some(ref category_filter) = self.args.category {
                if !market.category.as_ref().map_or(false, |c| c.to_lowercase().contains(&category_filter.to_lowercase())) {
                    continue;
                }
            }
            if let Some(min_vol) = self.args.min_volume {
                if market.volume.unwrap_or(0.0) < min_vol {
                    continue;
                }
            }
            
            markets.push(market);
        }

        // Sort by volume (descending) if available
        markets.sort_by(|a, b| {
            b.volume.unwrap_or(0.0).partial_cmp(&a.volume.unwrap_or(0.0)).unwrap_or(std::cmp::Ordering::Equal)
        });

        let limited_markets = &markets[..self.args.limit.min(markets.len())];
        info!("✅ Found {} markets (showing {})", markets.len(), limited_markets.len());
        
        self.display_markets_table(limited_markets);
        
        Ok(())
    }

    fn display_markets_table(&self, markets: &[RocksDbMarket]) {
        let mut table = Table::new();
        table.set_content_arrangement(ContentArrangement::Dynamic);
        
        if self.args.detailed {
            table.set_header(vec![
                Cell::new("ID").add_attribute(Attribute::Bold),
                Cell::new("Question").add_attribute(Attribute::Bold),
                Cell::new("Category").add_attribute(Attribute::Bold),
                Cell::new("Status").add_attribute(Attribute::Bold),
                Cell::new("Volume").add_attribute(Attribute::Bold),
                Cell::new("Tokens").add_attribute(Attribute::Bold),
            ]);
        } else {
            table.set_header(vec![
                Cell::new("ID").add_attribute(Attribute::Bold),
                Cell::new("Question").add_attribute(Attribute::Bold),
                Cell::new("Status").add_attribute(Attribute::Bold),
                Cell::new("Volume").add_attribute(Attribute::Bold),
            ]);
        }

        for market in markets {
            let status_cell = if market.active {
                Cell::new("Active").fg(Color::Green)
            } else if market.closed {
                Cell::new("Closed").fg(Color::Red)
            } else {
                Cell::new("Inactive").fg(Color::Yellow)
            };

            let volume_str = market.volume
                .map(|v| format!("${:.0}", v))
                .unwrap_or_else(|| "N/A".to_string());

            let mut row = vec![
                Cell::new(market.id.as_ref().unwrap_or(&"N/A".to_string())),
                Cell::new(truncate_text(&market.question, 50)),
                status_cell,
                Cell::new(volume_str),
            ];

            if self.args.detailed {
                row.insert(2, Cell::new(market.category.as_ref().unwrap_or(&"N/A".to_string())));
                row.push(Cell::new(market.tokens.len().to_string()));
            }

            table.add_row(row);
        }

        println!("{table}");
    }

    fn display_market_details(&self, market: &RocksDbMarket) {
        println!("📊 Market Details");
        println!("═══════════════════════════════════════");
        println!("ID: {}", market.id.as_ref().unwrap_or(&"N/A".to_string()));
        println!("Condition ID: {}", market.condition_id.as_ref().unwrap_or(&"N/A".to_string()));
        println!("Question: {}", market.question);
        if let Some(description) = &market.description {
            println!("Description: {}", description);
        }
        if let Some(category) = &market.category {
            println!("Category: {}", category);
        }
        println!("Status: {}", if market.active { "Active" } else if market.closed { "Closed" } else { "Inactive" });
        if let Some(volume) = market.volume {
            println!("Volume: ${:.2}", volume);
        }
        if let Some(volume_24hr) = market.volume_24hr {
            println!("24h Volume: ${:.2}", volume_24hr);
        }
        
        if !market.tokens.is_empty() {
            println!("\n🎯 Tokens:");
            for token in &market.tokens {
                println!("  • {} ({}): ${:.3}", token.outcome, token.token_id, token.price);
            }
        }
        
        if let Some(outcomes) = &market.outcomes {
            println!("\n📈 Outcomes: {}", outcomes.join(", "));
        }
    }
}

fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}...", &text[..max_len.saturating_sub(3)])
    }
} 