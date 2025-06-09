use anyhow::Result;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use std::fs::{self, File};
use std::io::{Write, Read};
use std::path::{Path, PathBuf};
use owo_colors::OwoColorize;

/// Storage configuration for market data
pub struct MarketStorage {
    output_dir: PathBuf,
    chunk_size_bytes: usize,
}

impl MarketStorage {
    /// Create a new storage instance
    pub fn new(output_dir: impl AsRef<Path>, chunk_size_mb: f64) -> Result<Self> {
        let output_dir = output_dir.as_ref().to_path_buf();
        fs::create_dir_all(&output_dir)?;
        
        Ok(Self {
            output_dir,
            chunk_size_bytes: (chunk_size_mb * 1024.0 * 1024.0) as usize,
        })
    }
    
    /// Get the chunk size in bytes
    pub fn chunk_size_bytes(&self) -> usize {
        self.chunk_size_bytes
    }
    
    /// Clear all stored data and state
    pub fn clear_all(&self) -> Result<()> {
        // Remove state files
        let state_files = ["fetch_state.json", "gamma_fetch_state.json"];
        for state_file in &state_files {
            let path = self.output_dir.join(state_file);
            if path.exists() {
                fs::remove_file(&path)?;
            }
        }
        
        // Remove all chunk files
        if let Ok(entries) = fs::read_dir(&self.output_dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if let Some(filename) = path.file_name() {
                        if let Some(filename_str) = filename.to_str() {
                            if (filename_str.starts_with("markets_chunk_") || 
                                filename_str.starts_with("gamma_markets_chunk_")) && 
                               filename_str.ends_with(".json") {
                                fs::remove_file(&path)?;
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Save state to file
    pub fn save_state<T: Serialize>(&self, filename: &str, state: &T) -> Result<()> {
        let path = self.output_dir.join(filename);
        let json = serde_json::to_string_pretty(state)?;
        let mut file = File::create(path)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }
    
    /// Load state from file
    pub fn load_state<T: for<'de> Deserialize<'de>>(&self, filename: &str) -> Result<Option<T>> {
        let path = self.output_dir.join(filename);
        if !path.exists() {
            return Ok(None);
        }
        
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let state: T = serde_json::from_str(&contents)?;
        Ok(Some(state))
    }
    
    /// Save a chunk of markets to file
    pub fn save_chunk(&self, chunk_number: usize, markets: &[Value], prefix: &str, verbose: bool) -> Result<()> {
        let filename = format!("{}_chunk_{:04}.json", prefix, chunk_number);
        let path = self.output_dir.join(&filename);
        
        let json = serde_json::to_string_pretty(markets)?;
        let mut file = File::create(path)?;
        file.write_all(json.as_bytes())?;
        
        if verbose {
            let size_mb = json.len() as f64 / 1024.0 / 1024.0;
            println!(
                "{}",
                format!("💾 Saved {} markets to {} ({:.2} MB)", 
                    markets.len(), 
                    filename, 
                    size_mb
                ).bright_blue()
            );
        }
        
        Ok(())
    }
    
    /// Get summary statistics
    pub fn get_summary(&self, chunk_count: usize, prefix: &str) -> Result<StorageSummary> {
        let mut total_size = 0u64;
        let mut total_markets = 0;
        
        for i in 1..=chunk_count {
            let filename = format!("{}_chunk_{:04}.json", prefix, i);
            let path = self.output_dir.join(&filename);
            
            if let Ok(metadata) = fs::metadata(&path) {
                total_size += metadata.len();
                
                // Count markets in chunk
                if let Ok(mut file) = File::open(&path) {
                    let mut contents = String::new();
                    if file.read_to_string(&mut contents).is_ok() {
                        if let Ok(markets) = serde_json::from_str::<Vec<Value>>(&contents) {
                            total_markets += markets.len();
                        }
                    }
                }
            }
        }
        
        Ok(StorageSummary {
            total_chunks: chunk_count,
            total_size_bytes: total_size,
            total_markets,
            output_dir: self.output_dir.clone(),
        })
    }
}

#[derive(Debug)]
pub struct StorageSummary {
    pub total_chunks: usize,
    pub total_size_bytes: u64,
    pub total_markets: usize,
    pub output_dir: PathBuf,
}

impl StorageSummary {
    pub fn display(&self) {
        println!("\n{}", "📊 Storage Summary:".bright_yellow());
        println!("{}", "─".repeat(50).bright_black());
        println!("Total markets: {}", self.total_markets);
        println!("Total chunks: {}", self.total_chunks);
        println!("Total size: {:.2} MB", self.total_size_bytes as f64 / 1024.0 / 1024.0);
        println!("Output directory: {}", self.output_dir.display());
    }
} 