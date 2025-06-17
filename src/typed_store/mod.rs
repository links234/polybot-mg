//! Typed RocksDB storage framework for market data
//! 
//! This module provides type-safe, polymorphic storage on a single RocksDB instance.
//! Each logical "table" gets its own prefix byte and strongly-typed key/value pairs.

pub mod codec;
pub mod table;
pub mod store;
pub mod context;
pub mod models;

pub use store::TypedStore;
pub use context::TypedDbContext;

// Re-export specific types that are used externally
// Note: These are used in the binary commands but may appear unused in lib compilation
#[allow(unused_imports)]
pub use models::{
    RocksDbMarket, Condition, Token, MarketIndex,
    MarketTable, MarketByConditionTable, ConditionTable, TokenTable, 
    TokensByConditionTable, MarketIndexTable,
    MarketCf, MarketByConditionCf, ConditionCf, TokenCf,
    TokensByConditionCf, MarketIndexCf, TokenIndexCf, ConditionIndexCf,
    ALL_COLUMN_FAMILIES
};