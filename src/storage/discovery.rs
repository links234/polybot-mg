//! Dataset discovery using the existing dataset manager

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::info;

use crate::datasets::{
    BinaryFormat, CommandInfo, DatasetCommandInfo, DatasetInfo, DatasetType, DetectedCommand,
    ExecutionContext, FileInfo, FileType, JsonSubtype, TextFormat, YamlPurpose,
};

#[derive(Debug, Clone)]
pub struct DiscoveredDataset {
    pub token_id: String,
    pub _session_id: String,
    pub market: String,
    pub _outcome: String,
    pub _start_time: DateTime<Utc>,
    pub _end_time: Option<DateTime<Utc>>,
    pub _update_count: u64,
    pub path: PathBuf,
    pub _has_snapshot: bool,
    pub _update_files_count: usize,
    // Enhanced metadata
    pub dataset_info: DatasetInfo,
}

#[derive(Debug)]
pub struct DatasetDiscovery {
    base_path: PathBuf,
}

impl DatasetDiscovery {
    pub fn new(base_path: impl AsRef<Path>) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
        }
    }


    /// Discover all available datasets using a unified approach
    pub async fn discover_datasets(&self) -> Result<Vec<DiscoveredDataset>> {
        info!("Looking for datasets in: {:?}", self.base_path);

        let mut discovered_datasets = Vec::new();

        // Scan the datasets directory directly for better organization
        let datasets_dir = self.base_path.clone();
        if datasets_dir.exists() {
            self.scan_datasets_directory(&datasets_dir, &mut discovered_datasets)?;
        }

        // Remove duplicates based on token_id and path
        discovered_datasets.sort_by(|a, b| a.token_id.cmp(&b.token_id));
        discovered_datasets.dedup_by(|a, b| a.token_id == b.token_id && a.path == b.path);

        info!("Found {} unique datasets", discovered_datasets.len());
        Ok(discovered_datasets)
    }

    /// Scan the datasets directory for organized dataset discovery
    fn scan_datasets_directory(
        &self,
        dir: &Path,
        datasets: &mut Vec<DiscoveredDataset>,
    ) -> Result<()> {
        if !dir.is_dir() {
            return Ok(());
        }

        let entries = std::fs::read_dir(dir)
            .with_context(|| format!("Failed to read directory: {}", dir.display()))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let dir_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");

                // Skip certain directories
                if dir_name == "selection" || dir_name.starts_with('.') {
                    continue;
                }

                // Check if this is a top-level dataset category (like bitcoin_price_bets)
                if self.is_dataset_category(&path) {
                    self.process_dataset_category(&path, dir_name, datasets)?;
                } else {
                    // Continue scanning subdirectories
                    self.scan_datasets_directory(&path, datasets)?;
                }
            }
        }

        Ok(())
    }

    /// Check if a directory is a dataset category (contains timestamped subdirectories)
    fn is_dataset_category(&self, dir: &Path) -> bool {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    // Check if it looks like a date directory (YYYY-MM-DD format)
                    if name.len() == 10
                        && name.chars().nth(4) == Some('-')
                        && name.chars().nth(7) == Some('-')
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Process a dataset category (like bitcoin_price_bets) and find the latest version
    fn process_dataset_category(
        &self,
        category_path: &Path,
        category_name: &str,
        datasets: &mut Vec<DiscoveredDataset>,
    ) -> Result<()> {
        let mut latest_dataset_path: Option<PathBuf> = None;
        let mut latest_date = String::new();

        // Find the latest date directory
        if let Ok(entries) = std::fs::read_dir(category_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if name > latest_date.as_str() {
                        latest_date = name.to_string();

                        // Look for the actual dataset directory within the date directory
                        if let Ok(date_entries) = std::fs::read_dir(&path) {
                            for date_entry in date_entries.flatten() {
                                let date_path = date_entry.path();
                                if date_path.is_dir() && self.has_dataset_files(&date_path) {
                                    latest_dataset_path = Some(date_path);
                                    break;
                                }
                            }
                        }

                        // If no subdirectory with dataset files, check the date directory itself
                        if latest_dataset_path.is_none() && self.has_dataset_files(&path) {
                            latest_dataset_path = Some(path);
                        }
                    }
                }
            }
        }

        // Create a dataset entry for the latest version
        if let Some(dataset_path) = latest_dataset_path {
            if let Ok(dataset_info) =
                self.create_dataset_info_from_path(&dataset_path, category_name)
            {
                let discovered = self.convert_dataset_info(dataset_info);
                datasets.push(discovered);
            }
        }

        Ok(())
    }

    /// Check if a directory contains dataset files (markets.json or dataset.yaml)
    fn has_dataset_files(&self, dir: &Path) -> bool {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let name = file_name.to_str().unwrap_or("");
                if name == "markets.json" || name == "dataset.yaml" {
                    return true;
                }
            }
        }
        false
    }

    /// Create DatasetInfo from a path and category name
    fn create_dataset_info_from_path(
        &self,
        path: &Path,
        category_name: &str,
    ) -> Result<DatasetInfo> {
        let mut files = Vec::new();
        let mut total_size = 0u64;

        // Scan files in the directory
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let file_path = entry.path();
                if file_path.is_file() {
                    let metadata = std::fs::metadata(&file_path)?;
                    let size = metadata.len();
                    total_size += size;

                    let file_name = file_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    files.push(FileInfo {
                        name: file_name,
                        relative_path: PathBuf::from(file_path.file_name().unwrap_or_default()),
                        size_bytes: size,
                        file_type: self.infer_file_type(&file_path),
                        modified_at: metadata.modified().ok().map(|t| t.into()),
                        content_hash: None,
                        metadata: Default::default(),
                    });
                }
            }
        }

        // Try to load dataset.yaml for metadata
        let dataset_yaml_path = path.join("dataset.yaml");
        let (dataset_type, command_info) = if dataset_yaml_path.exists() {
            if let Ok(metadata) = crate::datasets::load_dataset_metadata(path) {
                let dataset_type = self.parse_dataset_type(&metadata, category_name);
                let dataset_command_info = self.convert_command_info(&metadata.command_info);
                (dataset_type, dataset_command_info)
            } else {
                (
                    self.infer_dataset_type_from_name(category_name),
                    DatasetCommandInfo {
                        primary_command: None,
                        detected_commands: Vec::new(),
                        evidence: HashMap::new(),
                        confidence: 0.0,
                        execution_context: None,
                    },
                )
            }
        } else {
            (
                self.infer_dataset_type_from_name(category_name),
                DatasetCommandInfo {
                    primary_command: None,
                    detected_commands: Vec::new(),
                    evidence: HashMap::new(),
                    confidence: 0.0,
                    execution_context: None,
                },
            )
        };

        let metadata = std::fs::metadata(path)?;

        Ok(DatasetInfo {
            name: category_name.to_string(),
            path: path.to_path_buf(),
            dataset_type,
            command_info,
            size_bytes: total_size,
            file_count: files.len(),
            created_at: metadata.created().ok().map(|t| t.into()),
            modified_at: metadata.modified().ok().map(|t| t.into()),
            files,
            health_status: crate::datasets::DatasetHealthStatus::Healthy,
            warnings: Vec::new(),
            metrics: Default::default(),
        })
    }

    /// Infer file type from path
    fn infer_file_type(&self, path: &Path) -> FileType {
        match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
            "json" => FileType::Json {
                subtype: JsonSubtype::Data,
            },
            "yaml" | "yml" => FileType::Yaml {
                purpose: YamlPurpose::Configuration,
            },
            "txt" | "log" => FileType::Text {
                format: TextFormat::Plain,
            },
            _ => FileType::Binary {
                format: BinaryFormat::Unknown,
            },
        }
    }

    /// Parse dataset type from metadata
    fn parse_dataset_type(
        &self,
        metadata: &crate::datasets::DatasetMetadata,
        category_name: &str,
    ) -> DatasetType {
        match metadata.dataset_type.as_str() {
            "Pipeline" => {
                let pipeline_name = metadata
                    .additional_info
                    .get("pipeline_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or(category_name);
                DatasetType::Pipeline {
                    name: pipeline_name.to_string(),
                    version: None,
                }
            }
            "MarketData" => DatasetType::MarketData {
                source: crate::datasets::DataSource::ClobApi,
            },
            "AnalyzedMarkets" => {
                let source_dataset = metadata
                    .additional_info
                    .get("source_dataset")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                DatasetType::AnalyzedMarkets {
                    source_dataset,
                    filter_count: None,
                }
            }
            _ => self.infer_dataset_type_from_name(category_name),
        }
    }

    /// Infer dataset type from category name
    fn infer_dataset_type_from_name(&self, name: &str) -> DatasetType {
        if name.contains("raw") {
            DatasetType::MarketData {
                source: crate::datasets::DataSource::ClobApi,
            }
        } else if name.contains("bitcoin") || name.contains("analyzed") {
            DatasetType::AnalyzedMarkets {
                source_dataset: "raw_markets".to_string(),
                filter_count: None,
            }
        } else if name.contains("pipeline") {
            DatasetType::Pipeline {
                name: name.to_string(),
                version: None,
            }
        } else {
            DatasetType::Unknown
        }
    }

    /// Convert CommandInfo to DatasetCommandInfo
    fn convert_command_info(&self, command_info: &CommandInfo) -> DatasetCommandInfo {
        let detected_command = DetectedCommand {
            command: command_info.command.clone(),
            args: command_info.args.clone(),
            confidence: 1.0,
            evidence: vec![format!("Found in dataset.yaml")],
        };

        DatasetCommandInfo {
            primary_command: Some(command_info.command.clone()),
            detected_commands: vec![detected_command],
            evidence: HashMap::new(),
            confidence: 1.0,
            execution_context: Some(ExecutionContext {
                version: command_info.version.clone(),
                host: None,
                user: None,
                working_directory: None,
                environment: HashMap::new(),
                execution_time: command_info.executed_at,
                duration_seconds: None,
            }),
        }
    }

    /// Convert DatasetInfo to DiscoveredDataset
    fn convert_dataset_info(&self, dataset: DatasetInfo) -> DiscoveredDataset {
        // Use the dataset name directly as the token_id for better identification
        let token_id = dataset.name.clone();

        DiscoveredDataset {
            token_id,
            _session_id: dataset
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&dataset.name)
                .to_string(),
            market: self.extract_market_info(&dataset),
            _outcome: "N/A".to_string(), // Markets don't have specific outcomes
            _start_time: dataset.created_at.unwrap_or_else(|| Utc::now()),
            _end_time: dataset.modified_at,
            _update_count: dataset.file_count as u64,
            path: dataset.path.clone(),
            _has_snapshot: true, // All datasets are considered "snapshots" of market data
            _update_files_count: dataset.file_count,
            // Store the full dataset info for detailed display
            dataset_info: dataset,
        }
    }


    /// Extract market information from dataset
    fn extract_market_info(&self, dataset: &DatasetInfo) -> String {
        // For pipeline runs, extract info from dataset.yaml metadata
        if matches!(
            dataset.dataset_type,
            crate::datasets::DatasetType::Pipeline { .. }
        ) {
            // Try to extract pipeline name and info from additional_info
            if let Ok(metadata) = crate::datasets::load_dataset_metadata(&dataset.path) {
                if let Some(pipeline_name) = metadata
                    .additional_info
                    .get("pipeline_name")
                    .and_then(|v| v.as_str())
                {
                    let mut info = format!("Pipeline: {}", pipeline_name);

                    // Add step information if available
                    if let Some(successful_steps) = metadata
                        .additional_info
                        .get("successful_steps")
                        .and_then(|v| v.as_u64())
                    {
                        if let Some(total_steps) = metadata
                            .additional_info
                            .get("total_steps")
                            .and_then(|v| v.as_u64())
                        {
                            info.push_str(&format!(
                                " ({}/{} steps)",
                                successful_steps, total_steps
                            ));
                        }
                    }

                    // Add duration if available
                    if let Some(duration) = metadata
                        .additional_info
                        .get("total_duration_secs")
                        .and_then(|v| v.as_u64())
                    {
                        info.push_str(&format!(" - {}s", duration));
                    }

                    return info;
                }
            }

            // Fallback for pipeline runs
            return format!("Pipeline Run: {}", dataset.name);
        }

        // Try to get market description from the first market file
        for file in &dataset.files {
            if file.name.contains("markets") && file.name.ends_with(".json") {
                if let Ok(content) = std::fs::read_to_string(&dataset.path.join(&file.name)) {
                    if let Ok(markets) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(array) = markets.as_array() {
                            if let Some(first_market) = array.first() {
                                if let Some(question) =
                                    first_market.get("question").and_then(|q| q.as_str())
                                {
                                    return question.to_string();
                                }
                                if let Some(description) =
                                    first_market.get("description").and_then(|d| d.as_str())
                                {
                                    // Return first 100 chars of description
                                    let desc = description.chars().take(100).collect::<String>();
                                    return if description.len() > 100 {
                                        format!("{}...", desc)
                                    } else {
                                        desc
                                    };
                                }
                            }
                        }
                    }
                }
                break;
            }
        }

        format!("Dataset: {}", dataset.name)
    }


}
