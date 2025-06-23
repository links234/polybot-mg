//! Ultra-fast in-memory search index for market data
//! 
//! This module provides blazing-fast search capabilities using custom indexing
//! optimized for market data patterns and search requirements.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use anyhow::{Context, Result};
use tracing::{info, debug, warn};
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};

use super::database::GammaDatabase;
use super::types::GammaMarket;

/// Ultra-fast in-memory search index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchIndex {
    /// Term -> Market IDs mapping for full-text search
    term_index: HashMap<String, HashSet<String>>,
    /// Category -> Market IDs mapping
    category_index: HashMap<String, HashSet<String>>,
    /// Market ID -> Market mapping for fast retrieval
    market_cache: HashMap<String, GammaMarket>,
    /// Statistics about the index
    stats: IndexStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    pub total_markets: usize,
    pub total_terms: usize,
    pub total_categories: usize,
    pub index_size_bytes: usize,
    pub build_time_secs: f64,
}

/// Search index builder and manager
#[allow(dead_code)] // Search index API kept for future use
pub struct SearchIndexBuilder {
    db: GammaDatabase,
    index_path: std::path::PathBuf,
}

#[allow(dead_code)] // Search index API kept for future use
impl SearchIndexBuilder {
    /// Create new search index builder
    pub async fn new(db_path: &Path, index_path: &Path) -> Result<Self> {
        info!("Initializing ultra-fast search index builder");
        info!("Database path: {}", db_path.display());
        info!("Index path: {}", index_path.display());
        
        // Initialize database
        let db = GammaDatabase::new(db_path).await
            .context("Failed to initialize database")?;
        
        // Create index directory if it doesn't exist
        std::fs::create_dir_all(index_path)
            .context("Failed to create index directory")?;
        
        Ok(Self {
            db,
            index_path: index_path.to_path_buf(),
        })
    }
    
    /// Build search index from scratch
    pub async fn build_index(&mut self) -> Result<SearchIndex> {
        info!("🚀 Building ultra-fast search index from database");
        let start_time = std::time::Instant::now();
        
        // Get total market count
        let total_count = self.db.get_market_count().await?;
        info!("Found {} markets to index", total_count);
        
        if total_count == 0 {
            warn!("No markets found in database");
            return Ok(SearchIndex {
                term_index: HashMap::new(),
                category_index: HashMap::new(),
                market_cache: HashMap::new(),
                stats: IndexStats {
                    total_markets: 0,
                    total_terms: 0,
                    total_categories: 0,
                    index_size_bytes: 0,
                    build_time_secs: 0.0,
                },
            });
        }
        
        // Create progress bar
        let pb = ProgressBar::new(total_count);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} {msg}\\n[{elapsed_precise}] [{bar:50.cyan/blue}] {pos:>7}/{len:7} ({percent}%) | Speed: {per_sec} | ETA: {eta}")
                .unwrap()
                .progress_chars("█▓▒░")
        );
        pb.set_message("🚀 Starting ultra-fast index build...");
        
        // Initialize index structures
        let mut term_index: HashMap<String, HashSet<String>> = HashMap::new();
        let mut category_index: HashMap<String, HashSet<String>> = HashMap::new();
        let mut market_cache: HashMap<String, GammaMarket> = HashMap::new();
        
        // Process in batches for memory efficiency
        let batch_size = 5000;
        let mut offset = 0u64;
        let mut total_indexed = 0usize;
        let mut batch_number = 0;
        
        while offset < total_count {
            batch_number += 1;
            pb.set_message(format!("📊 Processing batch {} (offset: {})", batch_number, offset));
            
            // Fetch batch of markets using proper database query
            let batch_start = std::time::Instant::now();
            let query = format!("SELECT * FROM markets LIMIT {} START {}", batch_size, offset);
            let markets = self.db.execute_query(&query).await?;
            let fetch_time = batch_start.elapsed();
            
            if markets.is_empty() {
                break;
            }
            
            let batch_len = markets.len();
            debug!("Fetched {} markets in {:?}", batch_len, fetch_time);
            pb.set_message(format!("🔍 Indexing batch {} ({} markets)", batch_number, batch_len));
            
            // Index each market in the batch
            let index_start = std::time::Instant::now();
            for market in markets {
                self.index_market(&market, &mut term_index, &mut category_index, &mut market_cache);
                total_indexed += 1;
            }
            let index_time = index_start.elapsed();
            
            // Update progress
            pb.inc(batch_len as u64);
            let elapsed = start_time.elapsed();
            let rate = total_indexed as f64 / elapsed.as_secs_f64();
            pb.set_message(format!(
                "⚡ Indexed {}/{} markets | Batch {} | {:.0} markets/sec", 
                total_indexed, total_count, batch_number, rate
            ));
            
            offset += batch_size;
            
            // Log batch timing info
            if batch_number % 10 == 0 {
                info!(
                    "Batch {} complete: fetch={:.1}s, index={:.1}s, total={:.1}s",
                    batch_number,
                    fetch_time.as_secs_f64(),
                    index_time.as_secs_f64(),
                    (fetch_time + index_time).as_secs_f64()
                );
            }
        }
        
        let total_time = start_time.elapsed();
        let final_rate = total_indexed as f64 / total_time.as_secs_f64();
        
        // Calculate index statistics
        let total_terms = term_index.len();
        let total_categories = category_index.len();
        let index_size_bytes = self.calculate_index_size(&term_index, &category_index, &market_cache);
        
        let index = SearchIndex {
            term_index,
            category_index,
            market_cache,
            stats: IndexStats {
                total_markets: total_indexed,
                total_terms,
                total_categories,
                index_size_bytes,
                build_time_secs: total_time.as_secs_f64(),
            },
        };
        
        pb.finish_with_message(format!(
            "✅ Ultra-fast search index built: {} markets, {} terms, {} categories in {:.1}s ({:.0} markets/sec)", 
            total_indexed, 
            total_terms,
            total_categories,
            total_time.as_secs_f64(),
            final_rate
        ));
        
        info!("Index build complete: {} markets indexed", total_indexed);
        Ok(index)
    }
    
    /// Index a single market into the search structures
    fn index_market(
        &self,
        market: &GammaMarket,
        term_index: &mut HashMap<String, HashSet<String>>,
        category_index: &mut HashMap<String, HashSet<String>>,
        market_cache: &mut HashMap<String, GammaMarket>,
    ) {
        let market_id = market.id.0.to_string();
        
        // Cache the market for fast retrieval
        market_cache.insert(market_id.clone(), market.clone());
        
        // Index searchable text fields
        let searchable_text = vec![
            market.question.as_str(),
            market.description.as_deref().unwrap_or(""),
            market.slug.as_str(),
            &market_id,
        ].join(" ");
        
        // Tokenize and index terms
        let terms = self.tokenize(&searchable_text);
        for term in terms {
            term_index
                .entry(term)
                .or_insert_with(HashSet::new)
                .insert(market_id.clone());
        }
        
        // Index by category
        if let Some(ref category) = market.category {
            category_index
                .entry(category.clone())
                .or_insert_with(HashSet::new)
                .insert(market_id);
        }
    }
    
    /// Tokenize text into searchable terms
    fn tokenize(&self, text: &str) -> Vec<String> {
        text.to_lowercase()
            .split_whitespace()
            .filter(|term| term.len() >= 2) // Filter out very short terms
            .map(|term| {
                // Remove common punctuation and normalize
                term.chars()
                    .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                    .collect::<String>()
            })
            .filter(|term| !term.is_empty())
            .collect()
    }
    
    /// Calculate approximate index size in bytes
    fn calculate_index_size(
        &self,
        term_index: &HashMap<String, HashSet<String>>,
        category_index: &HashMap<String, HashSet<String>>,
        market_cache: &HashMap<String, GammaMarket>,
    ) -> usize {
        let term_size: usize = term_index.iter()
            .map(|(k, v)| k.len() + v.iter().map(|id| id.len()).sum::<usize>())
            .sum();
        
        let category_size: usize = category_index.iter()
            .map(|(k, v)| k.len() + v.iter().map(|id| id.len()).sum::<usize>())
            .sum();
        
        let cache_size: usize = market_cache.len() * 1024; // Rough estimate per market
        
        term_size + category_size + cache_size
    }
    
    /// Save index to disk
    pub async fn save_index(&self, index: &SearchIndex) -> Result<()> {
        let index_file = self.index_path.join("search_index.json");
        info!("💾 Saving search index to: {}", index_file.display());
        
        let serialized = serde_json::to_string_pretty(index)
            .context("Failed to serialize search index")?;
        
        tokio::fs::write(&index_file, serialized).await
            .context("Failed to write search index to disk")?;
        
        info!("✅ Search index saved successfully");
        Ok(())
    }
    
    /// Load index from disk
    pub async fn load_index(&self) -> Result<Option<SearchIndex>> {
        let index_file = self.index_path.join("search_index.json");
        
        if !index_file.exists() {
            info!("📭 No existing search index found");
            return Ok(None);
        }
        
        info!("📂 Loading search index from: {}", index_file.display());
        
        let serialized = tokio::fs::read_to_string(&index_file).await
            .context("Failed to read search index file")?;
        
        let index: SearchIndex = serde_json::from_str(&serialized)
            .context("Failed to deserialize search index")?;
        
        info!("✅ Search index loaded: {} markets, {} terms", 
              index.stats.total_markets, index.stats.total_terms);
        
        Ok(Some(index))
    }
}

#[allow(dead_code)] // Search index API kept for future use
impl SearchIndex {
    /// Search markets by query
    pub fn search(&self, query: &str, limit: Option<usize>) -> Vec<&GammaMarket> {
        if query.is_empty() {
            return Vec::new();
        }
        
        let terms = self.tokenize(query);
        if terms.is_empty() {
            return Vec::new();
        }
        
        // Find markets that contain ALL terms (AND search)
        let mut result_ids: Option<HashSet<String>> = None;
        
        for term in &terms {
            if let Some(market_ids) = self.term_index.get(term) {
                match result_ids {
                    None => result_ids = Some(market_ids.clone()),
                    Some(ref mut existing) => {
                        existing.retain(|id| market_ids.contains(id));
                    }
                }
            } else {
                // If any term is not found, no results
                return Vec::new();
            }
        }
        
        // Get markets from cache
        let mut results: Vec<&GammaMarket> = result_ids
            .unwrap_or_default()
            .iter()
            .filter_map(|id| self.market_cache.get(id))
            .collect();
        
        // Sort by volume (descending) for relevance
        results.sort_by(|a, b| {
            b.volume().partial_cmp(&a.volume()).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        // Apply limit
        if let Some(limit) = limit {
            results.truncate(limit);
        }
        
        results
    }
    
    /// Search by category
    pub fn search_by_category(&self, category: &str, limit: Option<usize>) -> Vec<&GammaMarket> {
        if let Some(market_ids) = self.category_index.get(category) {
            let mut results: Vec<&GammaMarket> = market_ids
                .iter()
                .filter_map(|id| self.market_cache.get(id))
                .collect();
            
            // Sort by volume (descending)
            results.sort_by(|a, b| {
                b.volume().partial_cmp(&a.volume()).unwrap_or(std::cmp::Ordering::Equal)
            });
            
            if let Some(limit) = limit {
                results.truncate(limit);
            }
            
            results
        } else {
            Vec::new()
        }
    }
    
    /// Get all available categories
    pub fn get_categories(&self) -> Vec<&String> {
        self.category_index.keys().collect()
    }
    
    /// Get index statistics
    pub fn get_stats(&self) -> &IndexStats {
        &self.stats
    }
    
    /// Tokenize text (same as builder)
    fn tokenize(&self, text: &str) -> Vec<String> {
        text.to_lowercase()
            .split_whitespace()
            .filter(|term| term.len() >= 2)
            .map(|term| {
                term.chars()
                    .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                    .collect::<String>()
            })
            .filter(|term| !term.is_empty())
            .collect()
    }
}

/// Helper function to get standard index path
pub fn get_index_path() -> std::path::PathBuf {
    std::path::PathBuf::from("./data/database/gamma-search")
}

/// Build or rebuild the search index
#[allow(dead_code)] // Search index API kept for future use
pub async fn build_search_index(rebuild: bool) -> Result<SearchIndex> {
    println!("🚀 Initializing ultra-fast search index builder...");
    
    let db_path = std::path::PathBuf::from("./data/database/gamma");
    let index_path = get_index_path();
    
    println!("📁 Database path: {}", db_path.display());
    println!("📁 Index path: {}", index_path.display());
    
    let mut builder = SearchIndexBuilder::new(&db_path, &index_path).await?;
    
    let index = if rebuild {
        println!("\\n🔨 Rebuilding search index from scratch (--force flag used)");
        builder.build_index().await?
    } else {
        // Check if index exists
        if let Some(existing_index) = builder.load_index().await? {
            println!("\\n📊 Existing search index found with {} markets", existing_index.stats.total_markets);
            
            // Check if we need to rebuild
            let db_count = builder.db.get_market_count().await?;
            if db_count > existing_index.stats.total_markets as u64 {
                let new_markets = db_count - existing_index.stats.total_markets as u64;
                println!("\\n🆕 Found {} new markets in database", new_markets);
                println!("🔨 Rebuilding index to include new markets...");
                builder.build_index().await?
            } else {
                println!("✅ Index is up to date");
                existing_index
            }
        } else {
            println!("\\n📭 No existing search index found, building from scratch...");
            builder.build_index().await?
        }
    };
    
    // Save the index
    builder.save_index(&index).await?;
    
    // Final statistics
    println!("\\n📊 Final Search Index Statistics:");
    let stats = index.get_stats();
    println!("  Total Markets: {}", stats.total_markets);
    println!("  Total Terms: {}", stats.total_terms);
    println!("  Total Categories: {}", stats.total_categories);
    println!("  Index Size: {:.2} MB", stats.index_size_bytes as f64 / 1024.0 / 1024.0);
    println!("  Build Time: {:.1}s", stats.build_time_secs);
    
    println!("\\n⚡ Ultra-fast search index is ready for blazing-fast search operations!");
    
    Ok(index)
}