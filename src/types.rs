//! Core type definitions for the Polybot project
//! 
//! This module contains strongly-typed structures that replace tuple usage
//! throughout the codebase, ensuring CLAUDE.md compliance with "no tuples in public APIs".


// Note: Orderbook-related types (PriceLevel, MarketDepth, AssetOrderBook, SpreadInfo)
// have been moved to src/execution/orderbook.rs for better organization

/// Category count information
/// Replaces (String, usize) tuples for category statistics
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryCount {
    pub category: String,
    pub count: usize,
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_count() {
        let category = CategoryCount {
            category: "Politics".to_string(),
            count: 42,
        };
        assert_eq!(category.category, "Politics");
        assert_eq!(category.count, 42);
    }
}