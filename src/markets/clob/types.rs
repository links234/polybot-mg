use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct FetchState {
    pub last_cursor: Option<String>,
    pub last_page: usize,
    pub total_markets_fetched: usize,
    pub chunk_number: usize,
    pub markets_in_current_chunk: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct GammaFetchState {
    pub last_offset: usize,
    pub total_markets_fetched: usize,
    pub chunk_number: usize,
    pub markets_in_current_chunk: usize,
}

/// Enhanced market data with volume and liquidity
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MarketWithVolume {
    /// Original market data
    #[serde(flatten)]
    pub market: serde_json::Value,

    /// Volume data (if available)
    pub volume_24hr: Option<f64>,
    pub volume_total: Option<f64>,

    /// Liquidity data (if available)
    pub liquidity: Option<f64>,
    pub bid_liquidity: Option<f64>,
    pub ask_liquidity: Option<f64>,

    /// Timestamp when this data was fetched
    pub fetched_at: chrono::DateTime<chrono::Utc>,
}
