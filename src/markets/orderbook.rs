use anyhow::Result;
use owo_colors::OwoColorize;
use polymarket_rs_client::ClobClient;
use rust_decimal::Decimal;

/// Show orderbook for a specific token
pub async fn show_orderbook(
    client: ClobClient,
    token_id: &str,
    depth: usize,
) -> Result<()> {
    println!(
        "{}", 
        format!("📈 Fetching orderbook for token {}...", token_id).bright_blue()
    );
    
    // Fetch orderbook
    let orderbook = client.get_order_book(token_id).await?;
    
    // Calculate mid price if both sides exist
    let mid_price = if !orderbook.bids.is_empty() && !orderbook.asks.is_empty() {
        // Find the best bid and ask
        let best_bid = orderbook.bids.iter()
            .map(|b| b.price)
            .max()
            .unwrap_or(Decimal::ZERO);
        let best_ask = orderbook.asks.iter()
            .map(|a| a.price)
            .min()
            .unwrap_or(Decimal::ONE);
        
        (best_bid + best_ask) / Decimal::from(2)
    } else {
        Decimal::new(5000, 4) // Default to 0.5 if no orders
    };
    
    // Filter bids to show only those within reasonable range of mid price (e.g., within 50%)
    let price_range = mid_price * Decimal::new(5, 1); // 0.5 = 50%
    let min_bid_price = mid_price - price_range;
    let max_ask_price = mid_price + price_range;
    
    // Sort and filter bids (highest first)
    let mut relevant_bids: Vec<_> = orderbook.bids.iter()
        .filter(|b| b.price >= min_bid_price)
        .collect();
    relevant_bids.sort_by(|a, b| b.price.cmp(&a.price));
    
    // Sort and filter asks (lowest first)
    let mut relevant_asks: Vec<_> = orderbook.asks.iter()
        .filter(|a| a.price <= max_ask_price)
        .collect();
    relevant_asks.sort_by(|a, b| a.price.cmp(&b.price));
    
    // Display market summary
    if !relevant_bids.is_empty() && !relevant_asks.is_empty() {
        let best_bid = relevant_bids[0].price;
        let best_ask = relevant_asks[0].price;
        let spread = best_ask - best_bid;
        let spread_pct = (spread / best_bid) * Decimal::from(100);
        
        println!("\n{}", "MARKET SUMMARY".bright_yellow());
        println!("{}", "─".repeat(40).bright_black());
        println!(
            "{} ${:.4} / ${:.4}",
            "Best Bid/Ask:".bright_white(),
            best_bid,
            best_ask
        );
        println!(
            "{} ${:.4} ({:.2}%)",
            "Spread:".bright_white(),
            spread,
            spread_pct
        );
        println!(
            "{} ${:.4}",
            "Mid Price:".bright_white(),
            mid_price
        );
    }
    
    // Display bids
    println!("\n{}", "BIDS (Buy Orders)".bright_green());
    println!("{}", "─".repeat(40).bright_black());
    
    if relevant_bids.is_empty() {
        println!("{}", "No relevant bids near market price".italic().bright_black());
    } else {
        println!(
            "{:>10} {:>15}",
            "Price".bright_white(),
            "Size".bright_white()
        );
        println!("{}", "─".repeat(40).bright_black());
        
        for bid in relevant_bids.iter().take(depth) {
            println!(
                "{:>10} {:>15}",
                format!("${:.4}", bid.price).bright_green(),
                format!("{:.2}", bid.size)
            );
        }
        
        // Show if there are more bids
        if relevant_bids.len() > depth {
            println!(
                "{}",
                format!("... {} more bids", relevant_bids.len() - depth).bright_black()
            );
        }
    }
    
    // Display asks
    println!("\n{}", "ASKS (Sell Orders)".bright_red());
    println!("{}", "─".repeat(40).bright_black());
    
    if relevant_asks.is_empty() {
        println!("{}", "No relevant asks near market price".italic().bright_black());
    } else {
        println!(
            "{:>10} {:>15}",
            "Price".bright_white(),
            "Size".bright_white()
        );
        println!("{}", "─".repeat(40).bright_black());
        
        for ask in relevant_asks.iter().take(depth) {
            println!(
                "{:>10} {:>15}",
                format!("${:.4}", ask.price).bright_red(),
                format!("{:.2}", ask.size)
            );
        }
        
        // Show if there are more asks
        if relevant_asks.len() > depth {
            println!(
                "{}",
                format!("... {} more asks", relevant_asks.len() - depth).bright_black()
            );
        }
    }
    
    // Show total liquidity info
    if !relevant_bids.is_empty() || !relevant_asks.is_empty() {
        println!("\n{}", "LIQUIDITY SUMMARY".bright_cyan());
        println!("{}", "─".repeat(40).bright_black());
        
        let total_bid_size: Decimal = relevant_bids.iter()
            .map(|b| b.size)
            .sum();
        let total_ask_size: Decimal = relevant_asks.iter()
            .map(|a| a.size)
            .sum();
        
        println!(
            "{} {} USDC across {} orders",
            "Total Bid Size:".bright_white(),
            format!("{:.2}", total_bid_size).bright_green(),
            relevant_bids.len()
        );
        println!(
            "{} {} USDC across {} orders",
            "Total Ask Size:".bright_white(),
            format!("{:.2}", total_ask_size).bright_red(),
            relevant_asks.len()
        );
    }
    
    Ok(())
} 