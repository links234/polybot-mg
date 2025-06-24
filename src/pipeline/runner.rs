//! Pipeline execution engine

use super::{Pipeline, PipelineContext, PipelineStep};
use crate::markets::datasets::save_command_metadata;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

/// Pipeline execution statistics
#[derive(Debug, Default)]
pub struct PipelineStats {
    pub total_steps: usize,
    pub successful_steps: usize,
    pub failed_steps: usize,
    pub total_duration: Duration,
}

/// Pipeline runner that executes workflows
pub struct PipelineRunner {
    /// Binary name to execute (e.g., "polybot")
    pub binary_name: String,
    /// Whether to show verbose output
    pub verbose: bool,
}

impl PipelineRunner {
    /// Create a new pipeline runner with specified binary name
    pub fn new(binary_name: String) -> Self {
        Self {
            binary_name,
            verbose: false,
        }
    }

    /// Create a new pipeline runner with auto-detection of cargo vs binary execution
    pub fn new_auto() -> Self {
        let (binary_name, detection_info) = Self::detect_execution_method();

        // Log detection info
        info!("🔍 Detected execution method: {}", detection_info);
        info!("📦 Using command: {}", binary_name);

        Self {
            binary_name,
            verbose: false,
        }
    }

    /// Detect whether we're running through cargo or as a standalone binary
    fn detect_execution_method() -> (String, String) {
        // Get current executable path
        let current_exe = std::env::current_exe();
        let args: Vec<String> = std::env::args().collect();

        debug!("Current executable: {:?}", current_exe);
        debug!("Command line args: {:?}", args);

        // Check if current executable path contains target/debug or target/release
        if let Ok(exe_path) = &current_exe {
            let exe_str = exe_path.to_string_lossy();

            if exe_str.contains("target/debug") {
                return (
                    "cargo run --".to_string(),
                    format!("Cargo development build ({})", exe_str),
                );
            }

            if exe_str.contains("target/release") {
                return (
                    "cargo run --".to_string(),
                    format!("Cargo release build ({})", exe_str),
                );
            }

            // Check if executable name starts with polybot (installed binary)
            if let Some(file_name) = exe_path.file_name().and_then(|n| n.to_str()) {
                if file_name.starts_with("polybot") && !exe_str.contains("target") {
                    return (
                        "polybot".to_string(),
                        format!("Standalone binary ({})", exe_str),
                    );
                }
            }
        }

        // Fallback: check environment variables that cargo sets
        if std::env::var("CARGO_PKG_NAME").is_ok() || std::env::var("CARGO_MANIFEST_DIR").is_ok() {
            return (
                "cargo run --".to_string(),
                "Cargo environment detected".to_string(),
            );
        }

        // Check if we can find cargo in the process hierarchy (last resort)
        if let Ok(exe_path) = &current_exe {
            let exe_str = exe_path.to_string_lossy();
            if exe_str.contains("cargo") {
                return (
                    "cargo run --".to_string(),
                    "Cargo in executable path".to_string(),
                );
            }
        }

        // Default to standalone binary
        (
            "polybot".to_string(),
            format!(
                "Default (standalone binary assumed, exe: {:?})",
                current_exe
            ),
        )
    }

    /// Set verbose output
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Execute a complete pipeline
    pub async fn execute_pipeline(
        &self,
        pipeline: &Pipeline,
        context: PipelineContext,
    ) -> Result<PipelineStats> {
        let start_time = Instant::now();
        let mut stats = PipelineStats {
            total_steps: pipeline.steps.len(),
            ..Default::default()
        };

        info!("🚀 Starting pipeline: {}", pipeline.name);
        if !pipeline.description.is_empty() {
            info!("📝 {}", pipeline.description);
        }

        // Show resolved parameters if verbose
        if self.verbose {
            info!("📋 Resolved parameters:");
            for (key, value) in &context.parameters {
                info!("  {} = {}", key, value);
            }
        }

        for (step_index, step) in pipeline.steps.iter().enumerate() {
            let step_num = step_index + 1;
            let step_result = self.execute_step(pipeline, step, &context, step_num).await;

            match step_result {
                Ok(()) => {
                    stats.successful_steps += 1;
                }
                Err(e) => {
                    stats.failed_steps += 1;

                    if step.continue_on_error {
                        warn!(
                            "⚠️  Step {}/{} failed but continuing: {}",
                            step_num, stats.total_steps, e
                        );
                    } else {
                        error!(
                            "❌ Step {}/{} failed, stopping pipeline: {}",
                            step_num, stats.total_steps, e
                        );

                        stats.total_duration = start_time.elapsed();
                        return Err(e);
                    }
                }
            }
        }

        stats.total_duration = start_time.elapsed();

        // Log summary
        info!("🎉 Pipeline completed!");
        info!(
            "📊 Summary: {}/{} steps successful, {} failed",
            stats.successful_steps, stats.total_steps, stats.failed_steps
        );
        info!("⏱️  Total duration: {:?}", stats.total_duration);

        // Save pipeline metadata if we have a pipeline output directory
        if let Some(output_dir) = context.parameters.get("pipeline_output_dir") {
            let pipeline_path = std::path::PathBuf::from(output_dir);

            // Create the output directory if it doesn't exist
            if let Err(e) = std::fs::create_dir_all(&pipeline_path) {
                warn!("Failed to create pipeline output directory: {}", e);
            } else {
                // Save pipeline metadata
                let mut additional_info = HashMap::new();
                additional_info.insert(
                    "dataset_type".to_string(),
                    serde_json::json!(format!("Pipeline({})", pipeline.name)),
                );
                additional_info.insert(
                    "pipeline_name".to_string(),
                    serde_json::json!(pipeline.name),
                );
                additional_info.insert(
                    "total_steps".to_string(),
                    serde_json::json!(stats.total_steps),
                );
                additional_info.insert(
                    "successful_steps".to_string(),
                    serde_json::json!(stats.successful_steps),
                );
                additional_info.insert(
                    "failed_steps".to_string(),
                    serde_json::json!(stats.failed_steps),
                );
                additional_info.insert(
                    "total_duration_secs".to_string(),
                    serde_json::json!(stats.total_duration.as_secs()),
                );

                // Add step information
                let steps_info: Vec<serde_json::Value> = pipeline
                    .steps
                    .iter()
                    .map(|step| {
                        serde_json::json!({
                            "name": step.name,
                            "command": step.command,
                            "args": step.args
                        })
                    })
                    .collect();
                additional_info.insert("steps".to_string(), serde_json::json!(steps_info));

                if let Err(e) = save_command_metadata(
                    &pipeline_path,
                    "pipeline",
                    &[pipeline.name.clone()],
                    Some(additional_info),
                ) {
                    warn!("Failed to save pipeline metadata: {}", e);
                }
            }
        }

        Ok(stats)
    }

    /// Execute a single pipeline step
    async fn execute_step(
        &self,
        pipeline: &Pipeline,
        step: &PipelineStep,
        context: &PipelineContext,
        step_num: usize,
    ) -> Result<()> {
        let total_steps = pipeline.steps.len();

        info!("🔄 Step {}/{}: {}", step_num, total_steps, step.name);

        if context.dry_run {
            let resolved_args: Vec<String> = step
                .args
                .iter()
                .map(|arg| pipeline.resolve_parameters(arg, context))
                .collect();

            info!(
                "   Dry run: {} {} {}",
                self.binary_name,
                step.command,
                resolved_args.join(" ")
            );
            return Ok(());
        }

        let step_start = Instant::now();

        // Build command based on binary type
        let mut cmd = if self.binary_name.starts_with("cargo run") {
            let mut cargo_cmd = Command::new("cargo");
            cargo_cmd.arg("run").arg("--").arg(&step.command);
            cargo_cmd
        } else {
            let mut binary_cmd = Command::new(&self.binary_name);
            binary_cmd.arg(&step.command);
            binary_cmd
        };

        // Add resolved arguments
        for arg in &step.args {
            let resolved_arg = pipeline.resolve_parameters(arg, context);
            cmd.arg(resolved_arg);
        }

        // Set environment variables
        for (key, value) in &step.env {
            let resolved_value = pipeline.resolve_parameters(value, context);
            cmd.env(key, resolved_value);
        }

        // Configure command execution
        if self.verbose {
            cmd.stdout(Stdio::inherit());
            cmd.stderr(Stdio::inherit());
        } else {
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
        }

        debug!("Executing command: {:?}", cmd);

        // Execute command
        let output = cmd.output()
            .with_context(|| format!("Failed to execute pipeline step '{}' (command: {} {}). Check that the binary is available and the command is correct", 
                step.name, self.binary_name, step.command))?;

        let duration = step_start.elapsed();

        if output.status.success() {
            info!(
                "✅ Step {}/{} completed in {:?}",
                step_num, total_steps, duration
            );

            if self.verbose && !output.stdout.is_empty() {
                info!("   Output: {}", String::from_utf8_lossy(&output.stdout));
            }
        } else {
            let stderr_output = String::from_utf8_lossy(&output.stderr);
            let stdout_output = String::from_utf8_lossy(&output.stdout);

            let error_details = if !stderr_output.is_empty() {
                format!("stderr: {}", stderr_output.trim())
            } else if !stdout_output.is_empty() {
                format!("stdout: {}", stdout_output.trim())
            } else {
                format!("Command exited with status: {}", output.status)
            };

            // Enhanced error message with context
            let enhanced_error = anyhow::anyhow!(
                "❌ Pipeline step '{}' failed.\n\
                 💥 Command: {} {} {}\n\
                 💥 Error details: {}\n\
                 💡 Suggestions:\n\
                 • Check the command arguments and parameters\n\
                 • Verify input files exist and have correct permissions\n\
                 • Ensure output directories are writable\n\
                 • Run with --verbose flag for more detailed output\n\
                 • Try running the failing command manually",
                step.name,
                self.binary_name,
                step.command,
                step.args.join(" "),
                error_details
            );

            return Err(enhanced_error);
        }

        Ok(())
    }

    /// List available pipelines in a directory
    pub fn list_pipelines(pipelines_dir: &str) -> Result<Vec<String>> {
        let dir = std::path::Path::new(pipelines_dir);
        if !dir.exists() {
            return Err(anyhow::anyhow!(
                "❌ Pipelines directory not found: '{}'\n\
                 💡 Suggestions:\n\
                 • Create the pipelines directory: mkdir -p \"{}\"\n\
                 • Check that you're running from the correct working directory\n\
                 • Verify the path is correct (use absolute path if needed)",
                pipelines_dir,
                pipelines_dir
            ));
        }

        let mut pipelines = vec![];

        for entry in std::fs::read_dir(dir)
            .with_context(|| format!("Failed to read pipelines directory: {}", dir.display()))?
        {
            let entry = entry.with_context(|| "Failed to process pipeline directory entry")?;

            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("yaml")
                || path.extension().and_then(|s| s.to_str()) == Some("yml")
            {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    pipelines.push(name.to_string());
                } else {
                    warn!(
                        "Skipping pipeline file with invalid name: {}",
                        path.display()
                    );
                }
            }
        }

        if pipelines.is_empty() {
            return Err(anyhow::anyhow!(
                "❌ No pipeline files found in directory: '{}'\n\
                 💡 Pipeline files should have .yaml or .yml extensions\n\
                 💡 Example pipeline files: pipeline1.yaml, daily_analysis.yml\n\
                 💡 Check the 'pipelines/' directory for examples",
                pipelines_dir
            ));
        }

        pipelines.sort();
        Ok(pipelines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_runner_creation() {
        let runner = PipelineRunner::new("polybot".to_string()).with_verbose(true);

        assert_eq!(runner.binary_name, "polybot");
        assert!(runner.verbose);
    }
}
