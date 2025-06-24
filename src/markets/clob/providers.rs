use anyhow::Result;
use async_trait::async_trait;
use owo_colors::OwoColorize;
use polymarket_rs_client::ClobClient;
use serde_json::Value;

/// Trait for market data providers
#[async_trait]
pub trait MarketDataProvider {
    /// Get the name of the provider
    fn name(&self) -> &str;

    /// Fetch a page of markets
    /// Returns (markets, next_page_token)
    async fn fetch_page(
        &mut self,
        page_token: Option<String>,
    ) -> Result<(Vec<Value>, Option<String>)>;

    /// Check if there are more pages
    fn has_more_pages(&self) -> bool;
}

/// CLOB API provider
pub struct ClobProvider {
    client: ClobClient,
    current_cursor: Option<String>,
}

impl ClobProvider {
    pub fn new(client: ClobClient) -> Self {
        Self {
            client,
            current_cursor: None,
        }
    }
}

#[async_trait]
impl MarketDataProvider for ClobProvider {
    fn name(&self) -> &str {
        "CLOB API"
    }

    async fn fetch_page(
        &mut self,
        page_token: Option<String>,
    ) -> Result<(Vec<Value>, Option<String>)> {
        // Use provided token or current cursor
        let cursor = page_token.or_else(|| self.current_cursor.clone());

        // Check if we've reached the end
        if let Some(ref c) = cursor {
            if c == "LTE=" || c.is_empty() {
                // Return empty results to signal end
                return Ok((vec![], None));
            }
        }

        // Fetch markets from CLOB API
        let response = self.client.get_markets(cursor.as_deref()).await?;

        // Extract markets and next cursor
        let (markets, next_cursor) = extract_markets_and_cursor(&response)?;

        // Update internal state
        self.current_cursor = next_cursor.clone();

        Ok((markets, next_cursor))
    }

    fn has_more_pages(&self) -> bool {
        match &self.current_cursor {
            None => true, // First page
            Some(cursor) => !cursor.is_empty() && cursor != "LTE=",
        }
    }
}

/// Gamma API provider
pub struct GammaProvider {
    client: reqwest::Client,
    current_offset: usize,
    limit: usize,
    last_batch_size: usize,
    has_reached_end: bool,
}

impl GammaProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            current_offset: 0,
            limit: 1000,
            last_batch_size: 0,
            has_reached_end: false,
        }
    }
}

#[async_trait]
impl MarketDataProvider for GammaProvider {
    fn name(&self) -> &str {
        "Gamma API"
    }

    async fn fetch_page(
        &mut self,
        page_token: Option<String>,
    ) -> Result<(Vec<Value>, Option<String>)> {
        // Check if we've already reached the end
        if self.has_reached_end {
            return Ok((vec![], None));
        }

        // Parse offset from token if provided
        if let Some(token) = page_token {
            if let Ok(offset) = token.parse::<usize>() {
                self.current_offset = offset;
            }
        }

        // Build URL
        let url = format!(
            "https://gamma-api.polymarket.com/markets?limit={}&offset={}&order=id&ascending=true",
            self.limit, self.current_offset
        );

        // Fetch markets
        let response = self.client.get(&url).send().await?;
        let markets: Vec<Value> = response.json().await?;

        // Update state
        self.last_batch_size = markets.len();

        // Check if we've reached the end
        if markets.len() < self.limit {
            self.has_reached_end = true;
        }

        let next_offset = self.current_offset + markets.len();
        self.current_offset = next_offset;

        // Return markets and next token (offset as string)
        let next_token = if self.has_reached_end || markets.is_empty() {
            None // No more pages
        } else {
            Some(next_offset.to_string())
        };

        Ok((markets, next_token))
    }

    fn has_more_pages(&self) -> bool {
        !self.has_reached_end
    }
}

/// Extract markets array and next cursor from CLOB response
fn extract_markets_and_cursor(response: &Value) -> Result<(Vec<Value>, Option<String>)> {
    let markets = if let Some(obj) = response.as_object() {
        if let Some(data) = obj.get("data") {
            data.as_array()
                .ok_or_else(|| anyhow::anyhow!("Expected 'data' field to be an array"))?
                .clone()
        } else {
            response
                .as_array()
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Expected response to contain 'data' array or be an array itself"
                    )
                })?
                .clone()
        }
    } else if let Some(array) = response.as_array() {
        array.clone()
    } else {
        return Err(anyhow::anyhow!("Unexpected response format"));
    };

    let next_cursor = if let Some(obj) = response.as_object() {
        let cursor = obj
            .get("next_cursor")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Debug logging for cursor
        if let Some(ref c) = cursor {
            if c == "LTE=" {
                println!("{}", "📍 API returned end cursor: LTE=".bright_yellow());
            }
        }

        cursor.filter(|s| !s.is_empty())
    } else {
        None
    };

    Ok((markets, next_cursor))
}
