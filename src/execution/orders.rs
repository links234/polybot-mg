use anyhow::Result;
use tracing::{info, warn, error};
use polymarket_rs_client::{
    ClobClient, OrderArgs, Side
};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

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



    /// Place a buy order with comprehensive response handling
    pub async fn place_buy_order(
        &mut self,
        client: &mut ClobClient,
        token_id: &str,
        price: Decimal,
        size: Decimal,
    ) -> Result<OrderPlacementResponse> {
        self.place_order_internal(client, token_id, price, size, OrderSide::Buy).await
    }

    /// Place a sell order with comprehensive response handling
    pub async fn place_sell_order(
        &mut self,
        client: &mut ClobClient,
        token_id: &str,
        price: Decimal,
        size: Decimal,
    ) -> Result<OrderPlacementResponse> {
        self.place_order_internal(client, token_id, price, size, OrderSide::Sell).await
    }

    /// Internal order placement logic
    async fn place_order_internal(
        &mut self,
        client: &mut ClobClient,
        token_id: &str,
        price: Decimal,
        size: Decimal,
        side: OrderSide,
    ) -> Result<OrderPlacementResponse> {
        let placement_time = Utc::now();
        
        // Display order information
        let side_display = match side {
            OrderSide::Buy => "💰 BUY".to_string(),
            OrderSide::Sell => "💸 SELL".to_string(),
        };
        
        info!("{} order for token {}...", side_display, token_id);
        info!("   Price: ${:.4} | Size: {} USDC", price, size);
        
        // Create order arguments
        let polymarket_side = match side {
            OrderSide::Buy => Side::BUY,
            OrderSide::Sell => Side::SELL,
        };
        
        let args = OrderArgs {
            price,
            size,
            side: polymarket_side,
            token_id: token_id.to_string(),
        };
        
        // Update statistics
        self.statistics.orders_placed += 1;
        
        // Create and post order
        let response = client.create_and_post_order(&args).await?;
        let parsed_response = Self::parse_order_response(response, token_id, side, price, size, placement_time)?;
        
        // Update statistics based on result
        if parsed_response.success {
            self.statistics.successful_orders += 1;
            self.statistics.total_volume_traded += size.to_f64().unwrap_or(0.0);
        } else {
            self.statistics.failed_orders += 1;
        }
        
        // Display result
        self.display_order_result(&parsed_response);
        
        Ok(parsed_response)
    }

    /// Cancel an order with comprehensive response handling
    pub async fn cancel_order(&mut self, client: &mut ClobClient, order_id: &str) -> Result<OrderCancellationResponse> {
        let cancellation_time = Utc::now();
        
        info!("🚫 Cancelling order {}...", order_id);
        
        // Cancel the order
        let response = client.cancel(order_id).await?;
        let parsed_response = Self::parse_cancellation_response(response, order_id, cancellation_time)?;
        
        // Update statistics
        self.statistics.orders_cancelled += 1;
        
        // Display result
        self.display_cancellation_result(&parsed_response);
        
        Ok(parsed_response)
    }

    /// List orders with enhanced filtering and typing
    pub async fn list_orders(
        &self,
        client: ClobClient,
        filters: OrderFilters,
    ) -> Result<OrderListResponse> {
        let query_time = Utc::now();
        
        info!("📋 Fetching open orders...");
        
        // Fetch orders from API
        let raw_orders = client.get_orders(None, None).await?;
        
        // Convert directly to enhanced orders (skip JSON conversion due to external type constraints)
        let enhanced_orders = self.convert_raw_orders_to_enhanced(raw_orders);
        
        // Apply filters
        let filtered_orders = self.apply_filters(&enhanced_orders, &filters);
        
        let response = OrderListResponse {
            total_count: enhanced_orders.len(),
            filtered_count: filtered_orders.len(),
            orders: filtered_orders,
            query_time,
            filters_applied: filters,
        };
        
        // Display orders
        self.display_order_list(&response);
        
        Ok(response)
    }

    /// Parse order placement response
    fn parse_order_response(
        response: serde_json::Value,
        token_id: &str,
        side: OrderSide,
        price: Decimal,
        size: Decimal,
        placement_time: DateTime<Utc>,
    ) -> Result<OrderPlacementResponse> {
        let success = response.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
        
        let order_details = if success {
            let order_id = response.get("orderId")
                .and_then(|v| v.as_str())
                .map(String::from);
            
            order_id.map(|id| PlacedOrderDetails {
                order_id: id,
                token_id: token_id.to_string(),
                side,
                price: price.to_f64().unwrap_or(0.0),
                size: size.to_f64().unwrap_or(0.0),
                status: OrderStatus::Open,
                created_at: placement_time,
                estimated_fees: None,
                market_price_at_placement: None,
            })
        } else {
            None
        };
        
        let error_message = if !success {
            response.get("errorMsg")
                .and_then(|v| v.as_str())
                .map(String::from)
                .or_else(|| Some("Unknown error occurred".to_string()))
        } else {
            None
        };
        
        Ok(OrderPlacementResponse {
            success,
            order_id: order_details.as_ref().map(|d| d.order_id.clone()),
            error_message,
            order_details,
            placement_time,
        })
    }

    /// Parse order cancellation response
    fn parse_cancellation_response(
        response: serde_json::Value,
        order_id: &str,
        cancellation_time: DateTime<Utc>,
    ) -> Result<OrderCancellationResponse> {
        let success = response.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
        
        let error_message = if !success {
            response.get("errorMsg")
                .and_then(|v| v.as_str())
                .map(String::from)
                .or_else(|| Some("Unknown error occurred".to_string()))
        } else {
            None
        };
        
        Ok(OrderCancellationResponse {
            success,
            order_id: order_id.to_string(),
            error_message,
            cancellation_time,
            was_partially_filled: false, // Would need to get this from order status
            filled_amount: None,
        })
    }

    /// Convert external order types directly to enhanced orders
    fn convert_raw_orders_to_enhanced(&self, raw_orders: Vec<impl std::fmt::Debug>) -> Vec<EnhancedOrder> {
        // Note: External OpenOrder type doesn't implement Serialize
        // Would need to access fields directly from the polymarket_rs_client::Order type
        warn!("⚠️  Order conversion temporarily disabled due to external type constraints");
        info!("   Found {} raw orders from API", raw_orders.len());
        Vec::new()
    }



    /// Apply filters to order list
    fn apply_filters(&self, orders: &[EnhancedOrder], filters: &OrderFilters) -> Vec<EnhancedOrder> {
        orders.iter()
            .filter(|order| {
                // Token ID filter
                if let Some(ref token_id) = filters.token_id {
                    if order.asset_id != *token_id {
                        return false;
                    }
                }
                
                // Side filter
                if let Some(ref side) = filters.side {
                    if order.side != *side {
                        return false;
                    }
                }
                
                // Status filter
                if let Some(ref status) = filters.status {
                    if order.status != *status {
                        return false;
                    }
                }
                
                // Price filters
                if let Some(min_price) = filters.min_price {
                    if order.price < min_price {
                        return false;
                    }
                }
                
                if let Some(max_price) = filters.max_price {
                    if order.price > max_price {
                        return false;
                    }
                }
                
                // Date filters
                if let Some(created_after) = filters.created_after {
                    if order.created_at < created_after {
                        return false;
                    }
                }
                
                if let Some(created_before) = filters.created_before {
                    if order.created_at > created_before {
                        return false;
                    }
                }
                
                true
            })
            .cloned()
            .collect()
    }

    /// Display order placement result
    fn display_order_result(&self, response: &OrderPlacementResponse) {
        if response.success {
            info!("\n✅ Order placed successfully!");
            if let Some(ref order_id) = response.order_id {
                info!("   Order ID: {}", order_id);
            }
            if let Some(ref details) = response.order_details {
                info!("   Details: {:.2} USDC @ ${:.4}", details.size, details.price);
            }
        } else {
            error!("\n❌ Order failed: {}", response.error_message.as_deref().unwrap_or("Unknown error"));
        }
    }

    /// Display order cancellation result
    fn display_cancellation_result(&self, response: &OrderCancellationResponse) {
        if response.success {
            info!("\n✅ Order cancelled successfully!");
            if response.was_partially_filled {
                if let Some(filled) = response.filled_amount {
                    info!("   Note: {:.2} USDC was filled before cancellation", filled);
                }
            }
        } else {
            error!("\n❌ Failed to cancel order: {}", response.error_message.as_deref().unwrap_or("Unknown error"));
        }
    }

    /// Display order list
    fn display_order_list(&self, response: &OrderListResponse) {
        if response.orders.is_empty() {
            info!("No orders found matching criteria.");
            return;
        }
        
        info!("\nFound {} orders (filtered from {}):", response.filtered_count, response.total_count);
        info!("{}", "─".repeat(120));
        
        // Header
        info!(
            "{:<15} {:<15} {:<6} {:>10} {:>10} {:>10} {:<12} {:<20}",
            "Order ID",
            "Token ID",
            "Side",
            "Price",
            "Size",
            "Filled",
            "Status",
            "Created",
        );
        info!("{}", "─".repeat(120));
        
        // Orders
        for order in &response.orders {
            // Format side
            let side_display = match order.side {
                OrderSide::Buy => "BUY".to_string(),
                OrderSide::Sell => "SELL".to_string(),
            };
            
            // Format status
            let status_display = match order.status {
                OrderStatus::Open => "OPEN".to_string(),
                OrderStatus::Filled => "FILLED".to_string(),
                OrderStatus::Cancelled => "CANCELLED".to_string(),
                OrderStatus::PartiallyFilled => "PARTIAL".to_string(),
                OrderStatus::Rejected => "REJECTED".to_string(),
                OrderStatus::Pending => "PENDING".to_string(),
            };
            
            // Truncate IDs for display
            let order_id_display = if order.id.len() > 12 {
                format!("{}...", &order.id[..12])
            } else {
                order.id.clone()
            };
            
            let token_id_display = if order.asset_id.len() > 12 {
                format!("{}...", &order.asset_id[..12])
            } else {
                order.asset_id.clone()
            };
            
            // Format date
            let created_display = order.created_at.format("%m-%d %H:%M:%S").to_string();
            
            info!(
                "{:<15} {:<15} {:<6} {:>10} {:>10} {:>10} {:<12} {:<20}",
                order_id_display,
                token_id_display,
                side_display,
                format!("${:.4}", order.price),
                format!("{:.2}", order.remaining_size),
                format!("{:.2}", order.filled_size),
                status_display,
                created_display,
            );
        }
        
        // Summary
        if self.config.enable_detailed_logging {
            info!("\nOrder Summary:");
            let buy_count = response.orders.iter().filter(|o| o.side == OrderSide::Buy).count();
            let sell_count = response.orders.iter().filter(|o| o.side == OrderSide::Sell).count();
            let total_volume: f64 = response.orders.iter().map(|o| o.original_size).sum();
            
            info!("  Buy orders: {} | Sell orders: {} | Total volume: {:.2} USDC", 
                     buy_count, sell_count, total_volume);
        }
    }


}

/// Legacy function wrappers for backward compatibility
pub async fn place_buy_order(
    client: &mut ClobClient,
    token_id: &str,
    price: Decimal,
    size: Decimal,
) -> Result<()> {
    let mut manager = OrderManager::new();
    manager.place_buy_order(client, token_id, price, size).await?;
    Ok(())
}

pub async fn place_sell_order(
    client: &mut ClobClient,
    token_id: &str,
    price: Decimal,
    size: Decimal,
) -> Result<()> {
    let mut manager = OrderManager::new();
    manager.place_sell_order(client, token_id, price, size).await?;
    Ok(())
}

pub async fn cancel_order(client: &mut ClobClient, order_id: &str) -> Result<()> {
    let mut manager = OrderManager::new();
    manager.cancel_order(client, order_id).await?;
    Ok(())
}

pub async fn list_orders(
    client: ClobClient,
    token_id: Option<String>,
) -> Result<()> {
    let manager = OrderManager::new();
    let filters = OrderFilters {
        token_id,
        ..Default::default()
    };
    manager.list_orders(client, filters).await?;
    Ok(())
} 