use std::path::{Path, PathBuf};

/// Default data directory (relative to current working directory)
pub const DEFAULT_DATA_DIR: &str = "./data";

/// Subdirectory paths relative to the data directory
pub const MARKETS_DIR: &str = "markets";
pub const MARKETS_CLOB_DIR: &str = "markets/clob";
pub const MARKETS_GAMMA_DIR: &str = "markets/gamma";
pub const MARKETS_CACHE_DIR: &str = "markets/cache";
pub const MARKETS_DATASETS_DIR: &str = "markets/datasets";
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
    pub fn root(&self) -> &Path {
        &self.root
    }
    
    /// Get the markets directory
    pub fn markets(&self) -> PathBuf {
        self.root.join(MARKETS_DIR)
    }
    
    /// Get the CLOB markets directory
    pub fn markets_clob(&self) -> PathBuf {
        self.root.join(MARKETS_CLOB_DIR)
    }
    
    /// Get the Gamma markets directory
    pub fn markets_gamma(&self) -> PathBuf {
        self.root.join(MARKETS_GAMMA_DIR)
    }
    
    /// Get the markets cache directory
    pub fn markets_cache(&self) -> PathBuf {
        self.root.join(MARKETS_CACHE_DIR)
    }
    
    /// Get the markets datasets directory
    pub fn markets_datasets(&self) -> PathBuf {
        self.root.join(MARKETS_DATASETS_DIR)
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
        std::fs::create_dir_all(self.markets())?;
        std::fs::create_dir_all(self.markets_clob())?;
        std::fs::create_dir_all(self.markets_gamma())?;
        std::fs::create_dir_all(self.markets_cache())?;
        std::fs::create_dir_all(self.markets_datasets())?;
        std::fs::create_dir_all(self.auth())?;
        std::fs::create_dir_all(self.logs())?;
        Ok(())
    }
} 