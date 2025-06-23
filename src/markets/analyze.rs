use anyhow::{anyhow, Result};
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use crate::cli::commands::analyze::AnalyzeArgs;
use crate::data_paths::DataPaths;
use crate::datasets::save_command_metadata;

/// Market analysis configuration and execution engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketAnalyzer {
    /// Name for the output dataset
    pub dataset_name: String,
    /// Source dataset path or name
    pub source_dataset: String,
    /// Analysis filters
    pub filters: AnalysisFilters,
    /// Output configuration
    pub output_config: OutputConfig,
}

/// Analysis filters for market data
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnalysisFilters {
    /// Minimum price for YES outcome (0-100)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_price: Option<f64>,
    /// Maximum price for YES outcome (0-100)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_price: Option<f64>,
    /// Filter: only active markets
    #[serde(default)]
    pub active_only: bool,
    /// Filter: only markets accepting orders
    #[serde(default)]
    pub accepting_orders_only: bool,
    /// Filter: only open markets (not closed)
    #[serde(default)]
    pub open_only: bool,
    /// Filter: exclude archived markets
    #[serde(default)]
    pub no_archived: bool,
    /// Filter: markets by category (comma-separated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<Vec<String>>,
    /// Filter: markets by tags (comma-separated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Filter: minimum order size
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_order_size: Option<f64>,
    /// Filter: created after date (ISO format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_after: Option<String>,
    /// Filter: ending before date (ISO format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ending_before: Option<String>,
    /// Filter: title must contain this text (case-insensitive)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_contains: Option<String>,
    /// Filter: title must match this regex pattern
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_regex: Option<String>,
    /// Filter: title must contain ANY of these keywords
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_contains_any: Option<Vec<String>>,
    /// Filter: title must contain ALL of these keywords
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_contains_all: Option<Vec<String>>,
    /// Filter: description must contain this text (case-insensitive)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_contains: Option<String>,
    /// Filter: fuzzy search in title and description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fuzzy_search: Option<String>,
    /// Filter: fuzzy search threshold (0.0-1.0)
    #[serde(default = "default_fuzzy_threshold")]
    pub fuzzy_threshold: f64,
    /// Filter: search in all text fields (title, description, tags)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_search: Option<String>,
}

fn default_fuzzy_threshold() -> f64 {
    0.7
}

/// Output configuration for analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Include detailed analysis in output
    #[serde(default)]
    pub detailed: bool,
    /// Show analysis summary
    #[serde(default)]
    pub summary: bool,
}

/// Analysis results and statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// Total markets processed
    pub total_markets: usize,
    /// Markets that passed filters
    pub filtered_markets: usize,
    /// Analysis execution time in milliseconds
    pub execution_time_ms: u64,
    /// Path to output dataset
    pub output_path: PathBuf,
    /// Summary statistics
    pub statistics: AnalysisStatistics,
}

/// Statistical summary of the analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisStatistics {
    /// Count of active markets
    pub active_count: usize,
    /// Count of markets accepting orders
    pub accepting_orders_count: usize,
    /// Count of closed markets
    pub closed_count: usize,
    /// Price distribution by range (0-10%, 10-20%, etc.)
    pub price_distribution: Vec<usize>,
    /// Category breakdown (top 10)
    pub top_categories: Vec<(String, usize)>,
}

impl MarketAnalyzer {
    /// Create a new MarketAnalyzer from CLI arguments
    pub fn from_args(args: AnalyzeArgs) -> Self {
        let categories = args
            .categories
            .map(|s| s.split(',').map(|c| c.trim().to_string()).collect());

        let tags = args
            .tags
            .map(|s| s.split(',').map(|t| t.trim().to_string()).collect());

        let title_contains_any = args
            .title_contains_any
            .map(|s| s.split(',').map(|k| k.trim().to_string()).collect());

        let title_contains_all = args
            .title_contains_all
            .map(|s| s.split(',').map(|k| k.trim().to_string()).collect());

        Self {
            dataset_name: args.dataset_name,
            source_dataset: args.source_dataset,
            filters: AnalysisFilters {
                min_price: args.min_price,
                max_price: args.max_price,
                active_only: args.active_only,
                accepting_orders_only: args.accepting_orders_only,
                open_only: args.open_only,
                no_archived: args.no_archived,
                categories,
                tags,
                min_order_size: args.min_order_size,
                created_after: args.created_after,
                ending_before: args.ending_before,
                title_contains: args.title_contains,
                title_regex: args.title_regex,
                title_contains_any,
                title_contains_all,
                description_contains: args.description_contains,
                fuzzy_search: args.fuzzy_search,
                fuzzy_threshold: args.fuzzy_threshold,
                text_search: args.text_search,
            },
            output_config: OutputConfig {
                detailed: args.detailed,
                summary: args.summary,
            },
        }
    }

    /// Execute the market analysis
    pub async fn execute(&self, data_paths: &DataPaths) -> Result<AnalysisResult> {
        let start_time = std::time::Instant::now();

        info!(
            "🔍 Analyzing markets from dataset '{}'...",
            self.source_dataset
        );

        // Resolve source dataset path
        let source_path = self.resolve_source_path(data_paths)?;
        info!("📂 Reading from: {}", source_path.display());

        // Create output dataset directory
        let output_path = data_paths.datasets().join(&self.dataset_name);
        fs::create_dir_all(&output_path)?;

        // Load and process markets
        let markets = self.load_markets(&source_path).await?;
        let filtered_markets = self.apply_filters(&markets)?;

        if filtered_markets.is_empty() {
            warn!("⚠️  No markets matched the filters");
            return Ok(AnalysisResult {
                total_markets: markets.len(),
                filtered_markets: 0,
                execution_time_ms: start_time.elapsed().as_millis() as u64,
                output_path,
                statistics: AnalysisStatistics::default(),
            });
        }

        // Save results
        self.save_results(&output_path, &filtered_markets).await?;

        // Calculate statistics
        let statistics = self.calculate_statistics(&filtered_markets);

        // Save command metadata
        self.save_command_metadata(&output_path, markets.len(), filtered_markets.len())?;

        let execution_time = start_time.elapsed().as_millis() as u64;

        // Show summary
        self.display_results(markets.len(), filtered_markets.len(), &statistics);

        info!("📁 Saved to: {}", output_path.display());

        Ok(AnalysisResult {
            total_markets: markets.len(),
            filtered_markets: filtered_markets.len(),
            execution_time_ms: execution_time,
            output_path,
            statistics,
        })
    }

    /// Resolve the source dataset path
    fn resolve_source_path(&self, data_paths: &DataPaths) -> Result<PathBuf> {
        let source_path =
            if self.source_dataset.starts_with('/') || self.source_dataset.starts_with("./") {
                // Absolute or relative path provided
                PathBuf::from(&self.source_dataset)
            } else {
                // Dataset name provided, look in datasets directory
                data_paths.datasets().join(&self.source_dataset)
            };

        if !source_path.exists() {
            return Err(anyhow!(
                "Source dataset not found: {}",
                source_path.display()
            ));
        }

        Ok(source_path)
    }

    /// Load markets from the source dataset
    async fn load_markets(&self, source_path: &Path) -> Result<Vec<Value>> {
        let mut all_markets = Vec::new();

        // Find data files in the source dataset
        let data_files = self.find_data_files(source_path)?;

        if data_files.is_empty() {
            return Err(anyhow!(
                "No market data files found in source dataset: {}",
                source_path.display()
            ));
        }

        info!("📊 Found {} data files to analyze", data_files.len());

        // Read and process all data files
        for file_path in data_files {
            debug!(
                "Processing: {}",
                file_path.file_name().unwrap().to_string_lossy()
            );

            let markets = self.read_market_file(&file_path)?;
            all_markets.extend(markets);
        }

        Ok(all_markets)
    }

    /// Find data files in the source dataset directory
    fn find_data_files(&self, source_path: &Path) -> Result<Vec<PathBuf>> {
        let mut data_files = Vec::new();

        let entries = fs::read_dir(source_path)?;
        for entry in entries.filter_map(|e| e.ok()) {
            let file_path = entry.path();
            if let Some(file_name) = file_path.file_name().and_then(|n| n.to_str()) {
                // Look for data files (JSON files that contain market data)
                if file_name.ends_with(".json")
                    && !file_name.starts_with('.')
                    && file_name != "metadata.json"
                    && file_name != "dataset.yaml"
                {
                    data_files.push(file_path);
                }
            }
        }

        data_files.sort();
        Ok(data_files)
    }

    /// Read and parse a market data file
    fn read_market_file(&self, file_path: &Path) -> Result<Vec<Value>> {
        let mut file = File::open(file_path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        // Try to parse as array of markets or single market
        let markets = if let Ok(array) = serde_json::from_str::<Vec<Value>>(&contents) {
            array
        } else if let Ok(single) = serde_json::from_str::<Value>(&contents) {
            vec![single]
        } else {
            warn!("⚠️  Skipping invalid JSON file: {}", file_path.display());
            Vec::new()
        };

        Ok(markets)
    }

    /// Apply filters to markets
    fn apply_filters(&self, markets: &[Value]) -> Result<Vec<Value>> {
        let mut filtered = Vec::new();
        let total = markets.len();

        for market in markets {
            if self.market_passes_filters(market)? {
                filtered.push(market.clone());
            }
        }

        info!(
            "📊 Loaded {} markets, filtered to {}",
            total,
            filtered.len()
        );
        Ok(filtered)
    }

    /// Check if a market passes all filters
    fn market_passes_filters(&self, market: &Value) -> Result<bool> {
        // Active filter
        if self.filters.active_only {
            if !market
                .get("active")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                return Ok(false);
            }
        }

        // Accepting orders filter
        if self.filters.accepting_orders_only {
            if !market
                .get("accepting_orders")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                return Ok(false);
            }
        }

        // Open filter (not closed)
        if self.filters.open_only {
            if market
                .get("closed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                return Ok(false);
            }
        }

        // Archived filter
        if self.filters.no_archived {
            if market
                .get("archived")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                return Ok(false);
            }
        }

        // Price filters
        if let Some(min_price) = self.filters.min_price {
            if !self.check_price_filter(market, min_price, true)? {
                return Ok(false);
            }
        }

        if let Some(max_price) = self.filters.max_price {
            if !self.check_price_filter(market, max_price, false)? {
                return Ok(false);
            }
        }

        // Category filter
        if let Some(ref categories) = self.filters.categories {
            if !self.check_category_filter(market, categories)? {
                return Ok(false);
            }
        }

        // Tags filter
        if let Some(ref tags) = self.filters.tags {
            if !self.check_tags_filter(market, tags)? {
                return Ok(false);
            }
        }

        // Minimum order size filter
        if let Some(min_order_size) = self.filters.min_order_size {
            let market_min_order = market
                .get("minimum_order_size")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            if market_min_order > min_order_size {
                return Ok(false);
            }
        }

        // Date filters
        if let Some(ref ending_before) = self.filters.ending_before {
            if !self.check_date_filter(market, ending_before)? {
                return Ok(false);
            }
        }

        // Title filters
        if let Some(ref title_contains) = self.filters.title_contains {
            if !self.check_title_filter(market, title_contains)? {
                return Ok(false);
            }
        }

        // Title regex filter
        if let Some(ref title_regex) = self.filters.title_regex {
            if !self.check_title_regex_filter(market, title_regex)? {
                return Ok(false);
            }
        }

        // Title contains any keywords filter
        if let Some(ref title_contains_any) = self.filters.title_contains_any {
            if !self.check_title_contains_any_filter(market, title_contains_any)? {
                return Ok(false);
            }
        }

        // Title contains all keywords filter
        if let Some(ref title_contains_all) = self.filters.title_contains_all {
            if !self.check_title_contains_all_filter(market, title_contains_all)? {
                return Ok(false);
            }
        }

        // Description filter
        if let Some(ref description_contains) = self.filters.description_contains {
            if !self.check_description_filter(market, description_contains)? {
                return Ok(false);
            }
        }

        // Fuzzy search filter
        if let Some(ref fuzzy_search) = self.filters.fuzzy_search {
            if !self.check_fuzzy_search_filter(
                market,
                fuzzy_search,
                self.filters.fuzzy_threshold,
            )? {
                return Ok(false);
            }
        }

        // Text search filter
        if let Some(ref text_search) = self.filters.text_search {
            if !self.check_text_search_filter(market, text_search)? {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Check price filter for YES token
    fn check_price_filter(&self, market: &Value, threshold: f64, is_min: bool) -> Result<bool> {
        if let Some(tokens) = market.get("tokens").and_then(|v| v.as_array()) {
            let yes_price = tokens
                .iter()
                .find(|t| {
                    t.get("outcome")
                        .and_then(|o| o.as_str())
                        .map(|s| s.to_lowercase() == "yes")
                        .unwrap_or(false)
                })
                .or_else(|| tokens.get(0))
                .and_then(|t| t.get("price"))
                .and_then(|p| p.as_f64());

            if let Some(price) = yes_price {
                return Ok(if is_min {
                    price >= threshold
                } else {
                    price <= threshold
                });
            }
        }
        Ok(true)
    }

    /// Check category filter
    fn check_category_filter(&self, market: &Value, categories: &[String]) -> Result<bool> {
        let market_category = market
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();

        Ok(categories
            .iter()
            .any(|cat| cat.to_lowercase() == market_category))
    }

    /// Check tags filter
    fn check_tags_filter(&self, market: &Value, required_tags: &[String]) -> Result<bool> {
        if let Some(market_tags) = market.get("tags").and_then(|v| v.as_array()) {
            let market_tag_strings: Vec<String> = market_tags
                .iter()
                .filter_map(|t| t.as_str())
                .map(|s| s.to_lowercase())
                .collect();

            // Check if all required tags are present
            for required_tag in required_tags {
                if !market_tag_strings.contains(&required_tag.to_lowercase()) {
                    return Ok(false);
                }
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Check date filter
    fn check_date_filter(&self, market: &Value, ending_before: &str) -> Result<bool> {
        if let Some(end_date) = market.get("end_date_iso").and_then(|v| v.as_str()) {
            if let (Ok(market_end), Ok(filter_date)) = (
                DateTime::parse_from_rfc3339(end_date),
                DateTime::parse_from_rfc3339(ending_before),
            ) {
                return Ok(market_end < filter_date);
            }
        }
        Ok(true)
    }

    /// Check if market title matches (also check question field as fallback)
    fn get_market_title(&self, market: &Value) -> String {
        market
            .get("question")
            .or_else(|| market.get("title"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase()
    }

    /// Check title filter
    fn check_title_filter(&self, market: &Value, title_contains: &str) -> Result<bool> {
        let title = self.get_market_title(market);
        Ok(title.contains(&title_contains.to_lowercase()))
    }

    /// Check title regex filter (using simple pattern matching for now)
    fn check_title_regex_filter(&self, market: &Value, pattern: &str) -> Result<bool> {
        let title = self.get_market_title(market);
        // For now, just do a simple contains check
        // TODO: Add proper regex support when regex crate is added
        Ok(title.contains(&pattern.to_lowercase()))
    }

    /// Check title contains any keywords filter
    fn check_title_contains_any_filter(&self, market: &Value, keywords: &[String]) -> Result<bool> {
        let title = self.get_market_title(market);
        Ok(keywords
            .iter()
            .any(|keyword| title.contains(&keyword.to_lowercase())))
    }

    /// Check title contains all keywords filter
    fn check_title_contains_all_filter(&self, market: &Value, keywords: &[String]) -> Result<bool> {
        let title = self.get_market_title(market);
        Ok(keywords
            .iter()
            .all(|keyword| title.contains(&keyword.to_lowercase())))
    }

    /// Check description filter
    fn check_description_filter(&self, market: &Value, description_contains: &str) -> Result<bool> {
        let description = market
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();

        Ok(description.contains(&description_contains.to_lowercase()))
    }

    /// Check fuzzy search filter with threshold
    fn check_fuzzy_search_filter(
        &self,
        market: &Value,
        search_term: &str,
        threshold: f64,
    ) -> Result<bool> {
        let title = self.get_market_title(market);
        let description = market
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();

        // Combine title and description for fuzzy search
        let combined_text = format!("{} {}", title, description);

        // Simple fuzzy matching: calculate percentage of matching characters
        let search_lower = search_term.to_lowercase();
        let matching_chars = search_lower
            .chars()
            .filter(|&c| combined_text.contains(c))
            .count();

        let similarity = matching_chars as f64 / search_lower.len() as f64;

        // Also check if all words in search term appear in the text
        let words_match = search_lower
            .split_whitespace()
            .all(|word| combined_text.contains(word));

        Ok(similarity >= threshold || words_match)
    }

    /// Check text search filter (searches in title, description, and tags)
    fn check_text_search_filter(&self, market: &Value, text_search: &str) -> Result<bool> {
        let title = self.get_market_title(market);
        let description = market
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();

        let tags = market
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|tags| {
                tags.iter()
                    .filter_map(|t| t.as_str())
                    .map(|s| s.to_lowercase())
                    .collect::<Vec<String>>()
                    .join(" ")
            })
            .unwrap_or_default();

        let combined_text = format!("{} {} {}", title, description, tags);
        Ok(combined_text.contains(&text_search.to_lowercase()))
    }

    /// Save analysis results to files
    async fn save_results(&self, output_path: &Path, markets: &[Value]) -> Result<()> {
        // Save filtered markets as JSON
        let markets_file = output_path.join("markets.json");
        let json = serde_json::to_string_pretty(markets)?;
        let mut file = File::create(&markets_file)?;
        file.write_all(json.as_bytes())?;

        // Save analysis configuration
        let config_file = output_path.join("analysis_config.yaml");
        let config_yaml = serde_yaml::to_string(self)?;
        let mut file = File::create(&config_file)?;
        file.write_all(config_yaml.as_bytes())?;

        Ok(())
    }

    /// Calculate analysis statistics
    fn calculate_statistics(&self, markets: &[Value]) -> AnalysisStatistics {
        let mut stats = AnalysisStatistics::default();
        let mut categories: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for market in markets {
            // Count status types
            if market
                .get("active")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                stats.active_count += 1;
            }
            if market
                .get("accepting_orders")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                stats.accepting_orders_count += 1;
            }
            if market
                .get("closed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                stats.closed_count += 1;
            }

            // Price distribution
            if let Some(tokens) = market.get("tokens").and_then(|v| v.as_array()) {
                if let Some(yes_token) = tokens.get(0) {
                    if let Some(price) = yes_token.get("price").and_then(|p| p.as_f64()) {
                        let bucket = ((price * 10.0).floor() as usize).min(10);
                        if stats.price_distribution.len() <= bucket {
                            stats.price_distribution.resize(bucket + 1, 0);
                        }
                        stats.price_distribution[bucket] += 1;
                    }
                }
            }

            // Category counting
            if let Some(category) = market.get("category").and_then(|v| v.as_str()) {
                *categories.entry(category.to_string()).or_insert(0) += 1;
            }
        }

        // Top categories
        let mut cat_vec: Vec<_> = categories.into_iter().collect();
        cat_vec.sort_by(|a, b| b.1.cmp(&a.1));
        stats.top_categories = cat_vec.into_iter().take(10).collect();

        stats
    }

    /// Save command metadata
    fn save_command_metadata(
        &self,
        output_path: &Path,
        total_markets: usize,
        filtered_markets: usize,
    ) -> Result<()> {
        let command_args = vec![
            self.dataset_name.clone(),
            "--source-dataset".to_string(),
            self.source_dataset.clone(),
        ];

        let mut additional_info = std::collections::HashMap::new();
        additional_info.insert(
            "dataset_type".to_string(),
            serde_json::json!("AnalyzedMarkets"),
        );
        additional_info.insert(
            "source_dataset".to_string(),
            serde_json::json!(self.source_dataset),
        );
        additional_info.insert(
            "total_markets_analyzed".to_string(),
            serde_json::json!(total_markets),
        );
        additional_info.insert(
            "markets_in_dataset".to_string(),
            serde_json::json!(filtered_markets),
        );

        if let Err(e) =
            save_command_metadata(output_path, "analyze", &command_args, Some(additional_info))
        {
            warn!("Warning: Failed to save command metadata: {}", e);
        }

        Ok(())
    }

    /// Display analysis results
    fn display_results(
        &self,
        total_markets: usize,
        filtered_markets: usize,
        statistics: &AnalysisStatistics,
    ) {
        info!(
            "✅ Created dataset '{}' with {} markets (from {} total)",
            self.dataset_name, filtered_markets, total_markets
        );

        if self.output_config.summary || self.output_config.detailed {
            self.show_analysis_summary(statistics);
        }
    }

    /// Show detailed analysis summary
    fn show_analysis_summary(&self, stats: &AnalysisStatistics) {
        info!("📊 Dataset Analysis");
        info!("{}", "─".repeat(50));

        info!("Active markets: {}", stats.active_count);
        info!("Accepting orders: {}", stats.accepting_orders_count);
        info!("Closed markets: {}", stats.closed_count);

        if !stats.price_distribution.is_empty() {
            info!("Price Distribution (YES outcome):");
            for (i, count) in stats.price_distribution.iter().enumerate() {
                if *count > 0 {
                    let range = if i == 10 {
                        "90-100%".to_string()
                    } else {
                        format!("{}-{}%", i * 10, (i + 1) * 10)
                    };
                    info!("  {}: {}", range, count);
                }
            }
        }

        if self.output_config.detailed && !stats.top_categories.is_empty() {
            info!("Categories:");
            for (category, count) in &stats.top_categories {
                info!("  {}: {}", category, count);
            }
        }
    }
}

impl Default for AnalysisStatistics {
    fn default() -> Self {
        Self {
            active_count: 0,
            accepting_orders_count: 0,
            closed_count: 0,
            price_distribution: Vec::new(),
            top_categories: Vec::new(),
        }
    }
}

/// Main entry point for market analysis
pub async fn analyze_markets(data_paths: DataPaths, args: AnalyzeArgs) -> Result<()> {
    let analyzer = MarketAnalyzer::from_args(args);
    analyzer.execute(&data_paths).await?;
    Ok(())
}
