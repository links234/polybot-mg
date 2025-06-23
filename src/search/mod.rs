//! Embedded search engine using Milli
//! 
//! This module provides lightning-fast search capabilities using Milli embedded directly
//! in the application. No external dependencies or servers required.

pub mod milli_service;
pub mod search_types;

pub use milli_service::MilliSearchService;
pub use search_types::*;