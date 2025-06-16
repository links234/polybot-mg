//! Pipeline command for running workflow scripts

use anyhow::Result;
use clap::Args;
use owo_colors::OwoColorize;
use std::collections::HashMap;
use tracing::info;

use crate::data_paths::DataPaths;
use crate::pipeline::{Pipeline, PipelineConfig, PipelineRunner};

#[derive(Args, Clone)]
pub struct PipelineArgs {
    /// Pipeline name to execute (not required when listing)
    pub name: Option<String>,

    /// Directory containing pipeline YAML files
    #[arg(long, default_value = "pipelines")]
    pub pipelines_dir: String,

    /// Additional parameters to pass to the pipeline (key=value format)
    #[arg(short, long)]
    pub param: Vec<String>,

    /// Run in dry-run mode (show commands without executing)
    #[arg(long)]
    pub dry_run: bool,

    /// List all available pipelines
    #[arg(long)]
    pub list: bool,
}

pub struct PipelineCommand {
    args: PipelineArgs,
}

impl PipelineCommand {
    pub fn new(args: PipelineArgs) -> Self {
        Self { args }
    }

    pub async fn execute(&self, _host: &str, _data_paths: DataPaths) -> Result<()> {
        let config = crate::pipeline::PipelineConfig::new()
            .with_pipelines_dir(&self.args.pipelines_dir);

        // List available pipelines
        if self.args.list {
            return self.list_pipelines(&config).await;
        }

        // If no pipeline name provided and no other flags, launch interactive TUI
        if self.args.name.is_none() && !self.args.list {
            return self.launch_interactive_tui(config).await;
        }

        // Validate that name is provided when not listing or using TUI
        let pipeline_name = match &self.args.name {
            Some(name) => name,
            None => {
                return Err(anyhow::anyhow!(
                    "Pipeline name is required. Use --list to see available pipelines or run without arguments for interactive mode."
                ));
            }
        };

        // Parse additional parameters
        let mut extra_params = HashMap::new();
        for param in &self.args.param {
            if let Some((key, value)) = param.split_once('=') {
                extra_params.insert(key.to_string(), value.to_string());
            } else {
                return Err(anyhow::anyhow!("Invalid parameter format: '{}'. Use key=value format.", param));
            }
        }

        // Load and execute pipeline
        let pipeline_path = config.pipeline_path(pipeline_name);
        
        if !std::path::Path::new(&pipeline_path).exists() {
            return Err(anyhow::anyhow!(
                "Pipeline '{}' not found at: {}\nRun 'polybot pipeline --list' to see available pipelines.",
                pipeline_name, pipeline_path
            ));
        }

        info!("Loading pipeline from: {}", pipeline_path);
        let pipeline = crate::pipeline::Pipeline::from_file(&pipeline_path)?;

        let mut context = pipeline.create_context(extra_params);
        context.dry_run = self.args.dry_run;

        // Use default verbose setting (can be enhanced later)
        let runner = crate::pipeline::PipelineRunner::new_auto()
            .with_verbose(false);

        println!("{}", format!("🔧 Loaded pipeline: {} ({})", 
            pipeline.name, pipeline_path).bright_blue());

        if self.args.dry_run {
            println!("{}", "🔍 Running in dry-run mode (no commands will be executed)".bright_yellow());
        }

        let stats = runner.execute_pipeline(&pipeline, context).await?;

        info!("Pipeline execution completed: {:?}", stats);

        Ok(())
    }

    /// Launch the interactive TUI for pipeline selection
    async fn launch_interactive_tui(&self, config: crate::pipeline::PipelineConfig) -> Result<()> {
        // Create and run the TUI
        let tui = crate::pipeline::PipelineTui::new(config.clone())?;
        
        match tui.run().await? {
            Some(selected_pipeline) => {
                println!("{}", format!("🚀 Selected pipeline: {}", selected_pipeline).bright_green());
                println!();

                // Parse additional parameters
                let mut extra_params = HashMap::new();
                for param in &self.args.param {
                    if let Some((key, value)) = param.split_once('=') {
                        extra_params.insert(key.to_string(), value.to_string());
                    } else {
                        return Err(anyhow::anyhow!("Invalid parameter format: '{}'. Use key=value format.", param));
                    }
                }

                // Load and execute the selected pipeline
                let pipeline_path = config.pipeline_path(&selected_pipeline);
                let pipeline = crate::pipeline::Pipeline::from_file(&pipeline_path)?;

                let mut context = pipeline.create_context(extra_params);
                context.dry_run = self.args.dry_run;

                let runner = crate::pipeline::PipelineRunner::new_auto()
                    .with_verbose(false);

                if self.args.dry_run {
                    println!("{}", "🔍 Running in dry-run mode (no commands will be executed)".bright_yellow());
                }

                let stats = runner.execute_pipeline(&pipeline, context).await?;
                info!("Pipeline execution completed: {:?}", stats);
            }
            None => {
                println!("{}", "Pipeline selection cancelled.".bright_yellow());
            }
        }

        Ok(())
    }

    async fn list_pipelines(&self, config: &PipelineConfig) -> Result<()> {
        println!("{}", "📋 Available Pipelines:".bright_blue());
        println!();

        let pipelines = PipelineRunner::list_pipelines(&config.pipelines_dir)?;

        if pipelines.is_empty() {
            println!("{}", format!("No pipelines found in directory: {}", config.pipelines_dir).bright_yellow());
            println!("{}", "Create pipeline YAML files in the pipelines/ directory to get started.".bright_cyan());
            return Ok(());
        }

        for pipeline_name in pipelines {
            let pipeline_path = config.pipeline_path(&pipeline_name);
            
            // Try to load pipeline to get description
            match Pipeline::from_file(&pipeline_path) {
                Ok(pipeline) => {
                    println!("{}", format!("📄 {}", pipeline_name).bright_green());
                    println!("   Name: {}", pipeline.name.bright_white());
                    
                    if !pipeline.description.is_empty() {
                        println!("   Description: {}", pipeline.description.bright_cyan());
                    }
                    
                    println!("   Steps: {}", pipeline.steps.len().to_string().bright_yellow());
                    println!("   Path: {}", pipeline_path.bright_black());
                    println!();
                }
                Err(e) => {
                    println!("{}", format!("❌ {} (invalid YAML: {})", pipeline_name, e).bright_red());
                    println!();
                }
            }
        }

        println!("{}", "Usage:".bright_blue());
        println!("  polybot pipeline <name>                 # Run a pipeline");
        println!("  polybot pipeline <name> --dry-run       # Preview pipeline execution");
        println!("  polybot pipeline <name> -p key=value    # Pass custom parameters");
        println!("  polybot pipeline <name> --verbose       # Show detailed output");

        Ok(())
    }
} 