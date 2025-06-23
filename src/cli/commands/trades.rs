//! Trade history command for viewing past trades

use anyhow::Result;
use clap::Args;
use chrono::{DateTime, Utc, Duration};
use crate::data_paths::DataPaths;
use crate::portfolio::{get_portfolio_service_handle, TradesFormatter};
use tracing::info;

#[derive(Args, Debug)]
pub struct TradesArgs {
    /// Number of trades to show
    #[arg(short, long, default_value = "20")]
    limit: usize,
    
    /// Show trades from the last N days
    #[arg(long)]
    days: Option<i64>,
    
    /// Start date (YYYY-MM-DD)
    #[arg(long)]
    from: Option<String>,
    
    /// End date (YYYY-MM-DD)
    #[arg(long)]
    to: Option<String>,
    
    /// Export trades to CSV
    #[arg(long)]
    export: bool,
    
    /// CSV export filename
    #[arg(long, default_value = "trades.csv")]
    output: String,
}

pub async fn trades(args: TradesArgs, host: &str, data_paths: DataPaths) -> Result<()> {
    println!("\n📈 Trade History\n");
    
    // Get portfolio service handle
    let service_handle = get_portfolio_service_handle(host, &data_paths).await?;
    
    // Determine date range
    let (start_date, end_date) = determine_date_range(&args)?;
    
    if let Some(start) = start_date {
        println!("📅 From: {}", start.format("%Y-%m-%d"));
    }
    if let Some(end) = end_date {
        println!("📅 To: {}", end.format("%Y-%m-%d"));
    }
    println!();
    
    // Fetch trade history
    info!("Fetching trade history...");
    let trades = service_handle.get_trade_history(start_date, end_date).await?;
    
    if trades.is_empty() {
        println!("No trades found in the specified period.");
        return Ok(());
    }
    
    // Export to CSV if requested
    if args.export {
        export_trades_to_csv(&trades, &args.output)?;
        println!("✅ Exported {} trades to {}", trades.len(), args.output);
        println!();
    }
    
    // Display trades
    let formatter = TradesFormatter::new(&trades);
    print!("{}", formatter.format_table(Some(args.limit)));
    
    // Show summary statistics
    println!("\n📊 Trade Summary:");
    println!("  Total Trades: {}", trades.len());
    
    let total_volume: rust_decimal::Decimal = trades.iter().map(|t| t.size).sum();
    println!("  Total Volume: ${:.2}", total_volume);
    
    let total_fees: rust_decimal::Decimal = trades.iter().map(|t| t.fee).sum();
    println!("  Total Fees: ${:.2}", total_fees);
    
    let buy_trades: Vec<_> = trades.iter()
        .filter(|t| matches!(t.side, crate::portfolio::OrderSide::Buy))
        .collect();
    let sell_trades: Vec<_> = trades.iter()
        .filter(|t| matches!(t.side, crate::portfolio::OrderSide::Sell))
        .collect();
    
    println!("  Buy Trades: {} (${:.2} volume)", 
        buy_trades.len(), 
        buy_trades.iter().map(|t| t.size).sum::<rust_decimal::Decimal>()
    );
    println!("  Sell Trades: {} (${:.2} volume)", 
        sell_trades.len(),
        sell_trades.iter().map(|t| t.size).sum::<rust_decimal::Decimal>()
    );
    
    // Show average trade size
    if !trades.is_empty() {
        let avg_size = total_volume / rust_decimal::Decimal::from(trades.len());
        println!("  Average Trade Size: ${:.2}", avg_size);
    }
    
    println!();
    println!("💡 Use --export to save trades to CSV");
    println!("💡 Use --days N to see trades from last N days");
    
    Ok(())
}

fn determine_date_range(args: &TradesArgs) -> Result<(Option<DateTime<Utc>>, Option<DateTime<Utc>>)> {
    let mut start_date = None;
    let mut end_date = None;
    
    // Handle --days option
    if let Some(days) = args.days {
        start_date = Some(Utc::now() - Duration::days(days));
        end_date = Some(Utc::now());
    }
    
    // Handle --from option
    if let Some(from_str) = &args.from {
        start_date = Some(parse_date(from_str)?);
    }
    
    // Handle --to option
    if let Some(to_str) = &args.to {
        end_date = Some(parse_date(to_str)?);
    }
    
    Ok((start_date, end_date))
}

fn parse_date(date_str: &str) -> Result<DateTime<Utc>> {
    use chrono::NaiveDate;
    
    let naive_date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map_err(|e| anyhow::anyhow!("Invalid date format '{}': {}. Use YYYY-MM-DD", date_str, e))?;
    
    Ok(DateTime::<Utc>::from_naive_utc_and_offset(
        naive_date.and_hms_opt(0, 0, 0).unwrap(),
        Utc
    ))
}

fn export_trades_to_csv(trades: &[crate::portfolio::TradeExecution], filename: &str) -> Result<()> {
    use std::fs::File;
    use std::io::Write;
    
    let mut file = File::create(filename)?;
    
    // Write CSV header
    writeln!(file, "Trade ID,Order ID,Market ID,Token ID,Side,Price,Size,Fee,Timestamp,Is Maker")?;
    
    // Write trade data
    for trade in trades {
        writeln!(
            file,
            "{},{},{},{},{},{},{},{},{},{}",
            trade.trade_id,
            trade.order_id,
            trade.market_id,
            trade.token_id,
            match trade.side {
                crate::portfolio::OrderSide::Buy => "BUY",
                crate::portfolio::OrderSide::Sell => "SELL",
            },
            trade.price,
            trade.size,
            trade.fee,
            trade.timestamp.format("%Y-%m-%d %H:%M:%S"),
            trade.is_maker
        )?;
    }
    
    Ok(())
}