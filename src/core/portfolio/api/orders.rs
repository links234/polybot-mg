//! Direct API implementation for fetching orders with proper typing

use crate::auth_env;
use crate::config;
use crate::data_paths::DataPaths;
use anyhow::{anyhow, Result};
use reqwest::header::{HeaderMap, HeaderValue};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json;
use tracing::{debug, info, warn};

/// User account balance information from Polymarket API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceInfo {
    pub bets: Decimal,
    pub cash: Decimal,
    pub equity_total: Decimal,
}

/// Order information from the Polymarket API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolymarketOrder {
    pub id: String,
    pub owner: String,
    pub market: String,
    pub asset_id: String,
    pub side: String,
    pub price: Decimal,
    #[serde(rename = "original_size")]
    pub size_structured: Decimal,
    #[serde(rename = "size_matched")]
    pub size_matched: String, // API returns string "0"
    pub status: String,
    #[serde(rename = "created_at")]
    pub created_at: u64,
    pub maker_address: String,
    pub outcome: String,
    pub expiration: String,
    pub order_type: String,
    #[serde(default)]
    pub associate_trades: Vec<serde_json::Value>,
    #[serde(rename = "feeRateBps")]
    pub fee_rate_bps: Option<i32>,
    pub nonce: Option<String>,
    #[serde(rename = "condition_id")]
    pub condition_id: Option<String>,
    #[serde(rename = "token_id")]
    pub token_id: Option<String>,
    #[serde(rename = "question_id")]
    pub question_id: Option<String>,
}

/// Response structure for the orders endpoint
#[derive(Debug, Deserialize)]
pub struct OrdersResponse {
    pub _orders: Vec<PolymarketOrder>,
    pub _next: Option<String>,
}


/// Fetch orders using the authenticated client's internal methods
/// Fetch user account balance from Polymarket API
pub async fn fetch_balance(
    host: &str,
    data_paths: &DataPaths,
    user_address: &str,
) -> Result<BalanceInfo> {
    // Load credentials
    let api_creds = config::load_credentials(data_paths)
        .await
        .map_err(|e| anyhow!("No credentials found. Run 'cargo run -- init' first: {}", e))?;

    debug!("Loaded credentials successfully");
    debug!("Using address: {}", user_address);

    // Try different potential balance endpoints
    let endpoints_to_try = vec![
        ("/balance", "balance"),
        ("/data/balance", "data/balance"),
        ("/user/balance", "user/balance"),
        ("/account/balance", "account/balance"),
        ("/user", "user"),
        ("/account", "account"),
        ("/positions", "positions"),
        ("/user/positions", "user/positions"),
    ];

    for (endpoint_path, endpoint_name) in endpoints_to_try {
        let api_url = format!("{}{}", host.trim_end_matches('/'), endpoint_path);
        info!("Trying balance endpoint: {} ({})", api_url, endpoint_name);

        // Build authentication headers
        let headers = build_auth_headers(
            &api_creds.api_key,
            &api_creds.secret,
            &api_creds.passphrase,
            user_address,
            "GET",
            endpoint_path,
            None,
        )?;

        let client = reqwest::Client::new();
        let response = client
            .get(&api_url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to send request: {}", e))?;

        let status = response.status();
        debug!("Balance response status for {}: {}", endpoint_name, status);

        if status.is_success() {
            // Parse the response
            let response_text = response
                .text()
                .await
                .map_err(|e| anyhow!("Failed to get response text: {}", e))?;

            debug!(
                "Balance API response from {}: {}",
                endpoint_name, response_text
            );
            info!(
                "Successful balance response from {}: {}",
                endpoint_name,
                &response_text[..response_text.len().min(200)]
            );

            // Try to parse as balance info
            if let Ok(balance) = serde_json::from_str::<BalanceInfo>(&response_text) {
                info!("Successfully parsed balance from {}", endpoint_name);
                return Ok(balance);
            }

            // If direct parsing fails, check if it's wrapped in an object
            if let Ok(wrapped) = serde_json::from_str::<serde_json::Value>(&response_text) {
                // Try extracting from different possible wrapper structures
                if let Some(data) = wrapped.get("data") {
                    if let Ok(balance) = serde_json::from_value::<BalanceInfo>(data.clone()) {
                        info!("Successfully parsed balance from {}.data", endpoint_name);
                        return Ok(balance);
                    }
                }
                if let Some(balance_obj) = wrapped.get("balance") {
                    if let Ok(balance) = serde_json::from_value::<BalanceInfo>(balance_obj.clone())
                    {
                        info!("Successfully parsed balance from {}.balance", endpoint_name);
                        return Ok(balance);
                    }
                }

                warn!(
                    "Got response from {} but couldn't parse as balance: {}",
                    endpoint_name, response_text
                );
            }
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "No error details".to_string());
            debug!(
                "Endpoint {} failed with status {}: {}",
                endpoint_name, status, error_text
            );
        }
    }

    Err(anyhow!("All balance endpoints failed. Balance information may not be available through the documented API."))
}


/// Build authentication headers for Polymarket API
pub fn build_auth_headers(
    api_key: &str,
    api_secret: &str,
    api_passphrase: &str,
    address: &str,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();

    // Generate authentication headers using the updated auth_env module
    let auth_headers = auth_env::build_l2_headers(
        &polymarket_rs_client::ApiCreds {
            api_key: api_key.to_string(),
            secret: api_secret.to_string(),
            passphrase: api_passphrase.to_string(),
        },
        address,
        method,
        path,
        body,
    )?;

    // Convert to reqwest HeaderMap
    for (key, value) in auth_headers {
        headers.insert(
            reqwest::header::HeaderName::from_bytes(key.as_bytes())?,
            HeaderValue::from_str(&value)?,
        );
    }

    Ok(headers)
}
