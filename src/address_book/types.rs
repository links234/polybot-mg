//! Type definitions for the address book system

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Address entry in the address book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressEntry {
    /// Ethereum address (checksummed)
    pub address: String,
    
    /// Optional human-readable label/name
    pub label: Option<String>,
    
    /// Optional description
    pub description: Option<String>,
    
    /// Address type/category
    pub address_type: AddressType,
    
    /// Custom tags for organization
    pub tags: Vec<String>,
    
    /// Metadata about the address
    pub metadata: AddressMetadata,
    
    /// Portfolio statistics (if tracked)
    pub stats: Option<AddressStats>,
    
    /// Whether this address is actively tracked
    pub is_active: bool,
    
    /// Custom notes
    pub notes: Option<String>,
}

/// Type of address
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AddressType {
    /// Own wallet address
    Own,
    /// Watched address (not owned)
    Watched,
    /// Smart contract address
    Contract,
    /// Exchange address
    Exchange,
    /// Market maker address
    MarketMaker,
    /// Other/Custom type
    Other,
}

impl std::fmt::Display for AddressType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AddressType::Own => write!(f, "Own"),
            AddressType::Watched => write!(f, "Watched"),
            AddressType::Contract => write!(f, "Contract"),
            AddressType::Exchange => write!(f, "Exchange"),
            AddressType::MarketMaker => write!(f, "Market Maker"),
            AddressType::Other => write!(f, "Other"),
        }
    }
}

/// Metadata about an address
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressMetadata {
    /// When the address was added
    pub added_at: DateTime<Utc>,
    
    /// Last time the address was updated
    pub updated_at: DateTime<Utc>,
    
    /// Last time portfolio data was fetched
    pub last_synced: Option<DateTime<Utc>>,
    
    /// Number of times this address was queried
    pub query_count: u64,
    
    /// Last query timestamp
    pub last_queried: Option<DateTime<Utc>>,
    
    /// ENS name if available
    pub ens_name: Option<String>,
    
    /// Whether this is the current active address
    pub is_current: bool,
}

/// Portfolio statistics for an address
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressStats {
    /// Total portfolio value
    pub total_value: Decimal,
    
    /// Number of active positions
    pub active_positions: usize,
    
    /// Number of active orders
    pub active_orders: usize,
    
    /// Total realized P&L (trading + rewards)
    pub total_realized_pnl: Decimal,
    
    /// Trading P&L (from matched position trades)
    #[serde(default)]
    pub trading_pnl: Decimal,
    
    /// Unmatched P&L (winning positions without tracked buys)
    #[serde(default, alias = "rewards_pnl")]
    pub unmatched_wins_pnl: Decimal,
    
    /// Total unrealized P&L
    pub total_unrealized_pnl: Decimal,
    
    /// Win rate percentage
    pub win_rate: Option<Decimal>,
    
    /// Total number of trades
    pub total_trades: usize,
    
    /// Total trading volume (sum of all trade sizes in USDC)
    #[serde(default)]
    pub total_volume: Decimal,
    
    /// Buy volume (sum of all buy trade sizes in USDC)
    #[serde(default)]
    pub buy_volume: Decimal,
    
    /// Sell volume (sum of all sell trade sizes in USDC)
    #[serde(default)]
    pub sell_volume: Decimal,
    
    /// Last trade timestamp
    pub last_trade: Option<DateTime<Utc>>,
    
    /// Statistics timestamp
    pub updated_at: DateTime<Utc>,
}

/// Label for quick identification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressLabel {
    pub address: String,
    pub label: String,
    pub color: Option<String>,
    pub icon: Option<String>,
}

/// Complete address book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressBook {
    /// Version of the address book format
    pub version: String,
    
    /// All address entries
    pub entries: HashMap<String, AddressEntry>,
    
    /// Quick lookup labels
    pub labels: HashMap<String, String>, // label -> address
    
    /// Current active address
    pub current_address: Option<String>,
    
    /// Address book metadata
    pub metadata: AddressBookMetadata,
}

/// Address book metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressBookMetadata {
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    
    /// Last modification timestamp
    pub updated_at: DateTime<Utc>,
    
    /// Total number of addresses
    pub total_addresses: usize,
    
    /// Number of own addresses
    pub own_addresses: usize,
    
    /// Number of watched addresses
    pub watched_addresses: usize,
}

/// Query parameters for searching addresses
#[derive(Debug, Clone, Default)]
pub struct AddressQuery {
    /// Search term (address, label, description, tags)
    pub search: Option<String>,
    
    /// Filter by address type
    pub address_type: Option<AddressType>,
    
    /// Filter by tags
    pub tags: Vec<String>,
    
    /// Only show active addresses
    pub active_only: bool,
    
    /// Sort field
    pub sort_by: AddressSortField,
    
    /// Sort order
    pub ascending: bool,
    
    /// Limit results
    pub limit: Option<usize>,
}

/// Fields to sort addresses by
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressSortField {
    Label,
    Address,
    AddedAt,
    UpdatedAt,
    TotalValue,
    QueryCount,
}

impl Default for AddressSortField {
    fn default() -> Self {
        AddressSortField::Label
    }
}

/// Filter for address queries
#[derive(Debug, Clone, Default)]
pub struct AddressFilter {
    /// Include own addresses
    pub _include_own: bool,
    
    /// Include watched addresses
    pub _include_watched: bool,
    
    /// Include contract addresses
    pub _include_contracts: bool,
    
    /// Minimum portfolio value
    pub _min_value: Option<Decimal>,
    
    /// Has active positions
    pub _has_positions: Option<bool>,
    
    /// Has active orders
    pub _has_orders: Option<bool>,
}

impl AddressEntry {
    /// Create a new address entry
    pub fn new(address: String, address_type: AddressType) -> Self {
        let now = Utc::now();
        Self {
            address,
            label: None,
            description: None,
            address_type,
            tags: Vec::new(),
            metadata: AddressMetadata {
                added_at: now,
                updated_at: now,
                last_synced: None,
                query_count: 0,
                last_queried: None,
                ens_name: None,
                is_current: false,
            },
            stats: None,
            is_active: true,
            notes: None,
        }
    }

    /// Get display name (label or shortened address)
    pub fn display_name(&self) -> String {
        if let Some(label) = &self.label {
            label.clone()
        } else if let Some(ens) = &self.metadata.ens_name {
            ens.clone()
        } else {
            format!("{}...{}", &self.address[..6], &self.address[self.address.len()-4..])
        }
    }

    /// Check if entry matches search term
    pub fn matches_search(&self, search: &str) -> bool {
        let search_lower = search.to_lowercase();
        
        // Check address
        if self.address.to_lowercase().contains(&search_lower) {
            return true;
        }
        
        // Check label
        if let Some(label) = &self.label {
            if label.to_lowercase().contains(&search_lower) {
                return true;
            }
        }
        
        // Check description
        if let Some(desc) = &self.description {
            if desc.to_lowercase().contains(&search_lower) {
                return true;
            }
        }
        
        // Check tags
        for tag in &self.tags {
            if tag.to_lowercase().contains(&search_lower) {
                return true;
            }
        }
        
        // Check ENS name
        if let Some(ens) = &self.metadata.ens_name {
            if ens.to_lowercase().contains(&search_lower) {
                return true;
            }
        }
        
        false
    }

    /// Update query metadata
    pub fn record_query(&mut self) {
        self.metadata.query_count += 1;
        self.metadata.last_queried = Some(Utc::now());
    }
}

impl AddressBook {
    /// Create a new empty address book
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            version: "1.0".to_string(),
            entries: HashMap::new(),
            labels: HashMap::new(),
            current_address: None,
            metadata: AddressBookMetadata {
                created_at: now,
                updated_at: now,
                total_addresses: 0,
                own_addresses: 0,
                watched_addresses: 0,
            },
        }
    }

    /// Add or update an address entry
    pub fn upsert_entry(&mut self, entry: AddressEntry) {
        let address = entry.address.clone();
        let is_new = !self.entries.contains_key(&address);
        
        // Update labels index
        if let Some(label) = &entry.label {
            self.labels.insert(label.clone(), address.clone());
        }
        
        // Insert entry
        self.entries.insert(address, entry);
        
        // Update metadata
        if is_new {
            self.metadata.total_addresses += 1;
        }
        self.update_counts();
        self.metadata.updated_at = Utc::now();
    }

    /// Remove an address entry
    pub fn remove_entry(&mut self, address: &str) -> Option<AddressEntry> {
        if let Some(entry) = self.entries.remove(address) {
            // Remove from labels
            if let Some(label) = &entry.label {
                self.labels.remove(label);
            }
            
            // Update metadata
            self.metadata.total_addresses -= 1;
            self.update_counts();
            self.metadata.updated_at = Utc::now();
            
            Some(entry)
        } else {
            None
        }
    }

    /// Get entry by address or label
    pub fn get_entry(&self, address_or_label: &str) -> Option<&AddressEntry> {
        // Try direct address lookup
        if let Some(entry) = self.entries.get(address_or_label) {
            return Some(entry);
        }
        
        // Try label lookup
        if let Some(address) = self.labels.get(address_or_label) {
            return self.entries.get(address);
        }
        
        None
    }

    /// Set current active address
    pub fn set_current(&mut self, address: &str) -> Result<(), String> {
        if !self.entries.contains_key(address) {
            return Err("Address not found in address book".to_string());
        }
        
        // Clear previous current
        for entry in self.entries.values_mut() {
            entry.metadata.is_current = false;
        }
        
        // Set new current
        if let Some(entry) = self.entries.get_mut(address) {
            entry.metadata.is_current = true;
            self.current_address = Some(address.to_string());
        }
        
        self.metadata.updated_at = Utc::now();
        Ok(())
    }

    /// Update address type counts
    fn update_counts(&mut self) {
        self.metadata.own_addresses = self.entries.values()
            .filter(|e| e.address_type == AddressType::Own)
            .count();
        
        self.metadata.watched_addresses = self.entries.values()
            .filter(|e| e.address_type == AddressType::Watched)
            .count();
    }

    /// Query addresses with filters
    pub fn query(&self, query: &AddressQuery) -> Vec<&AddressEntry> {
        let mut results: Vec<&AddressEntry> = self.entries.values()
            .filter(|entry| {
                // Apply search filter
                if let Some(search) = &query.search {
                    if !entry.matches_search(search) {
                        return false;
                    }
                }
                
                // Apply type filter
                if let Some(addr_type) = query.address_type {
                    if entry.address_type != addr_type {
                        return false;
                    }
                }
                
                // Apply tag filter
                if !query.tags.is_empty() {
                    let has_any_tag = query.tags.iter()
                        .any(|tag| entry.tags.contains(tag));
                    if !has_any_tag {
                        return false;
                    }
                }
                
                // Apply active filter
                if query.active_only && !entry.is_active {
                    return false;
                }
                
                true
            })
            .collect();
        
        // Sort results
        results.sort_by(|a, b| {
            let cmp = match query.sort_by {
                AddressSortField::Label => {
                    a.display_name().cmp(&b.display_name())
                }
                AddressSortField::Address => {
                    a.address.cmp(&b.address)
                }
                AddressSortField::AddedAt => {
                    a.metadata.added_at.cmp(&b.metadata.added_at)
                }
                AddressSortField::UpdatedAt => {
                    a.metadata.updated_at.cmp(&b.metadata.updated_at)
                }
                AddressSortField::TotalValue => {
                    let a_val = a.stats.as_ref().map(|s| s.total_value).unwrap_or_default();
                    let b_val = b.stats.as_ref().map(|s| s.total_value).unwrap_or_default();
                    a_val.cmp(&b_val)
                }
                AddressSortField::QueryCount => {
                    a.metadata.query_count.cmp(&b.metadata.query_count)
                }
            };
            
            if query.ascending {
                cmp
            } else {
                cmp.reverse()
            }
        });
        
        // Apply limit
        if let Some(limit) = query.limit {
            results.truncate(limit);
        }
        
        results
    }
}