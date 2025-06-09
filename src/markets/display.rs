use anyhow::Result;
use owo_colors::OwoColorize;
use serde_json::Value;

/// Display market information from CLOB API
pub fn display_market_info(market: &Value, idx: usize, detailed: bool) -> Result<()> {
    let question = market.get("question")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown");
    let condition_id = market.get("condition_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let slug = market.get("market_slug")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    
    println!(
        "{} {}",
        format!("{}.", idx).bright_black(),
        question.bright_white()
    );
    println!("   {} {}", "Condition ID:".bright_black(), condition_id.bright_yellow());
    println!("   {} {}", "Slug:".bright_black(), slug);
    
    // Display token information
    if let Some(tokens) = market.get("tokens").and_then(|v| v.as_array()) {
        println!("   {} ", "Tokens:".bright_black());
        for token in tokens {
            let outcome = token.get("outcome")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");
            let token_id = token.get("token_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let price = token.get("price")
                .and_then(|v| v.as_f64())
                .map(|p| format!("${:.4}", p))
                .unwrap_or_else(|| "N/A".to_string());
            
            println!(
                "     {} {} {} {} {}",
                "→".bright_black(),
                outcome.bright_cyan(),
                format!("({})", token_id).bright_black(),
                "Price:".bright_black(),
                price.bright_yellow()
            );
        }
    }
    
    if detailed {
        // Show additional details
        if let Some(active) = market.get("active").and_then(|v| v.as_bool()) {
            let status = if active { 
                format!("{}", "Yes".bright_green()) 
            } else { 
                format!("{}", "No".bright_red()) 
            };
            println!("   {} {}", "Active:".bright_black(), status);
        }
        
        if let Some(category) = market.get("category").and_then(|v| v.as_str()) {
            println!("   {} {}", "Category:".bright_black(), category);
        }
        
        if let Some(end_date) = market.get("end_date_iso").and_then(|v| v.as_str()) {
            println!("   {} {}", "End Date:".bright_black(), end_date);
        }
    }
    
    Ok(())
}

/// Display market information from Gamma API
pub fn display_gamma_market_info(market: &Value, idx: usize, detailed: bool) -> Result<()> {
    let title = market.get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown");
    let id = market.get("id")
        .and_then(|v| v.as_i64())
        .map(|i| i.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let slug = market.get("slug")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    
    println!(
        "{} {}",
        format!("{}.", idx).bright_black(),
        title.bright_white()
    );
    println!("   {} {}", "Gamma ID:".bright_black(), id.bright_yellow());
    println!("   {} {}", "Slug:".bright_black(), slug);
    
    if detailed {
        if let Some(volume) = market.get("volume").and_then(|v| v.as_f64()) {
            println!("   {} ${:.2}", "Volume:".bright_black(), volume);
        }
        
        if let Some(liquidity) = market.get("liquidity").and_then(|v| v.as_f64()) {
            println!("   {} ${:.2}", "Liquidity:".bright_black(), liquidity);
        }
        
        if let Some(active) = market.get("active").and_then(|v| v.as_bool()) {
            let status = if active { 
                format!("{}", "Yes".bright_green()) 
            } else { 
                format!("{}", "No".bright_red()) 
            };
            println!("   {} {}", "Active:".bright_black(), status);
        }
        
        if let Some(closed) = market.get("closed").and_then(|v| v.as_bool()) {
            let status = if closed { 
                format!("{}", "Yes".bright_red()) 
            } else { 
                format!("{}", "No".bright_green()) 
            };
            println!("   {} {}", "Closed:".bright_black(), status);
        }
    }
    
    Ok(())
} 