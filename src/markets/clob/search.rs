use super::display::{display_gamma_market_info, display_market_info};
use anyhow::Result;
use owo_colors::OwoColorize;
use polymarket_rs_client::ClobClient;

/// Search for markets by keyword
pub async fn search_markets(
    client: ClobClient,
    keyword: &str,
    detailed: bool,
    limit: usize,
) -> Result<()> {
    println!(
        "{}",
        format!("🔍 Searching for markets matching '{}'...", keyword).bright_blue()
    );

    // First, try to get markets from the API
    let markets_response = client.get_markets(None).await?;

    // Extract markets array
    let markets = if let Some(obj) = markets_response.as_object() {
        if let Some(data) = obj.get("data") {
            data.as_array()
                .ok_or_else(|| anyhow::anyhow!("Expected 'data' field to be an array"))?
        } else {
            markets_response
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("Expected response to be an array"))?
        }
    } else if let Some(array) = markets_response.as_array() {
        array
    } else {
        return Err(anyhow::anyhow!("Unexpected response format"));
    };

    // Filter markets by keyword
    let keyword_lower = keyword.to_lowercase();
    let found_markets: Vec<_> = markets
        .iter()
        .filter(|m| {
            let question = m
                .get("question")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();
            let slug = m
                .get("market_slug")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();
            let category = m
                .get("category")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();

            question.contains(&keyword_lower)
                || slug.contains(&keyword_lower)
                || category.contains(&keyword_lower)
        })
        .take(limit)
        .collect();

    if found_markets.is_empty() {
        println!("{}", "No markets found matching your search.".yellow());

        // Try searching with Gamma API as fallback
        println!("\n{}", "🌐 Trying Gamma API...".bright_cyan());
        search_markets_gamma(&keyword_lower, detailed, limit).await?;
        return Ok(());
    }

    // Display results
    println!(
        "\n{}",
        format!("Found {} markets:", found_markets.len()).bright_green()
    );
    println!("{}", "─".repeat(80).bright_black());

    for (idx, market) in found_markets.iter().enumerate() {
        display_market_info(market, idx + 1, detailed)?;

        if idx < found_markets.len() - 1 {
            println!("{}", "─".repeat(80).bright_black());
        }
    }

    Ok(())
}

/// Search markets using Gamma API
async fn search_markets_gamma(keyword: &str, detailed: bool, limit: usize) -> Result<()> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://gamma-api.polymarket.com/markets?limit={}&order=volume&ascending=false",
        limit * 10 // Get more to filter
    );

    let response = client.get(&url).send().await?;
    let markets: Vec<serde_json::Value> = response.json().await?;

    // Filter by keyword
    let found_markets: Vec<_> = markets
        .iter()
        .filter(|m| {
            let title = m
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();
            let slug = m
                .get("slug")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();

            title.contains(keyword) || slug.contains(keyword)
        })
        .take(limit)
        .collect();

    if found_markets.is_empty() {
        println!("{}", "No markets found in Gamma API either.".yellow());
        return Ok(());
    }

    println!(
        "\n{}",
        format!("Found {} markets in Gamma API:", found_markets.len()).bright_green()
    );
    println!("{}", "─".repeat(80).bright_black());

    for (idx, market) in found_markets.iter().enumerate() {
        display_gamma_market_info(market, idx + 1, detailed)?;

        if idx < found_markets.len() - 1 {
            println!("{}", "─".repeat(80).bright_black());
        }
    }

    Ok(())
}

/// Get detailed information about a specific market
pub async fn get_market_details(client: ClobClient, identifier: &str) -> Result<()> {
    println!(
        "{}",
        format!("📊 Fetching market details for '{}'...", identifier).bright_blue()
    );

    // Try to get the specific market
    // First, try as condition_id
    let market_response = client.get_market(identifier).await;

    match market_response {
        Ok(market) => {
            println!("\n{}", "Market Details (CLOB API):".bright_green());
            println!("{}", "─".repeat(80).bright_black());
            display_market_info(&market, 1, true)?;
        }
        Err(e) => {
            println!(
                "{}",
                format!("Could not find market in CLOB API: {}", e).yellow()
            );

            // Try Gamma API
            println!("\n{}", "🌐 Trying Gamma API...".bright_cyan());
            get_market_details_gamma(identifier).await?;
        }
    }

    Ok(())
}

/// Get market details from Gamma API
async fn get_market_details_gamma(identifier: &str) -> Result<()> {
    let client = reqwest::Client::new();

    // Try to find by slug or search
    let url = format!(
        "https://gamma-api.polymarket.com/markets?slug={}",
        identifier
    );

    let response = client.get(&url).send().await?;
    let markets: Vec<serde_json::Value> = response.json().await?;

    if let Some(market) = markets.first() {
        println!("\n{}", "Market Details (Gamma API):".bright_green());
        println!("{}", "─".repeat(80).bright_black());
        display_gamma_market_info(market, 1, true)?;

        // Try to get CLOB token IDs
        if let Some(clob_token_ids) = market.get("clob_token_ids").and_then(|v| v.as_array()) {
            println!("\n{}", "CLOB Token IDs:".bright_cyan());
            for (idx, token_id) in clob_token_ids.iter().enumerate() {
                if let Some(id) = token_id.as_str() {
                    println!(
                        "  {} Token {}: {}",
                        "→".bright_black(),
                        idx,
                        id.bright_yellow()
                    );
                }
            }
        }
    } else {
        println!("{}", "Market not found in Gamma API.".yellow());
    }

    Ok(())
}

/// Extract market information from a Polymarket URL
pub async fn get_market_from_url(url: &str) -> Result<()> {
    println!(
        "{}",
        format!("🌐 Fetching market information from URL: {}", url).bright_blue()
    );

    // Extract slug from URL
    let slug = if let Some(pos) = url.find("/event/") {
        let slug_part = &url[pos + 7..];
        slug_part.split('?').next().unwrap_or(slug_part)
    } else {
        return Err(anyhow::anyhow!("Invalid Polymarket URL format"));
    };

    println!("{}", format!("📍 Extracted slug: {}", slug).bright_cyan());

    // Try to fetch from Gamma API with different approaches
    let client = reqwest::Client::new();

    // First, try the events endpoint
    println!("\n{}", "🔍 Checking events endpoint...".bright_yellow());
    let events_url = format!("https://gamma-api.polymarket.com/events");
    let response = client.get(&events_url).send().await?;
    let events: Vec<serde_json::Value> = response.json().await?;

    // Search for the event by slug
    let matching_event = events
        .iter()
        .find(|e| e.get("slug").and_then(|s| s.as_str()) == Some(slug));

    if let Some(event) = matching_event {
        println!("{}", "✅ Found event!".bright_green());
        println!("\n{}", "Event Details:".bright_green());
        println!("{}", "─".repeat(80).bright_black());

        if let Some(title) = event.get("title").and_then(|v| v.as_str()) {
            println!("Title: {}", title.bright_white());
        }

        if let Some(id) = event.get("id").and_then(|v| v.as_i64()) {
            println!("Event ID: {}", id);
        }

        // Get markets for this event
        if let Some(markets) = event.get("markets").and_then(|v| v.as_array()) {
            println!(
                "\n{}",
                format!("Markets ({}):", markets.len()).bright_cyan()
            );
            println!("{}", "─".repeat(80).bright_black());

            for (idx, market) in markets.iter().enumerate() {
                if let Some(market_id) = market.get("id").and_then(|v| v.as_i64()) {
                    let title = market
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown");
                    let slug = market
                        .get("slug")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");

                    println!(
                        "\n{}. {} (ID: {})",
                        idx + 1,
                        title.bright_white(),
                        market_id
                    );
                    println!("   Slug: {}", slug);

                    // Get CLOB token IDs
                    if let Some(clob_token_ids) =
                        market.get("clob_token_ids").and_then(|v| v.as_array())
                    {
                        println!("   CLOB Token IDs:");
                        for (token_idx, token_id) in clob_token_ids.iter().enumerate() {
                            if let Some(id) = token_id.as_str() {
                                println!(
                                    "     {} Token {}: {}",
                                    "→".bright_black(),
                                    token_idx,
                                    id.bright_yellow()
                                );
                            }
                        }
                    }
                }
            }
        }
    } else {
        println!("{}", "❌ Event not found in Gamma API".bright_red());

        // Try searching markets directly
        println!("\n{}", "🔍 Searching markets directly...".bright_yellow());
        let markets_url = format!("https://gamma-api.polymarket.com/markets?limit=10000");
        let response = client.get(&markets_url).send().await?;
        let all_markets: Vec<serde_json::Value> = response.json().await?;

        // Search for markets that might be related
        let related_markets: Vec<_> = all_markets
            .iter()
            .filter(|m| {
                let title = m
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_lowercase();
                let market_slug = m
                    .get("slug")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_lowercase();
                title.contains("poland") && title.contains("president")
                    || market_slug.contains("poland") && market_slug.contains("president")
            })
            .collect();

        if !related_markets.is_empty() {
            println!(
                "{}",
                format!("\n✅ Found {} related markets:", related_markets.len()).bright_green()
            );
            println!("{}", "─".repeat(80).bright_black());

            for (idx, market) in related_markets.iter().enumerate() {
                let title = market
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let id = market.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                let slug = market
                    .get("slug")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                println!("\n{}. {} (ID: {})", idx + 1, title.bright_white(), id);
                println!("   Slug: {}", slug);

                // Get CLOB token IDs
                if let Some(clob_token_ids) =
                    market.get("clob_token_ids").and_then(|v| v.as_array())
                {
                    println!("   CLOB Token IDs:");
                    for (token_idx, token_id) in clob_token_ids.iter().enumerate() {
                        if let Some(id) = token_id.as_str() {
                            println!(
                                "     {} Token {}: {}",
                                "→".bright_black(),
                                token_idx,
                                id.bright_yellow()
                            );
                        }
                    }
                }
            }
        } else {
            println!("{}", "❌ No related markets found".bright_red());
        }
    }

    // Provide instructions for placing orders
    println!(
        "\n{}",
        "💡 To place orders on these markets:".bright_yellow()
    );
    println!("1. Use the token IDs shown above");
    println!("2. Run: polybot buy <TOKEN_ID> --price <PRICE> --size <SIZE> --yes");
    println!("3. Or: polybot sell <TOKEN_ID> --price <PRICE> --size <SIZE> --yes");

    Ok(())
}
