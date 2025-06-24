//! Embedded search engine using Milli
//! 
//! This module provides lightning-fast search capabilities using Milli embedded directly
//! in the application. No external dependencies or servers required.

pub mod milli_service;
pub mod search_types;

// Note: Types are available via search::milli_service::* and search::search_types::* when needed