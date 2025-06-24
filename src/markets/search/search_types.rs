//! Search types and filters for Milli integration

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use rust_decimal::prelude::ToPrimitive;
use crate::markets::gamma::types::GammaMarket;

/// Document structure optimized for Milli search
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDocument {
    /// Primary identifier for the market
    pub id: String,
    
    /// Condition ID for the market
    pub condition_id: String,
    
    /// Market question - primary searchable field
    pub question: String,
    
    /// Market description - secondary searchable field
    pub description: Option<String>,
    
    /// Market outcomes as searchable text
    pub outcomes: Vec<String>,
    
    /// Searchable outcomes as single string for better search
    pub outcomes_text: String,
    
    /// Category for faceted search
    pub category: Option<String>,
    
    /// Market slug for URL-based lookup
    pub slug: String,
    
    /// Financial metrics for sorting and filtering
    pub volume: f64,
    pub liquidity: f64,
    
    /// Status flags for filtering
    pub active: bool,
    pub closed: bool,
    pub archived: bool,
    pub restricted: bool,
    pub approved: bool,
    
    /// Timestamps for date-based filtering and sorting
    pub created_at: String,
    pub updated_at: String,
    pub end_date: Option<String>,
    
    /// Computed scores for ranking
    pub popularity_score: f64,
    pub relevance_boost: f64,
    
    /// Volume metrics for range filtering
    pub volume_24hr: f64,
    pub volume_1wk: f64,
    pub volume_1mo: f64,
    
    /// Additional searchable fields
    pub clob_token_ids: Vec<String>,
}

#[allow(dead_code)]
impl MarketDocument {
    /// Convert from GammaMarket to optimized search document
    pub fn from_gamma_market(market: &GammaMarket) -> Self {
        let outcomes_text = market.outcomes.join(" ");
        let volume = market.volume().to_f64().unwrap_or(0.0);
        let liquidity = market.liquidity.unwrap_or_default().to_f64().unwrap_or(0.0);
        
        // Calculate popularity score (volume + liquidity + activity bonuses)
        let popularity_score = volume * 0.7 + liquidity * 0.3 
            + if market.active { 100.0 } else { 0.0 }
            + if market.featured { 50.0 } else { 0.0 };
            
        // Relevance boost for trending/featured markets
        let relevance_boost = if market.featured { 2.0 } 
            else if market.new { 1.5 } 
            else { 1.0 };
        
        Self {
            id: market.id.0.to_string(),
            condition_id: market.condition_id.0.clone(),
            question: market.question.clone(),
            description: market.description.clone(),
            outcomes: market.outcomes.clone(),
            outcomes_text,
            category: market.category.clone(),
            slug: market.slug.clone(),
            volume,
            liquidity,
            active: market.active,
            closed: market.closed,
            archived: market.archived,
            restricted: market.restricted,
            approved: market.approved,
            created_at: market.created_at.to_rfc3339(),
            updated_at: market.updated_at.to_rfc3339(),
            end_date: market.end_date.map(|d| d.to_rfc3339()),
            popularity_score,
            relevance_boost,
            volume_24hr: market.volume_24hr.unwrap_or_default().to_f64().unwrap_or(0.0),
            volume_1wk: market.volume_1wk.unwrap_or_default().to_f64().unwrap_or(0.0),
            volume_1mo: market.volume_1mo.unwrap_or_default().to_f64().unwrap_or(0.0),
            clob_token_ids: market.clob_token_ids.iter().map(|id| id.0.clone()).collect(),
        }
    }
}

/// Search filters for precise query control
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    /// Categories to filter by
    pub categories: Option<Vec<String>>,
    
    /// Volume range filtering
    pub min_volume: Option<f64>,
    pub max_volume: Option<f64>,
    
    /// Status filters
    pub active_only: bool,
    pub closed_only: bool,
    pub approved_only: bool,
    
    /// Date range filtering
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
    pub ending_before: Option<DateTime<Utc>>,
    
    /// Sorting options
    pub sort_by: SortOption,
    pub sort_desc: bool,
    
    /// Pagination
    pub limit: usize,
    pub offset: usize,
}

/// Available sorting options
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub enum SortOption {
    #[default]
    Relevance,
    Volume,
    Liquidity,
    PopularityScore,
    CreatedAt,
    UpdatedAt,
    EndDate,
}

#[allow(dead_code)]
impl SortOption {
    pub fn field_name(&self) -> &'static str {
        match self {
            SortOption::Relevance => "_score",
            SortOption::Volume => "volume",
            SortOption::Liquidity => "liquidity", 
            SortOption::PopularityScore => "popularity_score",
            SortOption::CreatedAt => "created_at",
            SortOption::UpdatedAt => "updated_at",
            SortOption::EndDate => "end_date",
        }
    }
}

/// Search results with metadata
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SearchResults {
    /// Found documents
    pub documents: Vec<MarketDocument>,
    
    /// Total results found (before pagination)
    pub total_hits: usize,
    
    /// Search processing time in milliseconds
    pub processing_time_ms: u64,
    
    /// Query used for search
    pub query: String,
    
    /// Facet distributions for filtering
    pub facets: Option<std::collections::HashMap<String, std::collections::HashMap<String, usize>>>,
}

/// Quick search suggestions
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SearchSuggestion {
    pub text: String,
    pub market_count: usize,
    pub category: Option<String>,
}