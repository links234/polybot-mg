//! Canvas command for launching the egui-based trading interface

use anyhow::Result;
use clap::Args;
use tracing::info;

use crate::data_paths::DataPaths;
use crate::logging::{init_logging, LogMode, LoggingConfig};

#[derive(Args, Clone)]
pub struct CanvasArgs {
    /// Window width
    #[arg(long, default_value = "1200")]
    pub width: u32,

    /// Window height
    #[arg(long, default_value = "800")]
    pub height: u32,

    /// Enable dark mode
    #[arg(long)]
    pub dark_mode: bool,

    /// Window title
    #[arg(long, default_value = "Polybot Trading Canvas")]
    pub title: String,
}

pub struct CanvasCommand {
    args: CanvasArgs,
}

impl CanvasCommand {
    pub fn new(args: CanvasArgs) -> Self {
        Self { args }
    }

    pub async fn execute(&self, host: &str, data_paths: DataPaths) -> Result<()> {
        // Initialize logging with console and file output for GUI mode
        let log_config = LoggingConfig::new(LogMode::ConsoleAndFile, data_paths.clone());
        init_logging(log_config)?;

        info!("🎨 Starting Polybot Trading Canvas");
        info!("🌐 API Host: {}", host);
        info!("📁 Data Directory: {}", data_paths.root().display());

        // Call the GUI launcher function from the library
        crate::gui::launch_trading_canvas(
            self.args.width,
            self.args.height,
            self.args.dark_mode,
            &self.args.title,
            host,
            data_paths,
        )
        .await
    }
}
