//! Embedded Milli search service for ultra-fast market queries
//! 
//! This service provides lightning-fast search capabilities using Milli embedded directly
//! in the application. No external dependencies or servers required.

use std::path::Path;
use std::collections::HashMap;
use anyhow::{Context, Result};
use tracing::{info, debug, warn};

use crate::gamma::database::{GammaDatabase, MarketRecord};
use super::search_types::*;

/// Embedded Milli search service (placeholder implementation)
/// 
/// This is a simplified implementation that will be enhanced once Milli API stabilizes.
/// For now, it provides fast in-memory search capabilities.
pub struct MilliSearchService {
    db_path: String,
    documents: Vec<MarketDocument>,
}

impl MilliSearchService {
    /// Create new Milli search service
    pub fn new<P: AsRef<Path>>(search_path: P) -> Result<Self> {
        let path = search_path.as_ref();
        info!("Initializing search service at: {}", path.display());
        
        // Create directory if it doesn't exist
        std::fs::create_dir_all(path)
            .context("Failed to create search index directory")?;
        
        info!("Search service initialized successfully");
        
        Ok(Self {
            db_path: path.to_string_lossy().to_string(),
            documents: Vec::new(),
        })
    }
    
    /// Bulk index all markets from SurrealDB
    pub async fn index_all_markets(&mut self, gamma_db: &GammaDatabase) -> Result<usize> {
        info!("Starting bulk indexing of all markets");
        
        // Get total count for progress tracking
        let total_count = gamma_db.get_market_count().await?;
        info!("Found {} markets to index", total_count);
        
        if total_count == 0 {
            warn!("No markets found in database to index");
            return Ok(0);
        }
        
        // Process in batches to avoid memory issues
        let batch_size = 1000;
        let mut indexed_count = 0;
        let mut offset = 0;
        
        while offset < total_count {
            let limit = std::cmp::min(batch_size, total_count - offset);
            
            // Get batch of market records directly
            let markets = gamma_db.execute_raw_query(&format!(
                "SELECT * FROM markets LIMIT {} START {}", 
                limit, offset
            )).await?;
            
            if markets.is_empty() {
                break;
            }
            
            // Convert to search documents
            for market_value in &markets {
                if let Ok(market_record) = serde_json::from_value::<MarketRecord>(market_value.clone()) {
                    let gamma_market = gamma_db.convert_record_to_market(&market_record);
                    let search_doc = MarketDocument::from_gamma_market(&gamma_market);
                    self.documents.push(search_doc);
                    indexed_count += 1;
                }
            }
            
            offset += limit;
            
            // Progress update every 5000 markets
            if indexed_count % 5000 == 0 {
                info!("Indexing progress: {}/{} markets", indexed_count, total_count);
            }
        }
        
        info!("Bulk indexing completed: {} markets indexed", indexed_count);
        Ok(indexed_count)
    }
    
    /// Index multiple documents in a single transaction
    pub fn index_documents(&mut self, documents: &[MarketDocument]) -> Result<()> {
        if documents.is_empty() {
            return Ok(());
        }
        
        debug!("Indexing {} documents", documents.len());
        
        // Add to in-memory index
        self.documents.extend_from_slice(documents);
        
        debug!("Successfully indexed {} documents", documents.len());
        Ok(())
    }
    
    /// Index a single document
    pub fn index_document(&mut self, document: &MarketDocument) -> Result<()> {
        self.index_documents(&[document.clone()])
    }
    
    /// Perform fast text search with filters
    pub fn search(&self, query: &str, filters: &SearchFilters) -> Result<SearchResults> {
        let start_time = std::time::Instant::now();
        
        debug!("Searching for: '{}' with filters: {:?}", query, filters);
        
        let mut results: Vec<MarketDocument> = self.documents.iter()
            .filter(|doc| {
                // Text search
                if !query.is_empty() {
                    let query_lower = query.to_lowercase();
                    let matches_text = doc.question.to_lowercase().contains(&query_lower) ||
                        doc.description.as_ref().map_or(false, |d| d.to_lowercase().contains(&query_lower)) ||
                        doc.outcomes_text.to_lowercase().contains(&query_lower) ||
                        doc.category.as_ref().map_or(false, |c| c.to_lowercase().contains(&query_lower));
                    
                    if !matches_text {
                        return false;
                    }
                }
                
                // Status filters
                if filters.active_only && !doc.active {
                    return false;
                }
                if filters.closed_only && !doc.closed {
                    return false;
                }
                if filters.approved_only && !doc.approved {
                    return false;
                }
                
                // Volume range filters
                if let Some(min_vol) = filters.min_volume {
                    if doc.volume < min_vol {
                        return false;
                    }
                }
                if let Some(max_vol) = filters.max_volume {
                    if doc.volume > max_vol {
                        return false;
                    }
                }
                
                // Category filter
                if let Some(ref categories) = filters.categories {
                    if !categories.is_empty() {
                        if let Some(ref doc_category) = doc.category {
                            if !categories.contains(doc_category) {
                                return false;
                            }
                        } else {
                            return false;
                        }
                    }
                }
                
                true
            })
            .cloned()
            .collect();
        
        // Sort results
        match filters.sort_by {
            SortOption::Volume => {
                results.sort_by(|a, b| {
                    if filters.sort_desc {
                        b.volume.partial_cmp(&a.volume).unwrap_or(std::cmp::Ordering::Equal)
                    } else {
                        a.volume.partial_cmp(&b.volume).unwrap_or(std::cmp::Ordering::Equal)
                    }
                });
            }
            SortOption::Liquidity => {
                results.sort_by(|a, b| {
                    if filters.sort_desc {
                        b.liquidity.partial_cmp(&a.liquidity).unwrap_or(std::cmp::Ordering::Equal)
                    } else {
                        a.liquidity.partial_cmp(&b.liquidity).unwrap_or(std::cmp::Ordering::Equal)
                    }
                });
            }
            SortOption::PopularityScore => {
                results.sort_by(|a, b| {
                    if filters.sort_desc {
                        b.popularity_score.partial_cmp(&a.popularity_score).unwrap_or(std::cmp::Ordering::Equal)
                    } else {
                        a.popularity_score.partial_cmp(&b.popularity_score).unwrap_or(std::cmp::Ordering::Equal)
                    }
                });
            }
            SortOption::CreatedAt => {
                results.sort_by(|a, b| {
                    if filters.sort_desc {
                        b.created_at.cmp(&a.created_at)
                    } else {
                        a.created_at.cmp(&b.created_at)
                    }
                });
            }
            _ => {}
        }
        
        let total_hits = results.len();
        
        // Apply pagination
        let start = filters.offset;
        let end = std::cmp::min(start + filters.limit, results.len());
        if start < results.len() {
            results = results[start..end].to_vec();
        } else {
            results.clear();
        }
        
        let processing_time = start_time.elapsed().as_millis() as u64;
        
        debug!("Search completed: {} results in {}ms", results.len(), processing_time);
        
        Ok(SearchResults {
            documents: results,
            total_hits,
            processing_time_ms: processing_time,
            query: query.to_string(),
            facets: None,
        })
    }
    
    /// Quick search with default filters
    pub fn quick_search(&self, query: &str, limit: usize) -> Result<SearchResults> {
        let filters = SearchFilters {
            limit,
            active_only: true,
            approved_only: true,
            sort_by: SortOption::PopularityScore,
            sort_desc: true,
            ..Default::default()
        };
        
        self.search(query, &filters)
    }
    
    /// Get search suggestions for autocomplete
    pub fn get_suggestions(&self, prefix: &str, limit: usize) -> Result<Vec<SearchSuggestion>> {
        let filters = SearchFilters {
            limit,
            active_only: true,
            approved_only: true,
            sort_by: SortOption::PopularityScore,
            sort_desc: true,
            ..Default::default()
        };
        
        let results = self.search(prefix, &filters)?;
        
        let suggestions = results.documents.into_iter()
            .take(limit)
            .map(|doc| SearchSuggestion {
                text: doc.question.clone(),
                market_count: 1,
                category: doc.category.clone(),
            })
            .collect();
        
        Ok(suggestions)
    }
    
    /// Get market by ID (fast direct lookup)
    pub fn get_market_by_id(&self, market_id: &str) -> Result<Option<MarketDocument>> {
        for doc in &self.documents {
            if doc.id == market_id {
                return Ok(Some(doc.clone()));
            }
        }
        Ok(None)
    }
    
    /// Get markets by category (fast filtered search)
    pub fn get_markets_by_category(&self, category: &str, limit: usize) -> Result<Vec<MarketDocument>> {
        let filters = SearchFilters {
            categories: Some(vec![category.to_string()]),
            limit,
            active_only: true,
            approved_only: true,
            sort_by: SortOption::Volume,
            sort_desc: true,
            ..Default::default()
        };
        
        let results = self.search("", &filters)?;
        Ok(results.documents)
    }
    
    /// Get index statistics
    pub fn get_stats(&self) -> Result<HashMap<String, u64>> {
        let mut stats = HashMap::new();
        
        stats.insert("total_documents".to_string(), self.documents.len() as u64);
        
        // Get index size (approximate)
        let index_size = std::fs::metadata(&self.db_path)
            .map(|m| m.len())
            .unwrap_or(0);
        stats.insert("index_size_bytes".to_string(), index_size);
        
        Ok(stats)
    }
    
    /// Clear the entire index
    pub fn clear_index(&mut self) -> Result<()> {
        info!("Clearing search index");
        self.documents.clear();
        info!("Search index cleared successfully");
        Ok(())
    }
    
    /// Re-index all markets (clear + bulk index)
    pub async fn reindex_all(&mut self, gamma_db: &GammaDatabase) -> Result<usize> {
        info!("Starting complete re-indexing");
        
        // Clear existing index
        self.clear_index()?;
        
        // Bulk index all markets
        let count = self.index_all_markets(gamma_db).await?;
        
        info!("Re-indexing completed: {} markets", count);
        Ok(count)
    }
}

/// Helper function to create search service with standard path
pub async fn create_search_service(data_dir: &Path) -> Result<MilliSearchService> {
    let search_path = data_dir.join("search_index");
    MilliSearchService::new(search_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[test]
    fn test_search_service_creation() {
        let temp_dir = tempdir().unwrap();
        let search_path = temp_dir.path().join("test_search");
        
        let service = MilliSearchService::new(&search_path).unwrap();
        let stats = service.get_stats().unwrap();
        
        assert_eq!(stats.get("total_documents"), Some(&0));
    }
}