use crate::data_paths::DataPaths;
use crate::datasets::save_command_metadata;
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use clap::Args;
use std::collections::HashMap;
use tracing::{info, warn};

#[derive(Args, Clone)]
pub struct FetchAllMarketsArgs {
    /// Dataset name (will be created in the datasets directory)
    #[arg(long)]
    pub dataset_name: Option<String>,

    /// Clear previous state and start fresh
    #[arg(long)]
    pub clear_state: bool,

    /// Maximum file size in MB for each chunk (default: 100)
    #[arg(long, default_value = "100")]
    pub chunk_size_mb: f64,

    /// Use Gamma API instead of CLOB API (different data structure)
    #[arg(long)]
    pub use_gamma: bool,

    /// Cache resolution: seconds, minutes, hours, days
    #[arg(long, default_value = "hours")]
    pub cache_resolution: String,

    /// Cache duration (number of cache_resolution units)
    #[arg(long, default_value = "1")]
    pub cache_duration: u32,

    /// Force refresh cache (ignore existing cached data)
    #[arg(long)]
    pub force_refresh: bool,
}

#[derive(Clone)]
pub struct FetchAllMarketsCommand {
    args: FetchAllMarketsArgs,
}

impl FetchAllMarketsCommand {
    pub fn new(args: FetchAllMarketsArgs) -> Self {
        Self { args }
    }

    pub async fn execute(&self, host: &str, data_paths: DataPaths, verbose: bool) -> Result<()> {
        // Generate dataset name with timestamp if not provided
        let dataset_name = self.args.dataset_name.clone().unwrap_or_else(|| {
            let now = chrono::Utc::now();
            format!("markets_{}", now.format("%Y-%m-%d_%H-%M-%S"))
        });

        let datasets_path = data_paths.datasets();
        let output_dir = datasets_path.join(&dataset_name);

        info!("📊 Fetch All Markets - Dataset: {}", dataset_name);

        // Check for force refresh from environment or args
        let force_refresh = self.args.force_refresh
            || std::env::var("FORCE_REFRESH")
                .unwrap_or_default()
                .to_lowercase()
                == "true";

        // Check cache validity if not forcing refresh
        if !force_refresh && !self.args.clear_state {
            if let Ok(cache_valid) = self.check_cache_validity(&output_dir, verbose).await {
                if cache_valid {
                    info!("✅ Using cached dataset '{}' (still fresh)", dataset_name);
                    return Ok(());
                }
            }
        }

        if force_refresh && verbose {
            warn!("🔄 Force refresh enabled - ignoring cache");
        }

        // Check if parent directory exists and is valid
        if !datasets_path.exists() {
            std::fs::create_dir_all(&datasets_path).map_err(|e| {
                anyhow::anyhow!(
                    "❌ Failed to create datasets directory '{}'.\n\
                     💡 Error: {}\n\
                     💡 Check directory permissions and ensure the path is correct.",
                    datasets_path.display(),
                    e
                )
            })?;
        }

        // Create output directory
        if output_dir.exists() && self.args.clear_state {
            warn!("🗑️  Clearing existing data in: {}", output_dir.display());
            std::fs::remove_dir_all(&output_dir).map_err(|e| {
                anyhow::anyhow!(
                    "❌ Failed to clear existing dataset directory '{}'.\n\
                     💡 Error: {}\n\
                     💡 The directory might be in use or have permission restrictions.",
                    output_dir.display(),
                    e
                )
            })?;
        }

        if !output_dir.exists() {
            std::fs::create_dir_all(&output_dir).map_err(|e| {
                anyhow::anyhow!(
                    "❌ Failed to create dataset directory '{}'.\n\
                     💡 Error: {}\n\
                     💡 Check directory permissions and available disk space.",
                    output_dir.display(),
                    e
                )
            })?;
        }

        info!("📁 Dataset directory: {}", output_dir.display());

        // Execute the appropriate fetch method
        if self.args.use_gamma {
            info!("🌐 Using Gamma API for market data...");
            crate::markets::fetch_all_markets_gamma(
                &output_dir.to_string_lossy(),
                verbose,
                self.args.clear_state,
                self.args.chunk_size_mb,
            )
            .await?;
        } else {
            let client = crate::auth::get_authenticated_client(host, &data_paths).await?;
            crate::markets::fetch_all_markets(
                client,
                &output_dir.to_string_lossy(),
                verbose,
                self.args.clear_state,
                self.args.chunk_size_mb,
            )
            .await?;
        }

        // Save command metadata
        let mut command_args = vec!["--dataset-name".to_string(), dataset_name.clone()];

        if verbose {
            command_args.push("--verbose".to_string());
        }

        if self.args.clear_state {
            command_args.push("--clear-state".to_string());
        }

        if self.args.use_gamma {
            command_args.push("--use-gamma".to_string());
        }

        command_args.extend_from_slice(&[
            "--chunk-size-mb".to_string(),
            self.args.chunk_size_mb.to_string(),
            "--cache-resolution".to_string(),
            self.args.cache_resolution.clone(),
            "--cache-duration".to_string(),
            self.args.cache_duration.to_string(),
        ]);

        if self.args.force_refresh {
            command_args.push("--force-refresh".to_string());
        }

        // Enhanced metadata with cache information
        let mut additional_info = HashMap::new();
        additional_info.insert("dataset_name".to_string(), serde_json::json!(dataset_name));
        additional_info.insert(
            "api_source".to_string(),
            serde_json::json!(if self.args.use_gamma { "gamma" } else { "clob" }),
        );
        additional_info.insert(
            "chunk_size_mb".to_string(),
            serde_json::json!(self.args.chunk_size_mb),
        );
        additional_info.insert(
            "cache_resolution".to_string(),
            serde_json::json!(self.args.cache_resolution),
        );
        additional_info.insert(
            "cache_duration".to_string(),
            serde_json::json!(self.args.cache_duration),
        );
        additional_info.insert(
            "force_refresh".to_string(),
            serde_json::json!(self.args.force_refresh),
        );

        save_command_metadata(
            &output_dir,
            "fetch-all-markets",
            &command_args,
            Some(additional_info),
        )?;

        info!("✅ Successfully created dataset: {}", dataset_name);
        Ok(())
    }

    /// Check if cached data is still valid based on cache settings
    async fn check_cache_validity(
        &self,
        dataset_path: &std::path::Path,
        verbose: bool,
    ) -> Result<bool> {
        if !dataset_path.exists() {
            if verbose {
                warn!(
                    "📂 Dataset directory '{}' does not exist",
                    dataset_path.display()
                );
            }
            return Ok(false);
        }

        // Check for dataset.yaml metadata file
        let metadata_file = dataset_path.join("dataset.yaml");
        if !metadata_file.exists() {
            if verbose {
                warn!("📄 No dataset metadata found - treating as invalid cache");
            }
            return Ok(false);
        }

        // Read and parse metadata
        let metadata_content = std::fs::read_to_string(&metadata_file)
            .map_err(|e| anyhow::anyhow!("Failed to read dataset metadata: {}", e))?;

        let metadata: serde_yaml::Value = serde_yaml::from_str(&metadata_content)
            .map_err(|e| anyhow::anyhow!("Failed to parse dataset metadata: {}", e))?;

        // Extract creation timestamp
        let created_at_str = metadata
            .get("created_at")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Dataset metadata missing created_at field"))?;

        let created_at = DateTime::parse_from_rfc3339(created_at_str)
            .map_err(|e| anyhow::anyhow!("Invalid created_at timestamp: {}", e))?
            .with_timezone(&Utc);

        // Calculate cache expiration
        let cache_duration = self.parse_cache_duration()?;
        let expires_at = created_at + cache_duration;
        let now = Utc::now();

        let is_valid = now < expires_at;

        if verbose {
            let age = now.signed_duration_since(created_at);
            let remaining = expires_at.signed_duration_since(now);

            info!(
                "🕒 Cache check for '{}':",
                dataset_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            );
            info!(
                "   Created: {} ({})",
                created_at.format("%Y-%m-%d %H:%M:%S UTC"),
                self.format_duration(age)
            );
            info!(
                "   Expires: {} ({})",
                expires_at.format("%Y-%m-%d %H:%M:%S UTC"),
                if is_valid {
                    format!("in {}", self.format_duration(remaining))
                } else {
                    format!("{} ago", self.format_duration(-remaining))
                }
            );
            if is_valid {
                info!("   Status: ✅ Valid");
            } else {
                info!("   Status: ❌ Expired");
            }
        }

        Ok(is_valid)
    }

    /// Parse cache duration from configuration
    fn parse_cache_duration(&self) -> Result<Duration> {
        let base_duration = match self.args.cache_resolution.to_lowercase().as_str() {
            "seconds" | "second" | "s" => Duration::seconds(1),
            "minutes" | "minute" | "m" => Duration::minutes(1),
            "hours" | "hour" | "h" => Duration::hours(1),
            "days" | "day" | "d" => Duration::days(1),
            _ => {
                return Err(anyhow::anyhow!(
                    "Invalid cache resolution: '{}'. Must be one of: seconds, minutes, hours, days",
                    self.args.cache_resolution
                ))
            }
        };

        Ok(base_duration * self.args.cache_duration as i32)
    }

    /// Format duration for human-readable display
    fn format_duration(&self, duration: Duration) -> String {
        let abs_duration = duration.abs();

        if abs_duration.num_days() > 0 {
            format!("{} days", abs_duration.num_days())
        } else if abs_duration.num_hours() > 0 {
            format!("{} hours", abs_duration.num_hours())
        } else if abs_duration.num_minutes() > 0 {
            format!("{} minutes", abs_duration.num_minutes())
        } else {
            format!("{} seconds", abs_duration.num_seconds())
        }
    }
}
