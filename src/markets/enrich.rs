use anyhow::{Result, anyhow};
use owo_colors::OwoColorize;
use serde_json::{json, Value};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Semaphore, Mutex};
use tokio::time::sleep;
use futures::future::join_all;
use polymarket_rs_client::ClobClient;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use chrono::Utc;

use crate::auth::get_authenticated_client;
use crate::data_paths::DataPaths;
use crate::cli::EnrichArgs;

/// Structure for enriched market data
#[derive(serde::Serialize)]
struct EnrichedMarket {
    #[serde(flatten)]
    market: Value,
    
    enrichment: MarketEnrichment,
    
    enriched_at: String,
}

#[derive(serde::Serialize, Default)]
struct MarketEnrichment {
    // Orderbook data
    best_bid: Option<f64>,
    best_ask: Option<f64>,
    mid_price: Option<f64>,
    spread: Option<f64>,
    spread_percentage: Option<f64>,
    
    // Liquidity metrics
    total_bid_size: Option<f64>,
    total_ask_size: Option<f64>,
    bid_orders_count: Option<usize>,
    ask_orders_count: Option<usize>,
    
    // Volume data (from Gamma API if requested)
    volume_24hr: Option<f64>,
    volume_total: Option<f64>,
    
    // Status
    has_orderbook: bool,
    error: Option<String>,
}

/// Rate limiter for API calls
struct RateLimiter {
    semaphore: Arc<Semaphore>,
    last_request_times: Arc<Mutex<Vec<Instant>>>,
    requests_per_10s: usize,
}

impl RateLimiter {
    fn new(requests_per_10s: usize) -> Self {
        // Allow burst of up to 10 concurrent requests
        let concurrent_limit = requests_per_10s.min(10);
        Self {
            semaphore: Arc::new(Semaphore::new(concurrent_limit)),
            last_request_times: Arc::new(Mutex::new(Vec::new())),
            requests_per_10s,
        }
    }
    
    async fn acquire(&self) {
        let _permit = self.semaphore.acquire().await.unwrap();
        
        // Check rate limit (50 requests per 10 seconds)
        let mut times = self.last_request_times.lock().await;
        let now = Instant::now();
        
        // Remove requests older than 10 seconds
        times.retain(|&t| now.duration_since(t) < Duration::from_secs(10));
        
        // If we've hit the limit, wait
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

pub async fn enrich_markets(host: &str, data_paths: DataPaths, args: EnrichArgs) -> Result<()> {
    println!("{}", format!("💎 Enriching market dataset '{}'...", args.source_dataset).bright_blue());
    
    // Load source dataset
    let source_path = data_paths.markets_datasets().join(&args.source_dataset);
    if !source_path.exists() {
        return Err(anyhow!("Source dataset not found: {}", source_path.display()));
    }
    
    let markets_file = source_path.join("markets.json");
    let mut file = File::open(&markets_file)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    
    let markets: Vec<Value> = serde_json::from_str(&contents)?;
    println!("{}", format!("📊 Loaded {} markets from source dataset", markets.len()).bright_cyan());
    
    // Create output dataset directory
    let output_path = data_paths.markets_datasets().join(&args.output_dataset);
    fs::create_dir_all(&output_path)?;
    
    // Get authenticated CLOB client
    let client = Arc::new(get_authenticated_client(host, &data_paths).await?);
    
    // Create rate limiter (50 requests per 10 seconds for /books endpoint)
    let rate_limiter = Arc::new(RateLimiter::new(50));
    
    // Process markets in chunks
    let chunk_size = args.parallel.min(10); // Max 10 concurrent requests
    let start_idx = args.start_from.unwrap_or(0);
    
    // Progress tracking
    let progress = Arc::new(Mutex::new((0usize, 0usize))); // (processed, errors)
    let total_markets = markets.len();
    
    println!(
        "{}", 
        format!(
            "⚡ Processing {} markets with {} concurrent requests", 
            total_markets - start_idx,
            chunk_size
        ).bright_yellow()
    );
    
    let start_time = Instant::now();
    let mut all_enriched = Vec::new();
    
    // Process in chunks
    for chunk_start in (start_idx..markets.len()).step_by(chunk_size) {
        let chunk_end = (chunk_start + chunk_size).min(markets.len());
        let chunk_markets = &markets[chunk_start..chunk_end];
        
        // Create futures for this chunk
        let mut futures = Vec::new();
        
        for (idx, market) in chunk_markets.iter().enumerate() {
            let client = Arc::clone(&client);
            let rate_limiter = Arc::clone(&rate_limiter);
            let progress = Arc::clone(&progress);
            let market = market.clone();
            let args = args.clone();
            let market_idx = chunk_start + idx;
            
            let future = async move {
                // Acquire rate limit permit
                rate_limiter.acquire().await;
                
                // Process market
                let result = enrich_single_market(&client, &market, &args).await;
                
                // Update progress
                let mut prog = progress.lock().await;
                match &result {
                    Ok(_) => prog.0 += 1,
                    Err(_) => {
                        prog.0 += 1;
                        prog.1 += 1;
                    }
                }
                
                if args.progress {
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
                        ).bright_cyan()
                    );
                    std::io::stdout().flush().unwrap();
                }
                
                (market_idx, market, result)
            };
            
            futures.push(future);
        }
        
        // Wait for all futures in this chunk
        let chunk_results = join_all(futures).await;
        
        // Convert results to enriched markets
        for (_idx, market, result) in chunk_results {
            let enriched = match result {
                Ok(enriched_market) => enriched_market,
                Err(e) => EnrichedMarket {
                    market,
                    enrichment: MarketEnrichment {
                        error: Some(e.to_string()),
                        ..Default::default()
                    },
                    enriched_at: Utc::now().to_rfc3339(),
                }
            };
            all_enriched.push(enriched);
        }
        
        // Save progress periodically
        if all_enriched.len() % 50 == 0 {
            save_progress(&output_path, &all_enriched, all_enriched.len())?;
        }
    }
    
    if args.progress {
        println!(); // New line after progress
    }
    
    let elapsed = start_time.elapsed();
    let final_progress = progress.lock().await;
    let processed = final_progress.0;
    let errors = final_progress.1;
    
    // Save final results
    save_enriched_dataset(&output_path, &all_enriched, &args, &markets)?;
    
    // Show summary
    println!(
        "\n{}",
        format!(
            "✅ Enriched {} markets (errors: {}) in {:.1}s - {:.1} markets/sec",
            processed,
            errors,
            elapsed.as_secs_f64(),
            processed as f64 / elapsed.as_secs_f64()
        ).bright_green()
    );
    
    println!(
        "{}",
        format!("📁 Saved to: {}", output_path.display()).bright_blue()
    );
    
    Ok(())
}

async fn enrich_single_market(
    client: &ClobClient,
    market: &Value,
    args: &EnrichArgs,
) -> Result<EnrichedMarket> {
    let mut enrichment = MarketEnrichment::default();
    
    // Get token IDs for this market
    let tokens = market.get("tokens")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("Market has no tokens"))?;
    
    if tokens.is_empty() {
        return Err(anyhow!("Market has no tokens"));
    }
    
    // For binary markets, enrich the first (YES) token
    let token = &tokens[0];
    let token_id = token.get("token_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Token has no token_id"))?;
    
    // Fetch orderbook data if requested
    if args.include_orderbook || args.include_liquidity {
        match client.get_order_book(token_id).await {
            Ok(orderbook) => {
                enrichment.has_orderbook = true;
                
                // Calculate best bid/ask
                let best_bid = orderbook.bids.iter()
                    .map(|b| b.price)
                    .max();
                
                let best_ask = orderbook.asks.iter()
                    .map(|a| a.price)
                    .min();
                
                if let (Some(bid), Some(ask)) = (best_bid, best_ask) {
                    enrichment.best_bid = Some(bid.to_f64().unwrap_or(0.0));
                    enrichment.best_ask = Some(ask.to_f64().unwrap_or(0.0));
                    
                    let mid = (bid + ask) / Decimal::from(2);
                    enrichment.mid_price = Some(mid.to_f64().unwrap_or(0.0));
                    
                    let spread = ask - bid;
                    enrichment.spread = Some(spread.to_f64().unwrap_or(0.0));
                    
                    if bid > Decimal::ZERO {
                        let spread_pct = (spread / bid) * Decimal::from(100);
                        enrichment.spread_percentage = Some(spread_pct.to_f64().unwrap_or(0.0));
                    }
                }
                
                // Calculate liquidity metrics
                if args.include_liquidity {
                    let total_bid_size: Decimal = orderbook.bids.iter()
                        .map(|b| b.size)
                        .sum();
                    let total_ask_size: Decimal = orderbook.asks.iter()
                        .map(|a| a.size)
                        .sum();
                    
                    enrichment.total_bid_size = Some(total_bid_size.to_f64().unwrap_or(0.0));
                    enrichment.total_ask_size = Some(total_ask_size.to_f64().unwrap_or(0.0));
                    enrichment.bid_orders_count = Some(orderbook.bids.len());
                    enrichment.ask_orders_count = Some(orderbook.asks.len());
                }
            }
            Err(e) => {
                enrichment.error = Some(format!("Orderbook error: {}", e));
            }
        }
    }
    
    // Fetch volume data from Gamma API if requested
    if args.include_volume {
        if let Err(e) = fetch_gamma_volume(market, &mut enrichment).await {
            if enrichment.error.is_none() {
                enrichment.error = Some(format!("Volume error: {}", e));
            }
        }
    }
    
    Ok(EnrichedMarket {
        market: market.clone(),
        enrichment,
        enriched_at: Utc::now().to_rfc3339(),
    })
}

async fn fetch_gamma_volume(market: &Value, enrichment: &mut MarketEnrichment) -> Result<()> {
    // Try to find market in Gamma API by condition_id or slug
    let condition_id = market.get("condition_id")
        .and_then(|v| v.as_str());
    
    if let Some(condition_id) = condition_id {
        let client = reqwest::Client::new();
        let url = format!(
            "https://gamma-api.polymarket.com/markets?condition_id={}",
            condition_id
        );
        
        let response = client.get(&url).send().await?;
        let gamma_markets: Vec<Value> = response.json().await?;
        
        if let Some(gamma_market) = gamma_markets.first() {
            enrichment.volume_24hr = gamma_market.get("volume24hr")
                .and_then(|v| v.as_f64());
            enrichment.volume_total = gamma_market.get("volume")
                .and_then(|v| v.as_f64());
        }
    }
    
    Ok(())
}

fn save_progress(output_path: &PathBuf, markets: &[EnrichedMarket], processed: usize) -> Result<()> {
    let progress_file = output_path.join("enrichment_progress.json");
    let progress_data = json!({
        "processed": processed,
        "last_update": Utc::now().to_rfc3339(),
        "markets_count": markets.len()
    });
    
    let mut file = File::create(&progress_file)?;
    file.write_all(serde_json::to_string_pretty(&progress_data)?.as_bytes())?;
    
    Ok(())
}

fn save_enriched_dataset(
    output_path: &PathBuf,
    markets: &[EnrichedMarket],
    args: &EnrichArgs,
    source_markets: &[Value],
) -> Result<()> {
    // Save enriched markets
    let markets_file = output_path.join("enriched_markets.json");
    let json = serde_json::to_string_pretty(&markets)?;
    let mut file = File::create(&markets_file)?;
    file.write_all(json.as_bytes())?;
    
    // Create metadata
    let metadata = json!({
        "source_dataset": args.source_dataset,
        "output_dataset": args.output_dataset,
        "enriched_at": Utc::now().to_rfc3339(),
        "total_markets": source_markets.len(),
        "enriched_markets": markets.len(),
        "enrichment_options": {
            "include_orderbook": args.include_orderbook,
            "include_liquidity": args.include_liquidity,
            "include_volume": args.include_volume,
            "max_depth": args.max_depth,
        },
        "statistics": calculate_statistics(markets),
    });
    
    let metadata_file = output_path.join("metadata.json");
    let metadata_json = serde_json::to_string_pretty(&metadata)?;
    let mut file = File::create(&metadata_file)?;
    file.write_all(metadata_json.as_bytes())?;
    
    Ok(())
}

fn calculate_statistics(markets: &[EnrichedMarket]) -> Value {
    let mut markets_with_orderbook = 0;
    let mut markets_with_errors = 0;
    let mut total_spread_pct = 0.0;
    let mut spread_count = 0;
    
    for market in markets {
        if market.enrichment.has_orderbook {
            markets_with_orderbook += 1;
        }
        if market.enrichment.error.is_some() {
            markets_with_errors += 1;
        }
        if let Some(spread_pct) = market.enrichment.spread_percentage {
            total_spread_pct += spread_pct;
            spread_count += 1;
        }
    }
    
    let avg_spread_pct = if spread_count > 0 {
        total_spread_pct / spread_count as f64
    } else {
        0.0
    };
    
    json!({
        "markets_with_orderbook": markets_with_orderbook,
        "markets_with_errors": markets_with_errors,
        "average_spread_percentage": avg_spread_pct,
    })
} 