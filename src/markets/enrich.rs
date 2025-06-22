use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use futures::future::join_all;
use polymarket_rs_client::ClobClient;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};
use tokio::time::sleep;
use tracing::{info, warn};

use crate::cli::commands::enrich::EnrichArgs;
use crate::data_paths::DataPaths;
use crate::datasets::save_command_metadata;

/// Market enrichment configuration and execution engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketEnricher {
    /// Source dataset name
    pub source_dataset: String,
    /// Output dataset name
    pub output_dataset: String,
    /// Enrichment configuration
    pub config: EnrichmentConfig,
    /// Execution options
    pub execution_options: ExecutionOptions,
}

/// Configuration for market enrichment options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentConfig {
    /// Include orderbook data (best bid/ask, mid price, spread)
    #[serde(default)]
    pub include_orderbook: bool,
    /// Include liquidity metrics (order sizes, counts)
    #[serde(default)]
    pub include_liquidity: bool,
    /// Include volume data from Gamma API
    #[serde(default)]
    pub include_volume: bool,
    /// Maximum orderbook depth to analyze
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
}

/// Execution options for enrichment process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionOptions {
    /// Number of parallel requests
    #[serde(default = "default_parallel")]
    pub parallel: usize,
    /// Starting market index (for resuming)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_from: Option<usize>,
    /// Show progress during processing
    #[serde(default)]
    pub progress: bool,
}

/// Market data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Market {
    pub id: Option<String>,
    pub condition_id: Option<String>,
    pub question: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub tags: Option<Vec<String>>,
    pub tokens: Vec<MarketToken>,
    pub active: Option<bool>,
    pub closed: Option<bool>,
    pub archived: Option<bool>,
    pub accepting_orders: Option<bool>,
    pub minimum_order_size: Option<f64>,
    pub minimum_tick_size: Option<f64>,
    pub end_date_iso: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    /// Additional fields from source data
    #[serde(flatten)]
    pub additional_fields: HashMap<String, serde_json::Value>,
}

/// Market token information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketToken {
    pub token_id: String,
    pub outcome: Option<String>,
    pub price: Option<f64>,
    pub winner: Option<bool>,
    #[serde(flatten)]
    pub additional_fields: HashMap<String, serde_json::Value>,
}

/// Enriched market data with additional metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedMarket {
    /// Original market data
    #[serde(flatten)]
    pub market: Market,
    /// Enrichment data
    pub enrichment: MarketEnrichment,
    /// When this market was enriched
    pub enriched_at: DateTime<Utc>,
}

/// Market enrichment data
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MarketEnrichment {
    /// Orderbook metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orderbook: Option<OrderbookMetrics>,
    /// Liquidity metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liquidity: Option<LiquidityMetrics>,
    /// Volume metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<VolumeMetrics>,
    /// Processing status
    pub status: EnrichmentStatus,
}

/// Orderbook-derived metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderbookMetrics {
    pub best_bid: f64,
    pub best_ask: f64,
    pub mid_price: f64,
    pub spread: f64,
    pub spread_percentage: f64,
    pub bid_levels: usize,
    pub ask_levels: usize,
}

/// Liquidity metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityMetrics {
    pub total_bid_size: f64,
    pub total_ask_size: f64,
    pub bid_orders_count: usize,
    pub ask_orders_count: usize,
    pub liquidity_ratio: f64,    // bid_size / (bid_size + ask_size)
    pub market_depth_score: f64, // Combined metric
}

/// Volume metrics from external APIs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeMetrics {
    pub volume_24hr: Option<f64>,
    pub volume_total: Option<f64>,
    pub trade_count_24hr: Option<u64>,
    pub last_trade_time: Option<String>,
}

/// Enrichment processing status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentStatus {
    pub success: bool,
    pub has_orderbook: bool,
    pub processing_time_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<String>>,
}

impl Default for EnrichmentStatus {
    fn default() -> Self {
        Self {
            success: false,
            has_orderbook: false,
            processing_time_ms: 0,
            error: None,
            warnings: None,
        }
    }
}

/// Enrichment results and statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentResult {
    pub total_markets: usize,
    pub successfully_enriched: usize,
    pub failed_enrichments: usize,
    pub execution_time_ms: u64,
    pub output_path: PathBuf,
    pub statistics: EnrichmentStatistics,
}

/// Statistical summary of enrichment process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentStatistics {
    pub markets_with_orderbook: usize,
    pub markets_with_liquidity: usize,
    pub markets_with_volume: usize,
    pub average_spread_percentage: f64,
    pub average_processing_time_ms: f64,
    pub total_bid_volume: f64,
    pub total_ask_volume: f64,
}

/// Rate limiter for API calls
struct RateLimiter {
    semaphore: Arc<Semaphore>,
    last_request_times: Arc<Mutex<Vec<Instant>>>,
    requests_per_10s: usize,
}

impl RateLimiter {
    fn new(requests_per_10s: usize) -> Self {
        let concurrent_limit = requests_per_10s.min(10);
        Self {
            semaphore: Arc::new(Semaphore::new(concurrent_limit)),
            last_request_times: Arc::new(Mutex::new(Vec::new())),
            requests_per_10s,
        }
    }

    async fn acquire(&self) {
        let _permit = match self.semaphore.acquire().await {
            Ok(permit) => permit,
            Err(e) => {
                warn!("⚠️  Rate limiter semaphore error: {}", e);
                return;
            }
        };

        let mut times = self.last_request_times.lock().await;
        let now = Instant::now();

        times.retain(|&t| now.duration_since(t) < Duration::from_secs(10));

        if times.len() >= self.requests_per_10s {
            if let Some(&oldest) = times.first() {
                let wait_time = Duration::from_secs(10) - now.duration_since(oldest);
                if wait_time > Duration::ZERO {
                    sleep(wait_time + Duration::from_millis(100)).await;
                }
            }
        }

        times.push(now);
    }
}

impl MarketEnricher {
    /// Create a new MarketEnricher from CLI arguments
    pub fn from_args(args: EnrichArgs) -> Self {
        Self {
            source_dataset: args.source_dataset,
            output_dataset: args.output_dataset,
            config: EnrichmentConfig {
                include_orderbook: args.include_orderbook,
                include_liquidity: args.include_liquidity,
                include_volume: args.include_volume,
                max_depth: args.max_depth,
            },
            execution_options: ExecutionOptions {
                parallel: args.parallel,
                start_from: args.start_from,
                progress: args.progress,
            },
        }
    }

    /// Execute the market enrichment process
    pub async fn execute(&self, host: &str, data_paths: &DataPaths) -> Result<EnrichmentResult> {
        let start_time = Instant::now();

        info!("💎 Enriching market dataset '{}'...", self.source_dataset);

        // Create output directory
        let output_path = data_paths.datasets().join(&self.output_dataset);
        fs::create_dir_all(&output_path)?;

        info!("📁 Output directory: {}", output_path.display());

        // Load source dataset
        let source_path = self.resolve_source_path(data_paths)?;
        let markets = self.load_source_markets(&source_path).await?;

        // Get authenticated client
        let client = Arc::new(crate::auth::get_authenticated_client(host, data_paths).await?);

        // Process markets
        let enriched_markets = self.process_markets(&client, &markets).await?;

        // Save results
        self.save_results(&output_path, &enriched_markets).await?;

        // Calculate statistics
        let statistics = self.calculate_statistics(&enriched_markets);

        // Save command metadata
        self.save_command_metadata(&output_path, markets.len(), enriched_markets.len())?;

        let execution_time = start_time.elapsed().as_millis() as u64;

        // Display results
        self.display_results(markets.len(), &enriched_markets, &statistics);

        Ok(EnrichmentResult {
            total_markets: markets.len(),
            successfully_enriched: enriched_markets
                .iter()
                .filter(|m| m.enrichment.status.success)
                .count(),
            failed_enrichments: enriched_markets
                .iter()
                .filter(|m| !m.enrichment.status.success)
                .count(),
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

    /// Load markets from source dataset
    async fn load_source_markets(&self, source_path: &Path) -> Result<Vec<Market>> {
        if !source_path.exists() {
            return Err(anyhow!(
                "Source dataset not found: {}",
                source_path.display()
            ));
        }

        let markets_file = source_path.join("markets.json");
        let mut file = File::open(&markets_file)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        // Try to deserialize directly as Market structs first
        if let Ok(markets) = serde_json::from_str::<Vec<Market>>(&contents) {
            info!("📊 Loaded {} markets from source dataset", markets.len());
            return Ok(markets);
        }

        // Fallback: deserialize as Value and convert to Market structs
        let values: Vec<serde_json::Value> = serde_json::from_str(&contents)?;
        let markets: Vec<Market> = values
            .into_iter()
            .filter_map(|v| self.value_to_market(v))
            .collect();

        info!("📊 Loaded {} markets from source dataset", markets.len());
        Ok(markets)
    }

    /// Convert a serde_json::Value to Market struct
    fn value_to_market(&self, value: serde_json::Value) -> Option<Market> {
        // Extract tokens
        let tokens = value
            .get("tokens")
            .and_then(|v| v.as_array())
            .map(|tokens| {
                tokens
                    .iter()
                    .filter_map(|token| self.value_to_token(token.clone()))
                    .collect()
            })
            .unwrap_or_default();

        // Create additional_fields by excluding known fields
        let mut additional_fields = HashMap::new();
        if let serde_json::Value::Object(obj) = &value {
            for (key, val) in obj {
                match key.as_str() {
                    "id" | "condition_id" | "question" | "description" | "category" | "tags"
                    | "tokens" | "active" | "closed" | "archived" | "accepting_orders"
                    | "minimum_order_size" | "minimum_tick_size" | "end_date_iso"
                    | "created_at" | "updated_at" => {
                        // Skip known fields
                    }
                    _ => {
                        additional_fields.insert(key.clone(), val.clone());
                    }
                }
            }
        }

        Some(Market {
            id: value.get("id").and_then(|v| v.as_str()).map(String::from),
            condition_id: value
                .get("condition_id")
                .and_then(|v| v.as_str())
                .map(String::from),
            question: value
                .get("question")
                .and_then(|v| v.as_str())
                .map(String::from),
            description: value
                .get("description")
                .and_then(|v| v.as_str())
                .map(String::from),
            category: value
                .get("category")
                .and_then(|v| v.as_str())
                .map(String::from),
            tags: value.get("tags").and_then(|v| v.as_array()).map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect()
            }),
            tokens,
            active: value.get("active").and_then(|v| v.as_bool()),
            closed: value.get("closed").and_then(|v| v.as_bool()),
            archived: value.get("archived").and_then(|v| v.as_bool()),
            accepting_orders: value.get("accepting_orders").and_then(|v| v.as_bool()),
            minimum_order_size: value.get("minimum_order_size").and_then(|v| v.as_f64()),
            minimum_tick_size: value.get("minimum_tick_size").and_then(|v| v.as_f64()),
            end_date_iso: value
                .get("end_date_iso")
                .and_then(|v| v.as_str())
                .map(String::from),
            created_at: value
                .get("created_at")
                .and_then(|v| v.as_str())
                .map(String::from),
            updated_at: value
                .get("updated_at")
                .and_then(|v| v.as_str())
                .map(String::from),
            additional_fields,
        })
    }

    /// Convert a serde_json::Value to MarketToken struct
    fn value_to_token(&self, value: serde_json::Value) -> Option<MarketToken> {
        let token_id = value.get("token_id")?.as_str()?.to_string();

        let mut additional_fields = HashMap::new();
        if let serde_json::Value::Object(obj) = &value {
            for (key, val) in obj {
                match key.as_str() {
                    "token_id" | "outcome" | "price" | "winner" => {
                        // Skip known fields
                    }
                    _ => {
                        additional_fields.insert(key.clone(), val.clone());
                    }
                }
            }
        }

        Some(MarketToken {
            token_id,
            outcome: value
                .get("outcome")
                .and_then(|v| v.as_str())
                .map(String::from),
            price: value.get("price").and_then(|v| v.as_f64()),
            winner: value.get("winner").and_then(|v| v.as_bool()),
            additional_fields,
        })
    }

    /// Process all markets with enrichment
    async fn process_markets(
        &self,
        client: &Arc<ClobClient>,
        markets: &[Market],
    ) -> Result<Vec<EnrichedMarket>> {
        let rate_limiter = Arc::new(RateLimiter::new(50));
        let chunk_size = self.execution_options.parallel.min(10);
        let start_idx = self.execution_options.start_from.unwrap_or(0);

        let progress = Arc::new(Mutex::new((0usize, 0usize)));
        let total_markets = markets.len();

        info!(
            "⚡ Processing {} markets with {} concurrent requests",
            total_markets - start_idx,
            chunk_size
        );

        let start_time = Instant::now();
        let mut all_enriched = Vec::new();

        for chunk_start in (start_idx..markets.len()).step_by(chunk_size) {
            let chunk_end = (chunk_start + chunk_size).min(markets.len());
            let chunk_markets = &markets[chunk_start..chunk_end];

            let mut futures = Vec::new();

            for (idx, market) in chunk_markets.iter().enumerate() {
                let client = Arc::clone(client);
                let rate_limiter = Arc::clone(&rate_limiter);
                let progress = Arc::clone(&progress);
                let market = market.clone();
                let config = self.config.clone();
                let show_progress = self.execution_options.progress;
                let market_idx = chunk_start + idx;

                let future = async move {
                    rate_limiter.acquire().await;

                    let processing_start = Instant::now();
                    let result = Self::enrich_single_market(&client, &market, &config).await;
                    let processing_time = processing_start.elapsed().as_millis() as u64;

                    let enriched = match result {
                        Ok(mut enrichment) => {
                            enrichment.status.processing_time_ms = processing_time;
                            EnrichedMarket {
                                market,
                                enrichment,
                                enriched_at: Utc::now(),
                            }
                        }
                        Err(e) => EnrichedMarket {
                            market,
                            enrichment: MarketEnrichment {
                                status: EnrichmentStatus {
                                    success: false,
                                    has_orderbook: false,
                                    processing_time_ms: processing_time,
                                    error: Some(e.to_string()),
                                    warnings: None,
                                },
                                ..Default::default()
                            },
                            enriched_at: Utc::now(),
                        },
                    };

                    let mut prog = progress.lock().await;
                    prog.0 += 1;
                    if !enriched.enrichment.status.success {
                        prog.1 += 1;
                    }

                    if show_progress {
                        let elapsed = start_time.elapsed().as_secs();
                        let rate = if elapsed > 0 {
                            prog.0 as f64 / elapsed as f64
                        } else {
                            0.0
                        };

                        print!(
                            "\r{}",
                            format!(
                                "Processing market {}/{} (errors: {}) - {:.1} markets/sec",
                                prog.0 + start_idx,
                                total_markets,
                                prog.1,
                                rate
                            )
                        );
                        let _ = std::io::stdout().flush();
                    }

                    (market_idx, enriched)
                };

                futures.push(future);
            }

            let chunk_results = join_all(futures).await;

            for (_idx, enriched) in chunk_results {
                all_enriched.push(enriched);
            }

            // Save progress periodically
            if all_enriched.len() % 50 == 0 {
                // Progress saving will be handled in the main execute method
            }
        }

        if self.execution_options.progress {
            println!();
        }

        Ok(all_enriched)
    }

    /// Enrich a single market with additional data
    async fn enrich_single_market(
        client: &ClobClient,
        market: &Market,
        config: &EnrichmentConfig,
    ) -> Result<MarketEnrichment> {
        let mut enrichment = MarketEnrichment::default();
        let mut warnings = Vec::new();

        if market.tokens.is_empty() {
            return Err(anyhow!("Market has no tokens"));
        }

        let token = &market.tokens[0];
        let token_id = &token.token_id;

        // Fetch orderbook data if requested
        if config.include_orderbook || config.include_liquidity {
            match client.get_order_book(token_id).await {
                Ok(orderbook) => {
                    enrichment.status.has_orderbook = true;

                    // Calculate orderbook metrics
                    if config.include_orderbook {
                        if let Some(orderbook_metrics) =
                            Self::calculate_orderbook_metrics(&orderbook)
                        {
                            enrichment.orderbook = Some(orderbook_metrics);
                        } else {
                            warnings.push("Empty orderbook - no metrics available".to_string());
                        }
                    }

                    // Calculate liquidity metrics
                    if config.include_liquidity {
                        let liquidity_metrics = Self::calculate_liquidity_metrics(&orderbook);
                        enrichment.liquidity = Some(liquidity_metrics);
                    }
                }
                Err(e) => {
                    warnings.push(format!("Orderbook error: {}", e));
                }
            }
        }

        // Fetch volume data if requested
        if config.include_volume {
            match Self::fetch_volume_metrics(market).await {
                Ok(volume_metrics) => {
                    enrichment.volume = Some(volume_metrics);
                }
                Err(e) => {
                    warnings.push(format!("Volume error: {}", e));
                }
            }
        }

        enrichment.status = EnrichmentStatus {
            success: true,
            has_orderbook: enrichment.orderbook.is_some(),
            processing_time_ms: 0, // Will be set by caller
            error: None,
            warnings: if warnings.is_empty() {
                None
            } else {
                Some(warnings)
            },
        };

        Ok(enrichment)
    }

    /// Calculate orderbook metrics
    fn calculate_orderbook_metrics(
        orderbook: &polymarket_rs_client::OrderBookSummary,
    ) -> Option<OrderbookMetrics> {
        let best_bid = orderbook.bids.iter().map(|b| b.price).max()?;
        let best_ask = orderbook.asks.iter().map(|a| a.price).min()?;

        let best_bid_f64 = best_bid.to_f64()?;
        let best_ask_f64 = best_ask.to_f64()?;
        let mid_price = (best_bid_f64 + best_ask_f64) / 2.0;
        let spread = best_ask_f64 - best_bid_f64;
        let spread_percentage = if best_bid_f64 > 0.0 {
            (spread / best_bid_f64) * 100.0
        } else {
            0.0
        };

        Some(OrderbookMetrics {
            best_bid: best_bid_f64,
            best_ask: best_ask_f64,
            mid_price,
            spread,
            spread_percentage,
            bid_levels: orderbook.bids.len(),
            ask_levels: orderbook.asks.len(),
        })
    }

    /// Calculate liquidity metrics
    fn calculate_liquidity_metrics(
        orderbook: &polymarket_rs_client::OrderBookSummary,
    ) -> LiquidityMetrics {
        let total_bid_size: Decimal = orderbook.bids.iter().map(|b| b.size).sum();
        let total_ask_size: Decimal = orderbook.asks.iter().map(|a| a.size).sum();

        let total_bid_f64 = total_bid_size.to_f64().unwrap_or(0.0);
        let total_ask_f64 = total_ask_size.to_f64().unwrap_or(0.0);
        let total_liquidity = total_bid_f64 + total_ask_f64;

        let liquidity_ratio = if total_liquidity > 0.0 {
            total_bid_f64 / total_liquidity
        } else {
            0.0
        };

        let market_depth_score = (total_liquidity * 1000.0).sqrt();

        LiquidityMetrics {
            total_bid_size: total_bid_f64,
            total_ask_size: total_ask_f64,
            bid_orders_count: orderbook.bids.len(),
            ask_orders_count: orderbook.asks.len(),
            liquidity_ratio,
            market_depth_score,
        }
    }

    /// Fetch volume metrics from external API
    async fn fetch_volume_metrics(market: &Market) -> Result<VolumeMetrics> {
        let condition_id = market
            .condition_id
            .as_ref()
            .ok_or_else(|| anyhow!("Market has no condition_id"))?;

        let client = reqwest::Client::new();
        let url = format!(
            "https://gamma-api.polymarket.com/markets?condition_id={}",
            condition_id
        );

        let response = client.get(&url).send().await?;
        let gamma_markets: Vec<serde_json::Value> = response.json().await?;

        let gamma_market = gamma_markets
            .first()
            .ok_or_else(|| anyhow!("Market not found in Gamma API"))?;

        Ok(VolumeMetrics {
            volume_24hr: gamma_market.get("volume24hr").and_then(|v| v.as_f64()),
            volume_total: gamma_market.get("volume").and_then(|v| v.as_f64()),
            trade_count_24hr: gamma_market.get("trades24hr").and_then(|v| v.as_u64()),
            last_trade_time: gamma_market
                .get("lastTradeTime")
                .and_then(|v| v.as_str())
                .map(String::from),
        })
    }

    /// Save enrichment results
    async fn save_results(&self, output_path: &Path, markets: &[EnrichedMarket]) -> Result<()> {
        // Save enriched markets
        let markets_file = output_path.join("enriched_markets.json");
        let json = serde_json::to_string_pretty(&markets)?;
        let mut file = File::create(&markets_file)?;
        file.write_all(json.as_bytes())?;

        // Save enrichment configuration
        let config_file = output_path.join("enrichment_config.yaml");
        let config_yaml = serde_yaml::to_string(self)?;
        let mut file = File::create(&config_file)?;
        file.write_all(config_yaml.as_bytes())?;

        Ok(())
    }

    /// Calculate enrichment statistics
    fn calculate_statistics(&self, markets: &[EnrichedMarket]) -> EnrichmentStatistics {
        let mut markets_with_orderbook = 0;
        let mut markets_with_liquidity = 0;
        let mut markets_with_volume = 0;
        let mut total_spread_pct = 0.0;
        let mut spread_count = 0;
        let mut total_processing_time = 0u64;
        let mut total_bid_volume = 0.0;
        let mut total_ask_volume = 0.0;

        for market in markets {
            if market.enrichment.status.has_orderbook {
                markets_with_orderbook += 1;
            }
            if market.enrichment.liquidity.is_some() {
                markets_with_liquidity += 1;
            }
            if market.enrichment.volume.is_some() {
                markets_with_volume += 1;
            }

            if let Some(ref orderbook) = market.enrichment.orderbook {
                total_spread_pct += orderbook.spread_percentage;
                spread_count += 1;
            }

            if let Some(ref liquidity) = market.enrichment.liquidity {
                total_bid_volume += liquidity.total_bid_size;
                total_ask_volume += liquidity.total_ask_size;
            }

            total_processing_time += market.enrichment.status.processing_time_ms;
        }

        let avg_spread_pct = if spread_count > 0 {
            total_spread_pct / spread_count as f64
        } else {
            0.0
        };

        let avg_processing_time = if !markets.is_empty() {
            total_processing_time as f64 / markets.len() as f64
        } else {
            0.0
        };

        EnrichmentStatistics {
            markets_with_orderbook,
            markets_with_liquidity,
            markets_with_volume,
            average_spread_percentage: avg_spread_pct,
            average_processing_time_ms: avg_processing_time,
            total_bid_volume,
            total_ask_volume,
        }
    }

    /// Save command metadata
    fn save_command_metadata(
        &self,
        output_path: &Path,
        total_markets: usize,
        enriched_count: usize,
    ) -> Result<()> {
        let command_args = vec![self.source_dataset.clone(), self.output_dataset.clone()];

        let mut additional_info = HashMap::new();
        additional_info.insert(
            "dataset_type".to_string(),
            serde_json::json!("EnrichedMarkets"),
        );
        additional_info.insert(
            "source_dataset".to_string(),
            serde_json::json!(self.source_dataset),
        );
        additional_info.insert(
            "total_markets".to_string(),
            serde_json::json!(total_markets),
        );
        additional_info.insert(
            "enriched_markets".to_string(),
            serde_json::json!(enriched_count),
        );
        additional_info.insert(
            "include_orderbook".to_string(),
            serde_json::json!(self.config.include_orderbook),
        );
        additional_info.insert(
            "include_liquidity".to_string(),
            serde_json::json!(self.config.include_liquidity),
        );
        additional_info.insert(
            "include_volume".to_string(),
            serde_json::json!(self.config.include_volume),
        );

        if let Err(e) =
            save_command_metadata(output_path, "enrich", &command_args, Some(additional_info))
        {
            warn!("Warning: Failed to save command metadata: {}", e);
        }

        Ok(())
    }

    /// Display enrichment results
    fn display_results(
        &self,
        total_markets: usize,
        enriched_markets: &[EnrichedMarket],
        statistics: &EnrichmentStatistics,
    ) {
        let successful = enriched_markets
            .iter()
            .filter(|m| m.enrichment.status.success)
            .count();
        let failed = enriched_markets.len() - successful;

        info!(
            "✅ Enriched {} markets (errors: {}) - Success rate: {:.1}%",
            successful,
            failed,
            (successful as f64 / total_markets as f64) * 100.0
        );

        info!("📊 Enrichment Statistics");
        info!("{}", "─".repeat(50));

        info!(
            "Markets with orderbook: {}",
            statistics.markets_with_orderbook
        );
        info!(
            "Markets with liquidity data: {}",
            statistics.markets_with_liquidity
        );
        info!(
            "Markets with volume data: {}",
            statistics.markets_with_volume
        );

        if statistics.average_spread_percentage > 0.0 {
            info!(
                "Average spread: {:.2}%",
                statistics.average_spread_percentage
            );
        }

        info!(
            "Average processing time: {:.1}ms",
            statistics.average_processing_time_ms
        );

        if statistics.total_bid_volume > 0.0 || statistics.total_ask_volume > 0.0 {
            info!("Total bid volume: ${:.2}", statistics.total_bid_volume);
            info!("Total ask volume: ${:.2}", statistics.total_ask_volume);
        }

        info!("📁 Saved to: {}", self.output_dataset);
    }
}

impl Default for EnrichmentConfig {
    fn default() -> Self {
        Self {
            include_orderbook: false,
            include_liquidity: false,
            include_volume: false,
            max_depth: default_max_depth(),
        }
    }
}

impl Default for ExecutionOptions {
    fn default() -> Self {
        Self {
            parallel: default_parallel(),
            start_from: None,
            progress: false,
        }
    }
}

fn default_max_depth() -> usize {
    10
}
fn default_parallel() -> usize {
    5
}

/// Main entry point for market enrichment
pub async fn enrich_markets(host: &str, data_paths: DataPaths, args: EnrichArgs) -> Result<()> {
    let enricher = MarketEnricher::from_args(args);
    enricher.execute(host, &data_paths).await?;
    Ok(())
}
