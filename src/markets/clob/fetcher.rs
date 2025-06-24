use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, Write};
use tracing::{info, warn};

use super::providers::MarketDataProvider;
use super::storage::MarketStorage;

/// Strongly typed market data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Market {
    pub id: Option<String>,
    pub condition_id: Option<String>,
    pub question: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub tags: Option<Vec<String>>,
    pub tokens: Vec<MarketToken>,
    pub active: bool,
    pub closed: bool,
    pub archived: Option<bool>,
    pub accepting_orders: bool,
    pub minimum_order_size: Option<f64>,
    pub minimum_tick_size: Option<f64>,
    pub end_date_iso: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub volume: Option<f64>,
    pub volume_24hr: Option<f64>,
    pub liquidity: Option<f64>,
    pub outcomes: Option<Vec<String>>,
    pub outcome_prices: Option<Vec<f64>>,
    pub market_slug: Option<String>,
    pub creator: Option<String>,
    pub fee_rate: Option<f64>,
    /// Additional fields from API that don't have explicit mappings
    #[serde(flatten)]
    pub additional_fields: HashMap<String, serde_json::Value>,
}

/// Market token information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketToken {
    pub token_id: String,
    pub outcome: String,
    pub price: f64,
    pub winner: Option<bool>,
    pub volume: Option<f64>,
    pub volume_24hr: Option<f64>,
    pub supply: Option<f64>,
    pub market_cap: Option<f64>,
    /// Additional token fields
    #[serde(flatten)]
    pub additional_fields: HashMap<String, serde_json::Value>,
}

/// Generic market fetcher that works with any provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketFetcher<T: MarketDataProvider> {
    provider: T,
    storage: MarketStorage,
    config: FetcherConfig,
}

/// Configuration for the market fetcher
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetcherConfig {
    pub verbose: bool,
    pub chunk_size_bytes: usize,
    pub max_pages: Option<usize>,
    pub delay_between_requests_ms: u64,
    pub retry_attempts: usize,
    pub save_progress_every_n_pages: usize,
}

/// Progress tracking with comprehensive metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchProgress {
    pub pages_fetched: usize,
    pub markets_fetched: usize,
    pub chunks_saved: usize,
    pub start_time: DateTime<Utc>,
    pub last_update: DateTime<Utc>,
    pub estimated_total_markets: Option<usize>,
    pub estimated_completion_time: Option<DateTime<Utc>>,
    pub current_rate_per_second: f64,
    pub average_rate_per_second: f64,
    pub bytes_processed: u64,
    pub errors_encountered: usize,
}

/// Fetch operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchResult {
    pub total_markets_fetched: usize,
    pub total_chunks_saved: usize,
    pub total_pages_processed: usize,
    pub execution_time_seconds: f64,
    pub average_markets_per_second: f64,
    pub total_bytes_processed: u64,
    pub errors_encountered: usize,
    pub final_state: FetchOperationState,
}

/// Current state of fetch operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchOperationState {
    pub is_complete: bool,
    pub last_page_token: Option<String>,
    pub current_chunk_number: usize,
    pub markets_in_current_chunk: usize,
    pub completion_percentage: f64,
}

impl Default for FetcherConfig {
    fn default() -> Self {
        Self {
            verbose: false,
            chunk_size_bytes: 10 * 1024 * 1024, // 10MB
            max_pages: None,
            delay_between_requests_ms: 100,
            retry_attempts: 3,
            save_progress_every_n_pages: 25,
        }
    }
}

impl FetchProgress {
    fn new() -> Self {
        let now = Utc::now();
        Self {
            pages_fetched: 0,
            markets_fetched: 0,
            chunks_saved: 0,
            start_time: now,
            last_update: now,
            estimated_total_markets: None,
            estimated_completion_time: None,
            current_rate_per_second: 0.0,
            average_rate_per_second: 0.0,
            bytes_processed: 0,
            errors_encountered: 0,
        }
    }

    fn update_rates(&mut self) {
        let now = Utc::now();
        let elapsed = now.timestamp() - self.start_time.timestamp();

        if elapsed > 0 {
            self.average_rate_per_second = self.markets_fetched as f64 / elapsed as f64;
        }

        // Update estimated completion if we have enough data
        if self.pages_fetched > 10 && self.estimated_total_markets.is_none() {
            // Rough estimate based on typical market counts
            self.estimated_total_markets = Some(25000);
        }

        if let Some(total) = self.estimated_total_markets {
            if self.average_rate_per_second > 0.0 {
                let remaining_markets = total.saturating_sub(self.markets_fetched);
                let estimated_seconds = remaining_markets as f64 / self.average_rate_per_second;
                self.estimated_completion_time =
                    Some(now + chrono::Duration::seconds(estimated_seconds as i64));
            }
        }

        self.last_update = now;
    }

    fn display_detailed(&mut self, status: &str) {
        self.update_rates();

        let elapsed_seconds = (Utc::now().timestamp() - self.start_time.timestamp()) as f64;

        let progress_pct = if let Some(total) = self.estimated_total_markets {
            (self.markets_fetched as f64 / total as f64 * 100.0).min(100.0)
        } else {
            0.0
        };

        print!("\r{}", " ".repeat(120)); // Clear line
        print!(
            "\r{} {} [{:>3.0}%] Pages: {} | Markets: {} | Chunks: {} | {:.0}/s | {}",
            status,
            progress_bar(progress_pct),
            progress_pct,
            self.pages_fetched,
            self.markets_fetched,
            self.chunks_saved,
            self.average_rate_per_second,
            format_duration(std::time::Duration::from_secs(elapsed_seconds as u64))
        );

        if let Some(eta) = self.estimated_completion_time {
            let remaining = eta.timestamp() - Utc::now().timestamp();
            if remaining > 0 {
                print!(
                    " | ETA: {}",
                    format_duration(std::time::Duration::from_secs(remaining as u64))
                );
            }
        }

        let _ = io::stdout().flush();
    }
}

impl Market {
    /// Convert from legacy serde_json::Value to strongly typed Market
    pub fn from_value(value: serde_json::Value) -> Result<Self> {
        // Try direct deserialization first
        if let Ok(market) = serde_json::from_value::<Market>(value.clone()) {
            return Ok(market);
        }

        // Manual conversion for legacy data
        Self::from_value_manual(value)
    }

    fn from_value_manual(value: serde_json::Value) -> Result<Self> {
        let obj = value
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("Market data must be an object"))?;

        // Extract tokens
        let tokens = obj
            .get("tokens")
            .and_then(|v| v.as_array())
            .map(|tokens| {
                tokens
                    .iter()
                    .filter_map(|token| MarketToken::from_value(token.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();

        // Build additional_fields by excluding known fields
        let mut additional_fields = HashMap::new();
        for (key, val) in obj {
            match key.as_str() {
                "id" | "condition_id" | "question" | "description" | "category" | "tags"
                | "tokens" | "active" | "closed" | "archived" | "accepting_orders"
                | "minimum_order_size" | "minimum_tick_size" | "end_date_iso" | "created_at"
                | "updated_at" | "volume" | "volume_24hr" | "liquidity" | "outcomes"
                | "outcome_prices" | "market_slug" | "creator" | "fee_rate" => {
                    // Skip known fields
                }
                _ => {
                    additional_fields.insert(key.clone(), val.clone());
                }
            }
        }

        Ok(Market {
            id: obj.get("id").and_then(|v| v.as_str()).map(String::from),
            condition_id: obj
                .get("condition_id")
                .and_then(|v| v.as_str())
                .map(String::from),
            question: obj
                .get("question")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown Question")
                .to_string(),
            description: obj
                .get("description")
                .and_then(|v| v.as_str())
                .map(String::from),
            category: obj
                .get("category")
                .and_then(|v| v.as_str())
                .map(String::from),
            tags: obj.get("tags").and_then(|v| v.as_array()).map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect()
            }),
            tokens,
            active: obj.get("active").and_then(|v| v.as_bool()).unwrap_or(false),
            closed: obj.get("closed").and_then(|v| v.as_bool()).unwrap_or(false),
            archived: obj.get("archived").and_then(|v| v.as_bool()),
            accepting_orders: obj
                .get("accepting_orders")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            minimum_order_size: obj.get("minimum_order_size").and_then(|v| v.as_f64()),
            minimum_tick_size: obj.get("minimum_tick_size").and_then(|v| v.as_f64()),
            end_date_iso: obj
                .get("end_date_iso")
                .and_then(|v| v.as_str())
                .map(String::from),
            created_at: obj
                .get("created_at")
                .and_then(|v| v.as_str())
                .map(String::from),
            updated_at: obj
                .get("updated_at")
                .and_then(|v| v.as_str())
                .map(String::from),
            volume: obj.get("volume").and_then(|v| v.as_f64()),
            volume_24hr: obj.get("volume_24hr").and_then(|v| v.as_f64()),
            liquidity: obj.get("liquidity").and_then(|v| v.as_f64()),
            outcomes: obj.get("outcomes").and_then(|v| v.as_array()).map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect()
            }),
            outcome_prices: obj
                .get("outcome_prices")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect()),
            market_slug: obj
                .get("market_slug")
                .and_then(|v| v.as_str())
                .map(String::from),
            creator: obj
                .get("creator")
                .and_then(|v| v.as_str())
                .map(String::from),
            fee_rate: obj.get("fee_rate").and_then(|v| v.as_f64()),
            additional_fields,
        })
    }

    /// Calculate the approximate size in bytes for chunk management
    pub fn size_bytes(&self) -> usize {
        serde_json::to_string(self).map(|s| s.len()).unwrap_or(1000)
    }
}

impl MarketToken {
    /// Convert from serde_json::Value to MarketToken
    pub fn from_value(value: serde_json::Value) -> Result<Self> {
        if let Ok(token) = serde_json::from_value::<MarketToken>(value.clone()) {
            return Ok(token);
        }

        let obj = value
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("Token data must be an object"))?;

        let mut additional_fields = HashMap::new();
        for (key, val) in obj {
            match key.as_str() {
                "token_id" | "outcome" | "price" | "winner" | "volume" | "volume_24hr"
                | "supply" | "market_cap" => {
                    // Skip known fields
                }
                _ => {
                    additional_fields.insert(key.clone(), val.clone());
                }
            }
        }

        Ok(MarketToken {
            token_id: obj
                .get("token_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Token must have token_id"))?
                .to_string(),
            outcome: obj
                .get("outcome")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string(),
            price: obj.get("price").and_then(|v| v.as_f64()).unwrap_or(0.0),
            winner: obj.get("winner").and_then(|v| v.as_bool()),
            volume: obj.get("volume").and_then(|v| v.as_f64()),
            volume_24hr: obj.get("volume_24hr").and_then(|v| v.as_f64()),
            supply: obj.get("supply").and_then(|v| v.as_f64()),
            market_cap: obj.get("market_cap").and_then(|v| v.as_f64()),
            additional_fields,
        })
    }
}

fn progress_bar(percentage: f64) -> String {
    let width = 20;
    let filled = (percentage / 100.0 * width as f64) as usize;
    let empty = width - filled;
    format!("{}{}{}{}", "[", "█".repeat(filled), "░".repeat(empty), "]")
}

fn format_duration(duration: std::time::Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

impl<T: MarketDataProvider> MarketFetcher<T> {
    pub fn with_config(provider: T, storage: MarketStorage, config: FetcherConfig) -> Self {
        Self {
            provider,
            storage,
            config,
        }
    }

    /// Fetch all markets with resumable state and strong typing
    pub async fn fetch_all<S>(
        &mut self,
        state_filename: &str,
        chunk_prefix: &str,
    ) -> Result<FetchResult>
    where
        S: FetchState + Default,
    {
        // Load or initialize state
        let mut state: S = self.storage.load_state(state_filename)?.unwrap_or_default();

        if self.config.verbose && state.get_total_fetched() > 0 {
            info!(
                "📂 Resuming from {} (already fetched {} markets)",
                state.describe_position(),
                state.get_total_fetched()
            );
        }

        info!("🔄 Fetching all markets from {}...", self.provider.name());

        let mut progress = FetchProgress::new();
        let mut current_chunk: Vec<Market> = Vec::new();
        let mut current_chunk_size: usize = 0;
        let mut page_token = state.get_page_token();

        // Show initial progress
        info!("⏳ Starting fetch... (this may take a few minutes)");

        // Main fetch loop
        loop {
            // Check termination conditions
            if let Some(ref token) = page_token {
                if token == "LTE=" || token.is_empty() {
                    println!();
                    info!("📍 Reached end of data");
                    break;
                }
            }

            if !self.provider.has_more_pages() && page_token.is_none() {
                println!();
                info!("📍 No more pages to fetch");
                break;
            }

            // Check max pages limit
            if let Some(max_pages) = self.config.max_pages {
                if progress.pages_fetched >= max_pages {
                    println!();
                    info!("📍 Reached maximum page limit: {}", max_pages);
                    break;
                }
            }

            // Show progress
            progress.display_detailed("🔄 Fetching");

            // Add delay between requests
            if progress.pages_fetched > 0 && self.config.delay_between_requests_ms > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(
                    self.config.delay_between_requests_ms,
                ))
                .await;
            }

            // Fetch next page with retry logic
            let (raw_markets, next_token) = self.fetch_page_with_retry(page_token.clone()).await?;

            if raw_markets.is_empty() {
                println!();
                info!("📍 No more markets to fetch");
                break;
            }

            // Convert raw values to strongly typed Markets
            let markets = self.convert_raw_markets(raw_markets)?;

            progress.pages_fetched += 1;
            progress.markets_fetched += markets.len();

            // Process each market
            for market in markets {
                let market_size = market.size_bytes();
                progress.bytes_processed += market_size as u64;

                // Check if we should save current chunk
                if !current_chunk.is_empty()
                    && current_chunk_size + market_size > self.config.chunk_size_bytes
                {
                    // Save chunk
                    progress.display_detailed("💾 Saving chunk");

                    let chunk_number = state.get_chunk_number() + 1;
                    self.save_chunk_typed(chunk_number, &current_chunk, chunk_prefix)
                        .await?;

                    // Update state
                    state.update_after_chunk_save(chunk_number, current_chunk.len());
                    self.storage.save_state(state_filename, &state)?;

                    progress.chunks_saved += 1;

                    println!();
                    info!(
                        "💾 Saved chunk {} with {} markets",
                        chunk_number,
                        current_chunk.len()
                    );

                    current_chunk.clear();
                    current_chunk_size = 0;
                }

                current_chunk.push(market);
                current_chunk_size += market_size;
            }

            // Update page token and state
            page_token = next_token;
            state.update_page_token(page_token.clone());

            // Periodic state save
            if progress.pages_fetched % self.config.save_progress_every_n_pages == 0 {
                state.update_markets_in_chunk(current_chunk.len());
                self.storage.save_state(state_filename, &state)?;
            }

            // Check for end condition
            if let Some(ref token) = page_token {
                if token == "LTE=" || token.is_empty() {
                    state.update_markets_in_chunk(current_chunk.len());
                    self.storage.save_state(state_filename, &state)?;
                    break;
                }
            }
        }

        // Save final chunk
        if !current_chunk.is_empty() {
            progress.display_detailed("💾 Saving final chunk");

            let chunk_number = state.get_chunk_number() + 1;
            self.save_chunk_typed(chunk_number, &current_chunk, chunk_prefix)
                .await?;
            state.update_after_chunk_save(chunk_number, current_chunk.len());
            self.storage.save_state(state_filename, &state)?;
            progress.chunks_saved += 1;

            println!();
            info!(
                "💾 Saved final chunk {} with {} markets",
                chunk_number,
                current_chunk.len()
            );
        } else {
            println!();
        }

        // Create final result
        let elapsed_seconds = (Utc::now().timestamp() - progress.start_time.timestamp()) as f64;
        let result = FetchResult {
            total_markets_fetched: progress.markets_fetched,
            total_chunks_saved: progress.chunks_saved,
            total_pages_processed: progress.pages_fetched,
            execution_time_seconds: elapsed_seconds,
            average_markets_per_second: if elapsed_seconds > 0.0 {
                progress.markets_fetched as f64 / elapsed_seconds
            } else {
                0.0
            },
            total_bytes_processed: progress.bytes_processed,
            errors_encountered: progress.errors_encountered,
            final_state: FetchOperationState {
                is_complete: true,
                last_page_token: page_token,
                current_chunk_number: state.get_chunk_number(),
                markets_in_current_chunk: current_chunk.len(),
                completion_percentage: 100.0,
            },
        };

        // Display final summary
        self.display_final_summary(&result);

        Ok(result)
    }

    async fn fetch_page_with_retry(
        &mut self,
        page_token: Option<String>,
    ) -> Result<(Vec<serde_json::Value>, Option<String>)> {
        let mut last_error = None;

        for attempt in 1..=self.config.retry_attempts {
            match self.provider.fetch_page(page_token.clone()).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    if self.config.verbose {
                        warn!("⚠️  Fetch attempt {} failed: {}", attempt, e);
                    }
                    last_error = Some(e);
                    if attempt < self.config.retry_attempts {
                        let delay = std::time::Duration::from_millis(1000 * attempt as u64);
                        if self.config.verbose {
                            info!("🔄 Retrying in {}ms...", delay.as_millis());
                        }
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        match last_error {
            Some(error) => Err(anyhow::anyhow!(
                "❌ Failed to fetch page after {} attempts.\n\
                 💡 Last error: {}\n\
                 💡 This could be due to network issues, API rate limiting, or invalid credentials.\n\
                 💡 Try running again with --verbose for more details.",
                self.config.retry_attempts,
                error
            )),
            None => Err(anyhow::anyhow!(
                "❌ Fetch failed but no error details were captured.\n\
                 💡 This is unexpected - please report this issue."
            ))
        }
    }

    fn convert_raw_markets(&self, raw_markets: Vec<serde_json::Value>) -> Result<Vec<Market>> {
        let mut markets = Vec::new();
        let mut conversion_errors = 0;
        let mut invalid_condition_ids = 0;

        for raw_market in raw_markets {
            match Market::from_value(raw_market) {
                Ok(market) => {
                    // Filter out markets with empty or missing condition_id
                    match &market.condition_id {
                        Some(condition_id) if !condition_id.trim().is_empty() => {
                            markets.push(market);
                        }
                        _ => {
                            invalid_condition_ids += 1;
                            if self.config.verbose {
                                warn!(
                                    "Warning: Skipping market with missing/empty condition_id: {}",
                                    market.question
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    conversion_errors += 1;
                    if self.config.verbose {
                        warn!("Warning: Failed to convert market: {}", e);
                    }
                }
            }
        }

        if conversion_errors > 0 {
            warn!(
                "⚠️  {} markets could not be converted to typed format",
                conversion_errors
            );
        }

        if invalid_condition_ids > 0 {
            info!(
                "🔍 Filtered out {} markets with invalid/empty condition_id",
                invalid_condition_ids
            );
        }

        Ok(markets)
    }

    async fn save_chunk_typed(
        &self,
        chunk_number: usize,
        markets: &[Market],
        prefix: &str,
    ) -> Result<()> {
        // Convert back to Value for storage compatibility
        let values: Vec<serde_json::Value> = markets
            .iter()
            .map(|m| serde_json::to_value(m))
            .collect::<Result<Vec<_>, _>>()?;

        self.storage
            .save_chunk(chunk_number, &values, prefix, false)
    }

    fn display_final_summary(&self, result: &FetchResult) {
        info!(
            "✅ Successfully fetched {} markets from {} in {} chunks",
            result.total_markets_fetched,
            self.provider.name(),
            result.total_chunks_saved
        );

        info!(
            "⏱️  Total time: {} ({:.0} markets/sec)",
            format_duration(std::time::Duration::from_secs(
                result.execution_time_seconds as u64
            )),
            result.average_markets_per_second
        );

        info!(
            "📊 Processed {} MB across {} pages",
            result.total_bytes_processed / 1024 / 1024,
            result.total_pages_processed
        );

        if result.errors_encountered > 0 {
            warn!(
                "⚠️  {} errors encountered during fetch",
                result.errors_encountered
            );
        }
    }
}

/// Trait for fetch state management with strong typing
pub trait FetchState: Serialize + for<'de> Deserialize<'de> {
    fn get_page_token(&self) -> Option<String>;
    fn update_page_token(&mut self, token: Option<String>);
    fn get_total_fetched(&self) -> usize;
    fn get_chunk_number(&self) -> usize;
    fn describe_position(&self) -> String;
    fn update_markets_in_chunk(&mut self, count: usize);
    fn update_after_chunk_save(&mut self, new_chunk_number: usize, markets_saved: usize);
}

/// Enhanced FetchState implementation for CLOB
impl FetchState for super::types::FetchState {
    fn get_page_token(&self) -> Option<String> {
        self.last_cursor.clone()
    }

    fn update_page_token(&mut self, token: Option<String>) {
        self.last_cursor = token.clone();
        if let Some(ref t) = token {
            if t != "LTE=" && !t.is_empty() {
                self.last_page += 1;
            }
        } else if token.is_none() && self.last_page == 0 {
            self.last_page = 1;
        }
    }

    fn get_total_fetched(&self) -> usize {
        self.total_markets_fetched
    }

    fn get_chunk_number(&self) -> usize {
        self.chunk_number
    }

    fn update_markets_in_chunk(&mut self, count: usize) {
        self.markets_in_current_chunk = count;
    }

    fn update_after_chunk_save(&mut self, new_chunk_number: usize, markets_saved: usize) {
        self.chunk_number = new_chunk_number;
        self.total_markets_fetched += markets_saved;
        self.markets_in_current_chunk = 0;
    }

    fn describe_position(&self) -> String {
        format!("page {}", self.last_page)
    }
}

/// Enhanced FetchState implementation for Gamma
impl FetchState for super::types::GammaFetchState {
    fn get_page_token(&self) -> Option<String> {
        if self.last_offset > 0 {
            Some(self.last_offset.to_string())
        } else {
            None
        }
    }

    fn update_page_token(&mut self, token: Option<String>) {
        if let Some(token) = token {
            if let Ok(offset) = token.parse::<usize>() {
                self.last_offset = offset;
            }
        }
    }

    fn get_total_fetched(&self) -> usize {
        self.total_markets_fetched
    }

    fn get_chunk_number(&self) -> usize {
        self.chunk_number
    }

    fn update_markets_in_chunk(&mut self, count: usize) {
        self.markets_in_current_chunk = count;
    }

    fn update_after_chunk_save(&mut self, new_chunk_number: usize, markets_saved: usize) {
        self.chunk_number = new_chunk_number;
        self.total_markets_fetched += markets_saved;
        self.markets_in_current_chunk = 0;
    }

    fn describe_position(&self) -> String {
        format!("offset {}", self.last_offset)
    }
}
