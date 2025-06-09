use anyhow::{Result, anyhow};
use owo_colors::OwoColorize;
use serde_json::Value;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use chrono::{DateTime, Utc};

use crate::data_paths::DataPaths;
use crate::cli::AnalyzeArgs;

pub async fn analyze_markets(data_paths: DataPaths, args: AnalyzeArgs) -> Result<()> {
    println!("{}", format!("🔍 Analyzing {} markets...", args.source).bright_blue());
    
    // Determine source directory
    let source_dir = match args.source.as_str() {
        "clob" => data_paths.markets_clob(),
        "gamma" => data_paths.markets_gamma(),
        _ => return Err(anyhow!("Invalid source. Use 'clob' or 'gamma'")),
    };
    
    // Create dataset directory
    let dataset_dir = data_paths.markets_datasets().join(&args.dataset_name);
    fs::create_dir_all(&dataset_dir)?;
    
    // Load and filter markets
    let mut all_markets = Vec::new();
    let mut total_loaded = 0;
    
    // Read all chunk files
    let entries = fs::read_dir(&source_dir)?;
    let mut chunk_files: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.contains("_chunk_") && name.ends_with(".json"))
                .unwrap_or(false)
        })
        .collect();
    
    // Sort chunk files
    chunk_files.sort();
    
    for chunk_path in chunk_files {
        let mut file = File::open(&chunk_path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        
        let markets: Vec<Value> = serde_json::from_str(&contents)?;
        total_loaded += markets.len();
        
        // Apply filters
        for market in markets {
            if passes_filters(&market, &args)? {
                all_markets.push(market);
            }
        }
        
        print!("\r{}", format!("📊 Loaded {} markets, filtered to {}", 
            total_loaded, all_markets.len()).bright_cyan());
    }
    println!(); // New line after progress
    
    // Save filtered markets
    if all_markets.is_empty() {
        println!("{}", "⚠️  No markets matched the filters".yellow());
        return Ok(());
    }
    
    // Save as single file or chunks depending on size
    let output_path = dataset_dir.join("markets.json");
    let json = serde_json::to_string_pretty(&all_markets)?;
    let mut file = File::create(&output_path)?;
    file.write_all(json.as_bytes())?;
    
    // Save metadata
    let metadata = create_dataset_metadata(&args, all_markets.len(), total_loaded);
    let metadata_path = dataset_dir.join("metadata.json");
    let metadata_json = serde_json::to_string_pretty(&metadata)?;
    let mut file = File::create(&metadata_path)?;
    file.write_all(metadata_json.as_bytes())?;
    
    // Show summary
    println!(
        "\n{}",
        format!(
            "✅ Created dataset '{}' with {} markets (from {} total)",
            args.dataset_name,
            all_markets.len(),
            total_loaded
        ).bright_green()
    );
    
    if args.summary || args.detailed {
        show_analysis_summary(&all_markets, &args)?;
    }
    
    println!(
        "{}",
        format!("📁 Saved to: {}", dataset_dir.display()).bright_blue()
    );
    
    Ok(())
}

fn passes_filters(market: &Value, args: &AnalyzeArgs) -> Result<bool> {
    // Active filter
    if args.active_only {
        if !market.get("active").and_then(|v| v.as_bool()).unwrap_or(false) {
            return Ok(false);
        }
    }
    
    // Accepting orders filter
    if args.accepting_orders_only {
        if !market.get("accepting_orders").and_then(|v| v.as_bool()).unwrap_or(false) {
            return Ok(false);
        }
    }
    
    // Open filter (not closed)
    if args.open_only {
        if market.get("closed").and_then(|v| v.as_bool()).unwrap_or(false) {
            return Ok(false);
        }
    }
    
    // Archived filter
    if args.no_archived {
        if market.get("archived").and_then(|v| v.as_bool()).unwrap_or(false) {
            return Ok(false);
        }
    }
    
    // Price filters (check YES token price)
    if args.min_price.is_some() || args.max_price.is_some() {
        if let Some(tokens) = market.get("tokens").and_then(|v| v.as_array()) {
            // Find YES token (usually first, but check outcome)
            let yes_price = tokens.iter()
                .find(|t| {
                    t.get("outcome")
                        .and_then(|o| o.as_str())
                        .map(|s| s.to_lowercase() == "yes")
                        .unwrap_or(false)
                })
                .or_else(|| tokens.get(0))
                .and_then(|t| t.get("price"))
                .and_then(|p| p.as_f64());
            
            if let Some(price) = yes_price {
                if let Some(min) = args.min_price {
                    if price < min {
                        return Ok(false);
                    }
                }
                if let Some(max) = args.max_price {
                    if price > max {
                        return Ok(false);
                    }
                }
            }
        }
    }
    
    // Category filter
    if let Some(ref categories) = args.categories {
        let market_category = market.get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();
        
        let allowed_categories: Vec<String> = categories
            .split(',')
            .map(|c| c.trim().to_lowercase())
            .collect();
        
        if !allowed_categories.contains(&market_category) {
            return Ok(false);
        }
    }
    
    // Tags filter
    if let Some(ref tags) = args.tags {
        if let Some(market_tags) = market.get("tags").and_then(|v| v.as_array()) {
            let required_tags: Vec<String> = tags
                .split(',')
                .map(|t| t.trim().to_lowercase())
                .collect();
            
            let market_tag_strings: Vec<String> = market_tags
                .iter()
                .filter_map(|t| t.as_str())
                .map(|s| s.to_lowercase())
                .collect();
            
            // Check if all required tags are present
            for required_tag in &required_tags {
                if !market_tag_strings.contains(required_tag) {
                    return Ok(false);
                }
            }
        } else {
            return Ok(false);
        }
    }
    
    // Minimum order size filter
    if let Some(min_order_size) = args.min_order_size {
        let market_min_order = market.get("minimum_order_size")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        
        if market_min_order > min_order_size {
            return Ok(false);
        }
    }
    
    // Date filters
    if let Some(ref ending_before) = args.ending_before {
        if let Some(end_date) = market.get("end_date_iso").and_then(|v| v.as_str()) {
            if let (Ok(market_end), Ok(filter_date)) = (
                DateTime::parse_from_rfc3339(end_date),
                DateTime::parse_from_rfc3339(ending_before)
            ) {
                if market_end >= filter_date {
                    return Ok(false);
                }
            }
        }
    }
    
    Ok(true)
}

fn create_dataset_metadata(args: &AnalyzeArgs, filtered_count: usize, total_count: usize) -> Value {
    serde_json::json!({
        "dataset_name": args.dataset_name,
        "created_at": Utc::now().to_rfc3339(),
        "source": args.source,
        "total_markets_analyzed": total_count,
        "markets_in_dataset": filtered_count,
        "filters": {
            "min_price": args.min_price,
            "max_price": args.max_price,
            "active_only": args.active_only,
            "accepting_orders_only": args.accepting_orders_only,
            "open_only": args.open_only,
            "no_archived": args.no_archived,
            "categories": args.categories,
            "tags": args.tags,
            "min_order_size": args.min_order_size,
            "created_after": args.created_after,
            "ending_before": args.ending_before,
        }
    })
}

fn show_analysis_summary(markets: &[Value], args: &AnalyzeArgs) -> Result<()> {
    println!("\n{}", "📊 Dataset Analysis".bright_yellow());
    println!("{}", "─".repeat(50).bright_black());
    
    // Count by status
    let active_count = markets.iter()
        .filter(|m| m.get("active").and_then(|v| v.as_bool()).unwrap_or(false))
        .count();
    
    let accepting_orders = markets.iter()
        .filter(|m| m.get("accepting_orders").and_then(|v| v.as_bool()).unwrap_or(false))
        .count();
    
    let closed_count = markets.iter()
        .filter(|m| m.get("closed").and_then(|v| v.as_bool()).unwrap_or(false))
        .count();
    
    println!("Active markets: {}", active_count.to_string().bright_green());
    println!("Accepting orders: {}", accepting_orders.to_string().bright_green());
    println!("Closed markets: {}", closed_count.to_string().bright_red());
    
    // Price distribution
    let mut price_ranges = vec![0; 11]; // 0-10%, 10-20%, ..., 90-100%
    for market in markets {
        if let Some(tokens) = market.get("tokens").and_then(|v| v.as_array()) {
            if let Some(yes_token) = tokens.get(0) {
                if let Some(price) = yes_token.get("price").and_then(|p| p.as_f64()) {
                    let bucket = ((price * 10.0).floor() as usize).min(10);
                    price_ranges[bucket] += 1;
                }
            }
        }
    }
    
    println!("\n{}", "Price Distribution (YES outcome):".bright_cyan());
    for (i, count) in price_ranges.iter().enumerate() {
        if *count > 0 {
            let range = if i == 10 {
                "90-100%".to_string()
            } else {
                format!("{}-{}%", i * 10, (i + 1) * 10)
            };
            println!("  {}: {}", range, count);
        }
    }
    
    // Category breakdown
    if args.detailed {
        let mut categories: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for market in markets {
            if let Some(category) = market.get("category").and_then(|v| v.as_str()) {
                *categories.entry(category.to_string()).or_insert(0) += 1;
            }
        }
        
        if !categories.is_empty() {
            println!("\n{}", "Categories:".bright_cyan());
            let mut cat_vec: Vec<_> = categories.into_iter().collect();
            cat_vec.sort_by(|a, b| b.1.cmp(&a.1));
            for (category, count) in cat_vec.iter().take(10) {
                println!("  {}: {}", category, count);
            }
        }
    }
    
    Ok(())
} 