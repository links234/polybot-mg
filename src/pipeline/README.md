# Pipeline Module

The pipeline module provides a comprehensive workflow automation system that enables the orchestration of complex sequences of CLI commands. It implements a YAML-based configuration system with parameter resolution, state management, and robust error handling for automated data processing workflows.

## Core Purpose and Responsibilities

The pipeline module serves as the automation backbone for:
- **Workflow Orchestration**: Executing sequences of CLI commands in defined order
- **Parameter Management**: Dynamic parameter resolution with templating support  
- **State Tracking**: Progress monitoring and failure recovery
- **Configuration Management**: YAML-based pipeline definitions with validation
- **Environment Control**: Isolated execution environments for each step

## Architecture Overview

```
src/pipeline/
├── mod.rs          # Pipeline system interface and core types
├── config.rs       # Configuration management and discovery
├── runner.rs       # Pipeline execution engine
└── tui.rs          # Terminal UI for pipeline management
```

## Core Data Structures

### Pipeline Definition (`mod.rs`)

```rust
/// Pipeline configuration loaded from YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pipeline {
    /// Pipeline name
    pub name: String,
    /// Pipeline description
    pub description: String,
    /// Parameters that can be used in step arguments
    pub parameters: HashMap<String, String>,
    /// List of steps to execute
    pub steps: Vec<PipelineStep>,
}

/// A pipeline step that executes a CLI command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStep {
    /// Human-readable name for this step
    pub name: String,
    /// CLI command to execute (without the binary name)
    pub command: String,
    /// Arguments to pass to the command
    pub args: Vec<String>,
    /// Whether to continue on failure (default: false)
    pub continue_on_error: bool,
    /// Environment variables to set for this step
    pub env: HashMap<String, String>,
}
```

### Execution Context

```rust
/// Pipeline execution context with resolved parameters
#[derive(Debug, Clone)]
pub struct PipelineContext {
    /// Resolved parameters (including date variables)
    pub parameters: HashMap<String, String>,
    /// Whether to run in dry-run mode
    pub dry_run: bool,
}
```

## Parameter System

### Dynamic Parameter Resolution

The pipeline system supports sophisticated parameter templating:

```rust
impl Pipeline {
    /// Create execution context with resolved parameters
    pub fn create_context(&self, extra_params: HashMap<String, String>) -> PipelineContext {
        let mut parameters = self.parameters.clone();
        
        // Add date parameters
        let now = Local::now();
        parameters.insert("date".to_string(), now.format("%Y-%m-%d").to_string());
        parameters.insert("datetime".to_string(), now.format("%Y-%m-%d_%H-%M-%S").to_string());
        parameters.insert("timestamp".to_string(), now.timestamp().to_string());
        
        // Add pipeline-specific parameters
        parameters.insert("pipeline_output_dir".to_string(), format!("{}/pipeline_{}_{}", 
            DEFAULT_RUNS_DIR, 
            pipeline_name_snake, 
            now.format("%Y%m%d_%H%M%S")));
        
        // Override with extra parameters
        for (key, value) in extra_params {
            parameters.insert(key, value);
        }
        
        PipelineContext {
            parameters,
            dry_run: false,
        }
    }

    /// Resolve parameter placeholders in a string
    pub fn resolve_parameters(&self, text: &str, context: &PipelineContext) -> String {
        let mut result = text.to_string();
        let mut changed = true;
        let mut iterations = 0;
        let max_iterations = 10; // Prevent infinite loops
        
        while changed && iterations < max_iterations {
            changed = false;
            for (key, value) in &context.parameters {
                let placeholder = format!("${{{}}}", key);
                if result.contains(&placeholder) {
                    result = result.replace(&placeholder, value);
                    changed = true;
                }
            }
            iterations += 1;
        }
        
        result
    }
}
```

### Built-in Parameters

The system automatically provides:
- **Date/Time**: `${date}`, `${datetime}`, `${timestamp}`, `${year}`, `${month}`, `${day}`
- **Directories**: `${datasets_dir}`, `${runs_dir}`, `${pipeline_output_dir}`
- **Pipeline Context**: `${pipeline_name}`, execution metadata

## Configuration Management (`config.rs`)

```rust
/// Configuration for pipeline execution
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Directory containing pipeline YAML files
    pub pipelines_dir: String,
}

impl PipelineConfig {
    /// Get the full path to a pipeline file
    pub fn pipeline_path(&self, name: &str) -> String {
        let yaml_path = Path::new(&self.pipelines_dir).join(format!("{}.yaml", name));
        let yml_path = Path::new(&self.pipelines_dir).join(format!("{}.yml", name));
        
        if yaml_path.exists() {
            yaml_path.to_string_lossy().to_string()
        } else if yml_path.exists() {
            yml_path.to_string_lossy().to_string()
        } else {
            yaml_path.to_string_lossy().to_string() // Default to .yaml
        }
    }
}
```

## Pipeline Execution Engine (`runner.rs`)

### Pipeline Runner

```rust
/// Pipeline runner that executes workflows
pub struct PipelineRunner {
    /// Binary name to execute (e.g., "polybot")
    pub binary_name: String,
    /// Whether to show verbose output
    pub verbose: bool,
}

impl PipelineRunner {
    /// Create a new pipeline runner with auto-detection
    pub fn new_auto() -> Self {
        let (binary_name, detection_info) = Self::detect_execution_method();
        
        info!("🔍 Detected execution method: {}", detection_info);
        info!("📦 Using command: {}", binary_name);
        
        Self {
            binary_name,
            verbose: false,
        }
    }
}
```

### Execution Method Detection

The runner automatically detects whether it's running through Cargo or as a standalone binary:

```rust
/// Detect whether we're running through cargo or as a standalone binary
fn detect_execution_method() -> (String, String) {
    let current_exe = std::env::current_exe();
    
    if let Ok(exe_path) = &current_exe {
        let exe_str = exe_path.to_string_lossy();
        
        if exe_str.contains("target/debug") || exe_str.contains("target/release") {
            return (
                "cargo run --".to_string(),
                format!("Cargo build detected ({})", exe_str)
            );
        }
        
        if let Some(file_name) = exe_path.file_name().and_then(|n| n.to_str()) {
            if file_name.starts_with("polybot") && !exe_str.contains("target") {
                return (
                    "polybot".to_string(),
                    format!("Standalone binary ({})", exe_str)
                );
            }
        }
    }
    
    // Fallback checks...
    ("polybot".to_string(), "Default assumed".to_string())
}
```

### Step Execution

```rust
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
                    warn!("⚠️  Step {}/{} failed but continuing: {}", step_num, stats.total_steps, e);
                } else {
                    error!("❌ Step {}/{} failed, stopping pipeline: {}", step_num, stats.total_steps, e);
                    return Err(e);
                }
            }
        }
    }

    stats.total_duration = start_time.elapsed();
    self.save_pipeline_metadata(pipeline, &context, &stats).await?;
    
    Ok(stats)
}
```

## Usage Examples

### Basic Pipeline Definition

```yaml
# pipelines/daily_analysis.yaml
name: "Daily Market Analysis"
description: "Fetch latest market data and perform analysis"

parameters:
  analysis_type: "comprehensive"
  output_format: "json"

steps:
  - name: "Fetch Latest Markets"
    command: "fetch-all-markets"
    args:
      - "--chunk-size"
      - "20"
      - "--output-dir"
      - "${pipeline_output_dir}/raw_data"
      - "--verbose"
    
  - name: "Analyze Active Markets"
    command: "analyze"
    args:
      - "--input-dir"
      - "${pipeline_output_dir}/raw_data"
      - "--output-dir"
      - "${pipeline_output_dir}/analysis"
      - "--min-volume"
      - "1000"
      - "--active-only"
    
  - name: "Generate Report"
    command: "datasets"
    args:
      - "list"
      - "--output-format"
      - "${output_format}"
      - "--save-to"
      - "${pipeline_output_dir}/report.json"
```

### Pipeline Execution

```rust
use crate::pipeline::{Pipeline, PipelineRunner, PipelineConfig};

// Load pipeline from file
let config = PipelineConfig::default();
let pipeline_path = config.pipeline_path("daily_analysis");
let pipeline = Pipeline::from_file(&pipeline_path)?;

// Create execution context with custom parameters
let mut extra_params = HashMap::new();
extra_params.insert("analysis_type".to_string(), "focused".to_string());

let context = pipeline.create_context(extra_params);

// Execute pipeline
let runner = PipelineRunner::new_auto().with_verbose(true);
let stats = runner.execute_pipeline(&pipeline, context).await?;

println!("Pipeline completed: {}/{} steps successful", 
    stats.successful_steps, stats.total_steps);
```

### Dry Run Mode

```rust
// Test pipeline without execution
let mut context = pipeline.create_context(HashMap::new());
context.dry_run = true;

let stats = runner.execute_pipeline(&pipeline, context).await?;
// Shows resolved commands without executing them
```

## Integration Patterns

### With CLI Commands

Pipeline functionality is exposed through CLI:

```rust
// In src/cli/commands/pipeline.rs
use crate::pipeline::{Pipeline, PipelineRunner, PipelineConfig};

#[derive(Parser)]
pub struct PipelineCommand {
    #[command(subcommand)]
    pub action: PipelineAction,
}

#[derive(Subcommand)]
pub enum PipelineAction {
    /// List available pipelines
    List,
    /// Run a specific pipeline
    Run {
        /// Pipeline name
        name: String,
        /// Additional parameters
        #[arg(short, long)]
        param: Vec<String>,
        /// Dry run mode
        #[arg(long)]
        dry_run: bool,
    },
}
```

### With Dataset Management

Pipeline outputs integrate with the dataset system:

```rust
// Save pipeline metadata
let mut additional_info = HashMap::new();
additional_info.insert("dataset_type".to_string(), 
    serde_json::json!(format!("Pipeline({})", pipeline.name)));
additional_info.insert("pipeline_name".to_string(), 
    serde_json::json!(pipeline.name));

save_command_metadata(&pipeline_path, "pipeline", &[pipeline.name.clone()], 
    Some(additional_info))?;
```

### With TUI System

Pipelines can be managed through the terminal interface:

```rust
// In src/pipeline/tui.rs
use ratatui::{prelude::*, widgets::*};

pub fn render_pipeline_list(frame: &mut Frame, pipelines: &[String], selected: usize) {
    let items: Vec<ListItem> = pipelines.iter()
        .enumerate()
        .map(|(i, name)| {
            let style = if i == selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };
            ListItem::new(name.as_str()).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default()
            .borders(Borders::ALL)
            .title("Available Pipelines"));
    
    frame.render_widget(list, frame.size());
}
```

## Error Handling and Recovery

### Comprehensive Error Context

```rust
// Enhanced error messages with suggestions
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
```

### Recovery Strategies

- **Continue on Error**: Individual steps can be marked to continue on failure
- **State Preservation**: Pipeline progress is saved for manual recovery
- **Partial Success**: Successful steps are recorded even if later steps fail
- **Metadata Tracking**: Complete execution context is preserved

## Performance Considerations

### Resource Management

```rust
/// Pipeline execution statistics
#[derive(Debug, Default)]
pub struct PipelineStats {
    pub total_steps: usize,
    pub successful_steps: usize,
    pub failed_steps: usize,
    pub skipped_steps: usize,
    pub total_duration: Duration,
}
```

### Parallel Execution

While currently sequential, the architecture supports future parallelization:

```rust
// Future enhancement: parallel step groups
pub struct StepGroup {
    pub steps: Vec<PipelineStep>,
    pub execution_mode: ExecutionMode,
}

pub enum ExecutionMode {
    Sequential,
    Parallel { max_concurrency: usize },
    Conditional { condition: String },
}
```

## Advanced Features

### Environment Variable Support

```rust
// Set environment variables for specific steps
env:
  ANALYSIS_MODE: "detailed"
  OUTPUT_FORMAT: "${output_format}"
  TEMP_DIR: "${pipeline_output_dir}/temp"
```

### Conditional Execution

```rust
// Future: conditional step execution
- name: "Process Large Dataset"
  command: "analyze"
  condition: "${dataset_size} > 1000000"
  args: ["--optimize-memory"]
```

### Pipeline Composition

```rust
// Future: pipeline inclusion
includes:
  - "common/setup.yaml"
  - "common/cleanup.yaml"
```

The pipeline module provides a robust foundation for workflow automation while maintaining the flexibility to handle complex data processing scenarios. Its integration with the broader Polybot ecosystem ensures that automated workflows can leverage all available CLI functionality while providing comprehensive monitoring and error handling.