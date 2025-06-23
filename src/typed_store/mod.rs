//! Typed RocksDB storage framework for market data
//!
//! This module provides type-safe, polymorphic storage on a single RocksDB instance.
//! Each logical "table" gets its own prefix byte and strongly-typed key/value pairs.

pub mod codec;
pub mod context;
pub mod models;
pub mod store;
pub mod table;

pub use context::TypedDbContext;
pub use store::TypedStore;

// Re-export specific types that are used externally
// Note: These are used in the binary commands but may appear unused in lib compilation
#[allow(unused_imports)]
pub use models::{
    Condition, ConditionCf, ConditionIndexCf, ConditionTable, MarketByConditionCf,
    MarketByConditionTable, MarketCf, MarketIndex, MarketIndexCf, MarketIndexTable, MarketTable,
    RocksDbMarket, Token, TokenCf, TokenIndexCf, TokenTable, TokensByConditionCf,
    TokensByConditionTable, ALL_COLUMN_FAMILIES,
};
