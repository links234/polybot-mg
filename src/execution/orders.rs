use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

/// Order management system with strongly typed operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderManager {
    pub config: OrderConfig,
    pub statistics: OrderStatistics,
}

/// Configuration for order operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderConfig {
    pub default_timeout_seconds: u64,
    pub max_retry_attempts: usize,
    pub enable_order_validation: bool,
    pub enable_detailed_logging: bool,
}

/// Order operation statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrderStatistics {
    pub orders_placed: usize,
    pub orders_cancelled: usize,
    pub successful_orders: usize,
    pub failed_orders: usize,
    pub total_volume_traded: f64,
    pub session_start_time: Option<DateTime<Utc>>,
}

/// Strongly typed order placement response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderPlacementResponse {
    pub success: bool,
    pub order_id: Option<String>,
    pub error_message: Option<String>,
    pub order_details: Option<PlacedOrderDetails>,
    pub placement_time: DateTime<Utc>,
}

/// Details of a successfully placed order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacedOrderDetails {
    pub order_id: String,
    pub token_id: String,
    pub side: OrderSide,
    pub price: f64,
    pub size: f64,
    pub status: OrderStatus,
    pub created_at: DateTime<Utc>,
    pub estimated_fees: Option<f64>,
    pub market_price_at_placement: Option<f64>,
}

/// Order cancellation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderCancellationResponse {
    pub success: bool,
    pub order_id: String,
    pub error_message: Option<String>,
    pub cancellation_time: DateTime<Utc>,
    pub was_partially_filled: bool,
    pub filled_amount: Option<f64>,
}

/// Enhanced order information with additional metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedOrder {
    pub id: String,
    pub asset_id: String,
    pub side: OrderSide,
    pub price: f64,
    pub size: f64,
    pub original_size: f64,
    pub filled_size: f64,
    pub remaining_size: f64,
    pub status: OrderStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub filled_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub fees_paid: Option<f64>,
    pub average_fill_price: Option<f64>,
    pub market_info: Option<OrderMarketInfo>,
    /// Additional fields from API
    #[serde(flatten)]
    pub additional_fields: HashMap<String, serde_json::Value>,
}

/// Market information at time of order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderMarketInfo {
    pub market_question: Option<String>,
    pub token_outcome: Option<String>,
    pub market_price_at_order: Option<f64>,
    pub spread_at_order: Option<f64>,
    pub liquidity_at_order: Option<f64>,
}

/// Order side enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OrderSide {
    #[serde(rename = "BUY")]
    Buy,
    #[serde(rename = "SELL")]
    Sell,
}

/// Order status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OrderStatus {
    #[serde(rename = "OPEN")]
    Open,
    #[serde(rename = "FILLED")]
    Filled,
    #[serde(rename = "CANCELLED")]
    Cancelled,
    #[serde(rename = "PARTIALLY_FILLED")]
    PartiallyFilled,
    #[serde(rename = "REJECTED")]
    Rejected,
    #[serde(rename = "PENDING")]
    Pending,
}

/// Order list response with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderListResponse {
    pub orders: Vec<EnhancedOrder>,
    pub total_count: usize,
    pub filtered_count: usize,
    pub query_time: DateTime<Utc>,
    pub filters_applied: OrderFilters,
}

/// Filters for order queries
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrderFilters {
    pub token_id: Option<String>,
    pub side: Option<OrderSide>,
    pub status: Option<OrderStatus>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
    pub min_price: Option<f64>,
    pub max_price: Option<f64>,
}

impl Default for OrderConfig {
    fn default() -> Self {
        Self {
            default_timeout_seconds: 30,
            max_retry_attempts: 3,
            enable_order_validation: true,
            enable_detailed_logging: true,
        }
    }
}

impl OrderManager {
    pub fn new() -> Self {
        Self {
            config: OrderConfig::default(),
            statistics: OrderStatistics {
                session_start_time: Some(Utc::now()),
                ..Default::default()
            },
        }
    }

    /// Fetch orders directly from the Polymarket API using HTTP authentication
    pub async fn fetch_orders(
        &self,
        host: &str,
        data_paths: &crate::data_paths::DataPaths,
        user_address: &str,
    ) -> Result<Vec<EnhancedOrder>> {
        use crate::config;
        use crate::portfolio::orders_api::build_auth_headers;
        use anyhow::anyhow;

        info!(
            "Fetching orders from Polymarket API for user: {}",
            user_address
        );

        // Load credentials
        let api_creds = config::load_credentials(data_paths)
            .await
            .map_err(|e| anyhow!("No credentials found. Run 'cargo run -- init' first: {}", e))?;

        // Build the API URL
        let api_url = format!("{}/data/orders", host.trim_end_matches('/'));
        info!("Fetching orders from: {}", api_url);

        // Build authentication headers
        let headers = build_auth_headers(
            &api_creds.api_key,
            &api_creds.secret,
            &api_creds.passphrase,
            user_address,
            "GET",
            "/data/orders",
            None,
        )?;

        // Create HTTP client and make request
        let client = reqwest::Client::new();
        let response = client
            .get(&api_url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to send request: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "No error details".to_string());
            return Err(anyhow!(
                "API request failed with status {}: {}",
                status,
                error_text
            ));
        }

        // Get the response text
        let response_text = response
            .text()
            .await
            .map_err(|e| anyhow!("Failed to get response text: {}", e))?;

        // Parse as API response object
        #[derive(serde::Deserialize)]
        struct ApiResponse {
            data: Vec<crate::portfolio::orders_api::PolymarketOrder>,
            #[allow(dead_code)]
            next_cursor: Option<String>,
            #[allow(dead_code)]
            limit: u32,
            #[allow(dead_code)]
            count: u32,
        }

        let api_response: ApiResponse = serde_json::from_str(&response_text).map_err(|e| {
            anyhow!(
                "Failed to parse response JSON: {}. Response was: {}",
                e,
                response_text
            )
        })?;

        // Convert PolymarketOrder to EnhancedOrder
        let enhanced_orders: Vec<EnhancedOrder> = api_response
            .data
            .into_iter()
            .map(|poly_order| self.convert_polymarket_order_to_enhanced(poly_order))
            .collect();

        info!(
            "Successfully fetched and converted {} orders",
            enhanced_orders.len()
        );
        Ok(enhanced_orders)
    }

    /// Convert a PolymarketOrder to an EnhancedOrder
    fn convert_polymarket_order_to_enhanced(
        &self,
        poly_order: crate::portfolio::orders_api::PolymarketOrder,
    ) -> EnhancedOrder {
        use chrono::{TimeZone, Utc};
        use rust_decimal::prelude::ToPrimitive;

        // Parse side
        let side = match poly_order.side.as_str() {
            "BUY" => OrderSide::Buy,
            "SELL" => OrderSide::Sell,
            _ => OrderSide::Buy, // Default fallback
        };

        // Parse status
        let status = match poly_order.status.as_str() {
            "OPEN" => OrderStatus::Open,
            "FILLED" => OrderStatus::Filled,
            "CANCELLED" => OrderStatus::Cancelled,
            "PARTIALLY_FILLED" => OrderStatus::PartiallyFilled,
            "REJECTED" => OrderStatus::Rejected,
            "PENDING" => OrderStatus::Pending,
            _ => OrderStatus::Open, // Default fallback
        };

        // Convert timestamps (Polymarket uses Unix timestamps in seconds)
        let created_at = Utc
            .timestamp_opt(poly_order.created_at as i64, 0)
            .single()
            .unwrap_or_else(|| Utc::now());

        // Parse sizes
        let size_matched = poly_order
            .size_matched
            .parse::<rust_decimal::Decimal>()
            .unwrap_or_default();
        let filled_size = size_matched.to_f64().unwrap_or(0.0);
        let original_size = poly_order.size_structured.to_f64().unwrap_or(0.0);
        let remaining_size = original_size - filled_size;

        // Build additional fields map for extra data
        let mut additional_fields = HashMap::new();
        additional_fields.insert(
            "market".to_string(),
            serde_json::Value::String(poly_order.market.clone()),
        );
        additional_fields.insert(
            "owner".to_string(),
            serde_json::Value::String(poly_order.owner.clone()),
        );
        additional_fields.insert(
            "outcome".to_string(),
            serde_json::Value::String(poly_order.outcome.clone()),
        );
        additional_fields.insert(
            "order_type".to_string(),
            serde_json::Value::String(poly_order.order_type.clone()),
        );
        additional_fields.insert(
            "expiration".to_string(),
            serde_json::Value::String(poly_order.expiration.clone()),
        );
        additional_fields.insert(
            "maker_address".to_string(),
            serde_json::Value::String(poly_order.maker_address.clone()),
        );

        if let Some(fee_rate) = poly_order.fee_rate_bps {
            additional_fields.insert(
                "fee_rate_bps".to_string(),
                serde_json::Value::Number(serde_json::Number::from(fee_rate)),
            );
        }

        if let Some(condition_id) = poly_order.condition_id {
            additional_fields.insert(
                "condition_id".to_string(),
                serde_json::Value::String(condition_id),
            );
        }

        if let Some(question_id) = poly_order.question_id {
            additional_fields.insert(
                "question_id".to_string(),
                serde_json::Value::String(question_id),
            );
        }

        // Create market info
        let market_info = Some(OrderMarketInfo {
            market_question: Some(poly_order.market.clone()),
            token_outcome: Some(poly_order.outcome.clone()),
            market_price_at_order: None, // Not provided by API
            spread_at_order: None,       // Not provided by API
            liquidity_at_order: None,    // Not provided by API
        });

        EnhancedOrder {
            id: poly_order.id,
            asset_id: poly_order.asset_id,
            side,
            price: poly_order.price.to_f64().unwrap_or(0.0),
            size: original_size,
            original_size,
            filled_size,
            remaining_size,
            status,
            created_at,
            updated_at: None,         // Not provided by API
            filled_at: None,          // Not provided by API
            cancelled_at: None,       // Not provided by API
            fees_paid: None,          // Not provided by API
            average_fill_price: None, // Not provided by API
            market_info,
            additional_fields,
        }
    }

}
