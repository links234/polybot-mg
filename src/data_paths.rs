use std::path::{Path, PathBuf};

/// Default data directory (relative to current working directory)
pub const DEFAULT_DATA_DIR: &str = "./data";

/// Default datasets directory (relative to current working directory)
pub const DEFAULT_DATASETS_DIR: &str = "./data/datasets";

/// Default runs directory (relative to current working directory)
pub const DEFAULT_RUNS_DIR: &str = "./data/datasets/runs";

/// Subdirectory paths relative to the data directory
pub const DATASETS_DIR: &str = "datasets";
pub const RUNS_DIR: &str = "runs";
pub const AUTH_DIR: &str = "auth";
pub const LOGS_DIR: &str = "logs";

/// Helper struct to manage data paths
#[derive(Clone, Debug)]
pub struct DataPaths {
    root: PathBuf,
}

impl DataPaths {
    /// Create a new DataPaths instance with the given root directory
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }
    
    /// Get the root data directory
    pub fn root(&self) -> &PathBuf {
        &self.root
    }
    
    /// Get the datasets directory (default location for all dataset outputs)
    pub fn datasets(&self) -> PathBuf {
        self.root.join(DATASETS_DIR)
    }
    
    /// Get the runs directory (for pipeline and command outputs)
    pub fn runs(&self) -> PathBuf {
        self.datasets().join(RUNS_DIR)
    }
    
    /// Get the auth directory
    pub fn auth(&self) -> PathBuf {
        self.root.join(AUTH_DIR)
    }
    
    /// Get the logs directory
    pub fn logs(&self) -> PathBuf {
        self.root.join(LOGS_DIR)
    }
    
    /// Ensure all directories exist
    pub fn ensure_directories(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.root)?;
        std::fs::create_dir_all(self.datasets())?;
        std::fs::create_dir_all(self.runs())?;
        std::fs::create_dir_all(self.auth())?;
        std::fs::create_dir_all(self.logs())?;
        Ok(())
    }
} 