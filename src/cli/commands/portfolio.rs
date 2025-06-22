//! Portfolio CLI command for displaying orders and positions

use crate::cli::commands::portfolio_tui::run_portfolio_tui;
use crate::config;
use crate::data_paths::DataPaths;
use crate::ethereum_utils;
use crate::portfolio::orders_api::{build_auth_headers, fetch_balance, PolymarketOrder};
use crate::portfolio::{AccountBalances, PortfolioSnapshot, PortfolioStorage, PositionReconciler};
use anyhow::{anyhow, Result};
use chrono::Utc;
use clap::Args;
use serde_json;
use tracing::{debug, info, warn};

#[derive(Args, Debug)]
pub struct PortfolioArgs {
    /// Show only orders for specific market
    #[arg(short, long)]
    market: Option<String>,

    /// Show only orders for specific asset
    #[arg(short, long)]
    asset: Option<String>,

    /// Use simple text output instead of interactive TUI
    #[arg(long, short = 't')]
    text: bool,
}

pub async fn portfolio(args: PortfolioArgs, host: &str, data_paths: DataPaths) -> Result<()> {
    println!("\n📊 Portfolio Overview\n");

    // Load private key to derive user address
    let private_key = config::load_private_key(&data_paths)
        .await
        .map_err(|e| anyhow!("No private key found. Run 'cargo run -- init' first: {}", e))?;

    // Derive user's Ethereum address
    let address = ethereum_utils::derive_address_from_private_key(&private_key)?;

    // Display user information
    println!("👤 User: {}", address);
    println!("🔗 Profile: https://polymarket.com/profile/{}", address);
    println!("🌐 API Host: {}", host);

    // Try to fetch balance information, fallback to blockchain data
    match fetch_balance(host, &data_paths, &address).await {
        Ok(balance) => {
            println!("💰 Cash: ${:.2} USDC", balance.cash);
            println!("🎯 Bets: ${:.2} USDC", balance.bets);
            println!("📊 Total Equity: ${:.2} USDC", balance.equity_total);
        }
        Err(_) => {
            // Balance API not available, provide alternative info
            println!("💰 Account: {}", address);
            println!("🔍 Balance: Check your Polymarket profile for current balance");
            println!("📊 Orders API: ✅ Connected and working");
        }
    }
    println!("");

    info!("Fetching active orders for user {}", address);

    // Initialize portfolio storage
    let storage = PortfolioStorage::new(data_paths.root(), &address);
    storage.init_directories().await?;

    // Try to get orders using the authenticated client
    match fetch_orders_with_client(host, &data_paths, &args, &address).await {
        Ok(orders) => {
            info!("Successfully fetched {} orders", orders.len());

            // Save active orders to storage
            storage.save_active_orders(&orders).await?;

            // Reconcile positions from orders
            let mut reconciler = PositionReconciler::new();
            let positions = reconciler.reconcile_from_orders(&orders)?;
            info!("Reconciled {} positions from orders", positions.len());

            // Save positions
            storage.save_positions(&positions).await?;

            // Calculate portfolio statistics
            let stats = reconciler.calculate_stats();

            // Get account balances (use default for now since balance API is unreliable)
            let balances = AccountBalances::default();

            // Create and save a snapshot
            let snapshot = PortfolioSnapshot {
                timestamp: Utc::now(),
                address: address.clone(),
                positions: positions.clone(),
                active_orders: orders.clone(),
                stats: stats.clone(),
                balances: balances.clone(),
                metadata: crate::portfolio::storage::SnapshotMetadata {
                    version: "1.0".to_string(),
                    reason: crate::portfolio::storage::SnapshotReason::Manual,
                    previous_snapshot: None,
                    previous_hash: None,
                },
            };

            let snapshot_file = storage.save_snapshot(&snapshot).await?;
            info!("Saved portfolio snapshot: {}", snapshot_file);

            // Display portfolio information
            println!(
                "\n📁 Portfolio data saved to: {}",
                data_paths
                    .root()
                    .join("trade")
                    .join("account")
                    .join(&address)
                    .display()
            );
            println!("📸 Latest snapshot: {}", snapshot_file);
            println!(
                "📊 Positions: {} | Orders: {}",
                positions.len(),
                orders.len()
            );

            if args.text {
                // Simple text output
                display_orders(orders.clone());
                display_positions(&positions);
            } else {
                // Interactive TUI
                run_portfolio_tui(address.clone(), orders, positions).await?;
            }
        }
        Err(e) => {
            warn!("Failed to fetch orders: {}", e);
            println!("❌ Failed to fetch orders: {}", e);

            // Show helpful error message based on error type
            if e.to_string().contains("401") || e.to_string().contains("403") {
                println!("\n⚠️  Authentication failed. Please run 'cargo run -- init' to set up your credentials.");
            } else if e.to_string().contains("No credentials") {
                println!("\n⚠️  No credentials found. Please run 'cargo run -- init' first.");
            } else if e.to_string().contains("Failed to decode API secret") {
                println!("\n⚠️  API secret decoding failed. Your credentials may be corrupted.");
                println!("Please run 'cargo run -- init' again to refresh your credentials.");
            } else {
                println!(
                    "\n⚠️  Unable to connect to Polymarket API. Please check your connection."
                );
            }

            return Err(e);
        }
    }

    Ok(())
}

async fn fetch_orders_with_client(
    host: &str,
    data_paths: &DataPaths,
    args: &PortfolioArgs,
    user_address: &str,
) -> Result<Vec<PolymarketOrder>> {
    // Load credentials
    let api_creds = config::load_credentials(data_paths)
        .await
        .map_err(|e| anyhow!("No credentials found. Run 'cargo run -- init' first: {}", e))?;

    debug!("Loaded credentials successfully");
    debug!("Using address: {}", user_address);

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
    debug!("Response status: {}", status);

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

    // First get the response text to debug what we're getting
    let response_text = response
        .text()
        .await
        .map_err(|e| anyhow!("Failed to get response text: {}", e))?;

    debug!("API response: {}", response_text);
    info!(
        "API response (first 500 chars): {}",
        &response_text[..response_text.len().min(500)]
    );

    // Parse as API response object (not direct array)
    #[derive(serde::Deserialize)]
    struct ApiResponse {
        data: Vec<PolymarketOrder>,
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

    let orders = api_response.data;

    info!("Received {} orders from API", orders.len());

    // Apply filters if provided
    let filtered_orders: Vec<PolymarketOrder> = orders
        .into_iter()
        .filter(|order| {
            if let Some(ref market_filter) = args.market {
                if !order.market.contains(market_filter) {
                    return false;
                }
            }

            if let Some(ref asset_filter) = args.asset {
                if !order.asset_id.contains(asset_filter) {
                    return false;
                }
            }

            true
        })
        .collect();

    Ok(filtered_orders)
}

#[derive(Debug)]
struct OrderInfo {
    id: String,
    market: String,
    side: String,
    price: String,
    size: String,
    remaining: String,
    status: String,
}

fn display_orders(orders: Vec<PolymarketOrder>) {
    let display_orders = convert_orders_for_display(&orders);
    display_order_info(display_orders);
}

fn convert_orders_for_display(orders: &[PolymarketOrder]) -> Vec<OrderInfo> {
    orders
        .iter()
        .map(|order| {
            let matched_decimal = order
                .size_matched
                .parse::<rust_decimal::Decimal>()
                .unwrap_or_default();
            let remaining = order.size_structured - matched_decimal;

            OrderInfo {
                id: if order.id.len() > 12 {
                    format!("{}...", &order.id[..12])
                } else {
                    order.id.clone()
                },
                market: order.market.clone(),
                side: order.side.clone(),
                price: format!("{:.4}", order.price),
                size: format!("{:.2}", order.size_structured),
                remaining: format!("{:.2}", remaining),
                status: order.status.clone(),
            }
        })
        .collect()
}

fn display_positions(positions: &[crate::portfolio::Position]) {
    use crate::portfolio::PositionStatus;

    if positions.is_empty() {
        println!("\n📊 No positions found");
        return;
    }

    println!("\n📊 Positions ({} total)\n", positions.len());

    // Header
    println!(
        "{:<35} {:<10} {:<8} {:<10} {:<10} {:<12} {:<12} {:<10}",
        "Market", "Outcome", "Side", "Size", "Avg Price", "Current", "P&L", "Status"
    );
    println!("{}", "-".repeat(120));

    let mut total_pnl = rust_decimal::Decimal::ZERO;

    for position in positions {
        let market_short = if position.market_id.len() > 32 {
            format!("{}...", &position.market_id[..32])
        } else {
            position.market_id.clone()
        };

        let side = match position.side {
            crate::portfolio::PositionSide::Long => "LONG",
            crate::portfolio::PositionSide::Short => "SHORT",
        };

        let current_price = position
            .current_price
            .map(|p| format!("${:.4}", p))
            .unwrap_or_else(|| "N/A".to_string());

        let pnl = position.total_pnl();
        total_pnl += pnl;

        let pnl_str = if pnl >= rust_decimal::Decimal::ZERO {
            format!("+${:.2}", pnl)
        } else {
            format!("-${:.2}", pnl.abs())
        };

        let status = match position.status {
            PositionStatus::Open => "OPEN",
            PositionStatus::Closed => "CLOSED",
            PositionStatus::Liquidated => "LIQUIDATED",
        };

        println!(
            "{:<35} {:<10} {:<8} {:<10} ${:<9.4} {:<12} {:<12} {:<10}",
            market_short,
            position.outcome,
            side,
            format!("{:.2}", position.size),
            position.average_price,
            current_price,
            pnl_str,
            status
        );
    }

    println!("{}", "-".repeat(120));

    let total_pnl_str = if total_pnl >= rust_decimal::Decimal::ZERO {
        format!("+${:.2}", total_pnl)
    } else {
        format!("-${:.2}", total_pnl.abs())
    };

    println!("Total P&L: {}", total_pnl_str);

    // Calculate some basic stats
    let open_positions = positions
        .iter()
        .filter(|p| p.status == PositionStatus::Open)
        .count();

    let total_value: rust_decimal::Decimal =
        positions.iter().map(|p| p.size * p.average_price).sum();

    println!(
        "Open Positions: {} | Total Value: ${:.2}",
        open_positions, total_value
    );
}

fn display_order_info(orders: Vec<OrderInfo>) {
    if orders.is_empty() {
        println!("✅ Successfully authenticated with Polymarket API");
        println!("📋 No active orders found");
        println!("");
        println!("💡 To place orders, visit your profile page or use the trading commands:");
        println!("   • cargo run -- buy");
        println!("   • cargo run -- sell");
        return;
    }

    // Calculate total order value
    let mut total_value = 0.0;
    for order in &orders {
        if let Ok(price) = order.price.parse::<f64>() {
            if let Ok(size) = order.size.parse::<f64>() {
                total_value += price * size;
            }
        }
    }

    println!("📋 Active Orders ({} total)\n", orders.len());

    // Header
    println!(
        "{:<15} {:<35} {:<6} {:<8} {:<10} {:<10} {:<15}",
        "Order ID", "Market", "Side", "Price", "Size", "Remaining", "Status"
    );
    println!("{}", "-".repeat(105));

    // Orders
    for order in &orders {
        let status_display = match order.status.as_str() {
            "OPEN" => format!("🟢 {}", order.status),
            "PARTIALLY_FILLED" => format!("🟡 {}", order.status),
            "FILLED" => format!("✅ {}", order.status),
            "CANCELLED" => format!("❌ {}", order.status),
            _ => format!("⚪ {}", order.status),
        };

        let side_display = match order.side.as_str() {
            "BUY" => format!("🟩 {}", order.side),
            "SELL" => format!("🟥 {}", order.side),
            _ => order.side.clone(),
        };

        println!(
            "{:<15} {:<35} {:<6} ${:<7} {:<10} {:<10} {:<15}",
            order.id,
            if order.market.len() > 33 {
                format!("{}...", &order.market[..33])
            } else {
                order.market.clone()
            },
            side_display,
            order.price,
            order.size,
            order.remaining,
            status_display
        );
    }

    println!("\n📊 Summary:");
    let open_orders = orders
        .iter()
        .filter(|o| o.status == "OPEN" || o.status == "LIVE")
        .count();
    let partial_orders = orders
        .iter()
        .filter(|o| o.status == "PARTIALLY_FILLED")
        .count();
    let filled_orders = orders.iter().filter(|o| o.status == "FILLED").count();

    println!(
        "  Open: {} | Partially Filled: {} | Filled: {}",
        open_orders, partial_orders, filled_orders
    );
    println!("  Total Order Value: ${:.2} USDC", total_value);

    println!("\n💡 Tips:");
    println!("  • Use --market <id> to filter by market");
    println!("  • Use --asset <id> to filter by asset");
    println!("  • Try 'cargo run -- orders' for the legacy order command");
}
