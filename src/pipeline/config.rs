//! Pipeline configuration management

use std::path::Path;

/// Default pipelines directory
pub const DEFAULT_PIPELINES_DIR: &str = "pipelines";

/// Configuration for pipeline execution
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Directory containing pipeline YAML files
    pub pipelines_dir: String,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            pipelines_dir: DEFAULT_PIPELINES_DIR.to_string(),
        }
    }
}

impl PipelineConfig {
    /// Create a new pipeline configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the pipelines directory
    pub fn with_pipelines_dir<P: AsRef<Path>>(mut self, dir: P) -> Self {
        self.pipelines_dir = dir.as_ref().to_string_lossy().to_string();
        self
    }



    /// Get the full path to a pipeline file
    pub fn pipeline_path(&self, name: &str) -> String {
        let yaml_path = Path::new(&self.pipelines_dir).join(format!("{}.yaml", name));
        let yml_path = Path::new(&self.pipelines_dir).join(format!("{}.yml", name));
        
        if yaml_path.exists() {
            yaml_path.to_string_lossy().to_string()
        } else if yml_path.exists() {
            yml_path.to_string_lossy().to_string()
        } else {
            // Default to .yaml extension
            yaml_path.to_string_lossy().to_string()
        }
    }


}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_config_creation() {
        let config = PipelineConfig::new()
            .with_pipelines_dir("my_pipelines");
        
        assert_eq!(config.pipelines_dir, "my_pipelines");
    }

    #[test]
    fn test_pipeline_path_resolution() {
        let config = PipelineConfig::new()
            .with_pipelines_dir("test_pipelines");
        
        let path = config.pipeline_path("market_analysis");
        assert!(path.ends_with("market_analysis.yaml"));
        assert!(path.starts_with("test_pipelines"));
    }
} 