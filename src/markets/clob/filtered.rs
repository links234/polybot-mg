use super::cache::{fetch_and_cache_markets, MarketCache};
use anyhow::Result;
use owo_colors::OwoColorize;
use polymarket_rs_client::ClobClient;

/// List filtered markets (binary, active, sorted by volume)
pub async fn list_filtered_markets(
    client: ClobClient,
    limit: usize,
    refresh: bool,
    min_volume: Option<f64>,
    detailed: bool,
    min_price: Option<f64>,
    max_price: Option<f64>,
) -> Result<()> {
    // Load or fetch market cache
    let cache = if refresh {
        println!("{}", "🔄 Refreshing market cache...".bright_blue());
        fetch_and_cache_markets(&client).await?
    } else {
        match MarketCache::load()? {
            Some(cache) => {
                println!(
                    "{}",
                    format!(
                        "📂 Using cached data from {}",
                        cache.last_updated.format("%Y-%m-%d %H:%M:%S UTC")
                    )
                    .bright_cyan()
                );
                cache
            }
            None => {
                println!(
                    "{}",
                    "📂 Cache not found or expired, fetching fresh data...".bright_yellow()
                );
                fetch_and_cache_markets(&client).await?
            }
        }
    };

    // Filter binary and active markets
    let mut filtered_markets = cache.filter_binary_active();

    // Filter out resolved markets (YES or NO at 0% or 100%)
    filtered_markets.retain(|m| {
        let yes_price = m
            .tokens
            .iter()
            .find(|t| t.outcome.to_lowercase() == "yes")
            .map(|t| t.price)
            .unwrap_or(0.5);
        let no_price = m
            .tokens
            .iter()
            .find(|t| t.outcome.to_lowercase() == "no")
            .map(|t| t.price)
            .unwrap_or(0.5);

        // Exclude markets that are resolved (at 0% or 100%)
        yes_price > 0.001 && yes_price < 0.999 && no_price > 0.001 && no_price < 0.999
    });

    // Apply price range filter if specified
    if min_price.is_some() || max_price.is_some() {
        let min_p = min_price.unwrap_or(0.0);
        let max_p = max_price.unwrap_or(1.0);

        filtered_markets.retain(|m| {
            // Get YES price
            let yes_price = m
                .tokens
                .iter()
                .find(|t| t.outcome.to_lowercase() == "yes")
                .map(|t| t.price)
                .unwrap_or(0.5);

            yes_price >= min_p && yes_price <= max_p
        });
    }

    // Apply minimum volume filter if specified
    if let Some(min_vol) = min_volume {
        filtered_markets.retain(|m| m.volume >= min_vol);
    }

    // Sort by volume (descending)
    MarketCache::sort_by_volume(&mut filtered_markets);

    // Limit results
    let display_markets: Vec<_> = filtered_markets.into_iter().take(limit).collect();

    if display_markets.is_empty() {
        println!("{}", "No markets found matching criteria.".yellow());
        return Ok(());
    }

    // Display markets
    let title = if min_price.is_some() || max_price.is_some() {
        let min_p = (min_price.unwrap_or(0.0) * 100.0) as i32;
        let max_p = (max_price.unwrap_or(1.0) * 100.0) as i32;
        format!(
            "Top {} Binary Markets by Volume (Price Range: {}%-{}%)",
            display_markets.len(),
            min_p,
            max_p
        )
    } else {
        format!("Top {} Binary Markets by Volume", display_markets.len())
    };

    println!("\n{}", title.bright_green());
    println!("{}", "─".repeat(120).bright_black());

    if detailed {
        // Detailed view
        for (idx, market) in display_markets.iter().enumerate() {
            println!(
                "\n{} {}",
                format!("{}.", idx + 1).bright_black(),
                market.question.bright_white()
            );
            println!(
                "   {} {}",
                "Condition ID:".bright_black(),
                market.condition_id.bright_yellow()
            );
            println!("   {} {}", "Slug:".bright_black(), market.slug);
            println!(
                "   {} ${:.2} | {} ${:.2}",
                "Volume:".bright_black(),
                market.volume,
                "Liquidity:".bright_black(),
                market.liquidity
            );

            // Show token prices
            for token in &market.tokens {
                let color = if token.outcome.to_lowercase() == "yes" {
                    format!("${:.4}", token.price).bright_green().to_string()
                } else {
                    format!("${:.4}", token.price).bright_red().to_string()
                };
                println!(
                    "   {} {} {} {}",
                    "→".bright_black(),
                    token.outcome.bright_cyan(),
                    "Price:".bright_black(),
                    color
                );
                if detailed {
                    println!(
                        "     {} {}",
                        "Token ID:".bright_black(),
                        token.token_id.bright_black()
                    );
                }
            }

            if idx < display_markets.len() - 1 {
                println!("{}", "─".repeat(120).bright_black());
            }
        }
    } else {
        // Compact table view
        println!(
            "{:<4} {:<60} {:>12} {:>12} {:>8} {:>8}",
            "#".bright_white(),
            "Question".bright_white(),
            "Volume".bright_white(),
            "Liquidity".bright_white(),
            "YES".bright_white(),
            "NO".bright_white(),
        );
        println!("{}", "─".repeat(120).bright_black());

        for (idx, market) in display_markets.iter().enumerate() {
            let question = if market.question.len() > 57 {
                format!("{}...", &market.question[..57])
            } else {
                market.question.clone()
            };

            let yes_price = market
                .tokens
                .iter()
                .find(|t| t.outcome.to_lowercase() == "yes")
                .map(|t| t.price)
                .unwrap_or(0.0);
            let no_price = market
                .tokens
                .iter()
                .find(|t| t.outcome.to_lowercase() == "no")
                .map(|t| t.price)
                .unwrap_or(0.0);

            println!(
                "{:<4} {:<60} {:>12} {:>12} {:>8} {:>8}",
                format!("{}", idx + 1).bright_black(),
                question,
                format!("${:.0}", market.volume).bright_yellow(),
                format!("${:.0}", market.liquidity).bright_cyan(),
                format!("${:.3}", yes_price).bright_green(),
                format!("${:.3}", no_price).bright_red(),
            );
        }
    }

    // Summary statistics
    let total_volume: f64 = display_markets.iter().map(|m| m.volume).sum();
    let avg_volume = total_volume / display_markets.len() as f64;

    println!("\n{}", "SUMMARY".bright_yellow());
    println!("{}", "─".repeat(50).bright_black());
    println!("Total markets shown: {}", display_markets.len());
    println!("Combined volume: ${:.2}", total_volume);
    println!("Average volume: ${:.2}", avg_volume);

    Ok(())
}
