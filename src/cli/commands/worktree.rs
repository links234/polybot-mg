//! Git worktree helper command for managing multiple feature branches

use anyhow::{Result, anyhow, Context};
use clap::Args;
use std::path::{Path, PathBuf};
use std::fs;
use tracing::{info, warn, debug};

use crate::data_paths::DataPaths;

#[derive(Args, Debug)]
pub struct WorktreeArgs {
    /// Worktree subcommand
    #[command(subcommand)]
    pub command: WorktreeCommand,
}

#[derive(clap::Subcommand, Debug)]
pub enum WorktreeCommand {
    /// Create a new worktree with data and .env setup
    Create {
        /// Branch name for the new worktree
        branch: String,
        
        /// Custom path for worktree (default: ../polybot-{branch})
        #[arg(short, long)]
        path: Option<String>,
        
        /// Base branch to create from (default: main)
        #[arg(short, long, default_value = "main")]
        base: String,
        
        /// Don't copy data directory
        #[arg(long)]
        no_data: bool,
        
        /// Don't copy .env files
        #[arg(long)]
        no_env: bool,
        
        /// Don't copy credentials
        #[arg(long)]
        no_creds: bool,
    },
    
    /// List all worktrees
    List,
    
    /// Remove a worktree and its data
    Remove {
        /// Path to worktree to remove
        path: String,
        
        /// Force removal even if dirty
        #[arg(short, long)]
        force: bool,
    },
    
    /// Sync data from main worktree to current worktree
    Sync {
        /// Source worktree path (default: detect main)
        #[arg(short, long)]
        source: Option<String>,
        
        /// What to sync: data, env, creds, all
        #[arg(short, long, default_value = "data")]
        what: String,
    },
}

pub async fn worktree(args: WorktreeArgs, _host: &str, data_paths: DataPaths) -> Result<()> {
    match args.command {
        WorktreeCommand::Create { branch, path, base, no_data, no_env, no_creds } => {
            create_worktree(&branch, path.as_deref(), &base, !no_data, !no_env, !no_creds, &data_paths).await
        }
        WorktreeCommand::List => {
            list_worktrees().await
        }
        WorktreeCommand::Remove { path, force } => {
            remove_worktree(&path, force).await
        }
        WorktreeCommand::Sync { source, what } => {
            sync_worktree(source.as_deref(), &what, &data_paths).await
        }
    }
}

async fn create_worktree(
    branch: &str, 
    custom_path: Option<&str>, 
    base: &str,
    copy_data: bool,
    copy_env: bool, 
    copy_creds: bool,
    data_paths: &DataPaths
) -> Result<()> {
    println!("🌳 Creating new worktree for branch '{}'", branch);
    
    // Determine worktree path
    let worktree_path = if let Some(custom) = custom_path {
        PathBuf::from(custom)
    } else {
        // Default to ../polybot-{branch}
        let current_dir = std::env::current_dir()?;
        let parent = current_dir.parent()
            .ok_or_else(|| anyhow!("Cannot determine parent directory"))?;
        parent.join(format!("polybot-{}", branch))
    };
    
    info!("Worktree path: {}", worktree_path.display());
    
    // Check if worktree path already exists
    if worktree_path.exists() {
        return Err(anyhow!("Path already exists: {}", worktree_path.display()));
    }
    
    // Create git worktree
    println!("📂 Creating git worktree...");
    let output = std::process::Command::new("git")
        .args(["worktree", "add", "-b", branch, worktree_path.to_str().unwrap(), base])
        .output()
        .context("Failed to execute git worktree command")?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Git worktree creation failed: {}", stderr));
    }
    
    println!("✅ Git worktree created successfully");
    
    // Copy data directory if requested
    if copy_data {
        println!("📁 Copying data directory...");
        let source_data = data_paths.root();
        let target_data = worktree_path.join("data");
        
        if source_data.exists() {
            copy_directory_recursive(source_data, &target_data)
                .context("Failed to copy data directory")?;
            println!("✅ Data directory copied");
        } else {
            warn!("Source data directory doesn't exist: {}", source_data.display());
        }
    }
    
    // Copy .env files if requested
    if copy_env {
        println!("🔧 Copying environment files...");
        copy_env_files(&worktree_path)?;
    }
    
    // Copy credentials if requested
    if copy_creds {
        println!("🔐 Copying credentials...");
        copy_credentials(&data_paths.auth(), &worktree_path.join("data").join("auth"))?;
    }
    
    // Create helpful README
    create_worktree_readme(&worktree_path, branch, base)?;
    
    println!("\n🎉 Worktree setup complete!");
    println!("📍 Location: {}", worktree_path.display());
    println!("🚀 To start working:");
    println!("   cd {}", worktree_path.display());
    println!("   cargo run -- --help");
    
    Ok(())
}

async fn list_worktrees() -> Result<()> {
    println!("🌳 Git Worktrees:\n");
    
    let output = std::process::Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .output()
        .context("Failed to execute git worktree list")?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Git worktree list failed: {}", stderr));
    }
    
    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut current_worktree: Option<WorktreeInfo> = None;
    
    for line in output_str.lines() {
        if line.starts_with("worktree ") {
            if let Some(info) = current_worktree.take() {
                print_worktree_info(&info)?;
            }
            current_worktree = Some(WorktreeInfo {
                path: line.strip_prefix("worktree ").unwrap().to_string(),
                head: None,
                branch: None,
                bare: false,
            });
        } else if line.starts_with("HEAD ") {
            if let Some(ref mut info) = current_worktree {
                info.head = Some(line.strip_prefix("HEAD ").unwrap().to_string());
            }
        } else if line.starts_with("branch ") {
            if let Some(ref mut info) = current_worktree {
                info.branch = Some(line.strip_prefix("branch ").unwrap().to_string());
            }
        } else if line == "bare" {
            if let Some(ref mut info) = current_worktree {
                info.bare = true;
            }
        }
    }
    
    // Print the last worktree
    if let Some(info) = current_worktree {
        print_worktree_info(&info)?;
    }
    
    Ok(())
}

#[derive(Debug)]
struct WorktreeInfo {
    path: String,
    head: Option<String>,
    branch: Option<String>,
    bare: bool,
}

fn print_worktree_info(info: &WorktreeInfo) -> Result<()> {
    let path = Path::new(&info.path);
    let exists = path.exists();
    let is_main = info.branch.as_ref().map_or(false, |b| b.ends_with("/main") || b.ends_with("/master"));
    
    let status_icon = if !exists {
        "❌"
    } else if is_main {
        "🏠"
    } else {
        "🌿"
    };
    
    let branch_name = info.branch.as_ref()
        .map(|b| b.split('/').last().unwrap_or(b))
        .unwrap_or("(detached)");
    
    println!("{} {} ({})", status_icon, info.path, branch_name);
    
    if exists {
        // Check if data directory exists
        let data_dir = path.join("data");
        if data_dir.exists() {
            let data_size = get_directory_size(&data_dir)?;
            println!("   📁 Data: {} ({})", data_dir.display(), format_size(data_size));
        } else {
            println!("   📁 Data: ❌ Not found");
        }
        
        // Check for .env files
        let env_files = [".env", ".env.local", ".env.production"]
            .iter()
            .filter(|&env_file| path.join(env_file).exists())
            .count();
        
        if env_files > 0 {
            println!("   🔧 Environment: {} file(s)", env_files);
        } else {
            println!("   🔧 Environment: ❌ No .env files");
        }
        
        // Check for credentials
        let creds_dir = path.join("data").join("auth");
        if creds_dir.exists() {
            println!("   🔐 Credentials: ✅ Available");
        } else {
            println!("   🔐 Credentials: ❌ Not found");
        }
    } else {
        println!("   ⚠️  Directory not found (stale worktree)");
    }
    
    println!();
    Ok(())
}

async fn remove_worktree(path: &str, force: bool) -> Result<()> {
    let worktree_path = Path::new(path);
    
    if !worktree_path.exists() {
        return Err(anyhow!("Worktree path does not exist: {}", path));
    }
    
    println!("🗑️  Removing worktree: {}", path);
    
    // Check if worktree is dirty
    if !force {
        let output = std::process::Command::new("git")
            .args(["-C", path, "status", "--porcelain"])
            .output()
            .context("Failed to check git status")?;
        
        if !output.stdout.is_empty() {
            return Err(anyhow!(
                "Worktree has uncommitted changes. Use --force to remove anyway.\n\
                 Or commit/stash changes first."
            ));
        }
    }
    
    // Remove from git
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(path);
    
    let output = std::process::Command::new("git")
        .args(&args)
        .output()
        .context("Failed to remove git worktree")?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Git worktree removal failed: {}", stderr));
    }
    
    println!("✅ Worktree removed successfully");
    Ok(())
}

async fn sync_worktree(source: Option<&str>, what: &str, _data_paths: &DataPaths) -> Result<()> {
    let current_dir = std::env::current_dir()?;
    
    // Determine source worktree
    let source_path = if let Some(src) = source {
        PathBuf::from(src)
    } else {
        // Try to find main worktree
        find_main_worktree()?
    };
    
    if !source_path.exists() {
        return Err(anyhow!("Source worktree not found: {}", source_path.display()));
    }
    
    println!("🔄 Syncing from: {}", source_path.display());
    println!("🎯 Syncing to: {}", current_dir.display());
    
    match what {
        "data" => {
            println!("📁 Syncing data directory...");
            let source_data = source_path.join("data");
            let target_data = current_dir.join("data");
            
            if source_data.exists() {
                // Remove target first
                if target_data.exists() {
                    fs::remove_dir_all(&target_data)?;
                }
                copy_directory_recursive(&source_data, &target_data)?;
                println!("✅ Data synced");
            } else {
                warn!("Source data directory not found");
            }
        }
        "env" => {
            println!("🔧 Syncing environment files...");
            copy_env_files(&current_dir)?;
            println!("✅ Environment files synced");
        }
        "creds" => {
            println!("🔐 Syncing credentials...");
            let source_auth = source_path.join("data").join("auth");
            let target_auth = current_dir.join("data").join("auth");
            
            if source_auth.exists() {
                if target_auth.exists() {
                    fs::remove_dir_all(&target_auth)?;
                }
                copy_directory_recursive(&source_auth, &target_auth)?;
                println!("✅ Credentials synced");
            } else {
                warn!("Source credentials not found");
            }
        }
        "all" => {
            // Sync all types directly to avoid recursion
            println!("📁 Syncing data directory...");
            let source_data = source_path.join("data");
            let target_data = current_dir.join("data");
            
            if source_data.exists() {
                // Remove target first
                if target_data.exists() {
                    fs::remove_dir_all(&target_data)?;
                }
                copy_directory_recursive(&source_data, &target_data)?;
                println!("✅ Data synced");
            } else {
                warn!("Source data directory not found");
            }
            
            println!("🔧 Syncing environment files...");
            copy_env_files(&current_dir)?;
            println!("✅ Environment files synced");
            
            println!("🔐 Syncing credentials...");
            let source_auth = source_path.join("data").join("auth");
            let target_auth = current_dir.join("data").join("auth");
            
            if source_auth.exists() {
                if target_auth.exists() {
                    fs::remove_dir_all(&target_auth)?;
                }
                copy_directory_recursive(&source_auth, &target_auth)?;
                println!("✅ Credentials synced");
            } else {
                warn!("Source credentials not found");
            }
        }
        _ => {
            return Err(anyhow!("Unknown sync type: {}. Use: data, env, creds, or all", what));
        }
    }
    
    Ok(())
}

fn find_main_worktree() -> Result<PathBuf> {
    let output = std::process::Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .output()
        .context("Failed to get worktree list")?;
    
    let output_str = String::from_utf8_lossy(&output.stdout);
    
    for chunk in output_str.split("\n\n") {
        let lines: Vec<&str> = chunk.lines().collect();
        if lines.is_empty() {
            continue;
        }
        
        let mut path = None;
        let mut is_main = false;
        
        for line in &lines {
            if line.starts_with("worktree ") {
                path = Some(line.strip_prefix("worktree ").unwrap());
            } else if line.starts_with("branch ") {
                let branch = line.strip_prefix("branch ").unwrap();
                if branch.ends_with("/main") || branch.ends_with("/master") {
                    is_main = true;
                }
            }
        }
        
        if is_main && path.is_some() {
            return Ok(PathBuf::from(path.unwrap()));
        }
    }
    
    Err(anyhow!("Could not find main worktree"))
}

fn copy_directory_recursive(src: &Path, dst: &Path) -> Result<()> {
    if !src.exists() {
        return Ok(());
    }
    
    fs::create_dir_all(dst)?;
    
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        
        if src_path.is_dir() {
            copy_directory_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    
    Ok(())
}

fn copy_env_files(target_dir: &Path) -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let env_files = [".env", ".env.local", ".env.production", ".env.example"];
    
    for env_file in &env_files {
        let source = current_dir.join(env_file);
        let target = target_dir.join(env_file);
        
        if source.exists() {
            fs::copy(&source, &target)
                .context(format!("Failed to copy {}", env_file))?;
            debug!("Copied: {}", env_file);
        }
    }
    
    Ok(())
}

fn copy_credentials(source_auth: &Path, target_auth: &Path) -> Result<()> {
    if !source_auth.exists() {
        warn!("Source auth directory doesn't exist: {}", source_auth.display());
        return Ok(());
    }
    
    copy_directory_recursive(source_auth, target_auth)
        .context("Failed to copy credentials")?;
    
    Ok(())
}

fn create_worktree_readme(worktree_path: &Path, branch: &str, base: &str) -> Result<()> {
    let readme_content = format!(
        r#"# Polybot Worktree: {}

This is a git worktree for the `{}` branch, created from `{}`.

## Quick Start

```bash
# Install dependencies
cargo build

# Run portfolio command
cargo run -- portfolio

# Run any command with help
cargo run -- --help
```

## Worktree Management

```bash
# List all worktrees
cargo run -- worktree list

# Sync data from main worktree
cargo run -- worktree sync --what all

# Remove this worktree
cargo run -- worktree remove {}
```

## Data Structure

- `data/` - Application data (copied from main worktree)
- `.env*` - Environment files (copied from main worktree)
- `data/auth/` - API credentials (copied from main worktree)

## Important Notes

- This worktree shares git history but has independent working directory
- Data and credentials are copied, not linked
- Use `cargo run -- worktree sync` to update data from main branch
- Commit changes regularly to avoid conflicts

Created: {}
Branch: {}
Base: {}
"#,
        branch,
        branch,
        base,
        worktree_path.display(),
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        branch,
        base
    );
    
    let readme_path = worktree_path.join("WORKTREE.md");
    fs::write(readme_path, readme_content)
        .context("Failed to create worktree README")?;
    
    Ok(())
}

fn get_directory_size(dir: &Path) -> Result<u64> {
    let mut total_size = 0;
    
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                total_size += get_directory_size(&path)?;
            } else {
                total_size += entry.metadata()?.len();
            }
        }
    }
    
    Ok(total_size)
}

fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}