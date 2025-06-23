//! Storage layer for the address book system

use anyhow::{Result, Context};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{info, warn, debug};

use crate::address_book::types::*;

/// Address book storage errors
#[derive(Debug, thiserror::Error)]
pub enum AddressBookError {
    #[error("Address not found: {0}")]
    AddressNotFound(String),
    
    #[error("Address already exists: {0}")]
    AddressExists(String),
    
    #[error("Label already in use: {0}")]
    LabelExists(String),
    
    #[error("Invalid address format: {0}")]
    InvalidAddress(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

/// Address book storage manager
pub struct AddressBookStorage {
    /// Base directory for address book data
    base_dir: PathBuf,
    
    /// Address book file path
    address_book_path: PathBuf,
    
    /// Backup directory
    backup_dir: PathBuf,
}

impl AddressBookStorage {
    /// Create new storage manager
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Self {
        let base_dir = base_dir.as_ref().to_path_buf();
        let address_book_path = base_dir.join("address_book.json");
        let backup_dir = base_dir.join("backups");
        
        Self {
            base_dir,
            address_book_path,
            backup_dir,
        }
    }

    /// Initialize storage directories
    pub async fn init(&self) -> Result<()> {
        // Create base directory
        fs::create_dir_all(&self.base_dir).await
            .context("Failed to create address book directory")?;
        
        // Create backup directory
        fs::create_dir_all(&self.backup_dir).await
            .context("Failed to create backup directory")?;
        
        info!("Initialized address book storage at: {:?}", self.base_dir);
        Ok(())
    }

    /// Load address book from disk
    pub async fn load(&self) -> Result<AddressBook> {
        if !self.address_book_path.exists() {
            debug!("No address book found, creating new one");
            return Ok(AddressBook::new());
        }

        let content = fs::read_to_string(&self.address_book_path).await
            .context("Failed to read address book file")?;
        
        let address_book: AddressBook = serde_json::from_str(&content)
            .context("Failed to parse address book")?;
        
        info!("Loaded address book with {} addresses", address_book.entries.len());
        Ok(address_book)
    }

    /// Save address book to disk
    pub async fn save(&self, address_book: &AddressBook) -> Result<()> {
        // Create backup if file exists
        if self.address_book_path.exists() {
            self.create_backup().await?;
        }

        // Serialize address book
        let json = serde_json::to_string_pretty(address_book)
            .context("Failed to serialize address book")?;
        
        // Write to temporary file first
        let temp_path = self.address_book_path.with_extension("tmp");
        fs::write(&temp_path, json).await
            .context("Failed to write temporary file")?;
        
        // Rename to final path (atomic operation)
        fs::rename(&temp_path, &self.address_book_path).await
            .context("Failed to rename address book file")?;
        
        debug!("Saved address book with {} addresses", address_book.entries.len());
        Ok(())
    }

    /// Create backup of current address book
    async fn create_backup(&self) -> Result<()> {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let backup_path = self.backup_dir.join(format!("address_book_{}.json", timestamp));
        
        fs::copy(&self.address_book_path, &backup_path).await
            .context("Failed to create backup")?;
        
        debug!("Created backup at: {:?}", backup_path);
        
        // Clean old backups (keep last 10)
        self.clean_old_backups(10).await?;
        
        Ok(())
    }

    /// Clean old backup files
    async fn clean_old_backups(&self, keep_count: usize) -> Result<()> {
        let mut entries = fs::read_dir(&self.backup_dir).await?;
        let mut backups = Vec::new();
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(metadata) = entry.metadata().await {
                    if let Ok(modified) = metadata.modified() {
                        backups.push((path, modified));
                    }
                }
            }
        }
        
        // Sort by modification time, newest first
        backups.sort_by_key(|(_, time)| std::cmp::Reverse(*time));
        
        // Remove old backups
        for (path, _) in backups.into_iter().skip(keep_count) {
            if let Err(e) = fs::remove_file(&path).await {
                warn!("Failed to remove old backup {:?}: {}", path, e);
            } else {
                debug!("Removed old backup: {:?}", path);
            }
        }
        
        Ok(())
    }

    /// Export address book to CSV
    pub async fn export_csv(&self, path: &Path, address_book: &AddressBook) -> Result<()> {
        use std::io::Write;
        
        let mut file = std::fs::File::create(path)?;
        
        // Write header
        writeln!(file, "Address,Label,Type,Description,Tags,Added At,Last Synced,Total Value,Active")?;
        
        // Write entries
        for entry in address_book.entries.values() {
            writeln!(
                file,
                "{},{},{},{},{},{},{},{},{}",
                entry.address,
                entry.label.as_deref().unwrap_or(""),
                entry.address_type,
                entry.description.as_deref().unwrap_or(""),
                entry.tags.join(";"),
                entry.metadata.added_at.format("%Y-%m-%d %H:%M:%S"),
                entry.metadata.last_synced
                    .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_default(),
                entry.stats.as_ref()
                    .map(|s| s.total_value.to_string())
                    .unwrap_or_default(),
                entry.is_active
            )?;
        }
        
        info!("Exported {} addresses to CSV", address_book.entries.len());
        Ok(())
    }

    /// Import addresses from CSV
    pub async fn import_csv(&self, path: &Path) -> Result<Vec<AddressEntry>> {
        use std::io::{BufRead, BufReader};
        
        let file = std::fs::File::open(path)?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();
        let mut line_num = 0;
        
        for line in reader.lines() {
            line_num += 1;
            
            // Skip header
            if line_num == 1 {
                continue;
            }
            
            let line = line?;
            let parts: Vec<&str> = line.split(',').collect();
            
            if parts.len() < 9 {
                warn!("Skipping invalid line {}: insufficient columns", line_num);
                continue;
            }
            
            let address = parts[0].trim().to_string();
            let label = if parts[1].trim().is_empty() {
                None
            } else {
                Some(parts[1].trim().to_string())
            };
            
            let address_type = match parts[2].trim() {
                "Own" => AddressType::Own,
                "Watched" => AddressType::Watched,
                "Contract" => AddressType::Contract,
                "Exchange" => AddressType::Exchange,
                "Market Maker" => AddressType::MarketMaker,
                _ => AddressType::Other,
            };
            
            let description = if parts[3].trim().is_empty() {
                None
            } else {
                Some(parts[3].trim().to_string())
            };
            
            let tags: Vec<String> = if parts[4].trim().is_empty() {
                Vec::new()
            } else {
                parts[4].split(';').map(|s| s.trim().to_string()).collect()
            };
            
            let is_active = parts[8].trim() == "true";
            
            let mut entry = AddressEntry::new(address, address_type);
            entry.label = label;
            entry.description = description;
            entry.tags = tags;
            entry.is_active = is_active;
            
            entries.push(entry);
        }
        
        info!("Imported {} addresses from CSV", entries.len());
        Ok(entries)
    }


}

/// Validate Ethereum address format
pub fn validate_address(address: &str) -> Result<String, AddressBookError> {
    // Remove 0x prefix if present
    let address = address.trim();
    let cleaned = if address.starts_with("0x") || address.starts_with("0X") {
        &address[2..]
    } else {
        address
    };
    
    // Check length (40 hex chars)
    if cleaned.len() != 40 {
        return Err(AddressBookError::InvalidAddress(
            "Address must be 40 hexadecimal characters".to_string()
        ));
    }
    
    // Check if all characters are valid hex
    if !cleaned.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AddressBookError::InvalidAddress(
            "Address must contain only hexadecimal characters".to_string()
        ));
    }
    
    // Return checksummed address with 0x prefix
    Ok(format!("0x{}", cleaned.to_lowercase()))
}

/// Get checksum address (EIP-55) - simplified version
pub fn checksum_address(address: &str) -> String {
    // For now, just return lowercase with 0x prefix
    // A proper implementation would require adding sha3/keccak256 dependency
    let address = address.trim_start_matches("0x").trim_start_matches("0X");
    format!("0x{}", address.to_lowercase())
}