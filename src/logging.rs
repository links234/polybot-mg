use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use tracing_appender::non_blocking;
use tracing_subscriber::EnvFilter;

use crate::data_paths::DataPaths;

#[derive(Debug, Clone, PartialEq)]
pub enum LogMode {
    /// Console + file logging (for CLI mode)
    ConsoleAndFile,
    /// File-only logging (for TUI mode)
    FileOnly,
}

pub struct LoggingConfig {
    pub mode: LogMode,
    pub data_paths: DataPaths,
    pub session_id: String,
}

impl LoggingConfig {
    pub fn new(mode: LogMode, data_paths: DataPaths) -> Self {
        let session_id = generate_session_id();
        Self {
            mode,
            data_paths,
            session_id,
        }
    }

    pub fn log_file_path(&self) -> PathBuf {
        self.data_paths.logs().join(format!("polybot-{}.log", self.session_id))
    }
}

/// Initialize logging based on the configuration
pub fn init_logging(config: LoggingConfig) -> Result<()> {
    // Ensure logs directory exists
    config.data_paths.ensure_directories()?;

    // Get log level from environment or default to INFO
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    match config.mode {
        LogMode::ConsoleAndFile => {
            // Create per-session file appender
            let log_file = std::fs::File::create(config.log_file_path())
                .map_err(|e| anyhow::anyhow!("Failed to create log file: {}", e))?;
            
            let (file_writer, _file_guard) = non_blocking(log_file);
            
            // Store the guard to prevent it from being dropped
            std::mem::forget(_file_guard);

            // Console + file logging for CLI mode
            use tracing_subscriber::fmt::writer::MakeWriterExt;
            let multi_writer = std::io::stderr.and(file_writer);
            
            tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .with_writer(multi_writer)
                .with_ansi(true)
                .with_target(false)
                .compact()
                .init();
        }
        LogMode::FileOnly => {
            // Create per-session file appender
            let log_file = std::fs::File::create(config.log_file_path())
                .map_err(|e| anyhow::anyhow!("Failed to create log file: {}", e))?;
            
            let (file_writer, _file_guard) = non_blocking(log_file);
            
            // Store the guard to prevent it from being dropped
            std::mem::forget(_file_guard);

            // File-only logging for TUI mode
            tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .with_writer(file_writer)
                .with_ansi(false)
                .with_target(true)
                .with_thread_ids(true)
                .with_thread_names(true)
                .with_line_number(true)
                .with_file(true)
                .init();
        }
    }

    // Log session start
    tracing::info!(
        session_id = %config.session_id,
        mode = ?config.mode,
        log_file = %config.log_file_path().display(),
        "Logging initialized"
    );

    Ok(())
}

/// Generate a unique session ID with timestamp
fn generate_session_id() -> String {
    let now: DateTime<Utc> = Utc::now();
    format!("{}", now.format("%Y%m%d_%H%M%S_%3f"))
}

/// Log session end
pub fn log_session_end() {
    tracing::info!("Session ended");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_id_format() {
        let session_id = generate_session_id();
        // Should be in format: YYYYMMDD_HHMMSS_mmm
        assert_eq!(session_id.len(), 18);
        assert!(session_id.contains('_'));
    }

    #[test]
    fn test_logging_config() {
        let data_paths = DataPaths::new("/tmp/test");
        
        let config = LoggingConfig::new(LogMode::FileOnly, data_paths.clone());
        
        assert_eq!(config.mode, LogMode::FileOnly);
        assert!(config.log_file_path().starts_with(data_paths.logs()));
        assert!(config.log_file_path().to_string_lossy().contains("polybot-"));
    }
}