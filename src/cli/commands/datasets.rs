//! Dataset management command for listing, deleting, and managing pipeline outputs

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use tracing::{error, info, warn};

use crate::data_paths::{DataPaths, DATASETS_DIR, DEFAULT_DATASETS_DIR};
use crate::datasets::{
    format_bytes, DatasetManager, DatasetManagerConfig, DatasetTui, DatasetType,
};

#[derive(Args, Clone)]
pub struct DatasetsArgs {
    /// Base directory to scan for datasets
    #[arg(long, default_value = DEFAULT_DATASETS_DIR)]
    pub base_dir: String,

    /// Additional directories to scan (comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub scan_dirs: Vec<String>,

    /// Maximum depth for recursive scanning
    #[arg(long, default_value = "5")]
    pub max_depth: usize,

    /// List all datasets (default if no other action specified)
    #[arg(long)]
    pub list: bool,

    /// Show summary statistics
    #[arg(long)]
    pub summary: bool,

    /// Launch interactive TUI interface
    #[arg(long)]
    pub interactive: bool,

    /// Filter by dataset type
    #[arg(long)]
    pub filter_type: Option<String>,

    /// Delete specified datasets (comma-separated names)
    #[arg(long, value_delimiter = ',')]
    pub delete: Vec<String>,

    /// Force deletion without confirmation
    #[arg(long)]
    pub force: bool,

    /// Show detailed information for each dataset
    #[arg(long)]
    pub details: bool,
}

pub struct DatasetsCommand {
    args: DatasetsArgs,
}

impl DatasetsCommand {
    pub fn new(args: DatasetsArgs) -> Self {
        Self { args }
    }

    pub async fn execute(&self, _host: &str, _data_paths: DataPaths) -> Result<()> {
        let config = self.create_config()?;

        // Launch interactive TUI if requested or no specific action provided
        if self.args.interactive
            || (!self.args.list
                && !self.args.summary
                && self.args.delete.is_empty()
                && self.args.filter_type.is_none())
        {
            return self.launch_interactive_tui(config).await;
        }

        // Create dataset manager and scan
        let mut manager = DatasetManager::new(config);
        info!("Scanning for datasets...");
        manager.scan_datasets()?;

        // Handle deletion first
        if !self.args.delete.is_empty() {
            return self.handle_delete_datasets(&mut manager).await;
        }

        // Handle summary
        if self.args.summary {
            return self.show_summary(&manager).await;
        }

        // Default to listing datasets
        self.list_datasets(&manager).await
    }

    /// Create dataset manager configuration from CLI arguments
    fn create_config(&self) -> Result<DatasetManagerConfig> {
        let mut scan_dirs = vec![
            PathBuf::from(DEFAULT_DATASETS_DIR),
            PathBuf::from(DATASETS_DIR),
            PathBuf::from("datasets"),
            PathBuf::from("outputs"),
            PathBuf::from("results"),
            PathBuf::from("pipelines"),
        ];

        // Add additional scan directories
        for dir in &self.args.scan_dirs {
            scan_dirs.push(PathBuf::from(dir));
        }

        Ok(DatasetManagerConfig {
            base_dir: PathBuf::from(&self.args.base_dir),
            scan_dirs,
            recursive: true,
            max_depth: self.args.max_depth,
        })
    }

    /// Launch the interactive TUI interface
    async fn launch_interactive_tui(&self, config: DatasetManagerConfig) -> Result<()> {
        info!("🚀 Launching interactive dataset manager...");

        let tui = DatasetTui::new(config)?;
        tui.run().await?;

        info!("Dataset manager closed.");
        Ok(())
    }

    /// Handle dataset deletion
    async fn handle_delete_datasets(&self, manager: &mut DatasetManager) -> Result<()> {
        if self.args.delete.is_empty() {
            return Err(anyhow::anyhow!("No datasets specified for deletion"));
        }

        // Validate that all datasets exist
        let available_datasets: Vec<String> = manager
            .get_datasets()
            .iter()
            .map(|d| d.name.clone())
            .collect();

        let mut missing_datasets = Vec::new();
        for name in &self.args.delete {
            if !available_datasets.contains(name) {
                missing_datasets.push(name.clone());
            }
        }

        if !missing_datasets.is_empty() {
            return Err(anyhow::anyhow!(
                "Datasets not found: {}",
                missing_datasets.join(", ")
            ));
        }

        // Show what will be deleted
        warn!("Datasets to be deleted:");
        let mut total_size = 0u64;
        for name in &self.args.delete {
            if let Some(dataset) = manager.get_datasets().iter().find(|d| &d.name == name) {
                warn!(
                    "  {} {} {} ({})",
                    dataset.dataset_type.icon(),
                    dataset.name,
                    dataset.dataset_type.display_name(),
                    dataset.formatted_size()
                );
                total_size += dataset.size_bytes;
            }
        }
        warn!("Total size: {}", format_bytes(total_size));

        // Confirm deletion unless forced
        if !self.args.force {
            warn!("");
            warn!("⚠️  This action cannot be undone!");
            info!("Are you sure you want to delete these datasets? (y/N): ");

            use std::io::{self, Write};
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            if !input.trim().to_lowercase().starts_with('y') {
                info!("Deletion cancelled.");
                return Ok(());
            }
        }

        // Perform deletion
        info!("Deleting datasets...");
        match manager.delete_datasets(&self.args.delete) {
            Ok(deleted) => {
                info!("✅ Successfully deleted {} datasets", deleted.len());
                for name in deleted {
                    info!("  🗑️  {}", name);
                }
            }
            Err(e) => {
                error!("Failed to delete datasets: {}", e);
                return Err(e);
            }
        }

        Ok(())
    }

    /// Show summary statistics
    async fn show_summary(&self, manager: &DatasetManager) -> Result<()> {
        let summary = manager.get_summary();

        info!("📊 Dataset Summary");
        info!("");

        info!("Total Datasets: {}", summary.total_datasets);
        info!("Total Size: {}", summary.formatted_total_size());
        info!("Total Files: {}", summary.total_files);
        info!("Created Today: {}", summary.datasets_today);

        if !summary.type_counts.is_empty() {
            info!("");
            info!("By Type:");
            for (dataset_type, count) in &summary.type_counts {
                info!(
                    "  {} {} {} ({})",
                    dataset_type.icon(),
                    dataset_type.display_name(),
                    "datasets",
                    count
                );
            }
        }

        if let Some(last_scan) = summary.last_scan {
            info!("");
            info!("Last Scan: {}", last_scan.format("%Y-%m-%d %H:%M:%S"));
        }

        Ok(())
    }

    /// List all datasets
    async fn list_datasets(&self, manager: &DatasetManager) -> Result<()> {
        let datasets = manager.get_datasets();

        if datasets.is_empty() {
            warn!("📊 No datasets found.");
            info!("Run some pipelines to generate datasets.");
            return Ok(());
        }

        // Filter by type if specified
        let filtered_datasets: Vec<_> = if let Some(filter_type) = &self.args.filter_type {
            let target_type = Self::parse_dataset_type(filter_type)?;
            datasets
                .iter()
                .filter(|d| d.dataset_type == target_type)
                .collect()
        } else {
            datasets.iter().collect()
        };

        if filtered_datasets.is_empty() {
            warn!(
                "📊 No datasets found matching filter: {}",
                self.args.filter_type.as_ref().unwrap()
            );
            return Ok(());
        }

        info!("📊 Found {} datasets:", filtered_datasets.len());
        info!("");

        for dataset in &filtered_datasets {
            // Basic dataset info
            info!(
                "{} {} {} {} ({})",
                dataset.status_icon(),
                dataset.dataset_type.icon(),
                dataset.name,
                dataset.dataset_type.display_name(),
                dataset.formatted_size()
            );

            // Additional info
            info!(
                "   {} files • {} • {}",
                dataset.file_count,
                dataset.age(),
                dataset.path.display()
            );

            if self.args.details {
                // Show command information
                if !dataset.command_info.detected_commands.is_empty() {
                    info!("   Commands:");
                    for command in &dataset.command_info.detected_commands {
                        info!("     🔧 {}", command.command);
                    }
                    info!(
                        "     Confidence: {:.1}%",
                        dataset.command_info.confidence * 100.0
                    );
                }

                // Show warnings if any
                if !dataset.warnings.is_empty() {
                    warn!("   Warnings:");
                    for warning in &dataset.warnings {
                        warn!("     ⚠️  {:?}", warning);
                    }
                }

                // Show file breakdown
                if !dataset.files.is_empty() {
                    info!("   Files:");
                    for file in dataset.files.iter().take(5) {
                        // Show first 5 files
                        info!(
                            "     {} {} ({})",
                            file.file_type.icon(),
                            file.name,
                            format_bytes(file.size_bytes)
                        );
                    }
                    if dataset.files.len() > 5 {
                        info!("     ... and {} more files...", dataset.files.len() - 5);
                    }
                }
            }

            info!("");
        }

        // Show summary
        let total_size: u64 = filtered_datasets.iter().map(|d| d.size_bytes).sum();
        let total_files: usize = filtered_datasets.iter().map(|d| d.file_count).sum();

        info!(
            "Summary: {} datasets, {} total size, {} files",
            filtered_datasets.len(),
            format_bytes(total_size),
            total_files
        );

        Ok(())
    }

    /// Parse dataset type from string
    fn parse_dataset_type(type_str: &str) -> Result<DatasetType> {
        use crate::datasets::DataSource;

        let normalized = type_str.to_lowercase();
        match normalized.as_str() {
            "market_data" | "market-data" | "markets" => Ok(DatasetType::MarketData {
                source: DataSource::Unknown,
            }),
            "analyzed_markets" | "analyzed-markets" | "analyze" | "analysis" => {
                Ok(DatasetType::AnalyzedMarkets {
                    source_dataset: "unknown".to_string(),
                    filter_count: None,
                })
            }
            "enriched_markets" | "enriched-markets" | "enriched" | "enrich" => {
                Ok(DatasetType::EnrichedMarkets {
                    source_dataset: "unknown".to_string(),
                    enrichment_types: Vec::new(),
                })
            }
            "mixed" => Ok(DatasetType::Mixed {
                components: Vec::new(),
            }),
            "unknown" => Ok(DatasetType::Unknown),
            _ => {
                // Check if it's a pipeline name
                if normalized.starts_with("pipeline") {
                    let pipeline_name = normalized
                        .strip_prefix("pipeline")
                        .unwrap_or("")
                        .trim_start_matches('_')
                        .trim_start_matches('-');
                    if pipeline_name.is_empty() {
                        Ok(DatasetType::Pipeline {
                            name: "unknown".to_string(),
                            version: None,
                        })
                    } else {
                        Ok(DatasetType::Pipeline {
                            name: pipeline_name.to_string(),
                            version: None,
                        })
                    }
                } else {
                    Err(anyhow::anyhow!(
                        "Unknown dataset type: {}. Valid types: market_data, analyzed_markets, enriched_markets, mixed, unknown, pipeline_<name>",
                        type_str
                    ))
                }
            }
        }
    }
}
