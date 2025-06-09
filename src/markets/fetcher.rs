use anyhow::Result;
use owo_colors::OwoColorize;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use std::time::Instant;
use std::io::{self, Write};

use super::providers::MarketDataProvider;
use super::storage::MarketStorage;

/// Generic market fetcher that works with any provider
pub struct MarketFetcher<T: MarketDataProvider> {
    provider: T,
    storage: MarketStorage,
    verbose: bool,
}

/// Progress tracking
#[derive(Debug)]
pub struct FetchProgress {
    pub pages_fetched: usize,
    pub markets_fetched: usize,
    pub chunks_saved: usize,
    pub start_time: Instant,
    pub last_update: Instant,
}

impl FetchProgress {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            pages_fetched: 0,
            markets_fetched: 0,
            chunks_saved: 0,
            start_time: now,
            last_update: now,
        }
    }
    
    fn display_detailed(&self, status: &str) {
        let elapsed = self.start_time.elapsed();
        let rate = if elapsed.as_secs() > 0 {
            self.markets_fetched as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        };
        
        // Calculate estimated total based on current rate
        let estimated_total = if self.pages_fetched > 10 {
            // Rough estimate: average ~500 markets per page, ~50 pages total
            25000
        } else {
            0
        };
        
        let progress_pct = if estimated_total > 0 {
            (self.markets_fetched as f64 / estimated_total as f64 * 100.0).min(100.0)
        } else {
            0.0
        };
        
        print!("\r{}", " ".repeat(120)); // Clear line
        print!(
            "\r{} {} [{:>3.0}%] Pages: {} | Markets: {} | Chunks: {} | {:.0}/s | {}",
            status,
            progress_bar(progress_pct),
            progress_pct,
            self.pages_fetched.to_string().bright_cyan(),
            self.markets_fetched.to_string().bright_green(),
            self.chunks_saved.to_string().bright_yellow(),
            rate,
            format_duration(elapsed)
        );
        io::stdout().flush().unwrap();
    }
}

fn progress_bar(percentage: f64) -> String {
    let width = 20;
    let filled = (percentage / 100.0 * width as f64) as usize;
    let empty = width - filled;
    format!(
        "{}{}{}{}",
        "[".bright_black(),
        "█".repeat(filled).bright_green(),
        "░".repeat(empty).bright_black(),
        "]".bright_black()
    )
}

fn format_duration(duration: std::time::Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

impl<T: MarketDataProvider> MarketFetcher<T> {
    pub fn new(provider: T, storage: MarketStorage, verbose: bool) -> Self {
        Self {
            provider,
            storage,
            verbose,
        }
    }
    
    /// Fetch all markets with resumable state
    pub async fn fetch_all<S>(&mut self, state_filename: &str, chunk_prefix: &str) -> Result<()>
    where
        S: FetchState + Default,
    {
        // Load or initialize state
        let mut state: S = self.storage.load_state(state_filename)?
            .unwrap_or_default();
        
        if self.verbose && state.get_total_fetched() > 0 {
            println!(
                "{}",
                format!(
                    "📂 Resuming from {} (already fetched {} markets)",
                    state.describe_position(),
                    state.get_total_fetched()
                ).bright_cyan()
            );
        }
        
        println!(
            "{}",
            format!("🔄 Fetching all markets from {}...", self.provider.name()).bright_blue()
        );
        
        let mut progress = FetchProgress::new();
        let mut current_chunk: Vec<Value> = Vec::new();
        let mut current_chunk_size: usize = 0;  // Track chunk size efficiently
        let mut page_token = state.get_page_token();
        
        // Load existing chunk if resuming
        if state.get_chunk_number() > 0 && state.get_markets_in_chunk() > 0 {
            if let Ok(_summary) = self.storage.get_summary(state.get_chunk_number(), chunk_prefix) {
                if self.verbose {
                    println!(
                        "{}",
                        format!(
                            "📄 Continuing chunk {} with {} markets",
                            state.get_chunk_number(),
                            state.get_markets_in_chunk()
                        ).bright_cyan()
                    );
                }
            }
        }
        
        // Show initial progress
        println!("{}", "⏳ Starting fetch... (this may take a few minutes)".bright_yellow());
        
        // Main fetch loop
        loop {
            // Check if we should stop fetching
            if let Some(ref token) = page_token {
                if token == "LTE=" || token.is_empty() {
                    println!(); // New line after progress
                    println!("{}", "📍 Reached end of data".bright_yellow());
                    break;
                }
            }
            
            // Also check provider's state
            if !self.provider.has_more_pages() && page_token.is_none() {
                println!(); // New line after progress
                println!("{}", "📍 No more pages to fetch".bright_yellow());
                break;
            }
            
            // Show we're fetching
            progress.display_detailed("🔄 Fetching");
            
            // Fetch next page
            let (markets, next_token) = match self.provider.fetch_page(page_token.clone()).await {
                Ok(result) => result,
                Err(e) => {
                    println!(); // New line after progress
                    println!("{}", format!("❌ Error fetching page: {}", e).bright_red());
                    return Err(e);
                }
            };
            
            if markets.is_empty() {
                println!(); // New line after progress
                println!("{}", "📍 No more markets to fetch".bright_yellow());
                break;
            }
            
            progress.pages_fetched += 1;
            progress.markets_fetched += markets.len();
            
            // Process each market
            for market in markets {
                // Calculate size of this market
                let market_size = serde_json::to_string(&market)?.len();
                
                // Check if we should save current chunk
                if !current_chunk.is_empty() && current_chunk_size + market_size > self.storage.chunk_size_bytes() {
                    // Update status
                    progress.display_detailed("💾 Saving chunk");
                    
                    // Save chunk
                    let chunk_number = state.get_chunk_number() + 1;
                    self.storage.save_chunk(chunk_number, &current_chunk, chunk_prefix, false)?; // Don't print from storage
                    
                    // Update state
                    state.update_after_chunk_save(chunk_number, current_chunk.len());
                    self.storage.save_state(state_filename, &state)?;
                    
                    progress.chunks_saved += 1;
                    
                    // Show chunk saved message
                    println!(); // New line after progress
                    println!(
                        "{}",
                        format!(
                            "💾 Saved chunk {} with {} markets",
                            chunk_number.to_string().bright_yellow(),
                            current_chunk.len().to_string().bright_green()
                        ).bright_blue()
                    );
                    
                    current_chunk.clear();
                    current_chunk_size = 0;  // Reset chunk size
                }
                
                // Add market to current chunk
                current_chunk.push(market);
                current_chunk_size += market_size;
            }
            
            // Update page token
            page_token = next_token;
            state.update_page_token(page_token.clone());
            
            // Check if we've reached the end
            if let Some(ref token) = page_token {
                if token == "LTE=" || token.is_empty() {
                    // Save current state and break
                    state.update_markets_in_chunk(current_chunk.len());
                    self.storage.save_state(state_filename, &state)?;
                    break;
                }
            }
            
            // Show progress update
            progress.display_detailed("🔄 Fetching");
            
            // Periodic state save (less frequent logging)
            if progress.pages_fetched % 25 == 0 {
                state.update_markets_in_chunk(current_chunk.len());
                self.storage.save_state(state_filename, &state)?;
            }
        }
        
        // Save final chunk if it has data
        if !current_chunk.is_empty() {
            progress.display_detailed("💾 Saving final chunk");
            
            let chunk_number = state.get_chunk_number() + 1;
            self.storage.save_chunk(chunk_number, &current_chunk, chunk_prefix, false)?;
            state.update_after_chunk_save(chunk_number, current_chunk.len());
            self.storage.save_state(state_filename, &state)?;
            progress.chunks_saved += 1;
            
            println!(); // New line after progress
            println!(
                "{}",
                format!(
                    "💾 Saved final chunk {} with {} markets",
                    chunk_number.to_string().bright_yellow(),
                    current_chunk.len().to_string().bright_green()
                ).bright_blue()
            );
        } else {
            println!(); // New line after progress
        }
        
        // Final summary
        let elapsed = progress.start_time.elapsed();
        println!(
            "\n{}",
            format!(
                "✅ Successfully fetched {} markets from {} in {} chunks",
                progress.markets_fetched.to_string().bright_green(),
                self.provider.name(),
                progress.chunks_saved.to_string().bright_yellow()
            ).bright_green()
        );
        
        println!(
            "{}",
            format!(
                "⏱️  Total time: {} ({:.0} markets/sec)",
                format_duration(elapsed).bright_cyan(),
                (progress.markets_fetched as f64 / elapsed.as_secs_f64()).to_string().bright_green()
            )
        );
        
        if self.verbose {
            let summary = self.storage.get_summary(state.get_chunk_number(), chunk_prefix)?;
            summary.display();
        }
        
        Ok(())
    }
}

/// Trait for fetch state management
pub trait FetchState: Serialize + for<'de> Deserialize<'de> {
    fn get_page_token(&self) -> Option<String>;
    fn update_page_token(&mut self, token: Option<String>);
    fn get_total_fetched(&self) -> usize;
    fn get_chunk_number(&self) -> usize;
    fn get_markets_in_chunk(&self) -> usize;
    fn update_markets_in_chunk(&mut self, count: usize);
    fn update_after_chunk_save(&mut self, new_chunk_number: usize, markets_saved: usize);
    fn describe_position(&self) -> String;
}

/// Implement FetchState for CLOB state
impl FetchState for super::types::FetchState {
    fn get_page_token(&self) -> Option<String> {
        self.last_cursor.clone()
    }
    
    fn update_page_token(&mut self, token: Option<String>) {
        self.last_cursor = token.clone();
        // Only increment page if we're not at the end
        if let Some(ref t) = token {
            if t != "LTE=" && !t.is_empty() {
                self.last_page += 1;
            }
        } else if token.is_none() && self.last_page == 0 {
            // First page when starting fresh
            self.last_page = 1;
        }
    }
    
    fn get_total_fetched(&self) -> usize {
        self.total_markets_fetched
    }
    
    fn get_chunk_number(&self) -> usize {
        self.chunk_number
    }
    
    fn get_markets_in_chunk(&self) -> usize {
        self.markets_in_current_chunk
    }
    
    fn update_markets_in_chunk(&mut self, count: usize) {
        self.markets_in_current_chunk = count;
    }
    
    fn update_after_chunk_save(&mut self, new_chunk_number: usize, markets_saved: usize) {
        self.chunk_number = new_chunk_number;
        self.total_markets_fetched += markets_saved;
        self.markets_in_current_chunk = 0;
    }
    
    fn describe_position(&self) -> String {
        format!("page {}", self.last_page)
    }
}

/// Implement FetchState for Gamma state
impl FetchState for super::types::GammaFetchState {
    fn get_page_token(&self) -> Option<String> {
        if self.last_offset > 0 {
            Some(self.last_offset.to_string())
        } else {
            None
        }
    }
    
    fn update_page_token(&mut self, token: Option<String>) {
        if let Some(token) = token {
            if let Ok(offset) = token.parse::<usize>() {
                self.last_offset = offset;
            }
        }
    }
    
    fn get_total_fetched(&self) -> usize {
        self.total_markets_fetched
    }
    
    fn get_chunk_number(&self) -> usize {
        self.chunk_number
    }
    
    fn get_markets_in_chunk(&self) -> usize {
        self.markets_in_current_chunk
    }
    
    fn update_markets_in_chunk(&mut self, count: usize) {
        self.markets_in_current_chunk = count;
    }
    
    fn update_after_chunk_save(&mut self, new_chunk_number: usize, markets_saved: usize) {
        self.chunk_number = new_chunk_number;
        self.total_markets_fetched += markets_saved;
        self.markets_in_current_chunk = 0;
    }
    
    fn describe_position(&self) -> String {
        format!("offset {}", self.last_offset)
    }
} 