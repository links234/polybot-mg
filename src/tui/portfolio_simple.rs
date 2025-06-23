//! Simple portfolio display without full TUI (for environments where TUI doesn't work)

use crate::portfolio::orders_api::PolymarketOrder;
use anyhow::Result;
use comfy_table::{presets::UTF8_FULL, ContentArrangement, Table};
use owo_colors::OwoColorize;
use rust_decimal::Decimal;

#[allow(dead_code)]
pub fn display_portfolio_simple(user_address: &str, orders: &[PolymarketOrder]) -> Result<()> {
    // Clear screen and display header
    print!("\x1B[2J\x1B[1;1H");

    println!("{}", "═".repeat(100).bright_blue());
    println!("{}", "📊 POLYMARKET PORTFOLIO VIEW".bright_white().bold());
    println!("{}", "═".repeat(100).bright_blue());

    // User info section
    println!("\n{}", "USER INFORMATION".bright_yellow());
    println!("{}", "─".repeat(50).bright_black());
    println!("👤 Address: {}", user_address.bright_cyan());
    println!(
        "🔗 Profile: {}",
        format!("https://polymarket.com/profile/{}", user_address)
            .bright_blue()
            .underline()
    );

    // Summary section
    let total_order_value: Decimal = orders.iter().map(|o| o.price * o.size_structured).sum();

    let open_orders = orders
        .iter()
        .filter(|o| o.status == "LIVE" || o.status == "OPEN")
        .count();

    let filled_orders = orders.iter().filter(|o| o.status == "FILLED").count();

    println!("\n{}", "ACCOUNT SUMMARY".bright_yellow());
    println!("{}", "─".repeat(50).bright_black());
    println!("📊 Open Orders: {}", open_orders.to_string().bright_green());
    println!(
        "💰 Total Order Value: ${:.2} USDC",
        total_order_value.to_string().bright_green()
    );
    println!(
        "✅ Filled Orders: {}",
        filled_orders.to_string().bright_blue()
    );

    // Orders table
    println!("\n{}", "ACTIVE ORDERS".bright_yellow());
    println!("{}", "─".repeat(100).bright_black());

    if orders.is_empty() {
        println!("\n{}", "No active orders found".bright_black().italic());
    } else {
        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec![
                "Order ID", "Market", "Side", "Price", "Size", "Filled", "Status", "Outcome",
            ]);

        for order in orders {
            let id_short = if order.id.len() > 12 {
                format!("{}...", &order.id[..12])
            } else {
                order.id.clone()
            };

            let market_short = if order.market.len() > 20 {
                format!("{}...", &order.market[..20])
            } else {
                order.market.clone()
            };

            let side_display = match order.side.as_str() {
                "BUY" => order.side.bright_green().to_string(),
                "SELL" => order.side.bright_red().to_string(),
                _ => order.side.clone(),
            };

            let status_display = match order.status.as_str() {
                "LIVE" | "OPEN" => order.status.bright_green().to_string(),
                "FILLED" => order.status.bright_blue().to_string(),
                "CANCELLED" => order.status.bright_red().to_string(),
                _ => order.status.clone(),
            };

            let filled_decimal = order.size_matched.parse::<Decimal>().unwrap_or_default();

            table.add_row(vec![
                id_short,
                market_short,
                side_display,
                format!("${:.4}", order.price),
                format!("{:.2}", order.size_structured),
                format!("{:.2}", filled_decimal),
                status_display,
                order.outcome.clone(),
            ]);
        }

        println!("{}", table);
    }

    // Positions placeholder
    println!("\n{}", "POSITIONS".bright_yellow());
    println!("{}", "─".repeat(100).bright_black());
    println!(
        "\n{}",
        "No positions found. Positions will appear here once orders are filled."
            .bright_black()
            .italic()
    );

    // Footer
    println!("\n{}", "─".repeat(100).bright_black());
    println!(
        "{}",
        "Press Ctrl+C to exit | Use --text flag for simple view"
            .bright_black()
            .italic()
    );

    Ok(())
}
