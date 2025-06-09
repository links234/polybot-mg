use anyhow::Result;
use owo_colors::OwoColorize;
use polymarket_rs_client::{
    ClobClient, OrderArgs, Side
};
use rust_decimal::Decimal;

/// Place a buy order
pub async fn place_buy_order(
    client: &mut ClobClient,
    token_id: &str,
    price: Decimal,
    size: Decimal,
) -> Result<()> {
    println!(
        "{}",
        format!("💰 Placing BUY order for token {}...", token_id).bright_blue()
    );
    println!(
        "   {} ${:.4} | {} {} USDC",
        "Price:".bright_black(),
        price,
        "Size:".bright_black(),
        size
    );
    
    // Create order arguments
    let args = OrderArgs {
        price,
        size,
        side: Side::BUY,
        token_id: token_id.to_string(),
    };
    
    // Create and post order
    let response = client.create_and_post_order(&args).await?;
    
    // Parse response
    if let Some(success) = response.get("success").and_then(|v| v.as_bool()) {
        if success {
            if let Some(order_id) = response.get("orderId").and_then(|v| v.as_str()) {
                println!(
                    "\n{} Order placed successfully!",
                    "✅".bright_green()
                );
                println!(
                    "   {} {}",
                    "Order ID:".bright_black(),
                    order_id.bright_yellow()
                );
            }
        } else {
            let error_msg = response.get("errorMsg")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            println!(
                "\n{} Order failed: {}",
                "❌".bright_red(),
                error_msg.bright_red()
            );
        }
    }
    
    Ok(())
}

/// Place a sell order
pub async fn place_sell_order(
    client: &mut ClobClient,
    token_id: &str,
    price: Decimal,
    size: Decimal,
) -> Result<()> {
    println!(
        "{}",
        format!("💸 Placing SELL order for token {}...", token_id).bright_blue()
    );
    println!(
        "   {} ${:.4} | {} {} USDC",
        "Price:".bright_black(),
        price,
        "Size:".bright_black(),
        size
    );
    
    // Create order arguments
    let args = OrderArgs {
        price,
        size,
        side: Side::SELL,
        token_id: token_id.to_string(),
    };
    
    // Create and post order
    let response = client.create_and_post_order(&args).await?;
    
    // Parse response
    if let Some(success) = response.get("success").and_then(|v| v.as_bool()) {
        if success {
            if let Some(order_id) = response.get("orderId").and_then(|v| v.as_str()) {
                println!(
                    "\n{} Order placed successfully!",
                    "✅".bright_green()
                );
                println!(
                    "   {} {}",
                    "Order ID:".bright_black(),
                    order_id.bright_yellow()
                );
            }
        } else {
            let error_msg = response.get("errorMsg")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            println!(
                "\n{} Order failed: {}",
                "❌".bright_red(),
                error_msg.bright_red()
            );
        }
    }
    
    Ok(())
}

/// Cancel an order
pub async fn cancel_order(client: &mut ClobClient, order_id: &str) -> Result<()> {
    println!(
        "{}",
        format!("🚫 Cancelling order {}...", order_id).bright_blue()
    );
    
    // Cancel the order
    let response = client.cancel(order_id).await?;
    
    // Check response
    if let Some(success) = response.get("success").and_then(|v| v.as_bool()) {
        if success {
            println!(
                "\n{} Order cancelled successfully!",
                "✅".bright_green()
            );
        } else {
            let error_msg = response.get("errorMsg")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            println!(
                "\n{} Failed to cancel order: {}",
                "❌".bright_red(),
                error_msg.bright_red()
            );
        }
    }
    
    Ok(())
}

/// List open orders
pub async fn list_orders(
    client: ClobClient,
    token_id: Option<String>,
) -> Result<()> {
    println!("{}", "📋 Fetching open orders...".bright_blue());
    
    // Fetch orders - the API doesn't have a direct filter by token_id
    let orders = client.get_orders(None, None).await?;
    
    // Filter by token_id if provided
    let filtered_orders: Vec<_> = if let Some(token) = token_id {
        orders.into_iter()
            .filter(|o| o.asset_id == token)
            .collect()
    } else {
        orders
    };
    
    if filtered_orders.is_empty() {
        println!("{}", "No open orders found.".yellow());
        return Ok(());
    }
    
    // Display orders
    println!(
        "\n{}",
        format!("Found {} open orders:", filtered_orders.len()).bright_green()
    );
    println!("{}", "─".repeat(100).bright_black());
    
    // Header
    println!(
        "{:<15} {:<15} {:<6} {:>10} {:>10} {:<10} {:<20}",
        "Order ID".bright_white(),
        "Token ID".bright_white(),
        "Side".bright_white(),
        "Price".bright_white(),
        "Size".bright_white(),
        "Status".bright_white(),
        "Created".bright_white(),
    );
    println!("{}", "─".repeat(100).bright_black());
    
    // Orders
    for order in filtered_orders {
        // Format side with color
        let side_display = match order.side {
            Side::BUY => "BUY".bright_green().to_string(),
            Side::SELL => "SELL".bright_red().to_string(),
        };
        
        // Format status with color
        let status_display = match order.status.to_lowercase().as_str() {
            "open" => order.status.bright_green().to_string(),
            "filled" => order.status.bright_blue().to_string(),
            "cancelled" => order.status.bright_red().to_string(),
            _ => order.status.clone(),
        };
        
        // Truncate order ID for display
        let order_id_display = if order.id.len() > 12 {
            format!("{}...", &order.id[..12])
        } else {
            order.id.clone()
        };
        
        // Truncate token ID for display
        let token_id_display = if order.asset_id.len() > 12 {
            format!("{}...", &order.asset_id[..12])
        } else {
            order.asset_id.clone()
        };
        
        println!(
            "{:<15} {:<15} {:<6} {:>10} {:>10} {:<10} {:<20}",
            order_id_display.bright_yellow(),
            token_id_display,
            side_display,
            format!("${}", order.price),
            order.original_size,
            status_display,
            order.created_at.bright_black(),
        );
    }
    
    Ok(())
} 