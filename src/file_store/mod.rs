//! File-based storage system for market data
//!
//! Creates a structured file hierarchy:
//! - data/database/markets/condition/<condition_id>/
//! - data/database/markets/token/<token_id>/

use crate::markets::fetcher::{Market, MarketToken};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub struct FileStore {
    base_path: PathBuf,
}

impl FileStore {
    pub fn new(base_path: PathBuf) -> Result<Self> {
        // Ensure base directories exist
        let markets_path = base_path.join("markets");
        fs::create_dir_all(markets_path.join("condition"))?;
        fs::create_dir_all(markets_path.join("token"))?;
        fs::create_dir_all(markets_path.join("market"))?;

        Ok(Self { base_path })
    }

    /// Store a market and all its related data
    pub fn store_market(&self, market: &Market) -> Result<()> {
        // Store by condition_id
        if let Some(condition_id) = &market.condition_id {
            if !condition_id.trim().is_empty() {
                self.store_market_by_condition(condition_id, market)?;
            }
        }

        // Store by market_id
        if let Some(market_id) = &market.id {
            if !market_id.trim().is_empty() {
                self.store_market_by_id(market_id, market)?;
            }
        }

        // Store tokens
        for token in &market.tokens {
            self.store_token(&token, market)?;
        }

        Ok(())
    }

    fn store_market_by_condition(&self, condition_id: &str, market: &Market) -> Result<()> {
        let condition_path = self
            .base_path
            .join("markets")
            .join("condition")
            .join(sanitize_filename(condition_id));

        fs::create_dir_all(&condition_path)?;

        // Store market info
        let market_file = condition_path.join("market.json");
        let json = serde_json::to_string_pretty(market)?;
        fs::write(market_file, json)?;

        // Store metadata
        let metadata = ConditionMetadata {
            condition_id: condition_id.to_string(),
            question: market.question.clone(),
            description: market.description.clone(),
            category: market.category.clone(),
            tags: market.tags.clone(),
            outcomes: market.outcomes.clone(),
            token_ids: market.tokens.iter().map(|t| t.token_id.clone()).collect(),
            market_id: market.id.clone(),
            active: market.active,
            closed: market.closed,
            volume: market.volume,
            volume_24hr: market.volume_24hr,
            created_at: market.created_at.clone(),
            updated_at: market.updated_at.clone(),
        };

        let metadata_file = condition_path.join("metadata.json");
        let json = serde_json::to_string_pretty(&metadata)?;
        fs::write(metadata_file, json)?;

        // Store token list
        let tokens_file = condition_path.join("tokens.json");
        let json = serde_json::to_string_pretty(&market.tokens)?;
        fs::write(tokens_file, json)?;

        Ok(())
    }

    fn store_market_by_id(&self, market_id: &str, market: &Market) -> Result<()> {
        let market_path = self
            .base_path
            .join("markets")
            .join("market")
            .join(sanitize_filename(market_id));

        fs::create_dir_all(&market_path)?;

        let market_file = market_path.join("data.json");
        let json = serde_json::to_string_pretty(market)?;
        fs::write(market_file, json)?;

        Ok(())
    }

    fn store_token(&self, token: &MarketToken, market: &Market) -> Result<()> {
        let token_path = self
            .base_path
            .join("markets")
            .join("token")
            .join(sanitize_filename(&token.token_id));

        fs::create_dir_all(&token_path)?;

        // Store token data
        let token_data = TokenData {
            token_id: token.token_id.clone(),
            outcome: token.outcome.clone(),
            price: token.price,
            winner: token.winner,
            volume: token.volume,
            volume_24hr: token.volume_24hr,
            supply: token.supply,
            market_cap: token.market_cap,
            condition_id: market.condition_id.clone(),
            market_id: market.id.clone(),
            question: market.question.clone(),
            active: market.active,
            closed: market.closed,
        };

        let token_file = token_path.join("data.json");
        let json = serde_json::to_string_pretty(&token_data)?;
        fs::write(token_file, json)?;

        // Store market reference
        if let Some(market_id) = &market.id {
            let market_ref_file = token_path.join("market_reference.json");
            let market_ref = MarketReference {
                market_id: market_id.clone(),
                condition_id: market.condition_id.clone(),
                question: market.question.clone(),
            };
            let json = serde_json::to_string_pretty(&market_ref)?;
            fs::write(market_ref_file, json)?;
        }

        Ok(())
    }

    /// Get statistics about stored data
    pub fn get_stats(&self) -> Result<StoreStats> {
        let conditions_path = self.base_path.join("markets").join("condition");
        let tokens_path = self.base_path.join("markets").join("token");
        let markets_path = self.base_path.join("markets").join("market");

        let condition_count = count_directories(&conditions_path)?;
        let token_count = count_directories(&tokens_path)?;
        let market_count = count_directories(&markets_path)?;

        Ok(StoreStats {
            conditions: condition_count,
            tokens: token_count,
            markets: market_count,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ConditionMetadata {
    condition_id: String,
    question: String,
    description: Option<String>,
    category: Option<String>,
    tags: Option<Vec<String>>,
    outcomes: Option<Vec<String>>,
    token_ids: Vec<String>,
    market_id: Option<String>,
    active: bool,
    closed: bool,
    volume: Option<f64>,
    volume_24hr: Option<f64>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TokenData {
    token_id: String,
    outcome: String,
    price: f64,
    winner: Option<bool>,
    volume: Option<f64>,
    volume_24hr: Option<f64>,
    supply: Option<f64>,
    market_cap: Option<f64>,
    condition_id: Option<String>,
    market_id: Option<String>,
    question: String,
    active: bool,
    closed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct MarketReference {
    market_id: String,
    condition_id: Option<String>,
    question: String,
}

#[derive(Debug)]
pub struct StoreStats {
    pub conditions: usize,
    pub tokens: usize,
    pub markets: usize,
}

/// Sanitize filename to remove invalid characters
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

fn count_directories(path: &Path) -> Result<usize> {
    if !path.exists() {
        return Ok(0);
    }

    let count = fs::read_dir(path)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .count();

    Ok(count)
}
