use crate::data_paths::DataPaths;
use crate::markets::file_store::FileStore;
use crate::markets::clob::fetcher::Market;
use crate::tui::ProgressUpdate;
use crate::typed_store::{
    models::{
        Condition, ConditionCf, ConditionIndexCf, ConditionTable, MarketByConditionCf,
        MarketByConditionTable, MarketCf, MarketIndex, MarketIndexCf, MarketIndexTable,
        MarketTable, RocksDbMarket, Token, TokenIndexCf, TokensByConditionCf,
        TokensByConditionTable, ALL_COLUMN_FAMILIES,
    },
    TypedDbContext, TypedStore,
};
use anyhow::Result;
use clap::Args;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

#[derive(Args)]
#[command(about = "Index raw market data into database for fast queries")]
pub struct IndexArgs {
    /// Database path (default: ./data/database)
    #[arg(long, short = 'd')]
    pub db_path: Option<PathBuf>,

    /// Use file-based storage instead of RocksDB
    #[arg(long, default_value = "true")]
    pub use_file_store: bool,

    /// Use RocksDB storage (TypedDbContext with column families)
    #[arg(long)]
    pub rocksdb: bool,

    /// Directory containing market JSON chunks to index
    #[arg(long)]
    pub source_dir: Option<PathBuf>,

    /// Specific chunk files to index (comma-separated)
    #[arg(long)]
    pub chunk_files: Option<String>,

    /// Clear existing database before indexing
    #[arg(long)]
    pub clear: bool,

    /// Skip duplicate markets (based on market_id)
    #[arg(long, default_value = "true")]
    pub skip_duplicates: bool,

    /// Batch size for RocksDB writes
    #[arg(long, default_value = "1000")]
    pub batch_size: usize,

    /// Show detailed progress information
    #[arg(long)]
    pub detailed: bool,

    /// Number of parallel threads for processing (0 = auto-detect)
    #[arg(long, default_value = "0")]
    pub threads: usize,
}

pub struct IndexCommand {
    pub args: IndexArgs,
    pub progress_sender: Option<mpsc::UnboundedSender<ProgressUpdate>>,
}

impl IndexCommand {
    pub fn new(args: IndexArgs) -> Self {
        Self {
            args,
            progress_sender: None,
        }
    }

    pub fn with_progress_sender(mut self, sender: mpsc::UnboundedSender<ProgressUpdate>) -> Self {
        self.progress_sender = Some(sender);
        self
    }

    pub async fn execute(self, _host: &str, data_paths: DataPaths) -> Result<()> {
        // Initialize logging for CLI mode
        crate::logging::init_logging(crate::logging::LoggingConfig::new(
            crate::logging::LogMode::ConsoleAndFile,
            data_paths.clone(),
        ))?;

        // If no parameters provided, show TUI
        if self.args.source_dir.is_none() && self.args.chunk_files.is_none() && !self.args.clear {
            let mut tui = crate::tui::IndexTui::new(data_paths)?;
            return tui.run().await;
        }

        self.execute_internal(&data_paths).await
    }

    pub async fn execute_internal(&self, data_paths: &DataPaths) -> Result<()> {
        if self.args.rocksdb {
            // --rocksdb flag takes priority
            self.execute_rocksdb_typed(data_paths).await
        } else if self.args.use_file_store {
            // Default behavior: file-based storage
            self.execute_file_store(data_paths).await
        } else {
            // Fallback: legacy RocksDB when file store is explicitly disabled
            self.execute_rocksdb_legacy(data_paths).await
        }
    }

    async fn execute_file_store(&self, data_paths: &DataPaths) -> Result<()> {
        info!("🗄️ Starting market data indexing to file-based storage");

        // Determine database path
        let db_path = self
            .args
            .db_path
            .clone()
            .unwrap_or_else(|| PathBuf::from("./data/database"));

        info!("📂 Database path: {}", db_path.display());

        // Clear database if requested
        if self.args.clear && db_path.exists() {
            info!("🗑️ Clearing existing database");
            fs::remove_dir_all(&db_path)?;
        }

        // Create file store
        let store = FileStore::new(db_path)?;
        info!("✅ Created file-based storage");

        // Determine source files
        let chunk_files = self.get_chunk_files(data_paths)?;

        if chunk_files.is_empty() {
            error!("❌ No market chunk files found to index");
            return Ok(());
        }

        info!("📋 Found {} chunk files to index", chunk_files.len());

        // Process each chunk file
        let mut total_markets = 0;
        let mut skipped_markets = 0;

        for (i, chunk_file) in chunk_files.iter().enumerate() {
            info!(
                "🔄 Processing chunk {}/{}: {}",
                i + 1,
                chunk_files.len(),
                chunk_file.display()
            );

            let content = fs::read_to_string(chunk_file)?;

            // Parse markets from various formats
            let markets = if let Ok(market_array) =
                serde_json::from_str::<Vec<serde_json::Value>>(&content)
            {
                market_array
            } else if let Ok(market_obj) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(markets_array) = market_obj.get("markets").and_then(|v| v.as_array()) {
                    markets_array.clone()
                } else if market_obj.is_array() {
                    market_obj.as_array().unwrap().clone()
                } else {
                    vec![market_obj]
                }
            } else {
                error!("❌ Failed to parse market data from file");
                continue;
            };

            let mut chunk_markets = 0;
            let mut chunk_conditions = std::collections::HashSet::new();
            let mut chunk_tokens = 0;

            for market_value in markets {
                match Market::from_value(market_value) {
                    Ok(market) => {
                        // Skip if no condition_id
                        if market
                            .condition_id
                            .as_ref()
                            .map_or(true, |id| id.trim().is_empty())
                        {
                            skipped_markets += 1;
                            continue;
                        }

                        // Store the market
                        if let Err(e) = store.store_market(&market) {
                            warn!("⚠️ Failed to store market: {}", e);
                            continue;
                        }

                        chunk_markets += 1;
                        if let Some(cond_id) = &market.condition_id {
                            chunk_conditions.insert(cond_id.clone());
                        }
                        chunk_tokens += market.tokens.len();
                    }
                    Err(e) => {
                        if self.args.detailed {
                            warn!("⚠️ Failed to parse market: {}", e);
                        }
                        skipped_markets += 1;
                    }
                }
            }

            total_markets += chunk_markets;

            if self.args.detailed {
                info!(
                    "   ✅ Stored {} markets, {} conditions, {} tokens",
                    chunk_markets,
                    chunk_conditions.len(),
                    chunk_tokens
                );
            }
        }

        // Get final stats
        let stats = store.get_stats()?;

        // Final summary
        info!("✅ Indexing completed successfully!");
        info!("📊 Summary:");
        info!("   • Markets processed: {}", total_markets);
        info!("   • Markets skipped: {}", skipped_markets);
        info!("   • Unique conditions: {}", stats.conditions);
        info!("   • Unique tokens: {}", stats.tokens);
        info!("   • Files created:");
        info!("     - Condition directories: {}", stats.conditions);
        info!("     - Token directories: {}", stats.tokens);
        info!("     - Market directories: {}", stats.markets);

        Ok(())
    }

    async fn execute_rocksdb_typed(&self, data_paths: &DataPaths) -> Result<()> {
        // Set up Rayon thread pool
        if self.args.threads > 0 {
            rayon::ThreadPoolBuilder::new()
                .num_threads(self.args.threads)
                .build_global()
                .unwrap_or_else(|e| {
                    if self.progress_sender.is_none() {
                        warn!("Failed to set custom thread pool: {}, using default", e);
                    }
                });
        }

        // Only log to console if not in TUI mode (no progress sender)
        if self.progress_sender.is_none() {
            info!(
                "🚀 Using {} threads for parallel processing",
                rayon::current_num_threads()
            );
            info!("🗄️ Starting market data indexing to RocksDB");
        }

        // Send initial progress update
        if let Some(ref sender) = self.progress_sender {
            let _ = sender.send(ProgressUpdate::PhaseChange(
                crate::tui::IndexingPhase::Starting,
            ));
            let _ = sender.send(ProgressUpdate::Event(
                "🗄️ Starting market data indexing to RocksDB".to_string(),
            ));
        }

        // Determine database path
        let db_path = self
            .args
            .db_path
            .clone()
            .unwrap_or_else(|| PathBuf::from("./data/database/rocksdb"));

        // Only log to console if not in TUI mode
        if self.progress_sender.is_none() {
            info!("📂 Database path: {}", db_path.display());
        } else if let Some(ref sender) = self.progress_sender {
            let _ = sender.send(ProgressUpdate::Event(format!(
                "📂 Database path: {}",
                db_path.display()
            )));
        }

        // Clear database if requested
        if self.args.clear && db_path.exists() {
            if self.progress_sender.is_none() {
                info!("🗑️ Clearing existing database");
            } else if let Some(ref sender) = self.progress_sender {
                let _ = sender.send(ProgressUpdate::Event(
                    "🗑️ Clearing existing database".to_string(),
                ));
            }
            fs::remove_dir_all(&db_path)?;
        }

        // Open TypedDbContext with all column families
        let ctx = TypedDbContext::open(&db_path, ALL_COLUMN_FAMILIES.to_vec())?;
        if self.progress_sender.is_none() {
            info!("✅ Opened TypedDbContext database");
        } else if let Some(ref sender) = self.progress_sender {
            let _ = sender.send(ProgressUpdate::Event(
                "✅ Opened TypedDbContext database".to_string(),
            ));
        }

        // Determine source files
        let chunk_files = self.get_chunk_files(data_paths)?;

        if chunk_files.is_empty() {
            if self.progress_sender.is_none() {
                error!("❌ No market chunk files found to index");
            } else if let Some(ref sender) = self.progress_sender {
                let _ = sender.send(ProgressUpdate::Error(
                    "No market chunk files found to index".to_string(),
                ));
            }
            return Ok(());
        }

        if self.progress_sender.is_none() {
            info!("📋 Found {} chunk files to index", chunk_files.len());
        }

        // Change phase to processing files
        if let Some(ref sender) = self.progress_sender {
            let _ = sender.send(ProgressUpdate::PhaseChange(
                crate::tui::IndexingPhase::ProcessingFiles,
            ));
            let _ = sender.send(ProgressUpdate::Event(format!(
                "📋 Found {} chunk files to index",
                chunk_files.len()
            )));
        }

        // Create progress bars only if no progress sender (console mode)
        let use_progress_bars = self.progress_sender.is_none();
        let multi_progress = if use_progress_bars {
            Some(MultiProgress::new())
        } else {
            None
        };

        let overall_progress = if let Some(ref mp) = multi_progress {
            let bar = mp.add(ProgressBar::new(chunk_files.len() as u64));
            bar.set_style(
                ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} files ({eta})")
                .unwrap()
                .progress_chars("#>-")
            );
            bar.set_message("Processing chunk files");
            Some(bar)
        } else {
            None
        };

        // Thread-safe shared state
        let ctx = Arc::new(ctx);
        let conditions_map = Arc::new(Mutex::new(HashMap::<String, Condition>::new()));
        let tokens_by_condition = Arc::new(Mutex::new(HashMap::<String, Vec<Token>>::new()));
        let progress_sender = self.progress_sender.clone();
        let files_processed = Arc::new(Mutex::new(0usize));
        let total_markets = Arc::new(Mutex::new(0usize));
        let duplicate_markets = Arc::new(Mutex::new(0usize));

        // Process files in parallel
        let chunk_results: Vec<Result<ChunkProcessResult>> = chunk_files
            .par_iter()
            .enumerate()
            .map(|(i, chunk_file)| {
                let file_name = chunk_file
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                // Send progress update for TUI
                if let Some(ref sender) = progress_sender {
                    let _ = sender.send(ProgressUpdate::FileStart {
                        file_index: i + 1,
                        total_files: chunk_files.len(),
                        file_name: file_name.clone(),
                        market_count: 0,
                    });
                }

                // Only log to console if not in TUI mode
                if progress_sender.is_none() {
                    info!("🔄 Processing chunk {}: {}", i + 1, chunk_file.display());
                }

                // Process the chunk
                let result = self.process_chunk_parallel(
                    Arc::clone(&ctx),
                    chunk_file,
                    Arc::clone(&conditions_map),
                    Arc::clone(&tokens_by_condition),
                    &progress_sender,
                );

                // Update progress
                if let Some(ref bar) = overall_progress {
                    let mut processed = files_processed.lock().unwrap();
                    *processed += 1;
                    bar.set_position(*processed as u64);
                    bar.set_message(format!("Processed {} files", *processed));
                }

                // Send file complete update
                if let Ok(ref chunk_result) = result {
                    if let Some(ref sender) = progress_sender {
                        let _ = sender.send(ProgressUpdate::FileComplete {
                            duplicates: chunk_result.duplicates_skipped,
                        });
                    }

                    // Update totals
                    *total_markets.lock().unwrap() += chunk_result.markets_indexed;
                    *duplicate_markets.lock().unwrap() += chunk_result.duplicates_skipped;

                    if self.args.detailed && progress_sender.is_none() {
                        info!(
                            "   ✅ Indexed {} markets ({} duplicates skipped) from {}",
                            chunk_result.markets_indexed,
                            chunk_result.duplicates_skipped,
                            file_name
                        );
                    }
                }

                result
            })
            .collect();

        // Check for errors in parallel processing
        for result in chunk_results {
            result?;
        }

        let total_markets = *total_markets.lock().unwrap();
        let duplicate_markets = *duplicate_markets.lock().unwrap();
        let conditions_map = Arc::try_unwrap(conditions_map)
            .unwrap()
            .into_inner()
            .unwrap();
        let tokens_by_condition = Arc::try_unwrap(tokens_by_condition)
            .unwrap()
            .into_inner()
            .unwrap();

        // Complete the overall progress
        if let Some(bar) = overall_progress {
            bar.set_position(chunk_files.len() as u64);
            bar.finish_with_message("All files processed");
        }

        // Index aggregated conditions and tokens using batch operations
        if self.progress_sender.is_none() {
            info!("🔄 Indexing {} unique conditions", conditions_map.len());
        }

        // Send phase change for conditions
        if let Some(ref sender) = self.progress_sender {
            let _ = sender.send(ProgressUpdate::PhaseChange(
                crate::tui::IndexingPhase::IndexingConditions,
            ));
            let _ = sender.send(ProgressUpdate::ConditionCount(conditions_map.len()));
        }

        // Progress bar for conditions (only in console mode)
        let conditions_progress = if let Some(ref mp) = multi_progress {
            let bar = mp.add(ProgressBar::new(conditions_map.len() as u64));
            bar.set_style(
                ProgressStyle::default_bar()
                    .template("  {spinner:.cyan} [{bar:30.blue}] {pos}/{len} conditions")
                    .unwrap()
                    .progress_chars("=>-"),
            );
            Some(bar)
        } else {
            None
        };

        // Keep ctx as Arc for now
        let mut total_conditions = 0;
        let mut total_tokens = 0;

        // Batch conditions for parallel processing
        let conditions_vec: Vec<_> = conditions_map.into_iter().map(|(_, v)| v).collect();
        let batch_size = 1000;

        for (batch_idx, conditions_batch) in conditions_vec.chunks(batch_size).enumerate() {
            ctx.batch_write(|batch| {
                for condition in conditions_batch {
                    batch.put::<ConditionCf>(&condition.id, condition)?;
                    total_conditions += 1;
                }
                Ok(())
            })?;

            if let Some(ref bar) = conditions_progress {
                bar.set_position(((batch_idx + 1) * batch_size).min(conditions_vec.len()) as u64);
            }
        }

        if let Some(bar) = conditions_progress {
            bar.finish_with_message("✅ Conditions indexed");
        }

        info!("🔄 Indexing tokens by condition");

        // Send phase change for tokens
        if let Some(ref sender) = self.progress_sender {
            let _ = sender.send(ProgressUpdate::PhaseChange(
                crate::tui::IndexingPhase::IndexingTokens,
            ));
            let token_count: usize = tokens_by_condition.values().map(|v| v.len()).sum();
            let _ = sender.send(ProgressUpdate::TokenCount(token_count));
        }

        // Progress bar for tokens by condition (only in console mode)
        let tokens_progress = if let Some(ref mp) = multi_progress {
            let bar = mp.add(ProgressBar::new(tokens_by_condition.len() as u64));
            bar.set_style(
                ProgressStyle::default_bar()
                    .template("  {spinner:.magenta} [{bar:30.cyan}] {pos}/{len} token groups")
                    .unwrap()
                    .progress_chars("=>-"),
            );
            Some(bar)
        } else {
            None
        };

        // Batch tokens for parallel processing
        let tokens_vec: Vec<_> = tokens_by_condition.into_iter().collect();
        let batch_size = 1000;

        for (batch_idx, tokens_batch) in tokens_vec.chunks(batch_size).enumerate() {
            ctx.batch_write(|batch| {
                for (condition_id, tokens) in tokens_batch {
                    batch.put::<TokensByConditionCf>(condition_id, tokens)?;
                    total_tokens += tokens.len();
                }
                Ok(())
            })?;

            if let Some(ref bar) = tokens_progress {
                bar.set_position(((batch_idx + 1) * batch_size).min(tokens_vec.len()) as u64);
            }
        }

        if let Some(bar) = tokens_progress {
            bar.finish_with_message("✅ Token groups indexed");
        }

        // Send finalizing phase
        if let Some(ref sender) = self.progress_sender {
            let _ = sender.send(ProgressUpdate::PhaseChange(
                crate::tui::IndexingPhase::Finalizing,
            ));
        }

        // Final summary - only log to console if not in TUI mode
        if self.progress_sender.is_none() {
            info!("✅ Indexing completed successfully!");
            info!("📊 Summary:");
            info!("   • Markets indexed: {}", total_markets);
            info!("   • Conditions indexed: {}", total_conditions);
            info!("   • Tokens indexed: {}", total_tokens);
            info!("   • Duplicates skipped: {}", duplicate_markets);
            info!(
                "   • Database size: ~{:.2} MB",
                self.estimate_db_size(&db_path)?
            );
        }

        // Send completion
        if let Some(ref sender) = self.progress_sender {
            let _ = sender.send(ProgressUpdate::PhaseChange(
                crate::tui::IndexingPhase::Completed,
            ));
            let _ = sender.send(ProgressUpdate::Event(format!(
                "✅ Indexed {} markets, {} conditions, {} tokens",
                total_markets, total_conditions, total_tokens
            )));
            let _ = sender.send(ProgressUpdate::Complete);
        }

        Ok(())
    }

    async fn execute_rocksdb_legacy(&self, data_paths: &DataPaths) -> Result<()> {
        info!("🗄️ Starting market data indexing to RocksDB (legacy TypedStore)");

        // Determine database path
        let db_path = self
            .args
            .db_path
            .clone()
            .unwrap_or_else(|| data_paths.datasets().join("markets.db"));

        info!("📂 Database path: {}", db_path.display());

        // Clear database if requested
        if self.args.clear && db_path.exists() {
            info!("🗑️ Clearing existing database");
            fs::remove_dir_all(&db_path)?;
        }

        // Open RocksDB store
        let store = TypedStore::open(&db_path)?;
        info!("✅ Opened RocksDB database");

        // Determine source files
        let chunk_files = self.get_chunk_files(data_paths)?;

        if chunk_files.is_empty() {
            error!("❌ No market chunk files found to index");
            return Ok(());
        }

        info!("📋 Found {} chunk files to index", chunk_files.len());

        // Index each chunk file
        let mut total_markets = 0;
        let mut total_conditions = 0;
        let mut total_tokens = 0;
        let mut duplicate_markets = 0;

        let mut conditions_map: HashMap<String, Condition> = HashMap::new();
        let mut tokens_by_condition: HashMap<String, Vec<Token>> = HashMap::new();

        for (i, chunk_file) in chunk_files.iter().enumerate() {
            info!(
                "🔄 Processing chunk {}/{}: {}",
                i + 1,
                chunk_files.len(),
                chunk_file.display()
            );

            let chunk_result = self
                .process_chunk(
                    &store,
                    chunk_file,
                    &mut conditions_map,
                    &mut tokens_by_condition,
                )
                .await?;

            total_markets += chunk_result.markets_indexed;
            duplicate_markets += chunk_result.duplicates_skipped;

            if self.args.detailed {
                info!(
                    "   ✅ Indexed {} markets ({} duplicates skipped)",
                    chunk_result.markets_indexed, chunk_result.duplicates_skipped
                );
            }
        }

        // Index aggregated conditions and tokens
        info!("🔄 Indexing {} unique conditions", conditions_map.len());
        for condition in conditions_map.values() {
            store.put::<ConditionTable>(&condition.id, condition)?;
            total_conditions += 1;
        }

        info!("🔄 Indexing tokens by condition");
        for (condition_id, tokens) in tokens_by_condition.iter() {
            store.put::<TokensByConditionTable>(condition_id, tokens)?;
            total_tokens += tokens.len();
        }

        // Final summary
        info!("✅ Indexing completed successfully!");
        info!("📊 Summary:");
        info!("   • Markets indexed: {}", total_markets);
        info!("   • Conditions indexed: {}", total_conditions);
        info!("   • Tokens indexed: {}", total_tokens);
        info!("   • Duplicates skipped: {}", duplicate_markets);
        info!(
            "   • Database size: ~{:.2} MB",
            self.estimate_db_size(&db_path)?
        );

        Ok(())
    }

    fn get_chunk_files(&self, data_paths: &DataPaths) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        if let Some(ref chunk_files_str) = self.args.chunk_files {
            // Parse specific chunk files
            for file_str in chunk_files_str.split(',') {
                let file_path = PathBuf::from(file_str.trim());
                if file_path.exists() {
                    files.push(file_path);
                } else {
                    warn!("⚠️ Chunk file not found: {}", file_path.display());
                }
            }
        } else {
            // Auto-discover chunk files
            let source_dir = self
                .args
                .source_dir
                .clone()
                .unwrap_or_else(|| data_paths.datasets());

            if !source_dir.exists() {
                error!(
                    "❌ Source directory does not exist: {}",
                    source_dir.display()
                );
                return Ok(files);
            }

            // Find all JSON chunk files
            for entry in fs::read_dir(&source_dir)? {
                let entry = entry?;
                let path = entry.path();

                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    if file_name.starts_with("markets_chunk_") && file_name.ends_with(".json") {
                        files.push(path);
                    }
                }
            }

            // Sort files for consistent processing order
            files.sort();
        }

        Ok(files)
    }

    async fn process_chunk(
        &self,
        store: &TypedStore,
        chunk_file: &PathBuf,
        conditions_map: &mut HashMap<String, Condition>,
        tokens_by_condition: &mut HashMap<String, Vec<Token>>,
    ) -> Result<ChunkProcessResult> {
        let content = fs::read_to_string(chunk_file)?;

        // Try to parse as an array first (for chunk files), then as an object (for markets.json)
        let markets =
            if let Ok(market_array) = serde_json::from_str::<Vec<serde_json::Value>>(&content) {
                market_array
            } else if let Ok(market_obj) = serde_json::from_str::<serde_json::Value>(&content) {
                // For markets.json files that might have a wrapper object
                if let Some(markets_array) = market_obj.get("markets").and_then(|v| v.as_array()) {
                    markets_array.clone()
                } else if market_obj.is_array() {
                    market_obj.as_array().unwrap().clone()
                } else {
                    // Single market object
                    vec![market_obj]
                }
            } else {
                return Err(anyhow::anyhow!("Failed to parse market data from file"));
            };

        let mut markets_indexed = 0;
        let mut duplicates_skipped = 0;
        let mut batch_markets = Vec::new();
        let mut batch_indices = Vec::new();

        for market_value in markets {
            // Convert to strongly typed market
            let market = match Market::from_value(market_value) {
                Ok(m) => m,
                Err(e) => {
                    warn!("⚠️ Failed to parse market: {}", e);
                    continue;
                }
            };

            // Skip if no market ID or condition ID
            let market_id = match &market.id {
                Some(id) if !id.trim().is_empty() => id.clone(),
                _ => continue,
            };

            let _condition_id = match &market.condition_id {
                Some(id) if !id.trim().is_empty() => id.clone(),
                _ => continue,
            };

            // Check for duplicates if enabled
            if self.args.skip_duplicates {
                if store.exists::<MarketTable>(&market_id)? {
                    duplicates_skipped += 1;
                    continue;
                }
            }

            // Convert to RocksDB format
            let rocks_market = RocksDbMarket::from(market);

            // Extract condition data
            if let Some(condition) = rocks_market.extract_condition() {
                // Aggregate condition data (increment market count if exists)
                conditions_map
                    .entry(condition.id.clone())
                    .and_modify(|existing| existing.market_count += 1)
                    .or_insert(condition);
            }

            // Extract and group tokens by condition
            let tokens = rocks_market.extract_tokens();
            for token in tokens {
                if let Some(cond_id) = &token.condition_id {
                    tokens_by_condition
                        .entry(cond_id.clone())
                        .or_insert_with(Vec::new)
                        .push(token);
                }
            }

            // Add to batch
            batch_markets.push((market_id.clone(), rocks_market.clone()));

            if let Some(index) = rocks_market.create_index() {
                batch_indices.push((market_id.clone(), index));
            }

            // Write batch if full
            if batch_markets.len() >= self.args.batch_size {
                self.write_batch(store, &batch_markets, &batch_indices)?;
                markets_indexed += batch_markets.len();
                batch_markets.clear();
                batch_indices.clear();
            }
        }

        // Write remaining batch
        if !batch_markets.is_empty() {
            self.write_batch(store, &batch_markets, &batch_indices)?;
            markets_indexed += batch_markets.len();
        }

        Ok(ChunkProcessResult {
            markets_indexed,
            duplicates_skipped,
        })
    }

    fn process_chunk_parallel(
        &self,
        ctx: Arc<TypedDbContext>,
        chunk_file: &PathBuf,
        conditions_map: Arc<Mutex<HashMap<String, Condition>>>,
        tokens_by_condition: Arc<Mutex<HashMap<String, Vec<Token>>>>,
        progress_sender: &Option<mpsc::UnboundedSender<ProgressUpdate>>,
    ) -> Result<ChunkProcessResult> {
        // Send progress update instead of console logging
        if let Some(ref sender) = progress_sender {
            let _ = sender.send(ProgressUpdate::Event(format!(
                "📄 Processing file: {}",
                chunk_file.file_name().unwrap_or_default().to_string_lossy()
            )));
        }
        let content = fs::read_to_string(chunk_file)?;

        // Parse markets
        let markets =
            if let Ok(market_array) = serde_json::from_str::<Vec<serde_json::Value>>(&content) {
                market_array
            } else if let Ok(market_obj) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(markets_array) = market_obj.get("markets").and_then(|v| v.as_array()) {
                    markets_array.clone()
                } else if market_obj.is_array() {
                    market_obj.as_array().unwrap().clone()
                } else {
                    vec![market_obj]
                }
            } else {
                return Err(anyhow::anyhow!("Failed to parse market data from file"));
            };

        // Update progress with market count via channel only
        if let Some(ref sender) = progress_sender {
            let _ = sender.send(ProgressUpdate::Event(format!(
                "📊 Parsed {} markets from file",
                markets.len()
            )));
        }

        // Process markets in parallel chunks
        let chunk_size = 100; // Process 100 markets at a time
        let mut markets_indexed = 0;
        let mut duplicates_skipped = 0;

        for (chunk_idx, chunk) in markets.chunks(chunk_size).enumerate() {
            // Send progress update via channel instead of console logging
            if let Some(ref sender) = progress_sender {
                let _ = sender.send(ProgressUpdate::Event(format!(
                    "🔄 Processing chunk {} with {} markets",
                    chunk_idx + 1,
                    chunk.len()
                )));
            }
            // Process chunk of markets in parallel
            let results: Vec<_> = chunk
                .par_iter()
                .filter_map(|market_value| {
                    // Convert to strongly typed market
                    let market = match serde_json::from_value::<Market>(market_value.clone()) {
                        Ok(m) => {
                            debug!("✅ Successfully parsed market with id: {:?}", m.id);
                            m
                        }
                        Err(e) => {
                            warn!("⚠️ Failed to parse market: {}", e);
                            if let Some(id) = market_value.get("id") {
                                warn!("   Market ID field: {:?}", id);
                            }
                            return None;
                        }
                    };

                    // Skip if no market ID or condition ID
                    let market_id = match &market.id {
                        Some(id) if !id.trim().is_empty() => id.clone(),
                        _ => {
                            // Use condition_id as fallback for market_id if available
                            match &market.condition_id {
                                Some(cid) if !cid.trim().is_empty() => {
                                    debug!("📝 Using condition_id as market_id: {}", cid);
                                    format!("market_{}", cid)
                                }
                                _ => {
                                    warn!("⚠️ Skipping market: no valid market ID or condition ID");
                                    return None;
                                }
                            }
                        }
                    };

                    let condition_id = match &market.condition_id {
                        Some(id) if !id.trim().is_empty() => id.clone(),
                        _ => {
                            warn!("⚠️ Skipping market {}: no valid condition ID", market_id);
                            return None;
                        }
                    };

                    // Check for duplicates
                    if self.args.skip_duplicates {
                        if ctx.exists::<MarketCf>(&market_id).unwrap_or(false) {
                            return Some(Err(()));
                        }
                    }

                    // Convert to RocksDB format
                    let rocks_market = RocksDbMarket::from(market);

                    // Extract condition and tokens
                    let condition = rocks_market.extract_condition();
                    let tokens = rocks_market.extract_tokens();
                    let index = rocks_market.create_index();

                    Some(Ok((
                        market_id,
                        condition_id,
                        rocks_market,
                        condition,
                        tokens,
                        index,
                    )))
                })
                .collect();

            // Batch write to database
            let mut batch_markets = Vec::new();
            let mut batch_indices = Vec::new();

            for result in results {
                match result {
                    Ok((market_id, condition_id, rocks_market, condition, tokens, index)) => {
                        batch_markets.push((market_id.clone(), condition_id, rocks_market));
                        if let Some(idx) = index {
                            batch_indices.push((market_id, idx));
                        }

                        // Update conditions map
                        if let Some(cond) = condition {
                            let mut conditions = conditions_map.lock().unwrap();
                            conditions
                                .entry(cond.id.clone())
                                .and_modify(|existing| existing.market_count += 1)
                                .or_insert(cond);
                        }

                        // Update tokens map
                        let mut tokens_map = tokens_by_condition.lock().unwrap();
                        for token in tokens {
                            if let Some(cond_id) = &token.condition_id {
                                tokens_map
                                    .entry(cond_id.clone())
                                    .or_insert_with(Vec::new)
                                    .push(token);
                            }
                        }
                    }
                    Err(_) => {
                        duplicates_skipped += 1;
                    }
                }
            }

            // Write batch to database
            if !batch_markets.is_empty() {
                // Send progress update via channel instead of console logging
                if let Some(ref sender) = progress_sender {
                    let _ = sender.send(ProgressUpdate::Event(format!(
                        "📝 Writing batch of {} markets to database",
                        batch_markets.len()
                    )));
                }
                ctx.batch_write(|batch| {
                    for (market_id, condition_id, market) in &batch_markets {
                        batch.put::<MarketCf>(market_id, market)?;
                        batch.put::<MarketByConditionCf>(&condition_id, market)?;

                        // Update indices
                        if let Some(ref cond_id) = market.condition_id {
                            let token_ids: Vec<String> =
                                market.tokens.iter().map(|t| t.token_id.clone()).collect();

                            for token in &market.tokens {
                                batch.put::<TokenIndexCf>(&token.token_id, cond_id)?;
                            }

                            batch.put::<ConditionIndexCf>(cond_id, &token_ids)?;
                        }
                    }

                    for (market_id, index) in &batch_indices {
                        batch.put::<MarketIndexCf>(market_id, index)?;
                    }

                    Ok(())
                })?;

                markets_indexed += batch_markets.len();
            }

            // Send progress update
            if let Some(ref sender) = progress_sender {
                let _ = sender.send(ProgressUpdate::MarketProcessed {
                    markets_in_batch: batch_markets.len(),
                });
            }
        }

        Ok(ChunkProcessResult {
            markets_indexed,
            duplicates_skipped,
        })
    }

    fn write_batch(
        &self,
        store: &TypedStore,
        markets: &[(String, RocksDbMarket)],
        indices: &[(String, MarketIndex)],
    ) -> Result<()> {
        store.batch_write(|batch| {
            // Write markets by ID
            for (market_id, market) in markets {
                batch.put::<MarketTable>(market_id, market)?;

                // Also index by condition_id for fast condition-based queries
                if let Some(ref condition_id) = market.condition_id {
                    batch.put::<MarketByConditionTable>(condition_id, market)?;
                }
            }

            // Write search indices
            for (market_id, index) in indices {
                batch.put::<MarketIndexTable>(market_id, index)?;
            }

            Ok(())
        })?;

        Ok(())
    }

    fn estimate_db_size(&self, db_path: &PathBuf) -> Result<f64> {
        if !db_path.exists() {
            return Ok(0.0);
        }

        let mut size = 0u64;
        fn visit_dir(dir: &PathBuf, size: &mut u64) -> Result<()> {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    visit_dir(&path, size)?;
                } else {
                    *size += entry.metadata()?.len();
                }
            }
            Ok(())
        }

        visit_dir(db_path, &mut size)?;
        Ok(size as f64 / 1024.0 / 1024.0) // MB
    }
}

#[derive(Debug)]
struct ChunkProcessResult {
    markets_indexed: usize,
    duplicates_skipped: usize,
}
