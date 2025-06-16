//! Pipeline system for running workflow scripts composed of CLI commands

use anyhow::{Result, Context};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use crate::data_paths::{DEFAULT_RUNS_DIR, DEFAULT_DATASETS_DIR};

pub mod config;
pub mod runner;
pub mod tui;

pub use config::*;
pub use runner::*;
pub use tui::*;

/// A pipeline step that executes a CLI command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStep {
    /// Human-readable name for this step
    pub name: String,
    /// CLI command to execute (without the binary name)
    pub command: String,
    /// Arguments to pass to the command
    #[serde(default)]
    pub args: Vec<String>,
    /// Whether to continue on failure (default: false)
    #[serde(default)]
    pub continue_on_error: bool,
    /// Environment variables to set for this step
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// Pipeline configuration loaded from YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pipeline {
    /// Pipeline name
    pub name: String,
    /// Pipeline description
    #[serde(default)]
    pub description: String,
    /// Parameters that can be used in step arguments
    #[serde(default)]
    pub parameters: HashMap<String, String>,
    /// List of steps to execute
    pub steps: Vec<PipelineStep>,
}

/// Pipeline execution context with resolved parameters
#[derive(Debug, Clone)]
pub struct PipelineContext {
    /// Resolved parameters (including date variables)
    pub parameters: HashMap<String, String>,
    /// Whether to run in dry-run mode
    pub dry_run: bool,
}

impl Pipeline {
    /// Load pipeline from YAML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read pipeline file: {}", path.as_ref().display()))?;
        
        let pipeline: Pipeline = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse pipeline YAML: {}", path.as_ref().display()))?;
        
        Ok(pipeline)
    }

    /// Create execution context with resolved parameters
    pub fn create_context(&self, extra_params: HashMap<String, String>) -> PipelineContext {
        let mut parameters = self.parameters.clone();
        
        // Add date parameters
        let now = Local::now();
        parameters.insert("date".to_string(), now.format("%Y-%m-%d").to_string());
        parameters.insert("datetime".to_string(), now.format("%Y-%m-%d_%H-%M-%S").to_string());
        parameters.insert("timestamp".to_string(), now.timestamp().to_string());
        parameters.insert("year".to_string(), now.format("%Y").to_string());
        parameters.insert("month".to_string(), now.format("%m").to_string());
        parameters.insert("day".to_string(), now.format("%d").to_string());
        
        // Add default dataset directory parameters
        parameters.insert("datasets_dir".to_string(), DEFAULT_DATASETS_DIR.to_string());
        parameters.insert("runs_dir".to_string(), DEFAULT_RUNS_DIR.to_string());
        
        // Convert pipeline name to snake_case_lowercase
        let pipeline_name_snake = self.name
            .replace(" ", "_")
            .replace("-", "_")
            .to_lowercase();
        
        parameters.insert("pipeline_output_dir".to_string(), format!("{}/pipeline_{}_{}", 
            DEFAULT_RUNS_DIR, pipeline_name_snake, now.format("%Y%m%d_%H%M%S")));
        
        // Add extra parameters (override defaults)
        for (key, value) in extra_params {
            parameters.insert(key, value);
        }
        
        PipelineContext {
            parameters,
            dry_run: false,
        }
    }

    /// Resolve parameter placeholders in a string
    pub fn resolve_parameters(&self, text: &str, context: &PipelineContext) -> String {
        let mut result = text.to_string();
        let mut changed = true;
        let mut iterations = 0;
        let max_iterations = 10; // Prevent infinite loops
        
        // Keep resolving until no more substitutions are made or max iterations reached
        while changed && iterations < max_iterations {
            changed = false;
            let old_result = result.clone();
            
            for (key, value) in &context.parameters {
                let placeholder = format!("${{{}}}", key);
                if result.contains(&placeholder) {
                    result = result.replace(&placeholder, value);
                    changed = true;
                }
            }
            
            iterations += 1;
            
            // Safety check to prevent infinite loops
            if result == old_result {
                break;
            }
        }
        
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parameter_resolution() {
        let pipeline = Pipeline {
            name: "Test".to_string(),
            description: "Test pipeline".to_string(),
            parameters: HashMap::new(),
            steps: vec![],
        };
        
        let mut extra_params = HashMap::new();
        extra_params.insert("dataset_name".to_string(), "my_dataset".to_string());
        
        let context = pipeline.create_context(extra_params);
        let resolved = pipeline.resolve_parameters("--dataset-name results_${date}_${dataset_name}", &context);
        
        assert!(resolved.contains("results_"));
        assert!(resolved.contains("my_dataset"));
        assert!(!resolved.contains("${"));
    }
} 