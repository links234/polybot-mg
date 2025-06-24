//! Gamma API client implementation

#[allow(dead_code)]

use anyhow::{Result, Context};
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use tracing::{info, debug, warn};

use super::types::*;

/// Gamma API endpoints
#[allow(dead_code)]
pub struct GammaEndpoints {
    /// Base URL for Gamma API
    pub base_url: String,
    
    /// Positions endpoint
    pub positions: String,
    
    /// Activity endpoint
    pub activity: String,
    
    /// Holders endpoint
    pub holders: String,
}

impl Default for GammaEndpoints {
    fn default() -> Self {
        let base = "https://data-api.polymarket.com".to_string();
        Self {
            positions: format!("{}/positions", base),
            activity: format!("{}/activity", base),
            holders: format!("{}/holders", base),
            base_url: base,
        }
    }
}

/// Gamma API client
#[allow(dead_code)]
pub struct GammaApiClient {
    /// HTTP client
    client: Client,
    
    /// API endpoints
    endpoints: GammaEndpoints,
    
    /// Request timeout
    timeout: Duration,
}

#[allow(dead_code)]
impl GammaApiClient {
    /// Create new Gamma API client
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;
        
        Ok(Self {
            client,
            endpoints: GammaEndpoints::default(),
            timeout: Duration::from_secs(30),
        })
    }
    
    /// Create client with custom endpoints
    pub fn with_endpoints(endpoints: GammaEndpoints) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;
        
        Ok(Self {
            client,
            endpoints,
            timeout: Duration::from_secs(30),
        })
    }
    
    /// Fetch positions for an address
    pub async fn get_positions(&self, address: &str, query: Option<GammaQuery>) -> Result<Vec<GammaPosition>> {
        let mut url = format!("{}?user={}", self.endpoints.positions, address);
        
        if let Some(q) = query {
            url = self.add_query_params(url, &q);
        }
        
        debug!("Fetching positions from: {}", url);
        
        let response = self.client
            .get(&url)
            .timeout(self.timeout)
            .send()
            .await
            .context("Failed to send positions request")?;
        
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            debug!("Gamma API positions error - Status: {}, Body: {}", status, text);
            return Err(GammaError::ApiError(format!("HTTP {}: {}", status, text)).into());
        }
        
        let body = response.text().await
            .context("Failed to read positions response")?;
        
        debug!("Gamma API positions response body: {}", body);
        
        // Parse the response - it might be a direct array or wrapped in a data field
        let positions = if body.trim_start().starts_with('[') {
            // Direct array
            serde_json::from_str::<Vec<GammaPosition>>(&body)
                .context("Failed to parse positions response")?
        } else {
            // Try to parse as wrapped response
            let value: Value = serde_json::from_str(&body)
                .context("Failed to parse JSON response")?;
            
            if let Some(data) = value.get("data") {
                serde_json::from_value(data.clone())
                    .context("Failed to parse positions data")?
            } else {
                // Try to parse the whole response as positions
                serde_json::from_value(value)
                    .context("Failed to parse positions from response")?
            }
        };
        
        info!("Fetched {} positions for address {}", positions.len(), address);
        Ok(positions)
    }
    
    /// Fetch activity for an address
    pub async fn get_activity(&self, address: &str, query: Option<GammaQuery>) -> Result<Vec<GammaActivity>> {
        let mut url = format!("{}?user={}", self.endpoints.activity, address);
        
        if let Some(q) = query {
            url = self.add_query_params(url, &q);
        }
        
        debug!("Fetching activity from: {}", url);
        
        let response = self.client
            .get(&url)
            .timeout(self.timeout)
            .send()
            .await
            .context("Failed to send activity request")?;
        
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            debug!("Gamma API activity error - Status: {}, Body: {}", status, text);
            return Err(GammaError::ApiError(format!("HTTP {}: {}", status, text)).into());
        }
        
        let body = response.text().await
            .context("Failed to read activity response")?;
        
        debug!("Gamma API activity response body: {}", body);
        
        // Parse the response - handle different response formats
        let activity = if body.trim_start().starts_with('[') {
            // Direct array
            serde_json::from_str::<Vec<GammaActivity>>(&body)
                .context("Failed to parse activity response")?
        } else {
            // Try to parse as wrapped response
            let value: Value = serde_json::from_str(&body)
                .context("Failed to parse JSON response")?;
            
            if let Some(data) = value.get("data") {
                serde_json::from_value(data.clone())
                    .context("Failed to parse activity data")?
            } else {
                // Try to parse the whole response as activity
                serde_json::from_value(value)
                    .context("Failed to parse activity from response")?
            }
        };
        
        info!("Fetched {} activities for address {}", activity.len(), address);
        Ok(activity)
    }
    
    /// Fetch holders information for a market
    pub async fn get_holders(&self, market: &str, query: Option<GammaQuery>) -> Result<Vec<GammaHolder>> {
        let mut url = format!("{}?market={}", self.endpoints.holders, market);
        
        if let Some(q) = query {
            url = self.add_query_params(url, &q);
        }
        
        debug!("Fetching holders from: {}", url);
        
        let response = self.client
            .get(&url)
            .timeout(self.timeout)
            .send()
            .await
            .context("Failed to send holders request")?;
        
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(GammaError::ApiError(format!("HTTP {}: {}", status, text)).into());
        }
        
        let body = response.text().await
            .context("Failed to read holders response")?;
        
        // Parse the response
        let holders = if body.trim_start().starts_with('[') {
            // Direct array
            serde_json::from_str::<Vec<GammaHolder>>(&body)
                .context("Failed to parse holders response")?
        } else {
            // Try to parse as wrapped response
            let value: Value = serde_json::from_str(&body)
                .context("Failed to parse JSON response")?;
            
            if let Some(data) = value.get("data") {
                serde_json::from_value(data.clone())
                    .context("Failed to parse holders data")?
            } else {
                // Try to parse the whole response as holders
                serde_json::from_value(value)
                    .context("Failed to parse holders from response")?
            }
        };
        
        info!("Fetched {} holders for market {}", holders.len(), market);
        Ok(holders)
    }
    
    /// Get comprehensive user data (positions + activity)
    pub async fn get_user_data(&self, address: &str, query: Option<GammaQuery>) -> Result<(Vec<GammaPosition>, Vec<GammaActivity>)> {
        info!("Fetching comprehensive data for address: {}", address);
        
        // Fetch positions and activity in parallel
        let positions_future = self.get_positions(address, query.clone());
        let activity_future = self.get_activity(address, query);
        
        let (positions_result, activity_result) = tokio::try_join!(positions_future, activity_future)?;
        
        Ok((positions_result, activity_result))
    }
    
    /// Get ALL positions for an address with pagination
    pub async fn get_all_positions(&self, address: &str, base_query: Option<GammaQuery>) -> Result<Vec<GammaPosition>> {
        info!("Fetching ALL positions for address: {}", address);
        
        let mut all_positions = Vec::new();
        let mut current_cursor: Option<String> = None;
        let mut page_count = 0;
        let max_pages = 50; // Safety limit to prevent infinite loops
        
        loop {
            page_count += 1;
            if page_count > max_pages {
                warn!("Reached maximum page limit ({}) for positions fetch, stopping", max_pages);
                break;
            }
            
            // Build query for this page
            let mut query = base_query.clone().unwrap_or_default();
            query.limit = Some(1000); // Use large page size
            query.cursor = current_cursor.clone();
            
            debug!("Fetching positions page {} for {} (cursor: {:?})", page_count, address, current_cursor);
            
            let url = format!("{}?user={}", self.endpoints.positions, address);
            let url = self.add_query_params(url, &query);
            
            let response = self.client
                .get(&url)
                .timeout(self.timeout)
                .send()
                .await
                .context("Failed to send positions request")?;
            
            if !response.status().is_success() {
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                return Err(GammaError::ApiError(format!("HTTP {}: {}", status, text)).into());
            }
            
            let body = response.text().await
                .context("Failed to read positions response")?;
            
            // Try to parse as paginated response first
            if let Ok(paginated) = serde_json::from_str::<crate::markets::gamma_api::types::GammaResponse<Vec<GammaPosition>>>(&body) {
                all_positions.extend(paginated.data);
                
                if let Some(next_cursor) = paginated.next_cursor {
                    current_cursor = Some(next_cursor);
                    info!("Page {} complete: {} positions fetched, continuing with next page", page_count, all_positions.len());
                } else {
                    info!("Pagination complete after {} pages: {} total positions", page_count, all_positions.len());
                    break;
                }
            } else {
                // Fall back to parsing as direct array (non-paginated response)
                let positions = if body.trim_start().starts_with('[') {
                    serde_json::from_str::<Vec<GammaPosition>>(&body)
                        .context("Failed to parse positions response")?
                } else {
                    let value: serde_json::Value = serde_json::from_str(&body)
                        .context("Failed to parse JSON response")?;
                    
                    if let Some(data) = value.get("data") {
                        serde_json::from_value(data.clone())
                            .context("Failed to parse positions data")?
                    } else {
                        serde_json::from_value(value)
                            .context("Failed to parse positions from response")?
                    }
                };
                
                all_positions.extend(positions);
                info!("Non-paginated response: {} total positions", all_positions.len());
                break;
            }
        }
        
        info!("Fetched ALL {} positions for address {}", all_positions.len(), address);
        Ok(all_positions)
    }
    
    /// Get ALL activity for an address with pagination
    pub async fn get_all_activity(&self, address: &str, base_query: Option<GammaQuery>) -> Result<Vec<GammaActivity>> {
        info!("Fetching ALL activity for address: {}", address);
        
        let mut all_activity = Vec::new();
        let mut current_cursor: Option<String> = None;
        let mut page_count = 0;
        let max_pages = 100; // Higher limit for activity as it can be very large
        
        loop {
            page_count += 1;
            if page_count > max_pages {
                warn!("Reached maximum page limit ({}) for activity fetch, stopping", max_pages);
                break;
            }
            
            // Build query for this page
            let mut query = base_query.clone().unwrap_or_default();
            query.limit = Some(1000); // Use large page size
            query.cursor = current_cursor.clone();
            
            debug!("Fetching activity page {} for {} (cursor: {:?})", page_count, address, current_cursor);
            
            let url = format!("{}?user={}", self.endpoints.activity, address);
            let url = self.add_query_params(url, &query);
            
            let response = self.client
                .get(&url)
                .timeout(self.timeout)
                .send()
                .await
                .context("Failed to send activity request")?;
            
            if !response.status().is_success() {
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                return Err(GammaError::ApiError(format!("HTTP {}: {}", status, text)).into());
            }
            
            let body = response.text().await
                .context("Failed to read activity response")?;
            
            // Try to parse as paginated response first
            if let Ok(paginated) = serde_json::from_str::<crate::markets::gamma_api::types::GammaResponse<Vec<GammaActivity>>>(&body) {
                all_activity.extend(paginated.data);
                
                if let Some(next_cursor) = paginated.next_cursor {
                    current_cursor = Some(next_cursor);
                    info!("Page {} complete: {} activities fetched, continuing with next page", page_count, all_activity.len());
                } else {
                    info!("Pagination complete after {} pages: {} total activities", page_count, all_activity.len());
                    break;
                }
            } else {
                // Fall back to parsing as direct array (non-paginated response)
                let activity = if body.trim_start().starts_with('[') {
                    serde_json::from_str::<Vec<GammaActivity>>(&body)
                        .context("Failed to parse activity response")?
                } else {
                    let value: serde_json::Value = serde_json::from_str(&body)
                        .context("Failed to parse JSON response")?;
                    
                    if let Some(data) = value.get("data") {
                        serde_json::from_value(data.clone())
                            .context("Failed to parse activity data")?
                    } else {
                        serde_json::from_value(value)
                            .context("Failed to parse activity from response")?
                    }
                };
                
                all_activity.extend(activity);
                info!("Non-paginated response: {} total activities", all_activity.len());
                break;
            }
        }
        
        info!("Fetched ALL {} activities for address {}", all_activity.len(), address);
        Ok(all_activity)
    }
    
    /// Get comprehensive user data with ALL historical data via pagination
    pub async fn get_all_user_data(&self, address: &str, base_query: Option<GammaQuery>) -> Result<(Vec<GammaPosition>, Vec<GammaActivity>)> {
        info!("Fetching ALL comprehensive data for address: {}", address);
        
        // Fetch ALL positions and ALL activity in parallel
        let positions_future = self.get_all_positions(address, base_query.clone());
        let activity_future = self.get_all_activity(address, base_query);
        
        let (positions_result, activity_result) = tokio::try_join!(positions_future, activity_future)?;
        
        info!("Fetched complete dataset for {}: {} positions, {} activities", 
            address, positions_result.len(), activity_result.len());
        
        Ok((positions_result, activity_result))
    }
    
    /// Add query parameters to URL
    fn add_query_params(&self, mut url: String, query: &GammaQuery) -> String {
        let mut params = Vec::new();
        
        if let Some(limit) = query.limit {
            params.push(format!("limit={}", limit));
        }
        
        if let Some(cursor) = &query.cursor {
            params.push(format!("cursor={}", cursor));
        }
        
        if let Some(start_date) = query.start_date {
            params.push(format!("start_time={}", start_date.timestamp()));
        }
        
        if let Some(end_date) = query.end_date {
            params.push(format!("end_time={}", end_date.timestamp()));
        }
        
        if let Some(market) = &query.market {
            params.push(format!("market={}", market));
        }
        
        if let Some(asset_id) = &query.asset_id {
            params.push(format!("asset_id={}", asset_id));
        }
        
        if !params.is_empty() {
            url.push('&');
            url.push_str(&params.join("&"));
        }
        
        url
    }
    
    /// Test API connectivity
    pub async fn test_connection(&self) -> Result<bool> {
        debug!("Testing Gamma API connectivity");
        
        // Try a simple request to the positions endpoint with a dummy address
        let test_url = format!("{}?user=0x0000000000000000000000000000000000000000&limit=1", self.endpoints.positions);
        
        let response = self.client
            .get(&test_url)
            .timeout(Duration::from_secs(10))
            .send()
            .await;
        
        match response {
            Ok(resp) => {
                let is_ok = resp.status().is_success() || resp.status().as_u16() == 404; // 404 is OK for test
                info!("Gamma API connectivity test: {}", if is_ok { "OK" } else { "Failed" });
                Ok(is_ok)
            }
            Err(e) => {
                warn!("Gamma API connectivity test failed: {}", e);
                Ok(false)
            }
        }
    }
}