//! Version command for displaying polybot version information

use anyhow::Result;
use clap::Args;
use owo_colors::OwoColorize;

use crate::data_paths::DataPaths;

#[derive(Args, Clone)]
pub struct VersionArgs {}

pub struct VersionCommand {
    _args: VersionArgs,
}

impl VersionCommand {
    pub fn new(args: VersionArgs) -> Self {
        Self { _args: args }
    }

    pub async fn execute(&self, _host: &str, _data_paths: DataPaths) -> Result<()> {
        // Get version from Cargo.toml
        const VERSION: &str = env!("CARGO_PKG_VERSION");
        const PKG_NAME: &str = env!("CARGO_PKG_NAME");
        const PKG_DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");
        const PKG_AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
        
        println!("{} v{}", PKG_NAME.bright_blue().bold(), VERSION.bright_green());
        if !PKG_DESCRIPTION.is_empty() {
            println!("{}", PKG_DESCRIPTION);
        }
        if !PKG_AUTHORS.is_empty() {
            println!("Authors: {}", PKG_AUTHORS.bright_cyan());
        }
        
        // Additional build information
        println!();
        println!("{}", "Build Information:".bright_yellow());
        println!("  Profile: {}", if cfg!(debug_assertions) { "debug" } else { "release" });
        
        // Show target triple if available
        if let Ok(target) = std::env::var("TARGET") {
            println!("  Target: {}", target);
        }
        
        Ok(())
    }
}