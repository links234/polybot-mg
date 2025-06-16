use anyhow::Result;
use tracing::{info, warn};
use polymarket_rs_client::ClobClient;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde_json::Value;

/// List actively traded markets by checking orderbook activity
/// 
/// TODO: This is a placeholder implementation. The full implementation
/// needs to be migrated from the old markets.rs file, but requires
/// fixing several issues:
/// - Decimal type conversions
/// - Proper error handling
/// - Breaking down into smaller functions
pub async fn list_active_markets(
    client: ClobClient,
    limit: usize,
    min_price: Option<f64>,
    max_price: Option<f64>,
    min_spread: Option<f64>,
    max_spread: Option<f64>,
    detailed: bool,
) -> Result<()> {
    info!("🔄 Finding actively traded markets...");
    
    // First, get a list of markets
    info!("  📊 Fetching market list...");
    let mut all_markets = Vec::new();
    let mut cursor: Option<String> = None;
    
    // Fetch up to 50 pages (25,000 markets) to get good coverage
    for page in 0..50 {
        let response = client.get_markets(cursor.as_deref()).await?;
        let (markets, next_cursor) = extract_markets_and_cursor(&response)?;
        
        if markets.is_empty() {
            break;
        }
        
        all_markets.extend(markets);
        cursor = next_cursor;
        
        if cursor.is_none() || cursor.as_ref().map(|c| c.is_empty() || c == "LTE=").unwrap_or(true) {
            break;
        }
        
        // Show progress
        if (page + 1) % 10 == 0 {
            info!("    Fetched {} markets so far...", all_markets.len());
        }
    }
    
    info!("  ✓ Found {} total markets", all_markets.len());
    
    // Filter for binary, active markets
    let binary_active_markets: Vec<_> = all_markets.iter()
        .filter(|m| {
            // Check if it's active
            let active = m.get("active").and_then(|v| v.as_bool()).unwrap_or(false);
            if !active {
                return false;
            }
            
            // Check if it's binary (has exactly YES and NO tokens)
            if let Some(tokens) = m.get("tokens").and_then(|v| v.as_array()) {
                let has_yes = tokens.iter().any(|t| 
                    t.get("outcome").and_then(|o| o.as_str()).map(|s| s.to_lowercase() == "yes").unwrap_or(false)
                );
                let has_no = tokens.iter().any(|t| 
                    t.get("outcome").and_then(|o| o.as_str()).map(|s| s.to_lowercase() == "no").unwrap_or(false)
                );
                tokens.len() == 2 && has_yes && has_no
            } else {
                false
            }
        })
        .collect();
    
    info!("  ✓ Found {} binary active markets", binary_active_markets.len());
    
    // Now check orderbooks for these markets
    info!("📈 Checking orderbooks for trading activity...");
    
    #[derive(Debug)]
    struct ActiveMarket {
        market: serde_json::Value,
        yes_token_id: String,
        yes_price: f64,
        no_price: f64,
        spread: f64,
        bid_depth: f64,
        ask_depth: f64,
        total_liquidity: f64,
    }
    
    let mut active_markets = Vec::new();
    let mut checked = 0;
    
    for market in binary_active_markets.iter() {
        checked += 1;
        
        // Get token IDs - skip if market structure is invalid
        let tokens = match market.get("tokens").and_then(|v| v.as_array()) {
            Some(tokens) => tokens,
            None => {
                warn!("⚠️  Skipping market with invalid tokens structure");
                continue;
            }
        };
        
        let yes_token = match tokens.iter().find(|t| 
            t.get("outcome").and_then(|o| o.as_str()).map(|s| s.to_lowercase() == "yes").unwrap_or(false)
        ) {
            Some(token) => token,
            None => {
                warn!("⚠️  Skipping market without YES token");
                continue;
            }
        };
        
        let no_token = match tokens.iter().find(|t| 
            t.get("outcome").and_then(|o| o.as_str()).map(|s| s.to_lowercase() == "no").unwrap_or(false)
        ) {
            Some(token) => token,
            None => {
                warn!("⚠️  Skipping market without NO token");
                continue;
            }
        };
        
        let yes_token_id = match yes_token.get("token_id").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => {
                warn!("⚠️  Skipping market with invalid YES token ID");
                continue;
            }
        };
        let yes_price = yes_token.get("price").and_then(|v| v.as_f64()).unwrap_or(0.5);
        let no_price = no_token.get("price").and_then(|v| v.as_f64()).unwrap_or(0.5);
        
        // Apply price filter
        if let Some(min_p) = min_price {
            if yes_price < min_p {
                continue;
            }
        }
        if let Some(max_p) = max_price {
            if yes_price > max_p {
                continue;
            }
        }
        
        // Skip resolved markets
        if yes_price <= 0.001 || yes_price >= 0.999 || no_price <= 0.001 || no_price >= 0.999 {
            continue;
        }
        
        // Fetch orderbook for YES token
        match client.get_order_book(yes_token_id).await {
            Ok(orderbook) => {
                // Check if there are active orders
                if orderbook.bids.is_empty() || orderbook.asks.is_empty() {
                    continue;
                }
                
                // Calculate spread
                let best_bid = orderbook.bids.iter().map(|b| b.price).max().unwrap_or(Decimal::ZERO);
                let best_ask = orderbook.asks.iter().map(|a| a.price).min().unwrap_or(Decimal::ONE);
                let spread = (best_ask - best_bid).to_f64().unwrap_or(1.0);
                let spread_pct = spread * 100.0;
                
                // Apply spread filter
                if let Some(min_s) = min_spread {
                    if spread_pct < min_s {
                        continue;
                    }
                }
                if let Some(max_s) = max_spread {
                    if spread_pct > max_s {
                        continue;
                    }
                }
                
                // Calculate liquidity
                let bid_depth: Decimal = orderbook.bids.iter().map(|b| b.size).sum();
                let ask_depth: Decimal = orderbook.asks.iter().map(|a| a.size).sum();
                let total_liquidity = (bid_depth + ask_depth).to_f64().unwrap_or(0.0);
                
                // Only include markets with meaningful liquidity
                if total_liquidity < 10.0 {
                    continue;
                }
                
                active_markets.push(ActiveMarket {
                    market: (*market).clone(),
                    yes_token_id: yes_token_id.to_string(),
                    yes_price,
                    no_price,
                    spread: spread_pct,
                    bid_depth: bid_depth.to_f64().unwrap_or(0.0),
                    ask_depth: ask_depth.to_f64().unwrap_or(0.0),
                    total_liquidity,
                });
                
                // Show progress
                if active_markets.len() % 5 == 0 {
                    info!("    Found {} active markets so far (checked {} markets)...", active_markets.len(), checked);
                }
                
                // Stop if we have enough
                if active_markets.len() >= limit * 2 {
                    break;
                }
            }
            Err(_) => {
                // Skip markets where we can't fetch orderbook
                continue;
            }
        }
    }
    
    info!("  ✓ Found {} actively traded markets", active_markets.len());
    
    // Sort by liquidity
    active_markets.sort_by(|a, b| {
        b.total_liquidity.partial_cmp(&a.total_liquidity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    
    // Display results
    let display_markets: Vec<_> = active_markets.into_iter().take(limit).collect();
    
    if display_markets.is_empty() {
        warn!("No actively traded markets found matching criteria.");
        return Ok(());
    }
    
    info!("Top {} Actively Traded Markets", display_markets.len());
    info!("{}", "─".repeat(120));
    
    if detailed {
        // Detailed view
        for (idx, market_info) in display_markets.iter().enumerate() {
            let question = market_info.market.get("question")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");
            
            println!(
                "\n{} {}",
                format!("{}.", idx + 1),
                question
            );
            
            println!(
                "   {} YES: ${:.3} | NO: ${:.3}",
                "Prices:",
                market_info.yes_price,
                market_info.no_price
            );
            
            println!(
                "   {} {:.1}% | {} ${:.0} bid / ${:.0} ask",
                "Spread:",
                market_info.spread,
                "Depth:",
                market_info.bid_depth,
                market_info.ask_depth
            );
            
            println!(
                "   {} ${:.0}",
                "Total Liquidity:",
                market_info.total_liquidity
            );
            
            if detailed {
                println!(
                    "   {} {}",
                    "YES Token:",
                    market_info.yes_token_id
                );
            }
            
            if idx < display_markets.len() - 1 {
                println!("{}", "─".repeat(120));
            }
        }
    } else {
        // Compact table view
        println!(
            "{:<4} {:<50} {:>8} {:>8} {:>8} {:>12} {:>10}",
            "#",
            "Question",
            "YES",
            "NO",
            "Spread",
            "Liquidity",
            "Depth",
        );
        println!("{}", "─".repeat(120));
        
        for (idx, market_info) in display_markets.iter().enumerate() {
            let question = market_info.market.get("question")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");
            let question = if question.len() > 47 {
                format!("{}...", &question[..47])
            } else {
                question.to_string()
            };
            
            println!(
                "{:<4} {:<50} {:>8} {:>8} {:>8} {:>12} {:>10}",
                format!("{}", idx + 1),
                question,
                format!("${:.3}", market_info.yes_price),
                format!("${:.3}", market_info.no_price),
                format!("{:.1}%", market_info.spread),
                format!("${:.0}", market_info.total_liquidity),
                format!("${:.0}", market_info.bid_depth + market_info.ask_depth),
            );
        }
    }
    
    // Summary
    let total_liquidity: f64 = display_markets.iter().map(|m| m.total_liquidity).sum();
    let avg_spread: f64 = display_markets.iter().map(|m| m.spread).sum::<f64>() / display_markets.len() as f64;
    
    println!("\nSUMMARY");
    println!("{}", "─".repeat(50));
    info!("Markets shown: {}", display_markets.len());
    info!("Total liquidity: ${:.0}", total_liquidity);
    info!("Average spread: {:.1}%", avg_spread);
    
    Ok(())
}

/// Extract markets array and next cursor from response
fn extract_markets_and_cursor(response: &Value) -> Result<(Vec<Value>, Option<String>)> {
    // Based on Polymarket docs, response should have this structure:
    // {
    //   "limit": number,
    //   "count": number,
    //   "next_cursor": string,
    //   "data": Market[]
    // }
    
    // First, try to extract markets from the expected structure
    let markets = if let Some(obj) = response.as_object() {
        if let Some(data) = obj.get("data") {
            data.as_array()
                .ok_or_else(|| anyhow::anyhow!("Expected 'data' field to be an array"))?
                .clone()
        } else {
            // Fallback: maybe response is directly an array
            response.as_array()
                .ok_or_else(|| anyhow::anyhow!("Expected response to contain 'data' array or be an array itself"))?
                .clone()
        }
    } else if let Some(array) = response.as_array() {
        // Response is directly an array
        array.clone()
    } else {
        return Err(anyhow::anyhow!("Unexpected response format"));
    };
    
    // Try to extract next cursor from the expected location
    let next_cursor = if let Some(obj) = response.as_object() {
        obj.get("next_cursor")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty() && s != "LTE=") // "LTE=" means the end
    } else {
        None
    };
    
    Ok((markets, next_cursor))
} 