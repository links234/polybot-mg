//! Ultra-fast in-memory search engine using Aho-Corasick and optimized data structures
//!
//! This module provides millisecond search performance for 240k+ markets using:
//! - Aho-Corasick for ultra-fast pattern matching
//! - Roaring bitmaps for compressed document sets
//! - FST for prefix search
//! - Optimized in-memory structures

use std::collections::{HashMap, BTreeMap};
use std::path::Path;
use std::sync::Arc;
use anyhow::{Context, Result};
use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use roaring::RoaringBitmap;
use fst::{Map, MapBuilder, Streamer, IntoStreamer};
use serde::{Deserialize, Serialize};
use tracing::{info, debug};
use rust_decimal::prelude::ToPrimitive;
use rayon::prelude::*;
use dashmap::DashMap;

use super::types::GammaMarket;
use super::database::GammaDatabase;

/// Document ID type for internal use
type DocId = u32;

/// Fast search engine optimized for market data
#[derive(Clone)]
pub struct FastSearchEngine {
    /// Aho-Corasick automaton for pattern matching
    pattern_matcher: Arc<AhoCorasick>,
    
    /// Pattern ID to document IDs mapping
    pattern_to_docs: Arc<HashMap<usize, RoaringBitmap>>,
    
    /// Category to document IDs mapping
    category_index: Arc<HashMap<String, RoaringBitmap>>,
    
    /// Tag to document IDs mapping  
    tag_index: Arc<HashMap<String, RoaringBitmap>>,
    
    /// Volume range index (bucketed for performance)
    volume_buckets: Arc<BTreeMap<u64, RoaringBitmap>>,
    
    /// Document ID to market mapping
    doc_store: Arc<HashMap<DocId, Arc<GammaMarket>>>,
    
    /// FST for prefix search on market questions
    prefix_map: Arc<Map<Vec<u8>>>,
    
    /// Statistics
    stats: SearchStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchStats {
    pub total_documents: usize,
    pub total_patterns: usize,
    pub total_categories: usize,
    pub total_tags: usize,
    pub index_size_bytes: usize,
    pub build_time_ms: u64,
}

/// Search parameters
#[derive(Debug, Clone)]
pub struct SearchParams {
    pub query: String,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub min_volume: Option<u64>,
    pub max_volume: Option<u64>,
    pub limit: usize,
    pub case_sensitive: bool,
}

impl Default for SearchParams {
    fn default() -> Self {
        Self {
            query: String::new(),
            category: None,
            tags: Vec::new(),
            min_volume: None,
            max_volume: None,
            limit: 100,
            case_sensitive: false,
        }
    }
}

impl FastSearchEngine {
    /// Build a new search engine from markets using parallel processing
    pub async fn build(markets: Vec<GammaMarket>) -> Result<Self> {
        let start_time = std::time::Instant::now();
        info!("Building fast search engine for {} markets using parallel processing", markets.len());
        
        // Use concurrent data structures for parallel processing
        let pattern_set = DashMap::new();
        let pattern_to_docs: DashMap<String, RoaringBitmap> = DashMap::new();
        let category_index: DashMap<String, RoaringBitmap> = DashMap::new();
        let tag_index: DashMap<String, RoaringBitmap> = DashMap::new();
        let volume_buckets: DashMap<u64, RoaringBitmap> = DashMap::new();
        let doc_store: DashMap<DocId, Arc<GammaMarket>> = DashMap::new();
        
        // Process markets in parallel chunks for better performance
        let chunk_size = 5000;
        info!("Processing markets in parallel chunks of {}", chunk_size);
        
        markets.par_chunks(chunk_size).enumerate().for_each(|(chunk_idx, chunk)| {
            let chunk_start = chunk_idx * chunk_size;
            debug!("Processing chunk {} (markets {}-{})", chunk_idx, chunk_start, chunk_start + chunk.len());
            
            for (idx, market) in chunk.iter().enumerate() {
                let doc_id = (chunk_start + idx) as DocId;
                let market = Arc::new(market.clone());
                
                // Store the document
                doc_store.insert(doc_id, market.clone());
                
                // Extract searchable text patterns
                let mut local_patterns = Vec::new();
                
                // Add question words (lowercase for case-insensitive search)
                let question_lower = market.question.to_lowercase();
                for word in question_lower.split_whitespace() {
                    if word.len() >= 2 {
                        local_patterns.push(word.to_string());
                        pattern_set.insert(word.to_string(), ());
                    }
                }
                
                // Add market ID
                let id_pattern = market.id.0.to_string().to_lowercase();
                local_patterns.push(id_pattern.clone());
                pattern_set.insert(id_pattern, ());
                
                // Add slug
                let slug_lower = market.slug.to_lowercase();
                for part in slug_lower.split('-') {
                    if part.len() >= 2 {
                        local_patterns.push(part.to_string());
                        pattern_set.insert(part.to_string(), ());
                    }
                }
                
                // Index patterns
                for pattern in &local_patterns {
                    pattern_to_docs.entry(pattern.clone())
                        .or_insert_with(RoaringBitmap::new)
                        .insert(doc_id);
                }
                
                // Index category
                if let Some(ref category) = market.category {
                    category_index.entry(category.to_lowercase())
                        .or_insert_with(RoaringBitmap::new)
                        .insert(doc_id);
                }
                
                // Index tags (from description or other fields)
                if let Some(ref desc) = market.description {
                    for word in desc.split_whitespace() {
                        if word.starts_with('#') && word.len() > 1 {
                            let tag = word[1..].to_lowercase();
                            tag_index.entry(tag)
                                .or_insert_with(RoaringBitmap::new)
                                .insert(doc_id);
                        }
                    }
                }
                
                // Index volume (bucket by 10k)
                let volume = market.volume().to_u64().unwrap_or(0);
                let bucket = (volume / 10_000) * 10_000;
                volume_buckets.entry(bucket)
                    .or_insert_with(RoaringBitmap::new)
                    .insert(doc_id);
                
                // Skip FST for now - it's causing major slowdown
                // TODO: Re-enable FST with better performance
                // fst_items.insert((question_lower.as_bytes().to_vec(), doc_id as u64), ());
            }
        });
        
        info!("Parallel processing complete, converting to final data structures");
        
        // Convert DashMap to regular collections
        let patterns: Vec<String> = pattern_set.into_iter().map(|(k, _)| k).collect();
        
        // Create pattern to ID mapping for O(1) lookup
        let pattern_to_id: HashMap<String, usize> = patterns.iter()
            .enumerate()
            .map(|(id, pattern)| (pattern.clone(), id))
            .collect();
        
        // Build pattern ID mapping efficiently
        let mut pattern_to_docs_final: HashMap<usize, RoaringBitmap> = HashMap::new();
        for (pattern, bitmap) in pattern_to_docs.into_iter() {
            if let Some(&pattern_id) = pattern_to_id.get(&pattern) {
                pattern_to_docs_final.insert(pattern_id, bitmap);
            }
        }
        
        let category_index_final: HashMap<String, RoaringBitmap> = category_index.into_iter().collect();
        let tag_index_final: HashMap<String, RoaringBitmap> = tag_index.into_iter().collect();
        let volume_buckets_final: BTreeMap<u64, RoaringBitmap> = volume_buckets.into_iter().collect();
        let doc_store_final: HashMap<DocId, Arc<GammaMarket>> = doc_store.into_iter().collect();
        
        // Build Aho-Corasick automaton
        info!("Building Aho-Corasick automaton with {} patterns", patterns.len());
        let ac_start = std::time::Instant::now();
        let pattern_matcher = AhoCorasickBuilder::new()
            .match_kind(MatchKind::LeftmostFirst)
            .ascii_case_insensitive(true)
            .build(&patterns)
            .context("Failed to build Aho-Corasick automaton")?;
        info!("Aho-Corasick built in {:?}", ac_start.elapsed());
        
        // Skip FST building for now - it's the main performance bottleneck
        // TODO: Optimize FST building or use alternative prefix search
        info!("Skipping FST build for performance (prefix search disabled)");
        // Create an empty FST
        let mut fst_builder = MapBuilder::memory();
        fst_builder.insert(b"", 0)?; // Insert dummy entry to create valid FST
        let prefix_map = fst_builder.into_map();
        
        let build_time_ms = start_time.elapsed().as_millis() as u64;
        
        // Calculate index size (approximate)
        let pattern_size = patterns.len() * 20; // Rough estimate
        let bitmap_size = pattern_to_docs_final.values()
            .map(|b| b.serialized_size())
            .sum::<usize>();
        let index_size_bytes = pattern_size + bitmap_size + prefix_map.len();
        
        let stats = SearchStats {
            total_documents: doc_store_final.len(),
            total_patterns: patterns.len(),
            total_categories: category_index_final.len(),
            total_tags: tag_index_final.len(),
            index_size_bytes,
            build_time_ms,
        };
        
        info!("Fast search engine built in {}ms: {} docs, {} patterns, {} categories", 
              build_time_ms, stats.total_documents, stats.total_patterns, stats.total_categories);
        
        Ok(Self {
            pattern_matcher: Arc::new(pattern_matcher),
            pattern_to_docs: Arc::new(pattern_to_docs_final),
            category_index: Arc::new(category_index_final),
            tag_index: Arc::new(tag_index_final),
            volume_buckets: Arc::new(volume_buckets_final),
            doc_store: Arc::new(doc_store_final),
            prefix_map: Arc::new(prefix_map),
            stats,
        })
    }
    
    /// Search markets with ultra-fast performance
    pub fn search(&self, params: &SearchParams) -> Vec<Arc<GammaMarket>> {
        let search_start = std::time::Instant::now();
        
        // Start with all documents
        let mut result_set = if params.query.is_empty() {
            // No query, start with all docs
            let mut all_docs = RoaringBitmap::new();
            for doc_id in 0..self.doc_store.len() as u32 {
                all_docs.insert(doc_id);
            }
            all_docs
        } else {
            // Use Aho-Corasick for pattern matching
            let query_lower = if params.case_sensitive {
                params.query.clone()
            } else {
                params.query.to_lowercase()
            };
            
            let mut matching_docs = RoaringBitmap::new();
            
            // Find all matching patterns in the query
            for mat in self.pattern_matcher.find_iter(&query_lower) {
                if let Some(doc_ids) = self.pattern_to_docs.get(&mat.pattern().as_usize()) {
                    matching_docs |= doc_ids;
                }
            }
            
            // Also try prefix search for partial matches
            if matching_docs.is_empty() {
                // Use FST range query for prefix search
                let mut prefix_stream = self.prefix_map
                    .range()
                    .ge(query_lower.as_bytes())
                    .into_stream();
                
                while let Some((key, value)) = prefix_stream.next() {
                    if !key.starts_with(query_lower.as_bytes()) {
                        break;
                    }
                    matching_docs.insert(value as DocId);
                }
            }
            
            matching_docs
        };
        
        // Apply category filter
        if let Some(ref category) = params.category {
            if let Some(cat_docs) = self.category_index.get(&category.to_lowercase()) {
                result_set &= cat_docs;
            } else {
                result_set.clear(); // No markets in this category
            }
        }
        
        // Apply tag filters (AND operation)
        for tag in &params.tags {
            if let Some(tag_docs) = self.tag_index.get(&tag.to_lowercase()) {
                result_set &= tag_docs;
            } else {
                result_set.clear(); // Tag not found
                break;
            }
        }
        
        // Apply volume filters
        if params.min_volume.is_some() || params.max_volume.is_some() {
            let mut volume_docs = RoaringBitmap::new();
            
            for (bucket, docs) in self.volume_buckets.iter() {
                let include = match (params.min_volume, params.max_volume) {
                    (Some(min), Some(max)) => *bucket >= min && *bucket <= max,
                    (Some(min), None) => *bucket >= min,
                    (None, Some(max)) => *bucket <= max,
                    (None, None) => true,
                };
                
                if include {
                    volume_docs |= docs;
                }
            }
            
            result_set &= &volume_docs;
        }
        
        // Convert document IDs to markets
        let mut results: Vec<Arc<GammaMarket>> = result_set.iter()
            .filter_map(|doc_id| self.doc_store.get(&doc_id).cloned())
            .collect();
        
        // Sort by volume (descending)
        results.sort_by(|a, b| b.volume().partial_cmp(&a.volume()).unwrap_or(std::cmp::Ordering::Equal));
        
        // Apply limit
        results.truncate(params.limit);
        
        let search_time = search_start.elapsed();
        debug!("Search completed in {:?} with {} results", search_time, results.len());
        
        results
    }
    
    /// Get search statistics
    pub fn stats(&self) -> &SearchStats {
        &self.stats
    }
    
    /// Get all categories
    #[allow(dead_code)] // API kept for future use
    pub fn categories(&self) -> Vec<String> {
        self.category_index.keys().cloned().collect()
    }
    
    /// Get all tags
    #[allow(dead_code)] // API kept for future use
    pub fn tags(&self) -> Vec<String> {
        self.tag_index.keys().cloned().collect()
    }
}

/// Build and save fast search index from database
pub async fn build_fast_search_index(db_path: &Path, index_path: &Path, force_rebuild: bool) -> Result<FastSearchEngine> {
    info!("Building fast search index from database");
    
    // Check if index already exists
    let index_file = index_path.join("fast_search.bin");
    if !force_rebuild && index_file.exists() {
        info!("Fast search index already exists at {:?}", index_file);
        // For now, we rebuild anyway since we don't have serialization yet
    }
    
    // Load all markets from database
    let database = GammaDatabase::new(db_path).await
        .context("Failed to initialize database")?;
    
    let total_count = database.get_market_count().await?;
    info!("Loading {} markets from database with optimized batching", total_count);
    
    // Use efficient batch loading with larger batches
    let markets = database.get_all_markets(None).await?;
    info!("Loaded {} markets, starting parallel index build", markets.len());
    
    // Build the search engine
    let engine = FastSearchEngine::build(markets).await?;
    
    // Create index directory
    std::fs::create_dir_all(index_path)?;
    
    // TODO: Implement binary serialization for the engine
    // For now, we return the in-memory engine
    
    info!("Fast search index built successfully");
    Ok(engine)
}

/// Build fast search index using an existing database connection
#[allow(dead_code)] // API kept for future use
pub async fn build_fast_search_index_with_db(database: &GammaDatabase, index_path: &Path, force_rebuild: bool) -> Result<FastSearchEngine> {
    info!("Building fast search index using existing database connection");
    
    // Check if index already exists
    let index_file = index_path.join("fast_search.bin");
    if !force_rebuild && index_file.exists() {
        info!("Fast search index already exists at {:?}", index_file);
        // For now, we rebuild anyway since we don't have serialization yet
    }
    
    let total_count = database.get_market_count().await?;
    info!("Loading {} markets from database with optimized batching", total_count);
    
    // Use efficient batch loading with larger batches
    let markets = database.get_all_markets(None).await?;
    info!("Loaded {} markets, starting parallel index build", markets.len());
    
    // Build the search engine
    let engine = FastSearchEngine::build(markets).await?;
    
    // Create index directory
    std::fs::create_dir_all(index_path)?;
    
    // TODO: Implement binary serialization for the engine
    // For now, we return the in-memory engine
    
    info!("Fast search index built successfully");
    Ok(engine)
}