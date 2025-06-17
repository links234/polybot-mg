//! Dataset management system for pipeline outputs and market data

use anyhow::{Result, Context};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;

pub mod manager;
pub mod selection;
pub mod tui;

pub use manager::*;
pub use selection::*;
pub use tui::*;

/// Comprehensive dataset information with enhanced metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetInfo {
    /// Dataset name/identifier
    pub name: String,
    /// Full path to the dataset directory
    pub path: PathBuf,
    /// Dataset type (inferred from name or contents)
    pub dataset_type: DatasetType,
    /// Information about CLI commands that produced this dataset
    pub command_info: DatasetCommandInfo,
    /// Total size in bytes
    pub size_bytes: u64,
    /// Number of files in the dataset
    pub file_count: usize,
    /// Creation time (UTC)
    pub created_at: Option<DateTime<Utc>>,
    /// Last modified time (UTC)
    pub modified_at: Option<DateTime<Utc>>,
    /// List of files in the dataset
    pub files: Vec<FileInfo>,
    /// Dataset health and completeness status
    pub health_status: DatasetHealthStatus,
    /// Any errors or warnings about the dataset
    pub warnings: Vec<DatasetWarning>,
    /// Dataset metrics and statistics
    pub metrics: DatasetMetrics,
}

/// Information about a file within a dataset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    /// File name
    pub name: String,
    /// Relative path from dataset root
    pub relative_path: PathBuf,
    /// File size in bytes
    pub size_bytes: u64,
    /// File type based on extension/content
    pub file_type: FileType,
    /// Last modified time (UTC)
    pub modified_at: Option<DateTime<Utc>>,
    /// File content hash (if computed)
    pub content_hash: Option<String>,
    /// File-specific metadata
    pub metadata: FileMetadata,
}

/// Dataset health and completeness status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DatasetHealthStatus {
    /// Dataset is complete and healthy
    Healthy,
    /// Dataset has minor issues but is usable
    Warning,
    /// Dataset has serious issues
    Corrupted,
    /// Dataset is incomplete/in progress
    Incomplete,
    /// Dataset is empty or missing critical files
    Empty,
}

/// Dataset warning information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetWarning {
    /// Warning category
    pub category: WarningCategory,
    /// Human-readable warning message
    pub message: String,
    /// Affected file or component
    pub affected_file: Option<String>,
    /// When the warning was detected
    pub detected_at: DateTime<Utc>,
    /// Severity level
    pub severity: WarningSeverity,
}

/// Warning categories
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WarningCategory {
    /// Missing expected files
    MissingFiles,
    /// Corrupted or invalid files
    CorruptedFiles,
    /// Inconsistent metadata
    InconsistentMetadata,
    /// Large file sizes that might indicate issues
    LargeFiles,
    /// Old datasets that might be stale
    StaleData,
    /// Permission or access issues
    AccessIssues,
}

/// Warning severity levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum WarningSeverity {
    /// Low severity, informational
    Info,
    /// Medium severity, should be addressed
    Warning,
    /// High severity, likely to cause issues
    Error,
    /// Critical severity, immediate attention required
    Critical,
}

/// Dataset metrics and statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DatasetMetrics {
    /// Number of records/entries in the dataset
    pub record_count: Option<usize>,
    /// Data quality score (0.0 to 1.0)
    pub quality_score: Option<f64>,
    /// Processing time to create this dataset
    pub processing_time_seconds: Option<f64>,
    /// Memory usage during creation
    pub memory_usage_mb: Option<f64>,
    /// Compression ratio if applicable
    pub compression_ratio: Option<f64>,
    /// Dataset version/revision
    pub version: Option<String>,
}

/// File-specific metadata
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileMetadata {
    /// Number of lines for text files
    pub line_count: Option<usize>,
    /// Number of JSON objects for JSON files
    pub json_object_count: Option<usize>,
    /// Schema information for structured files
    pub schema_info: Option<SchemaInfo>,
    /// Encoding information
    pub encoding: Option<String>,
    /// Compression type if applicable
    pub compression: Option<String>,
}

/// Schema information for structured data files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaInfo {
    /// Field names and types
    pub fields: HashMap<String, String>,
    /// Schema version
    pub version: Option<String>,
    /// Whether schema is validated
    pub is_validated: bool,
}

/// Enhanced dataset type with more granular categorization
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DatasetType {
    /// Pipeline output (contains the pipeline name and version)
    Pipeline { name: String, version: Option<String> },
    /// Output from fetch-all-markets command
    MarketData { source: DataSource },
    /// Output from analyze command (filtered markets)
    AnalyzedMarkets { source_dataset: String, filter_count: Option<usize> },
    /// Output from enrich command (enriched with real-time data)
    EnrichedMarkets { source_dataset: String, enrichment_types: Vec<EnrichmentType> },
    /// User-created token selections
    TokenSelection { name: String, token_count: usize },
    /// Mixed dataset with multiple command outputs
    Mixed { components: Vec<String> },
    /// Unknown/unidentified dataset
    Unknown,
}

/// Data source for market data
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DataSource {
    /// Polymarket CLOB API
    ClobApi,
    /// Polymarket Gamma API
    GammaApi,
    /// Combined sources
    Mixed,
    /// Unknown source
    Unknown,
}

/// Types of enrichment applied to market data
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EnrichmentType {
    /// Real-time orderbook data
    Orderbook,
    /// Liquidity metrics
    Liquidity,
    /// Volume and trading data
    Volume,
    /// Price history
    PriceHistory,
    /// Market sentiment analysis
    Sentiment,
    /// Custom enrichment
    Custom(String),
}

/// Enhanced command information with execution context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetCommandInfo {
    /// Primary command used to create this dataset
    pub primary_command: Option<String>,
    /// All CLI commands that appear to have been used
    pub detected_commands: Vec<DetectedCommand>,
    /// Evidence for command detection
    pub evidence: HashMap<String, Vec<String>>,
    /// Detection confidence level (0.0 to 1.0)
    pub confidence: f64,
    /// Command execution environment
    pub execution_context: Option<ExecutionContext>,
}

/// Information about a detected command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedCommand {
    /// Command name
    pub command: String,
    /// Detected arguments
    pub args: Vec<String>,
    /// Confidence for this specific command (0.0 to 1.0)
    pub confidence: f64,
    /// Evidence that supports this command detection
    pub evidence: Vec<String>,
}

/// Command execution context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// Tool version used
    pub version: String,
    /// Host/machine identifier
    pub host: Option<String>,
    /// User who executed the command
    pub user: Option<String>,
    /// Working directory when command was executed
    pub working_directory: Option<String>,
    /// Environment variables (filtered for security)
    pub environment: HashMap<String, String>,
    /// Command execution time
    pub execution_time: DateTime<Utc>,
    /// Command duration in seconds
    pub duration_seconds: Option<f64>,
}

/// Enhanced file type classification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileType {
    /// JSON data file with subtype
    Json { subtype: JsonSubtype },
    /// YAML configuration or metadata file
    Yaml { purpose: YamlPurpose },
    /// Log file with level
    Log { level: LogLevel },
    /// Binary data file
    Binary { format: BinaryFormat },
    /// Text file
    Text { format: TextFormat },
    /// Compressed archive
    Archive { format: ArchiveFormat },
    /// Unknown file type
    Unknown,
}

/// JSON file subtypes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JsonSubtype {
    /// Market data chunk
    MarketChunk,
    /// State/progress file
    State,
    /// Configuration file
    Config,
    /// Metadata file
    Metadata,
    /// Analysis results
    AnalysisResults,
    /// Generic JSON data
    Data,
}

/// YAML file purposes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum YamlPurpose {
    /// Dataset metadata
    DatasetMetadata,
    /// Configuration
    Configuration,
    /// Pipeline definition
    Pipeline,
    /// Schema definition
    Schema,
}

/// Log levels
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    /// Debug level logs
    Debug,
    /// Info level logs
    Info,
    /// Warning level logs
    Warning,
    /// Error level logs
    Error,
    /// Mixed or unknown level
    Mixed,
}

/// Binary file formats
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryFormat {
    /// Parquet data file
    Parquet,
    /// SQLite database
    SQLite,
    /// Protocol Buffers
    Protobuf,
    /// MessagePack
    MessagePack,
    /// Unknown binary format
    Unknown,
}

/// Text file formats
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextFormat {
    /// CSV file
    Csv,
    /// TSV file
    Tsv,
    /// Plain text
    Plain,
    /// Markdown
    Markdown,
}

/// Archive formats
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArchiveFormat {
    /// ZIP archive
    Zip,
    /// Gzip compressed
    Gzip,
    /// Tar archive
    Tar,
    /// 7zip archive
    SevenZip,
}

impl DatasetType {
    /// Get a display name for the dataset type
    pub fn display_name(&self) -> String {
        match self {
            DatasetType::Pipeline { name, version } => {
                if let Some(v) = version {
                    format!("Pipeline: {} (v{})", name, v)
                } else {
                    format!("Pipeline: {}", name)
                }
            },
            DatasetType::MarketData { source } => {
                format!("Market Data ({})", source.display_name())
            },
            DatasetType::AnalyzedMarkets { source_dataset, filter_count } => {
                if let Some(count) = filter_count {
                    format!("Analyzed Markets from {} ({} filters)", source_dataset, count)
                } else {
                    format!("Analyzed Markets from {}", source_dataset)
                }
            },
            DatasetType::EnrichedMarkets { source_dataset, enrichment_types } => {
                if enrichment_types.is_empty() {
                    format!("Enriched Markets from {}", source_dataset)
                } else {
                    let types = enrichment_types.iter()
                        .map(|t| t.display_name())
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("Enriched Markets from {} ({})", source_dataset, types)
                }
            },
            DatasetType::TokenSelection { name, token_count } => {
                format!("Token Selection: {} ({} tokens)", name, token_count)
            },
            DatasetType::Mixed { components } => {
                if components.is_empty() {
                    "Mixed Output".to_string()
                } else {
                    format!("Mixed ({})", components.join(", "))
                }
            },
            DatasetType::Unknown => "Unknown".to_string(),
        }
    }

    /// Get an emoji icon for the dataset type
    pub fn icon(&self) -> &'static str {
        match self {
            DatasetType::Pipeline { .. } => "🔧",
            DatasetType::MarketData { .. } => "📄",
            DatasetType::AnalyzedMarkets { .. } => "📊",
            DatasetType::EnrichedMarkets { .. } => "✨",
            DatasetType::TokenSelection { .. } => "⭐",
            DatasetType::Mixed { .. } => "🔄",
            DatasetType::Unknown => "❓",
        }
    }

    /// Infer dataset type from directory analysis with enhanced detection
    pub fn from_dir_analysis(name: &str, files: &[FileInfo], metadata: Option<&DatasetMetadata>) -> Self {
        // If we have metadata, use it
        if let Some(meta) = metadata {
            return Self::from_metadata(meta);
        }
        
        // Detect patterns in files
        let mut has_market_chunks = false;
        let mut has_state_files = false;
        let mut has_analysis_results = false;
        let mut has_enrichment_data = false;
        let source_dataset = None;
        
        for file in files {
            match &file.file_type {
                FileType::Json { subtype } => {
                    match subtype {
                        JsonSubtype::MarketChunk => has_market_chunks = true,
                        JsonSubtype::State => has_state_files = true,
                        JsonSubtype::AnalysisResults => has_analysis_results = true,
                        _ => {}
                    }
                },
                _ => {}
            }
            
            if file.name.contains("enriched") {
                has_enrichment_data = true;
            }
            
            // Try to extract source dataset from config files
            if file.name == "dataset.yaml" || file.name == "analysis_config.yaml" {
                // Could parse config to get source dataset
            }
        }
        
        // Determine dataset type based on evidence
        if name.contains("pipeline") {
            let pipeline_name = extract_pipeline_name(name);
            return DatasetType::Pipeline { 
                name: pipeline_name, 
                version: None 
            };
        }
        
        if has_enrichment_data {
            return DatasetType::EnrichedMarkets {
                source_dataset: source_dataset.unwrap_or_else(|| "unknown".to_string()),
                enrichment_types: detect_enrichment_types(files),
            };
        }
        
        if has_analysis_results {
            return DatasetType::AnalyzedMarkets {
                source_dataset: source_dataset.unwrap_or_else(|| "unknown".to_string()),
                filter_count: None,
            };
        }
        
        if has_market_chunks || has_state_files {
            return DatasetType::MarketData {
                source: detect_data_source(files),
            };
        }
        
        DatasetType::Unknown
    }
    
    /// Create dataset type from metadata
    fn from_metadata(metadata: &DatasetMetadata) -> Self {
        match metadata.dataset_type.as_str() {
            "MarketData" => DatasetType::MarketData { source: DataSource::Unknown },
            "AnalyzedMarkets" => DatasetType::AnalyzedMarkets {
                source_dataset: metadata.additional_info
                    .get("source_dataset")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                filter_count: metadata.additional_info
                    .get("filter_count")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize),
            },
            "EnrichedMarkets" => DatasetType::EnrichedMarkets {
                source_dataset: metadata.additional_info
                    .get("source_dataset")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                enrichment_types: Vec::new(), // Could be extracted from metadata
            },
            "Pipeline" => DatasetType::Pipeline {
                name: metadata.additional_info
                    .get("pipeline_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                version: metadata.additional_info
                    .get("pipeline_version")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            },
            _ => DatasetType::Unknown,
        }
    }
}

impl DataSource {
    pub fn display_name(&self) -> &'static str {
        match self {
            DataSource::ClobApi => "CLOB API",
            DataSource::GammaApi => "Gamma API", 
            DataSource::Mixed => "Mixed Sources",
            DataSource::Unknown => "Unknown",
        }
    }
}

impl EnrichmentType {
    pub fn display_name(&self) -> String {
        match self {
            EnrichmentType::Orderbook => "Orderbook".to_string(),
            EnrichmentType::Liquidity => "Liquidity".to_string(),
            EnrichmentType::Volume => "Volume".to_string(),
            EnrichmentType::PriceHistory => "Price History".to_string(),
            EnrichmentType::Sentiment => "Sentiment".to_string(),
            EnrichmentType::Custom(name) => name.clone(),
        }
    }
}

impl FileType {
    /// Enhanced file type detection from filename and content
    pub fn from_filename_and_content(name: &str, _content_sample: Option<&[u8]>) -> Self {
        let extension = Path::new(name)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");
        
        match extension.to_lowercase().as_str() {
            "json" => {
                let subtype = if name.contains("chunk") {
                    JsonSubtype::MarketChunk
                } else if name.contains("state") || name.contains("progress") {
                    JsonSubtype::State
                } else if name.contains("config") {
                    JsonSubtype::Config
                } else if name.contains("metadata") {
                    JsonSubtype::Metadata
                } else if name.contains("analysis") || name.contains("results") {
                    JsonSubtype::AnalysisResults
                } else {
                    JsonSubtype::Data
                };
                FileType::Json { subtype }
            },
            "yaml" | "yml" => {
                let purpose = if name == "dataset.yaml" {
                    YamlPurpose::DatasetMetadata
                } else if name.contains("config") {
                    YamlPurpose::Configuration
                } else if name.contains("pipeline") {
                    YamlPurpose::Pipeline
                } else if name.contains("schema") {
                    YamlPurpose::Schema
                } else {
                    YamlPurpose::Configuration
                };
                FileType::Yaml { purpose }
            },
            "log" => FileType::Log { level: LogLevel::Mixed },
            "parquet" => FileType::Binary { format: BinaryFormat::Parquet },
            "db" | "sqlite" | "sqlite3" => FileType::Binary { format: BinaryFormat::SQLite },
            "csv" => FileType::Text { format: TextFormat::Csv },
            "tsv" => FileType::Text { format: TextFormat::Tsv },
            "md" => FileType::Text { format: TextFormat::Markdown },
            "txt" => FileType::Text { format: TextFormat::Plain },
            "zip" => FileType::Archive { format: ArchiveFormat::Zip },
            "gz" => FileType::Archive { format: ArchiveFormat::Gzip },
            "tar" => FileType::Archive { format: ArchiveFormat::Tar },
            "7z" => FileType::Archive { format: ArchiveFormat::SevenZip },
            _ => FileType::Unknown,
        }
    }


    /// Get an icon for the file type
    pub fn icon(&self) -> &'static str {
        match self {
            FileType::Json { .. } => "📄",
            FileType::Yaml { .. } => "⚙️",
            FileType::Log { .. } => "📝",
            FileType::Binary { .. } => "💾",
            FileType::Text { .. } => "📄",
            FileType::Archive { .. } => "🗜️",
            FileType::Unknown => "❓",
        }
    }
}







impl DatasetInfo {
    /// Format the dataset size in human-readable form
    pub fn formatted_size(&self) -> String {
        format_bytes(self.size_bytes)
    }

    /// Get the dataset age as a human-readable string
    pub fn age(&self) -> String {
        if let Some(created_at) = &self.created_at {
            let now = Utc::now();
            let duration = now.signed_duration_since(*created_at);
            
            if duration.num_days() > 0 {
                format!("{} days ago", duration.num_days())
            } else if duration.num_hours() > 0 {
                format!("{} hours ago", duration.num_hours())
            } else if duration.num_minutes() > 0 {
                format!("{} minutes ago", duration.num_minutes())
            } else {
                "Just now".to_string()
            }
        } else {
            "Unknown".to_string()
        }
    }

    /// Check if this dataset is from today
    pub fn is_today(&self) -> bool {
        if let Some(created_at) = &self.created_at {
            let now = Utc::now();
            created_at.date_naive() == now.date_naive()
        } else {
            false
        }
    }

    /// Get a status indicator for the dataset
    pub fn status_icon(&self) -> &'static str {
        match self.health_status {
            DatasetHealthStatus::Healthy => "✅",
            DatasetHealthStatus::Warning => "⚠️",
            DatasetHealthStatus::Corrupted => "❌", 
            DatasetHealthStatus::Incomplete => "🔄",
            DatasetHealthStatus::Empty => "📭",
        }
    }
    
}

/// Format bytes in human-readable form with enhanced precision
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB"];
    const THRESHOLD: u64 = 1024;

    if bytes == 0 {
        return "0 B".to_string();
    }

    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= THRESHOLD as f64 && unit_index < UNITS.len() - 1 {
        size /= THRESHOLD as f64;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else if size >= 100.0 {
        format!("{:.0} {}", size, UNITS[unit_index])
    } else if size >= 10.0 {
        format!("{:.1} {}", size, UNITS[unit_index])
    } else {
        format!("{:.2} {}", size, UNITS[unit_index])
    }
}

/// Enhanced command metadata creation with full execution context
pub fn save_command_metadata(
    dataset_path: &Path,
    command: &str,
    args: &[String],
    additional_info: Option<HashMap<String, serde_json::Value>>,
) -> Result<()> {
    let metadata = DatasetMetadata {
        name: dataset_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string(),
        description: format!("Dataset generated by {} command", command),
        command_info: CommandInfo {
            command: command.to_string(),
            args: args.to_vec(),
            executed_at: Utc::now(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        dataset_type: infer_dataset_type_from_command(command),
        created_at: Utc::now(),
        additional_info: additional_info.unwrap_or_default(),
    };
    
    let yaml_content = serde_yaml::to_string(&metadata)
        .context("Failed to serialize dataset metadata to YAML")?;
    let metadata_path = dataset_path.join("dataset.yaml");
    fs::write(&metadata_path, yaml_content)
        .context("Failed to write dataset metadata file")?;
    
    Ok(())
}

/// Comprehensive dataset metadata structure with enhanced typing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetMetadata {
    /// Dataset name
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Information about the command that generated this dataset
    pub command_info: CommandInfo,
    /// Type of dataset
    pub dataset_type: String,
    /// When the dataset was created (UTC)
    pub created_at: DateTime<Utc>,
    /// Additional command-specific information
    #[serde(default)]
    pub additional_info: HashMap<String, serde_json::Value>,
}

/// Enhanced command execution information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandInfo {
    /// Command name (e.g., "fetch-all-markets", "analyze", "enrich")
    pub command: String,
    /// Command arguments
    pub args: Vec<String>,
    /// When the command was executed (UTC)
    pub executed_at: DateTime<Utc>,
    /// Version of the tool that generated this
    pub version: String,
}

/// Load dataset metadata from a YAML file with enhanced error handling
pub fn load_dataset_metadata(dataset_path: &Path) -> Result<DatasetMetadata> {
    let metadata_path = dataset_path.join("dataset.yaml");
    let content = fs::read_to_string(&metadata_path)
        .context(format!("Failed to read metadata file: {}", metadata_path.display()))?;
    let metadata: DatasetMetadata = serde_yaml::from_str(&content)
        .context("Failed to parse dataset metadata YAML")?;
    Ok(metadata)
}

/// Helper functions for dataset type inference
fn extract_pipeline_name(dir_name: &str) -> String {
    if let Some(name) = dir_name.strip_prefix("pipeline_") {
        name.split('_').next().unwrap_or("unknown").to_string()
    } else {
        dir_name.to_string()
    }
}

fn detect_enrichment_types(files: &[FileInfo]) -> Vec<EnrichmentType> {
    let mut types = Vec::new();
    
    for file in files {
        if file.name.contains("orderbook") {
            types.push(EnrichmentType::Orderbook);
        }
        if file.name.contains("liquidity") {
            types.push(EnrichmentType::Liquidity);
        }
        if file.name.contains("volume") {
            types.push(EnrichmentType::Volume);
        }
    }
    
    types.sort();
    types.dedup();
    types
}

fn detect_data_source(files: &[FileInfo]) -> DataSource {
    let mut has_clob_indicators = false;
    let mut has_gamma_indicators = false;
    
    for file in files {
        if file.name.contains("clob") {
            has_clob_indicators = true;
        }
        if file.name.contains("gamma") {
            has_gamma_indicators = true;
        }
    }
    
    match (has_clob_indicators, has_gamma_indicators) {
        (true, true) => DataSource::Mixed,
        (true, false) => DataSource::ClobApi,
        (false, true) => DataSource::GammaApi,
        (false, false) => DataSource::Unknown,
    }
}

fn infer_dataset_type_from_command(command: &str) -> String {
    match command {
        "fetch-all-markets" => "MarketData".to_string(),
        "analyze" => "AnalyzedMarkets".to_string(),
        "enrich" => "EnrichedMarkets".to_string(),
        "pipeline" => "Pipeline".to_string(),
        _ => "Unknown".to_string(),
    }
} 