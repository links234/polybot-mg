# Datasets Module

The datasets module provides comprehensive data management capabilities for pipeline outputs, market data, and analysis results. It implements intelligent dataset discovery, metadata management, health monitoring, and lifecycle operations with strong typing and detailed classification systems.

## Core Purpose and Responsibilities

The datasets module serves as the central data management system for:
- **Dataset Discovery**: Intelligent scanning and classification of data directories
- **Metadata Management**: Comprehensive metadata tracking with command provenance
- **Health Monitoring**: Dataset validation, completeness checking, and health scoring
- **Lifecycle Operations**: Creation, monitoring, analysis, and cleanup of datasets
- **Integration Bridge**: Connecting CLI operations with persistent data storage

## Architecture Overview

```
src/datasets/
├── mod.rs          # Core types and dataset classification system
├── manager.rs      # Dataset scanning, discovery, and management operations
└── tui.rs          # Terminal UI for dataset browsing and management
```

## Core Data Structures

### Comprehensive Dataset Information (`mod.rs`)

```rust
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
    /// Creation and modification timestamps
    pub created_at: Option<DateTime<Utc>>,
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
```

### Enhanced Dataset Classification

```rust
/// Enhanced dataset type with granular categorization
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
    /// Mixed dataset with multiple command outputs
    Mixed { components: Vec<String> },
    /// Unknown/unidentified dataset
    Unknown,
}

/// Data source classification
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DataSource {
    ClobApi,    // Polymarket CLOB API
    GammaApi,   // Polymarket Gamma API
    Mixed,      // Combined sources
    Unknown,
}

/// Types of enrichment applied to market data
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EnrichmentType {
    Orderbook,      // Real-time orderbook data
    Liquidity,      // Liquidity metrics
    Volume,         // Volume and trading data
    PriceHistory,   // Price history
    Sentiment,      // Market sentiment analysis
    Custom(String), // Custom enrichment
}
```

### File-Level Analysis

```rust
/// Information about a file within a dataset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub name: String,
    pub relative_path: PathBuf,
    pub size_bytes: u64,
    pub file_type: FileType,
    pub modified_at: Option<DateTime<Utc>>,
    pub content_hash: Option<String>,
    pub metadata: FileMetadata,
}

/// Enhanced file type classification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileType {
    Json { subtype: JsonSubtype },
    Yaml { purpose: YamlPurpose },
    Log { level: LogLevel },
    Binary { format: BinaryFormat },
    Text { format: TextFormat },
    Archive { format: ArchiveFormat },
    Unknown,
}
```

## Dataset Manager (`manager.rs`)

### Intelligent Discovery System

```rust
/// Manager for dataset operations
pub struct DatasetManager {
    config: DatasetManagerConfig,
    datasets: Vec<DatasetInfo>,
    last_scan: Option<DateTime<Local>>,
}

impl DatasetManager {
    /// Scan for datasets in the configured directories
    pub fn scan_datasets(&mut self) -> Result<()> {
        info!("Scanning for datasets...");
        self.datasets.clear();

        // Scan multiple directories
        let base_dir = self.config.base_dir.clone();
        let scan_dirs = self.config.scan_dirs.clone();

        self.scan_directory(&base_dir, 0)?;
        for scan_dir in &scan_dirs {
            if scan_dir.exists() {
                self.scan_directory(scan_dir, 0)?;
            }
        }

        // Sort by creation time (newest first)
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
}
```

### Multi-Criteria Dataset Detection

The manager uses sophisticated heuristics to identify datasets:

```rust
/// Check if a directory looks like a dataset
fn is_dataset_directory(&self, path: &Path, name: &str) -> bool {
    // Skip hidden directories and non-dataset directories
    if name.starts_with('.') || name.starts_with("target") || name == "src" {
        return false;
    }

    // Priority 1: Check for dataset.yaml metadata file
    if path.join("dataset.yaml").exists() {
        return true;
    }

    // Priority 2: Check for legacy .command_info.json file
    if path.join(".command_info.json").exists() {
        return true;
    }

    // Priority 3: Check for pipeline output patterns
    if name.contains("pipeline") || name.contains("analysis") || name.contains("fetch") {
        return self.has_data_files(path);
    }

    // Priority 4: Check for timestamp patterns
    if self.has_timestamp_pattern(name) {
        return self.has_data_files(path);
    }

    // Priority 5: Check for dataset structure
    self.has_dataset_structure(path)
}
```

### Command Detection System

```rust
/// Analyze which CLI commands likely produced this dataset
fn analyze_commands(&self, dir_path: &Path, dir_name: &str, files: &[FileInfo]) -> DatasetCommandInfo {
    // Priority 1: Check for YAML metadata file
    if let Ok(metadata) = load_dataset_metadata(dir_path) {
        return DatasetCommandInfo {
            primary_command: Some(metadata.command_info.command),
            detected_commands: vec![DetectedCommand {
                command: metadata.command_info.command,
                args: metadata.command_info.args,
                confidence: 1.0,
                evidence: vec!["Found dataset.yaml metadata file".to_string()],
            }],
            confidence: 1.0,
            execution_context: Some(ExecutionContext {
                version: metadata.command_info.version,
                execution_time: metadata.command_info.executed_at,
                // ... additional context
            }),
        };
    }

    // Priority 2: Check for legacy JSON metadata
    if let Ok(metadata) = self.load_legacy_command_metadata(dir_path) {
        // Parse legacy metadata...
    }

    // Priority 3: Heuristic detection based on file patterns
    self.analyze_commands_heuristic(dir_name, files)
}
```

## Health Monitoring System

### Dataset Health Assessment

```rust
/// Dataset health and completeness status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DatasetHealthStatus {
    Healthy,      // Dataset is complete and healthy
    Warning,      // Dataset has minor issues but is usable
    Corrupted,    // Dataset has serious issues
    Incomplete,   // Dataset is incomplete/in progress
    Empty,        // Dataset is empty or missing critical files
}

/// Dataset warning information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetWarning {
    pub category: WarningCategory,
    pub message: String,
    pub affected_file: Option<String>,
    pub detected_at: DateTime<Utc>,
    pub severity: WarningSeverity,
}

/// Warning categories
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WarningCategory {
    MissingFiles,           // Missing expected files
    CorruptedFiles,         // Corrupted or invalid files
    InconsistentMetadata,   // Inconsistent metadata
    LargeFiles,             // Large file sizes that might indicate issues
    StaleData,              // Old datasets that might be stale
    AccessIssues,           // Permission or access issues
}
```

### Health Scoring Algorithm

```rust
impl DatasetInfo {
    /// Calculate a health score for the dataset
    pub fn health_score(&self) -> f64 {
        let mut score = 1.0f64;
        
        // Deduct points for warnings
        for warning in &self.warnings {
            match warning.severity {
                WarningSeverity::Info => score -= 0.05,
                WarningSeverity::Warning => score -= 0.15,
                WarningSeverity::Error => score -= 0.3,
                WarningSeverity::Critical => score -= 0.5,
            }
        }
        
        // Factor in dataset completeness
        match self.health_status {
            DatasetHealthStatus::Healthy => {},
            DatasetHealthStatus::Warning => score -= 0.1,
            DatasetHealthStatus::Corrupted => score -= 0.6,
            DatasetHealthStatus::Incomplete => score -= 0.3,
            DatasetHealthStatus::Empty => score = 0.0,
        }
        
        score.max(0.0f64)
    }
}
```

## Usage Examples

### Basic Dataset Scanning

```rust
use crate::datasets::{DatasetManager, DatasetManagerConfig};

// Create manager with default configuration
let mut manager = DatasetManager::default();

// Scan for datasets
manager.scan_datasets()?;

// Display discovered datasets
for dataset in manager.get_datasets() {
    println!("{} {} - {} ({})", 
        dataset.dataset_type.icon(),
        dataset.name,
        dataset.dataset_type.display_name(),
        dataset.formatted_size()
    );
    
    if dataset.health_status != DatasetHealthStatus::Healthy {
        println!("  {} Health: {:?}", dataset.status_icon(), dataset.health_status);
    }
}
```

### Dataset Analysis and Filtering

```rust
// Get datasets by type
let market_data = manager.get_datasets_by_type(DatasetType::MarketData { 
    source: DataSource::ClobApi 
});

println!("Found {} CLOB market datasets", market_data.len());

// Get summary statistics
let summary = manager.get_summary();
println!("Total: {} datasets, {} files, {}", 
    summary.total_datasets,
    summary.total_files,
    summary.formatted_total_size()
);

// Find datasets created today
let today_datasets: Vec<_> = manager.get_datasets()
    .iter()
    .filter(|d| d.is_today())
    .collect();

println!("Created today: {} datasets", today_datasets.len());
```

### Health Monitoring

```rust
// Check dataset health
for dataset in manager.get_datasets() {
    let health_score = dataset.health_score();
    
    if health_score < 0.8 {
        println!("⚠️  Dataset {} has poor health score: {:.2}", 
            dataset.name, health_score);
        
        for warning in &dataset.warnings {
            println!("  - {}: {}", warning.category, warning.message);
        }
    }
}

// Filter unhealthy datasets
let unhealthy_datasets: Vec<_> = manager.get_datasets()
    .iter()
    .filter(|d| d.health_status != DatasetHealthStatus::Healthy)
    .collect();
```

### Dataset Cleanup

```rust
// Delete specific datasets
let datasets_to_delete = vec![
    "old_analysis_20231201".to_string(),
    "incomplete_fetch_20231202".to_string(),
];

let deleted = manager.delete_datasets(&datasets_to_delete)?;
println!("Deleted {} datasets", deleted.len());

// Delete by criteria (example function)
fn cleanup_old_datasets(manager: &mut DatasetManager) -> Result<usize> {
    let cutoff = Utc::now() - chrono::Duration::days(30);
    
    let old_datasets: Vec<String> = manager.get_datasets()
        .iter()
        .filter(|d| {
            d.created_at.map(|ct| ct < cutoff).unwrap_or(false)
        })
        .map(|d| d.name.clone())
        .collect();
    
    let deleted = manager.delete_datasets(&old_datasets)?;
    Ok(deleted.len())
}
```

## Integration Patterns

### With CLI Commands

The datasets module integrates with CLI for management operations:

```rust
// In src/cli/commands/datasets.rs
use crate::datasets::{DatasetManager, DatasetManagerConfig};

#[derive(Subcommand)]
pub enum DatasetAction {
    /// List discovered datasets
    List {
        #[arg(long)]
        output_format: Option<String>,
        #[arg(long)]  
        save_to: Option<String>,
    },
    /// Show detailed dataset information
    Info {
        /// Dataset name or pattern
        name: String,
    },
    /// Delete datasets
    Delete {
        /// Dataset names to delete
        names: Vec<String>,
        #[arg(long)]
        confirm: bool,
    },
    /// Run dataset health check
    Health,
}
```

### With Pipeline System

Datasets are created and tracked by pipeline operations:

```rust
// Save pipeline metadata when creating datasets
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
    
    let yaml_content = serde_yaml::to_string(&metadata)?;
    let metadata_path = dataset_path.join("dataset.yaml");
    fs::write(&metadata_path, yaml_content)?;
    
    Ok(())
}
```

### With TUI System

The datasets module provides TUI components for interactive browsing:

```rust
// In src/datasets/tui.rs
use ratatui::{prelude::*, widgets::*};

pub fn render_dataset_list(
    frame: &mut Frame, 
    datasets: &[DatasetInfo], 
    selected: usize
) {
    let items: Vec<ListItem> = datasets.iter()
        .enumerate()
        .map(|(i, dataset)| {
            let style = if i == selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };
            
            let content = format!("{} {} - {} ({})",
                dataset.dataset_type.icon(),
                dataset.name,
                dataset.dataset_type.display_name(),
                dataset.formatted_size()
            );
            
            ListItem::new(content).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default()
            .borders(Borders::ALL)
            .title(format!("Datasets ({} found)", datasets.len())));
    
    frame.render_widget(list, frame.size());
}
```

## Performance Considerations

### Efficient Scanning

```rust
/// Configuration for dataset scanning
#[derive(Debug, Clone)]
pub struct DatasetManagerConfig {
    pub base_dir: PathBuf,
    pub scan_dirs: Vec<PathBuf>,
    pub recursive: bool,
    pub max_depth: usize,
}

impl Default for DatasetManagerConfig {
    fn default() -> Self {
        Self {
            base_dir: PathBuf::from(DEFAULT_DATASETS_DIR),
            scan_dirs: vec![
                PathBuf::from(DEFAULT_DATASETS_DIR),
                PathBuf::from(DEFAULT_RUNS_DIR),
                // ... additional common directories
            ],
            recursive: true,
            max_depth: 5, // Prevent excessive depth scanning
        }
    }
}
```

### Memory Optimization

- **Lazy Loading**: File content is analyzed on-demand
- **Streaming**: Large datasets are processed incrementally
- **Caching**: Scan results are cached to avoid repeated filesystem operations
- **Batch Operations**: Multiple file operations are batched for efficiency

### Storage Efficiency

```rust
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

    match unit_index {
        0 => format!("{} {}", bytes, UNITS[unit_index]),
        _ if size >= 100.0 => format!("{:.0} {}", size, UNITS[unit_index]),
        _ if size >= 10.0 => format!("{:.1} {}", size, UNITS[unit_index]),
        _ => format!("{:.2} {}", size, UNITS[unit_index]),
    }
}
```

## Strong Typing and Error Handling

Following CLAUDE.md requirements:

### No Tuples in Public APIs

```rust
// Strong typing instead of tuples
pub struct DatasetSummary {
    pub total_datasets: usize,
    pub total_size_bytes: u64,
    pub total_files: usize,
    pub datasets_today: usize,
    pub type_counts: HashMap<DatasetType, usize>,
    pub last_scan: Option<DateTime<Local>>,
}

// Instead of returning (usize, u64, usize)
impl DatasetManager {
    pub fn get_summary(&self) -> DatasetSummary { /* ... */ }
}
```

### Comprehensive Error Context

```rust
// Rich error messages with actionable suggestions
let dataset = self.datasets.iter()
    .find(|d| d.name == dataset_name)
    .ok_or_else(|| anyhow::anyhow!(
        "❌ Dataset not found: '{}'\n\
         💡 Available datasets: {}\n\
         💡 Try running 'datasets list' to see all available datasets",
        dataset_name,
        self.datasets.iter().map(|d| &d.name).collect::<Vec<_>>().join(", ")
    ))?;
```

### Type-Safe Classification

The module uses strongly-typed enums for all classification:
- `DatasetType` for dataset categorization
- `FileType` with specific subtypes for file classification
- `DataSource` for provenance tracking
- `HealthStatus` for dataset condition assessment

The datasets module provides a comprehensive foundation for data lifecycle management, ensuring that all data produced by the Polybot system is properly tracked, classified, and maintained with detailed provenance and health monitoring.