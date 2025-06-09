use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use chrono::{DateTime, Utc, Duration};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketCache {
    pub last_updated: DateTime<Utc>,
    pub markets: Vec<CachedMarket>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedMarket {
    pub condition_id: String,
    pub question: String,
    pub slug: String,
    pub active: bool,
    pub is_binary: bool,
    pub volume: f64,
    pub liquidity: f64,
    pub tokens: Vec<TokenInfo>,
    pub last_price: Option<f64>,
    pub gamma_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub token_id: String,
    pub outcome: String,
    pub price: f64,
}

impl MarketCache {
    /// Load cache from file
    pub fn load() -> Result<Option<Self>> {
        let cache_path = get_cache_path()?;
        
        if !cache_path.exists() {
            return Ok(None);
        }
        
        let contents = std::fs::read_to_string(&cache_path)?;
        let cache: MarketCache = serde_json::from_str(&contents)?;
        
        // Check if cache is still valid (e.g., less than 1 hour old)
        let age = Utc::now() - cache.last_updated;
        if age > Duration::hours(1) {
            return Ok(None); // Cache is too old
        }
        
        Ok(Some(cache))
    }
    
    /// Save cache to file
    pub fn save(&self) -> Result<()> {
        let cache_path = get_cache_path()?;
        let json = serde_json::to_string_pretty(self)?;
        
        // Ensure directory exists
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        std::fs::write(&cache_path, json)?;
        Ok(())
    }
    
    /// Filter markets by criteria
    pub fn filter_binary_active(&self) -> Vec<&CachedMarket> {
        self.markets
            .iter()
            .filter(|m| m.is_binary && m.active)
            .collect()
    }
    
    /// Sort markets by volume (descending)
    pub fn sort_by_volume(markets: &mut Vec<&CachedMarket>) {
        markets.sort_by(|a, b| {
            b.volume.partial_cmp(&a.volume).unwrap_or(std::cmp::Ordering::Equal)
        });
    }
}

/// Get the cache file path
fn get_cache_path() -> Result<PathBuf> {
    let cache_dir = directories::ProjectDirs::from("com", "polybot", "polybot")
        .ok_or_else(|| anyhow::anyhow!("Could not determine cache directory"))?
        .cache_dir()
        .to_path_buf();
    
    std::fs::create_dir_all(&cache_dir)?;
    Ok(cache_dir.join("market_cache.json"))
}

/// Fetch and merge market data from both CLOB and Gamma APIs
pub async fn fetch_and_cache_markets(clob_client: &polymarket_rs_client::ClobClient) -> Result<MarketCache> {
    use owo_colors::OwoColorize;
    
    println!("{}", "🔄 Fetching market data from APIs...".bright_blue());
    
    // Fetch from CLOB API
    let clob_markets = fetch_clob_markets(clob_client).await?;
    println!("{}", format!("  ✓ Fetched {} markets from CLOB API", clob_markets.len()).bright_green());
    
    // Fetch from Gamma API for volume data
    let gamma_markets = fetch_gamma_markets().await?;
    println!("{}", format!("  ✓ Fetched {} markets from Gamma API", gamma_markets.len()).bright_green());
    
    // Create a map of Gamma markets by condition_id for quick lookup
    let gamma_map: HashMap<String, serde_json::Value> = gamma_markets
        .into_iter()
        .filter_map(|m| {
            // Gamma API uses "conditionId" (camelCase) not "condition_id"
            if let Some(condition_id) = m.get("conditionId").and_then(|id| id.as_str()) {
                Some((condition_id.to_string(), m))
            } else {
                None
            }
        })
        .collect();
    
    // Merge data
    let mut cached_markets = Vec::new();
    
    for clob_market in clob_markets {
        if let Some(condition_id) = clob_market.get("condition_id").and_then(|v| v.as_str()) {
            let gamma_data = gamma_map.get(condition_id);
            
            // Extract tokens
            let tokens: Vec<TokenInfo> = clob_market
                .get("tokens")
                .and_then(|v| v.as_array())
                .map(|tokens| {
                    tokens.iter()
                        .filter_map(|t| {
                            Some(TokenInfo {
                                token_id: t.get("token_id")?.as_str()?.to_string(),
                                outcome: t.get("outcome")?.as_str()?.to_string(),
                                price: t.get("price")?.as_f64()?,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            
            // Determine if it's binary (exactly 2 outcomes: Yes/No)
            let is_binary = tokens.len() == 2 && 
                tokens.iter().any(|t| t.outcome.to_lowercase() == "yes") &&
                tokens.iter().any(|t| t.outcome.to_lowercase() == "no");
            
            let cached_market = CachedMarket {
                condition_id: condition_id.to_string(),
                question: clob_market.get("question")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string(),
                slug: clob_market.get("market_slug")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                active: clob_market.get("active")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                is_binary,
                volume: gamma_data
                    .and_then(|g| g.get("volume"))
                    .and_then(|v| {
                        // Try as f64 first, then as string
                        v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse::<f64>().ok()))
                    })
                    .unwrap_or(0.0),
                liquidity: gamma_data
                    .and_then(|g| g.get("liquidity"))
                    .and_then(|v| {
                        // Try as f64 first, then as string
                        v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse::<f64>().ok()))
                    })
                    .unwrap_or(0.0),
                tokens: tokens.clone(),
                last_price: tokens.iter()
                    .find(|t| t.outcome.to_lowercase() == "yes")
                    .map(|t| t.price),
                gamma_id: gamma_data
                    .and_then(|g| g.get("id"))
                    .and_then(|v| v.as_i64()),
            };
            
            cached_markets.push(cached_market);
        }
    }
    
    let cache = MarketCache {
        last_updated: Utc::now(),
        markets: cached_markets,
    };
    
    // Save to cache
    cache.save()?;
    println!("{}", format!("  ✓ Cached {} markets to disk", cache.markets.len()).bright_green());
    
    Ok(cache)
}

/// Fetch markets from CLOB API
async fn fetch_clob_markets(client: &polymarket_rs_client::ClobClient) -> Result<Vec<serde_json::Value>> {
    let mut all_markets = Vec::new();
    let mut cursor: Option<String> = None;
    
    // Fetch more pages to get better coverage
    for _ in 0..20 {
        let response = client.get_markets(cursor.as_deref()).await?;
        
        // Extract markets and cursor
        let (markets, next_cursor) = extract_markets_and_cursor(&response)?;
        all_markets.extend(markets);
        
        cursor = next_cursor;
        if cursor.is_none() || cursor.as_ref().map(|c| c.is_empty()).unwrap_or(true) {
            break;
        }
    }
    
    Ok(all_markets)
}

/// Fetch markets from Gamma API
async fn fetch_gamma_markets() -> Result<Vec<serde_json::Value>> {
    let client = reqwest::Client::new();
    let mut all_markets = Vec::new();
    let limit = 1000;
    let mut offset = 0;
    
    // Fetch multiple pages to get more markets
    for _ in 0..10 {
        let url = format!(
            "https://gamma-api.polymarket.com/markets?limit={}&offset={}&order=volume&ascending=false",
            limit, offset
        );
        
        let response = client.get(&url).send().await?;
        let markets: Vec<serde_json::Value> = response.json().await?;
        
        if markets.is_empty() {
            break;
        }
        
        all_markets.extend(markets);
        offset += limit;
    }
    
    Ok(all_markets)
}

/// Extract markets array and next cursor from CLOB response
fn extract_markets_and_cursor(response: &serde_json::Value) -> Result<(Vec<serde_json::Value>, Option<String>)> {
    let markets = if let Some(obj) = response.as_object() {
        if let Some(data) = obj.get("data") {
            data.as_array()
                .ok_or_else(|| anyhow::anyhow!("Expected 'data' field to be an array"))?
                .clone()
        } else {
            response.as_array()
                .ok_or_else(|| anyhow::anyhow!("Expected response to contain 'data' array or be an array itself"))?
                .clone()
        }
    } else if let Some(array) = response.as_array() {
        array.clone()
    } else {
        return Err(anyhow::anyhow!("Unexpected response format"));
    };
    
    let next_cursor = if let Some(obj) = response.as_object() {
        obj.get("next_cursor")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty() && s != "LTE=")
    } else {
        None
    };
    
    Ok((markets, next_cursor))
} 