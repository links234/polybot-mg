//! Market data provider trait definitions

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

/// Trait for market data providers
/// 
/// This trait abstracts different sources of market data (CLOB API, Gamma API, etc.)
/// allowing for a unified interface to fetch market information.
#[async_trait]
pub trait MarketDataProvider: Send + Sync {
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
    
    /// Get provider statistics (optional)
    async fn get_stats(&self) -> ProviderStats {
        ProviderStats::default()
    }
    
    /// Reset the provider to start from the beginning
    async fn reset(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Statistics for a market data provider
#[derive(Debug, Clone, Default)]
pub struct ProviderStats {
    /// Total markets fetched
    pub total_markets: usize,
    
    /// Total pages fetched
    pub pages_fetched: usize,
    
    /// Total API calls made
    pub api_calls: usize,
    
    /// Errors encountered
    pub error_count: usize,
    
    /// Average response time in milliseconds
    pub avg_response_time_ms: Option<f64>,
}