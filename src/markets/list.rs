use anyhow::Result;
use owo_colors::OwoColorize;
use polymarket_rs_client::ClobClient;

/// List active markets with optional filtering
pub async fn list_markets(
    client: ClobClient,
    filter: Option<String>,
    limit: usize,
) -> Result<()> {
    println!("{}", "📊 Fetching active markets...".bright_blue());
    
    // Fetch markets from API - get_markets returns a Value
    let markets_response = client.get_markets(None).await?;
    
    // Check if the response is directly an array or wrapped in an object
    let markets = if let Some(array) = markets_response.as_array() {
        // Response is directly an array
        array
    } else if let Some(data) = markets_response.get("data") {
        // Response might be wrapped in a "data" field
        data.as_array()
            .ok_or_else(|| anyhow::anyhow!("Expected 'data' field to be an array"))?
    } else if let Some(markets_field) = markets_response.get("markets") {
        // Or maybe in a "markets" field
        markets_field.as_array()
            .ok_or_else(|| anyhow::anyhow!("Expected 'markets' field to be an array"))?
    } else {
        return Err(anyhow::anyhow!("Expected markets array in response"));
    };
    
    // Filter markets if requested
    let filtered_markets: Vec<_> = if let Some(keyword) = filter {
        markets
            .iter()
            .filter(|m| {
                let question = m.get("question")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_lowercase();
                let slug = m.get("slug")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_lowercase();
                question.contains(&keyword.to_lowercase()) || slug.contains(&keyword.to_lowercase())
            })
            .take(limit)
            .collect()
    } else {
        markets.iter().take(limit).collect()
    };
    
    if filtered_markets.is_empty() {
        println!("{}", "No markets found matching criteria.".yellow());
        return Ok(());
    }
    
    // Display markets
    println!("\n{}", format!("Found {} markets:", filtered_markets.len()).bright_green());
    println!("{}", "─".repeat(80).bright_black());
    
    for (idx, market) in filtered_markets.iter().enumerate() {
        let question = market.get("question")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");
        let slug = market.get("slug")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        
        println!(
            "{} {}",
            format!("{}.", idx + 1).bright_black(),
            question.bright_white()
        );
        println!("   {} {}", "Slug:".bright_black(), slug);
        
        // Display token information
        if let Some(tokens) = market.get("tokens").and_then(|v| v.as_array()) {
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
                    "   {} {} {} {} {}",
                    "→".bright_black(),
                    outcome.bright_cyan(),
                    format!("({})", token_id).bright_black(),
                    "Price:".bright_black(),
                    price.bright_yellow()
                );
            }
        }
        
        if idx < filtered_markets.len() - 1 {
            println!("{}", "─".repeat(80).bright_black());
        }
    }
    
    Ok(())
} 