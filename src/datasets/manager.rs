//! Dataset manager for scanning and managing pipeline outputs

use super::{DatasetInfo, DatasetType, FileInfo, FileType, format_bytes, DatasetCommandInfo, load_dataset_metadata};
use crate::data_paths::{DEFAULT_DATASETS_DIR, DEFAULT_RUNS_DIR, DATASETS_DIR, RUNS_DIR};
use anyhow::{Result, Context};
use chrono::{DateTime, Local};
use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use tracing::{debug, warn, info};
use serde_json;

/// Configuration for dataset scanning
#[derive(Debug, Clone)]
pub struct DatasetManagerConfig {
    /// Base directory to scan for datasets
    pub base_dir: PathBuf,
    /// Additional directories to scan
    pub scan_dirs: Vec<PathBuf>,
    /// Whether to scan subdirectories recursively
    pub recursive: bool,
    /// Maximum depth for recursive scanning
    pub max_depth: usize,
}

impl Default for DatasetManagerConfig {
    fn default() -> Self {
        Self {
            base_dir: PathBuf::from(DEFAULT_DATASETS_DIR),
            scan_dirs: vec![
                PathBuf::from(DEFAULT_DATASETS_DIR),
                PathBuf::from(DEFAULT_RUNS_DIR),
                PathBuf::from(format!("{}/{}", DEFAULT_DATASETS_DIR, RUNS_DIR)),
                PathBuf::from(DATASETS_DIR),
                PathBuf::from("datasets"),
                PathBuf::from("outputs"),
                PathBuf::from("results"),
                PathBuf::from("pipelines"),
            ],
            recursive: true,
            max_depth: 5, // Increased for deeper nesting
        }
    }
}

/// Manager for dataset operations
pub struct DatasetManager {
    config: DatasetManagerConfig,
    datasets: Vec<DatasetInfo>,
    last_scan: Option<DateTime<Local>>,
}

impl DatasetManager {
    /// Create a new dataset manager
    pub fn new(config: DatasetManagerConfig) -> Self {
        Self {
            config,
            datasets: Vec::new(),
            last_scan: None,
        }
    }


    /// Scan for datasets in the configured directories
    pub fn scan_datasets(&mut self) -> Result<()> {
        info!("Scanning for datasets...");
        self.datasets.clear();

        // Clone the paths to avoid borrowing issues
        let base_dir = self.config.base_dir.clone();
        let scan_dirs = self.config.scan_dirs.clone();

        // Scan base directory
        self.scan_directory(&base_dir, 0)?;

        // Scan additional directories
        for scan_dir in &scan_dirs {
            if scan_dir.exists() {
                self.scan_directory(scan_dir, 0)?;
            }
        }

        // Sort datasets by creation time (newest first)
        self.datasets.sort_by(|a, b| {
            match (a.created_at, b.created_at) {
                (Some(a_time), Some(b_time)) => b_time.cmp(&a_time),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.name.cmp(&b.name),
            }
        });

        self.last_scan = Some(Local::now());
        info!("Found {} datasets", self.datasets.len());
        Ok(())
    }

    /// Scan a specific directory for datasets
    fn scan_directory(&mut self, dir: &Path, depth: usize) -> Result<()> {
        // Clone config values to avoid borrowing issues
        let max_depth = self.config.max_depth;
        let recursive = self.config.recursive;
        
        if depth >= max_depth {
            return Ok(());
        }

        debug!("Scanning directory: {}", dir.display());

        let entries = fs::read_dir(dir)
            .with_context(|| format!("Failed to read directory: {}", dir.display()))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let dir_name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");

                // Check if this looks like a dataset directory
                if self.is_dataset_directory(&path, dir_name) {
                    match self.analyze_dataset(&path) {
                        Ok(dataset) => {
                            debug!("Found dataset: {}", dataset.name);
                            self.datasets.push(dataset);
                        }
                        Err(e) => {
                            warn!("Failed to analyze dataset {}: {}", path.display(), e);
                        }
                    }
                } else if recursive && depth < max_depth {
                    // Recursively scan subdirectories
                    if let Err(e) = self.scan_directory(&path, depth + 1) {
                        warn!("Failed to scan subdirectory {}: {}", path.display(), e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if a directory looks like a dataset
    fn is_dataset_directory(&self, path: &Path, name: &str) -> bool {
        // Skip hidden directories and common non-dataset directories
        if name.starts_with('.') || name.starts_with("target") || name == "src" || name == "tests" {
            return false;
        }

        // First priority: check for dataset.yaml metadata file
        if path.join("dataset.yaml").exists() {
            return true;
        }

        // Second priority: check for legacy .command_info.json file
        if path.join(".command_info.json").exists() {
            return true;
        }

        // Third priority: check for pipeline output patterns
        if name.contains("pipeline") || name.contains("analysis") || name.contains("fetch") 
            || name.contains("monitor") || name.contains("market") || name.contains("data") 
            || name.contains("results") || name.contains("enriched") {
            return self.has_data_files(path);
        }

        // Fourth priority: check for date/timestamp patterns (common in automated outputs)
        if self.has_timestamp_pattern(name) {
            return self.has_data_files(path);
        }

        // Fifth priority: check for common dataset structure
        self.has_dataset_structure(path)
    }

    /// Check if directory name has timestamp patterns
    fn has_timestamp_pattern(&self, name: &str) -> bool {
        // Check for various timestamp formats
        name.matches('-').count() >= 2 ||  // YYYY-MM-DD or similar
        name.matches('_').count() >= 2 ||  // YYYY_MM_DD or YYYYMMDD_HHMMSS
        name.len() >= 8 && name.chars().take(8).all(|c| c.is_ascii_digit()) // YYYYMMDD
    }

    /// Check if directory has data files (JSON, CSV, etc.)
    fn has_data_files(&self, path: &Path) -> bool {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.filter_map(|e| e.ok()) {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".json") || name.ends_with(".csv") || name.ends_with(".parquet") 
                        || name.contains("chunk") || name.contains("data") || name.contains("markets") {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Check if directory has typical dataset structure
    fn has_dataset_structure(&self, path: &Path) -> bool {
        let mut has_data = false;
        let mut has_metadata = false;
        
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.filter_map(|e| e.ok()) {
                if let Some(name) = entry.file_name().to_str() {
                    // Check for data files
                    if name.ends_with(".json") || name.ends_with(".csv") || name.contains("chunk") 
                        || name.contains("data") || name.contains("markets") || name.contains("results") {
                        has_data = true;
                    }
                    
                    // Check for metadata files (any format)
                    if name.contains("metadata") || name.contains("info") || name.contains("config")
                        || name.ends_with(".yaml") || name.ends_with(".yml") {
                        has_metadata = true;
                    }
                }
            }
        }
        
        // A dataset should have both data and some form of metadata/info
        has_data && (has_metadata || self.has_recognizable_data_pattern(path))
    }

    /// Check for recognizable data patterns that indicate this is likely a dataset
    fn has_recognizable_data_pattern(&self, path: &Path) -> bool {
        if let Ok(entries) = fs::read_dir(path) {
            let file_count = entries.count();
            // If there are multiple files, it's likely a dataset
            return file_count > 1;
        }
        false
    }

    /// Analyze a dataset directory and create DatasetInfo
    fn analyze_dataset(&self, path: &Path) -> Result<DatasetInfo> {
        let name = path.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid directory name"))?
            .to_string();

        let mut files = Vec::new();
        let mut total_size = 0u64;
        let mut warnings = Vec::new();
        let mut created_at = None;

        // Scan files in the dataset directory
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.filter_map(|e| e.ok()) {
                let file_path = entry.path();
                
                if file_path.is_file() {
                    match self.analyze_file(&file_path) {
                        Ok(file_info) => {
                            total_size += file_info.size_bytes;
                            
                            // Use the oldest file time as dataset creation time
                            if let Some(file_time) = file_info.modified_at {
                                if created_at.is_none() || created_at.unwrap() > file_time {
                                    created_at = Some(file_time);
                                }
                            }
                            
                            files.push(file_info);
                        }
                        Err(e) => {
                            warnings.push(format!("Failed to analyze file {}: {}", 
                                file_path.display(), e));
                        }
                    }
                }
            }
        }

        // Sort files by name
        files.sort_by(|a, b| a.name.cmp(&b.name));

        // Use the new generic detection system
        let dataset_type = DatasetType::from_dir_analysis(&name, &files, None);
        let command_info = self.analyze_commands(&path, &name, &files);
        let is_complete = self.check_dataset_completeness_generic(&command_info, &files, &mut warnings);

        // Convert string warnings to DatasetWarning structs
        use super::{DatasetWarning, WarningCategory, WarningSeverity, DatasetHealthStatus, DatasetMetrics};
        use chrono::Utc;
        
        let dataset_warnings: Vec<DatasetWarning> = warnings.into_iter().map(|msg| {
            DatasetWarning {
                category: WarningCategory::InconsistentMetadata,
                message: msg,
                affected_file: None,
                detected_at: Utc::now(),
                severity: WarningSeverity::Warning,
            }
        }).collect();

        // Determine health status based on completeness and warnings
        let health_status = if !is_complete {
            DatasetHealthStatus::Incomplete
        } else if !dataset_warnings.is_empty() {
            DatasetHealthStatus::Warning
        } else {
            DatasetHealthStatus::Healthy
        };

        // Find the most recent modification time
        let modified_at = files.iter()
            .filter_map(|f| f.modified_at)
            .max()
            .map(|local_time| local_time.with_timezone(&Utc));

        Ok(DatasetInfo {
            name,
            path: path.to_path_buf(),
            dataset_type,
            command_info,
            size_bytes: total_size,
            file_count: files.len(),
            created_at: created_at.map(|local_time| local_time.with_timezone(&Utc)),
            modified_at,
            files,
            health_status,
            warnings: dataset_warnings,
            metrics: DatasetMetrics::default(),
        })
    }

    /// Analyze a single file and create FileInfo
    fn analyze_file(&self, path: &Path) -> Result<FileInfo> {
        let name = path.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid file name"))?
            .to_string();

        let metadata = fs::metadata(path)
            .with_context(|| format!("Failed to read file metadata: {}", path.display()))?;

        let size_bytes = metadata.len();
        let file_type = FileType::from_filename_and_content(&name, None);
        
        let modified_at = metadata.modified()
            .ok()
            .map(|system_time| DateTime::<Local>::from(system_time))
            .map(|local_time| local_time.with_timezone(&chrono::Utc));

        use super::FileMetadata;

        Ok(FileInfo {
            name: name.clone(),
            relative_path: PathBuf::from(&name),
            size_bytes,
            file_type,
            modified_at,
            content_hash: None,
            metadata: FileMetadata::default(),
        })
    }

    /// Analyze which CLI commands likely produced this dataset
    fn analyze_commands(&self, dir_path: &Path, dir_name: &str, files: &[FileInfo]) -> DatasetCommandInfo {
        let mut detected_commands = Vec::new();
        let mut evidence = HashMap::new();
        let mut confidence_scores = Vec::new();

        // First priority: check for YAML metadata file
        if let Ok(metadata) = load_dataset_metadata(dir_path) {
            let command = metadata.command_info.command.clone();
            detected_commands.push(command.clone());
            
            let mut cmd_evidence = vec![
                "Found dataset.yaml metadata file".to_string(),
                format!("Command: {}", command),
                format!("Version: {}", metadata.command_info.version),
                format!("Executed at: {}", metadata.command_info.executed_at.format("%Y-%m-%d %H:%M:%S UTC")),
            ];
            
            if !metadata.command_info.args.is_empty() {
                cmd_evidence.push(format!("Args: {}", metadata.command_info.args.join(" ")));
            }
            
            if !metadata.description.is_empty() {
                cmd_evidence.push(format!("Description: {}", metadata.description));
            }
            
            evidence.insert(command, cmd_evidence);
            confidence_scores.push(1.0); // Perfect confidence for YAML metadata
            
            // Return early with perfect confidence
            use super::DetectedCommand;
            let detected_commands: Vec<DetectedCommand> = detected_commands.into_iter().map(|cmd| {
                DetectedCommand {
                    command: cmd.clone(),
                    args: Vec::new(),
                    confidence: 1.0,
                    evidence: evidence.get(&cmd).cloned().unwrap_or_default(),
                }
            }).collect();
            
            return DatasetCommandInfo {
                primary_command: detected_commands.first().map(|cmd| cmd.command.clone()),
                detected_commands,
                evidence,
                confidence: 1.0,
                execution_context: None,
            };
        }

        // Second priority: check for legacy JSON metadata file
        if let Ok(metadata) = self.load_legacy_command_metadata(dir_path) {
            if let Some(command) = metadata.get("command").and_then(|v| v.as_str()) {
                detected_commands.push(command.to_string());
                
                let mut cmd_evidence = vec![
                    "Found legacy .command_info.json metadata file".to_string(),
                    format!("Command: {}", command),
                ];
                
                if let Some(args) = metadata.get("args").and_then(|v| v.as_array()) {
                    cmd_evidence.push(format!("Args: {}", args.iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")));
                }
                
                if let Some(executed_at) = metadata.get("executed_at").and_then(|v| v.as_str()) {
                    cmd_evidence.push(format!("Executed at: {}", executed_at));
                }
                
                evidence.insert(command.to_string(), cmd_evidence);
                confidence_scores.push(0.95); // High confidence for legacy metadata
                
                // Return early with high confidence
                let confidence = confidence_scores.iter().sum::<f32>() / confidence_scores.len() as f32;
                let detected_commands: Vec<super::DetectedCommand> = detected_commands.into_iter().map(|cmd| {
                    super::DetectedCommand {
                        command: cmd.clone(),
                        args: Vec::new(),
                        confidence: 0.95,
                        evidence: evidence.get(&cmd).cloned().unwrap_or_default(),
                    }
                }).collect();
                
                return DatasetCommandInfo {
                    primary_command: detected_commands.first().map(|cmd| cmd.command.clone()),
                    detected_commands,
                    evidence,
                    confidence: confidence as f64,
                    execution_context: None,
                };
            }
        }

        // Fall back to heuristic detection if no metadata files found
        self.analyze_commands_heuristic(dir_name, files)
    }
    
    /// Heuristic command detection based on file patterns and directory names
    fn analyze_commands_heuristic(&self, dir_name: &str, files: &[FileInfo]) -> DatasetCommandInfo {
        let mut detected_commands = Vec::new();
        let mut evidence = HashMap::new();
        let mut confidence_scores = Vec::new();

        // Check for fetch-all-markets command evidence
        let mut fetch_evidence = Vec::new();
        for file in files {
            if matches!(file.file_type, FileType::Json { subtype: super::JsonSubtype::MarketChunk }) {
                fetch_evidence.push(format!("Market chunk file: {}", file.name));
            }
            if matches!(file.file_type, FileType::Json { subtype: super::JsonSubtype::State }) && (file.name.contains("fetch") || file.name.contains("state")) {
                fetch_evidence.push(format!("Fetch state file: {}", file.name));
            }
        }
        if !fetch_evidence.is_empty() {
            detected_commands.push("fetch-all-markets".to_string());
            evidence.insert("fetch-all-markets".to_string(), fetch_evidence);
            confidence_scores.push(0.9); // High confidence for chunk files
        }

        // Check for analyze command evidence
        let mut analyze_evidence = Vec::new();
        for file in files {
            if file.name.contains("filtered") || file.name.contains("analyzed") {
                analyze_evidence.push(format!("Filtered/analyzed file: {}", file.name));
            }
        }
        if !analyze_evidence.is_empty() {
            detected_commands.push("analyze".to_string());
            evidence.insert("analyze".to_string(), analyze_evidence);
            confidence_scores.push(0.8);
        }

        // Check for enrich command evidence
        let mut enrich_evidence = Vec::new();
        for file in files {
            if file.name.contains("enriched") {
                enrich_evidence.push(format!("Enriched file: {}", file.name));
            }
        }
        if !enrich_evidence.is_empty() {
            detected_commands.push("enrich".to_string());
            evidence.insert("enrich".to_string(), enrich_evidence);
            confidence_scores.push(0.8);
        }

        // Check for pipeline command evidence
        if dir_name.contains("pipeline") || files.iter().any(|f| f.name.contains("pipeline")) {
            let mut pipeline_evidence = vec![format!("Directory name suggests pipeline: {}", dir_name)];
            for file in files {
                if file.name.contains("pipeline") {
                    pipeline_evidence.push(format!("Pipeline-related file: {}", file.name));
                }
            }
            detected_commands.push("pipeline".to_string());
            evidence.insert("pipeline".to_string(), pipeline_evidence);
            confidence_scores.push(0.7);
        }

        // Calculate overall confidence
        let confidence = if confidence_scores.is_empty() {
            0.0
        } else {
            confidence_scores.iter().sum::<f32>() / confidence_scores.len() as f32
        };

        let detected_commands: Vec<super::DetectedCommand> = detected_commands.into_iter().map(|cmd| {
            super::DetectedCommand {
                command: cmd.clone(),
                args: Vec::new(),
                confidence: confidence as f64,
                evidence: evidence.get(&cmd).cloned().unwrap_or_default(),
            }
        }).collect();

        DatasetCommandInfo {
            primary_command: detected_commands.first().map(|cmd| cmd.command.clone()),
            detected_commands,
            evidence,
            confidence: confidence as f64,
            execution_context: None,
        }
    }
    
    /// Load legacy command metadata from a dataset directory
    fn load_legacy_command_metadata(&self, dataset_path: &Path) -> Result<serde_json::Value> {
        let metadata_path = dataset_path.join(".command_info.json");
        let contents = std::fs::read_to_string(&metadata_path)?;
        let metadata: serde_json::Value = serde_json::from_str(&contents)?;
        Ok(metadata)
    }

    /// Check if a dataset appears to be complete based on detected commands
    fn check_dataset_completeness_generic(&self, command_info: &DatasetCommandInfo, files: &[FileInfo], warnings: &mut Vec<String>) -> bool {
        if command_info.detected_commands.is_empty() {
            warnings.push("No recognizable CLI commands detected".to_string());
            return !files.is_empty(); // At least has some files
        }

        let mut all_complete = true;

        for command in &command_info.detected_commands {
            match command.command.as_str() {
                "fetch-all-markets" => {
                    let has_chunks = files.iter().any(|f| matches!(f.file_type, FileType::Json { subtype: super::JsonSubtype::MarketChunk }));
                    let has_state = files.iter().any(|f| matches!(f.file_type, FileType::Json { subtype: super::JsonSubtype::State }));
                    
                    if !has_chunks && !has_state {
                        warnings.push("fetch-all-markets: Missing market data files".to_string());
                        all_complete = false;
                    }
                }
                "analyze" => {
                    let has_filtered = files.iter().any(|f| f.name.contains("filtered") || f.name.contains("analyzed"));
                    
                    if !has_filtered {
                        warnings.push("analyze: Missing filtered/analyzed output files".to_string());
                        all_complete = false;
                    }
                }
                "enrich" => {
                    let has_enriched = files.iter().any(|f| f.name.contains("enriched"));
                    
                    if !has_enriched {
                        warnings.push("enrich: Missing enriched output files".to_string());
                        all_complete = false;
                    }
                }
                "pipeline" => {
                    // For pipelines, expect to see evidence of multiple commands
                    let sub_commands: Vec<_> = command_info.detected_commands.iter()
                        .filter(|cmd| cmd.command != "pipeline")
                        .collect();
                    
                    if sub_commands.is_empty() {
                        warnings.push("pipeline: No sub-commands detected in pipeline output".to_string());
                        all_complete = false;
                    }
                }
                _ => {
                    // Unknown command, can't validate
                    warnings.push(format!("Unknown command '{}' - cannot validate completeness", command.command));
                }
            }
        }

        all_complete
    }

    /// Get all discovered datasets
    pub fn get_datasets(&self) -> &[DatasetInfo] {
        &self.datasets
    }




    /// Delete a dataset
    pub fn delete_dataset(&mut self, dataset_name: &str) -> Result<()> {
        let dataset = self.datasets.iter()
            .find(|d| d.name == dataset_name)
            .ok_or_else(|| anyhow::anyhow!("Dataset not found: {}", dataset_name))?;

        info!("Deleting dataset: {}", dataset.name);
        
        // Remove the directory and all its contents
        fs::remove_dir_all(&dataset.path)
            .with_context(|| format!("Failed to delete dataset directory: {}", dataset.path.display()))?;

        // Remove from our list
        self.datasets.retain(|d| d.name != dataset_name);

        info!("Successfully deleted dataset: {}", dataset_name);
        Ok(())
    }

    /// Delete multiple datasets
    pub fn delete_datasets(&mut self, dataset_names: &[String]) -> Result<Vec<String>> {
        let mut deleted = Vec::new();
        let mut errors = Vec::new();

        for name in dataset_names {
            match self.delete_dataset(name) {
                Ok(()) => deleted.push(name.clone()),
                Err(e) => errors.push(format!("{}: {}", name, e)),
            }
        }

        if !errors.is_empty() {
            return Err(anyhow::anyhow!("Failed to delete some datasets: {}", errors.join(", ")));
        }

        Ok(deleted)
    }

    /// Get summary statistics
    pub fn get_summary(&self) -> DatasetSummary {
        let mut type_counts = HashMap::new();
        let mut total_size = 0u64;
        let mut total_files = 0usize;
        let mut today_count = 0usize;

        for dataset in &self.datasets {
            *type_counts.entry(dataset.dataset_type.clone()).or_insert(0) += 1;
            total_size += dataset.size_bytes;
            total_files += dataset.file_count;
            
            if dataset.is_today() {
                today_count += 1;
            }
        }

        DatasetSummary {
            total_datasets: self.datasets.len(),
            total_size_bytes: total_size,
            total_files,
            datasets_today: today_count,
            type_counts,
            last_scan: self.last_scan,
        }
    }
}

/// Summary statistics for datasets
#[derive(Debug, Clone)]
pub struct DatasetSummary {
    pub total_datasets: usize,
    pub total_size_bytes: u64,
    pub total_files: usize,
    pub datasets_today: usize,
    pub type_counts: HashMap<DatasetType, usize>,
    pub last_scan: Option<DateTime<Local>>,
}

impl DatasetSummary {
    /// Format total size in human-readable form
    pub fn formatted_total_size(&self) -> String {
        format_bytes(self.total_size_bytes)
    }
} 