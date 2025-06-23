use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tracing::info;

/// Storage configuration for market data
#[derive(Debug, Clone, Serialize, Deserialize)]
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
                            if (filename_str.starts_with("markets_chunk_")
                                || filename_str.starts_with("gamma_markets_chunk_"))
                                && filename_str.ends_with(".json")
                            {
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
    pub fn save_chunk(
        &self,
        chunk_number: usize,
        markets: &[Value],
        prefix: &str,
        verbose: bool,
    ) -> Result<()> {
        let filename = format!("{}_chunk_{:04}.json", prefix, chunk_number);
        let path = self.output_dir.join(&filename);

        let json = serde_json::to_string_pretty(markets)?;
        let mut file = File::create(path)?;
        file.write_all(json.as_bytes())?;

        if verbose {
            let size_mb = json.len() as f64 / 1024.0 / 1024.0;
            info!(
                "💾 Saved {} markets to {} ({:.2} MB)",
                markets.len(),
                filename,
                size_mb
            );
        }

        Ok(())
    }
}
