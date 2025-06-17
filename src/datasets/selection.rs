//! Custom user token selections for streaming

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, debug};

/// A user-defined selection of tokens to watch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSelection {
    /// Unique name for this selection
    pub name: String,
    
    /// Human-readable description
    pub description: Option<String>,
    
    /// When this selection was created
    pub created_at: DateTime<Utc>,
    
    /// When this selection was last modified
    pub modified_at: DateTime<Utc>,
    
    /// The selected token IDs
    pub tokens: Vec<TokenInfo>,
    
    /// Tags for categorizing selections
    pub tags: Vec<String>,
    
    /// Additional metadata
    pub metadata: SelectionMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    /// The token ID
    pub token_id: String,
    
    /// Optional human-readable name/description
    pub name: Option<String>,
    
    /// Optional market question/description
    pub market: Option<String>,
    
    /// When this token was added to the selection
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionMetadata {
    /// Who created this selection (could be username, system, etc.)
    pub created_by: String,
    
    /// Version for backward compatibility
    pub version: u32,
    
    /// Optional notes about this selection
    pub notes: Option<String>,
}

/// Represents a market from the datasets
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Market {
    pub question: Option<String>,
    pub description: Option<String>,
    pub tokens: Vec<Token>,
    pub tags: Option<Vec<String>>,
}

/// Represents a token within a market
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Token {
    pub token_id: String,
    pub outcome: Option<String>,
    pub price: Option<f64>,
}

/// Represents an implicit selection derived from datasets
#[derive(Debug, Clone)]
pub struct ImplicitSelection {
    pub name: String,
    pub description: String,
    pub source_path: PathBuf,
    pub tokens: Vec<TokenInfo>,
    pub tags: Vec<String>,
    pub _dataset_type: String,
    pub created_at: DateTime<Utc>,
}

/// Manages token selections
pub struct SelectionManager {
    base_path: PathBuf,
    datasets_path: PathBuf,
}

impl SelectionManager {
    pub fn new(data_path: impl AsRef<Path>) -> Self {
        let data_path = data_path.as_ref();
        Self {
            base_path: data_path.join("datasets").join("selection"),
            datasets_path: data_path.join("datasets"),
        }
    }
    
    /// Ensure the selections directory exists
    pub fn ensure_directory(&self) -> Result<()> {
        fs::create_dir_all(&self.base_path)
            .context("Failed to create selections directory")?;
        Ok(())
    }
    
    /// Discover implicit selections from all datasets
    pub fn discover_implicit_selections(&self) -> Result<Vec<ImplicitSelection>> {
        let mut selections = Vec::new();
        
        if !self.datasets_path.exists() {
            debug!("Datasets path does not exist: {}", self.datasets_path.display());
            return Ok(selections);
        }
        
        // Walk through all dataset directories
        for entry in fs::read_dir(&self.datasets_path)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() && path.file_name().and_then(|n| n.to_str()) != Some("selection") {
                if let Ok(dataset_selections) = self.discover_selections_in_dataset(&path) {
                    selections.extend(dataset_selections);
                }
            }
        }
        
        Ok(selections)
    }
    
    /// Discover selections within a specific dataset directory
    fn discover_selections_in_dataset(&self, dataset_path: &Path) -> Result<Vec<ImplicitSelection>> {
        let mut selections = Vec::new();
        let dataset_name = dataset_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        
        debug!("Scanning dataset: {}", dataset_name);
        
        // Look for markets.json files recursively
        self.find_markets_files(dataset_path, &mut |markets_file| {
            if let Ok(tokens) = self.extract_tokens_from_markets_file(markets_file) {
                if !tokens.is_empty() {
                    // Create a selection for this markets file
                    let relative_path = markets_file.strip_prefix(&self.datasets_path)
                        .unwrap_or(markets_file);
                    
                    let selection_name = format!("dataset_{}", 
                        relative_path.to_string_lossy()
                            .replace(['/', '\\'], "_")
                            .replace(".json", ""));
                    
                    let description = format!("Auto-discovered from {} ({} tokens)", 
                        relative_path.display(), tokens.len());
                    
                    let selection = ImplicitSelection {
                        name: selection_name,
                        description,
                        source_path: markets_file.to_path_buf(),
                        tokens,
                        tags: vec!["auto-discovered".to_string(), dataset_name.to_string()],
                        _dataset_type: self.infer_dataset_type(dataset_path),
                        created_at: self.get_file_creation_time(markets_file),
                    };
                    
                    selections.push(selection);
                }
            }
        })?;
        
        Ok(selections)
    }
    
    /// Find all markets.json files in a directory recursively
    fn find_markets_files<F>(&self, dir: &Path, callback: &mut F) -> Result<()>
    where
        F: FnMut(&Path),
    {
        if !dir.is_dir() {
            return Ok(());
        }
        
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                self.find_markets_files(&path, callback)?;
            } else if path.file_name().and_then(|n| n.to_str()) == Some("markets.json") {
                callback(&path);
            }
        }
        
        Ok(())
    }
    
    /// Extract tokens from a markets.json file
    fn extract_tokens_from_markets_file(&self, file_path: &Path) -> Result<Vec<TokenInfo>> {
        let content = fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read markets file: {}", file_path.display()))?;
        
        let markets: Vec<Market> = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse markets JSON: {}", file_path.display()))?;
        
        let mut tokens = Vec::new();
        let now = Utc::now();
        
        for market in markets {
            let market_question = market.question.clone();
            
            for token in market.tokens {
                let name = if let Some(outcome) = &token.outcome {
                    if let Some(question) = &market_question {
                        Some(format!("{} - {}", question, outcome))
                    } else {
                        Some(outcome.clone())
                    }
                } else {
                    market_question.clone()
                };
                
                tokens.push(TokenInfo {
                    token_id: token.token_id,
                    name,
                    market: market_question.clone(),
                    added_at: now,
                });
            }
        }
        
        Ok(tokens)
    }
    
    /// Infer the dataset type from the directory structure and contents
    fn infer_dataset_type(&self, dataset_path: &Path) -> String {
        let name = dataset_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        
        // Check for dataset.yaml file to get accurate type information
        let dataset_yaml = dataset_path.join("dataset.yaml");
        if dataset_yaml.exists() {
            if let Ok(content) = std::fs::read_to_string(&dataset_yaml) {
                if let Ok(parsed) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                    if let Some(dataset_type) = parsed.get("dataset_type").and_then(|v| v.as_str()) {
                        return match dataset_type {
                            "MarketData" => "Raw Market Data".to_string(),
                            "AnalyzedMarkets" => "Analyzed Markets".to_string(),
                            "EnrichedMarkets" => "Enriched Markets".to_string(),
                            "Pipeline" => {
                                // Try to get pipeline name for more context
                                if let Some(additional_info) = parsed.get("additional_info") {
                                    if let Some(pipeline_name) = additional_info.get("pipeline_name").and_then(|v| v.as_str()) {
                                        format!("Pipeline: {}", pipeline_name)
                                    } else {
                                        "Pipeline Output".to_string()
                                    }
                                } else {
                                    "Pipeline Output".to_string()
                                }
                            }
                            _ => format!("Dataset: {}", dataset_type),
                        };
                    }
                }
            }
        }
        
        // Fallback to directory name inference
        if name.contains("raw") {
            "Raw Markets".to_string()
        } else if name.contains("bitcoin") {
            "Bitcoin Markets".to_string()
        } else if name.contains("pipeline") || name.contains("runs") {
            "Pipeline Output".to_string()
        } else if name.contains("analyzed") || name.contains("analysis") {
            "Analyzed Markets".to_string()
        } else if name.contains("enriched") {
            "Enriched Markets".to_string()
        } else {
            "Market Dataset".to_string()
        }
    }
    
    /// Get file creation time (or fall back to current time)
    fn get_file_creation_time(&self, file_path: &Path) -> DateTime<Utc> {
        fs::metadata(file_path)
            .and_then(|meta| meta.created())
            .map(|time| time.into())
            .unwrap_or_else(|_| Utc::now())
    }
    
    /// List all available selections (both explicit and implicit)
    pub fn list_all_selections(&self) -> Result<Vec<String>> {
        let mut all_selections = Vec::new();
        
        // Add explicit user-created selections
        if let Ok(explicit) = self.list_selections() {
            all_selections.extend(explicit);
        }
        
        // Add implicit selections from datasets
        if let Ok(implicit) = self.discover_implicit_selections() {
            for selection in implicit {
                all_selections.push(selection.name);
            }
        }
        
        all_selections.sort();
        all_selections.dedup(); // Remove duplicates
        Ok(all_selections)
    }
    
    /// List all available selections
    pub fn list_selections(&self) -> Result<Vec<String>> {
        self.ensure_directory()?;
        
        let mut selections = Vec::new();
        let entries = fs::read_dir(&self.base_path)
            .context("Failed to read selections directory")?;
        
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    selections.push(name.to_string());
                }
            }
        }
        
        selections.sort();
        Ok(selections)
    }
    
    /// Load a selection by name (explicit or implicit)
    pub fn load_selection(&self, name: &str) -> Result<TokenSelection> {
        // Try to load as explicit selection first
        let explicit_path = self.get_selection_path(name);
        if explicit_path.exists() {
            let content = fs::read_to_string(&explicit_path)
                .with_context(|| format!("Failed to read selection file: {}", explicit_path.display()))?;
            
            let selection: TokenSelection = serde_json::from_str(&content)
                .with_context(|| format!("Failed to parse selection JSON: {}", name))?;
            
            return Ok(selection);
        }
        
        // Try to find as implicit selection
        let implicit_selections = self.discover_implicit_selections()?;
        if let Some(implicit) = implicit_selections.iter().find(|s| s.name == name) {
            return Ok(self.convert_implicit_to_explicit(implicit));
        }
        
        Err(anyhow::anyhow!("Selection '{}' not found", name))
    }
    
    /// Convert an implicit selection to a TokenSelection
    fn convert_implicit_to_explicit(&self, implicit: &ImplicitSelection) -> TokenSelection {
        TokenSelection {
            name: implicit.name.clone(),
            description: Some(implicit.description.clone()),
            created_at: implicit.created_at,
            modified_at: implicit.created_at,
            tokens: implicit.tokens.clone(),
            tags: implicit.tags.clone(),
            metadata: SelectionMetadata {
                created_by: "auto-discovery".to_string(),
                version: 1,
                notes: Some(format!("Auto-discovered from: {}", implicit.source_path.display())),
            },
        }
    }
    
    /// Save a selection
    pub fn save_selection(&self, selection: &TokenSelection) -> Result<()> {
        self.ensure_directory()?;
        
        let path = self.get_selection_path(&selection.name);
        let json = serde_json::to_string_pretty(selection)
            .context("Failed to serialize selection")?;
        
        fs::write(&path, json)
            .with_context(|| format!("Failed to write selection file: {}", path.display()))?;
        
        info!("Saved selection '{}' with {} tokens", selection.name, selection.tokens.len());
        Ok(())
    }
    
    /// Delete a selection
    pub fn delete_selection(&self, name: &str) -> Result<()> {
        let path = self.get_selection_path(name);
        
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("Failed to delete selection: {}", name))?;
            info!("Deleted selection '{}'", name);
        }
        
        Ok(())
    }
    
    /// Get all tokens from a selection (explicit or implicit)
    pub fn get_tokens(&self, name: &str) -> Result<Vec<String>> {
        let selection = self.load_selection(name)?;
        Ok(selection.tokens.into_iter().map(|t| t.token_id).collect())
    }
    
    /// Get detailed information about all selections (both explicit and implicit)
    pub fn get_all_selections_info(&self) -> Result<Vec<(TokenSelection, bool)>> {
        let mut all_selections = Vec::new();
        
        // Get explicit selections
        let explicit_names = self.list_selections()?;
        for name in explicit_names {
            if let Ok(selection) = self.load_selection(&name) {
                all_selections.push((selection, true)); // true = explicit
            }
        }
        
        // Get implicit selections
        let implicit_selections = self.discover_implicit_selections()?;
        for implicit in implicit_selections {
            let selection = self.convert_implicit_to_explicit(&implicit);
            all_selections.push((selection, false)); // false = implicit
        }
        
        Ok(all_selections)
    }
    
    /// Create a new selection
    pub fn create_selection(
        &self,
        name: String,
        description: Option<String>,
        tokens: Vec<String>,
    ) -> Result<TokenSelection> {
        let now = Utc::now();
        
        let token_infos: Vec<TokenInfo> = tokens
            .into_iter()
            .map(|token_id| TokenInfo {
                token_id,
                name: None,
                market: None,
                added_at: now,
            })
            .collect();
        
        let selection = TokenSelection {
            name,
            description,
            created_at: now,
            modified_at: now,
            tokens: token_infos,
            tags: Vec::new(),
            metadata: SelectionMetadata {
                created_by: "user".to_string(),
                version: 1,
                notes: None,
            },
        };
        
        Ok(selection)
    }
    
    /// Add tokens to an existing selection
    pub fn add_tokens(&self, name: &str, tokens: Vec<String>) -> Result<()> {
        let mut selection = self.load_selection(name)?;
        let now = Utc::now();
        
        // Get existing token IDs to avoid duplicates
        let existing: HashSet<String> = selection.tokens
            .iter()
            .map(|t| t.token_id.clone())
            .collect();
        
        // Add new tokens
        for token_id in tokens {
            if !existing.contains(&token_id) {
                selection.tokens.push(TokenInfo {
                    token_id,
                    name: None,
                    market: None,
                    added_at: now,
                });
            }
        }
        
        selection.modified_at = now;
        self.save_selection(&selection)?;
        
        Ok(())
    }
    
    /// Remove tokens from a selection
    pub fn remove_tokens(&self, name: &str, tokens: &[String]) -> Result<()> {
        let mut selection = self.load_selection(name)?;
        let tokens_set: HashSet<&str> = tokens.iter().map(|s| s.as_str()).collect();
        
        selection.tokens.retain(|t| !tokens_set.contains(t.token_id.as_str()));
        selection.modified_at = Utc::now();
        
        self.save_selection(&selection)?;
        Ok(())
    }
    
    fn get_selection_path(&self, name: &str) -> PathBuf {
        self.base_path.join(format!("{}.json", name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_selection_management() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SelectionManager::new(temp_dir.path());
        
        // Create a selection
        let selection = manager.create_selection(
            "my_favorites".to_string(),
            Some("My favorite prediction markets".to_string()),
            vec![
                "token123".to_string(),
                "token456".to_string(),
            ],
        ).unwrap();
        
        // Save it
        manager.save_selection(&selection).unwrap();
        
        // List selections
        let selections = manager.list_selections().unwrap();
        assert_eq!(selections.len(), 1);
        assert_eq!(selections[0], "my_favorites");
        
        // Load it back
        let loaded = manager.load_selection("my_favorites").unwrap();
        assert_eq!(loaded.name, "my_favorites");
        assert_eq!(loaded.tokens.len(), 2);
        
        // Add more tokens
        manager.add_tokens("my_favorites", vec!["token789".to_string()]).unwrap();
        
        // Verify
        let updated = manager.load_selection("my_favorites").unwrap();
        assert_eq!(updated.tokens.len(), 3);
        
        // Delete it
        manager.delete_selection("my_favorites").unwrap();
        let selections = manager.list_selections().unwrap();
        assert_eq!(selections.len(), 0);
    }
}