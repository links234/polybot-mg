//! Address book module for managing and tracking multiple Ethereum addresses
//!
//! This module provides a comprehensive system for storing, managing, and querying
//! multiple Ethereum addresses with descriptions and metadata.

pub mod types;
pub mod storage;
pub mod service;
pub mod commands;
pub mod db;

// Re-export core types
pub use types::{
    AddressEntry, AddressStats,
};

// Re-export storage
// pub use storage::{AddressBookStorage, AddressBookError};

// Re-export service
pub use service::{
    AddressBookServiceHandle, AddressBookCommand,
};

// Re-export commands
pub use commands::{
    AddressCommand,
};