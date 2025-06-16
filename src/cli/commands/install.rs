//! Install command for setting up polybot system-wide

use anyhow::Result;
use clap::Args;
use owo_colors::OwoColorize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::data_paths::DataPaths;

#[derive(Args, Clone)]
pub struct InstallArgs {
    /// Overwrite existing installation without prompting
    #[arg(long)]
    pub overwrite: bool,
    
    /// Custom installation directory (defaults to ~/.local/bin)
    #[arg(long)]
    pub bin_dir: Option<String>,
}

pub struct InstallCommand {
    args: InstallArgs,
}

impl InstallCommand {
    pub fn new(args: InstallArgs) -> Self {
        Self { args }
    }

    pub async fn execute(&self, _host: &str, data_paths: DataPaths) -> Result<()> {
        println!("{}", "🔧 Polybot Installation Setup".bright_blue().bold());
        println!("═══════════════════════════════════════");

        // Determine installation directory
        let install_dir = self.get_install_directory()?;
        let target_path = install_dir.join("polybot");

        // Ensure bin directory exists
        if !install_dir.exists() {
            println!("📁 Creating installation directory: {}", install_dir.display());
            fs::create_dir_all(&install_dir)?;
        }

        // Check if already installed
        if target_path.exists() && !self.args.overwrite {
            println!("⚠️  {} already exists", target_path.display().to_string().yellow());
            
            if !self.prompt_overwrite()? {
                println!("{}", "❌ Installation cancelled by user".red());
                return Ok(());
            }
        }

        // Get current executable path
        let current_exe = env::current_exe()?;
        println!("📋 Source: {}", current_exe.display());
        println!("📋 Target: {}", target_path.display());

        // Copy executable
        println!("📦 Copying executable...");
        fs::copy(&current_exe, &target_path)?;

        // Make executable on Unix systems
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&target_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&target_path, perms)?;
            println!("🔐 Set executable permissions");
        }

        // Set up data directories
        println!("📁 Setting up data directories...");
        self.setup_data_directories(&data_paths)?;

        // Check PATH
        self.check_path_setup(&install_dir)?;

        println!("{}", "✅ Installation completed successfully!".bright_green().bold());
        println!();
        println!("{}", "🚀 Getting Started:".bright_cyan().bold());
        println!("   1. Restart your terminal or run: source ~/.bashrc");
        println!("   2. Verify installation: polybot --version");
        println!("   3. Initialize authentication: polybot init --pk YOUR_PRIVATE_KEY");
        println!("   4. Browse markets: polybot markets");
        println!();
        
        Ok(())
    }

    fn get_install_directory(&self) -> Result<PathBuf> {
        if let Some(custom_dir) = &self.args.bin_dir {
            return Ok(PathBuf::from(custom_dir));
        }

        // Try ~/.local/bin first (preferred)
        if let Some(home) = dirs::home_dir() {
            let local_bin = home.join(".local").join("bin");
            return Ok(local_bin);
        }

        // Fallback to /usr/local/bin if no home directory
        Ok(PathBuf::from("/usr/local/bin"))
    }

    fn prompt_overwrite(&self) -> Result<bool> {
        use std::io::{self, Write};

        print!("Do you want to overwrite the existing installation? [y/N]: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let response = input.trim().to_lowercase();
        Ok(response == "y" || response == "yes")
    }

    fn setup_data_directories(&self, data_paths: &DataPaths) -> Result<()> {
        // Create essential directories
        let dirs_to_create = [
            data_paths.datasets(),
            data_paths.logs(),
            data_paths.datasets().parent().unwrap().join("config"),
        ];

        for dir in &dirs_to_create {
            if !dir.exists() {
                fs::create_dir_all(dir)?;
                println!("   📂 Created: {}", dir.display());
            }
        }

        Ok(())
    }

    fn check_path_setup(&self, install_dir: &Path) -> Result<()> {
        let install_dir_str = install_dir.to_string_lossy();
        
        // Check if directory is in PATH
        if let Ok(path_var) = env::var("PATH") {
            if path_var.split(':').any(|p| p == install_dir_str) {
                println!("✅ {} is already in your PATH", install_dir.display());
                return Ok(());
            }
        }

        // Directory not in PATH, provide setup instructions
        println!("{}", "⚠️  PATH Setup Required".yellow().bold());
        println!("The installation directory is not in your PATH.");
        println!();
        println!("{}", "To add it to your PATH, run one of these commands:".cyan());
        println!();

        // Detect shell and provide appropriate command
        let shell_info = self.detect_shell();
        match shell_info.as_str() {
            "bash" => {
                println!("For Bash:");
                println!("   echo 'export PATH=\"{}:$PATH\"' >> ~/.bashrc", install_dir_str);
            }
            "zsh" => {
                println!("For Zsh:");
                println!("   echo 'export PATH=\"{}:$PATH\"' >> ~/.zshrc", install_dir_str);
            }
            "fish" => {
                println!("For Fish:");
                println!("   fish_add_path {}", install_dir_str);
            }
            _ => {
                println!("For your shell, add this to your shell configuration file:");
                println!("   export PATH=\"{}:$PATH\"", install_dir_str);
            }
        }

        println!();
        println!("Or temporarily add to current session:");
        println!("   export PATH=\"{}:$PATH\"", install_dir_str);

        Ok(())
    }

    fn detect_shell(&self) -> String {
        // Try to detect shell from SHELL environment variable
        if let Ok(shell_path) = env::var("SHELL") {
            if let Some(shell_name) = Path::new(&shell_path).file_name() {
                if let Some(shell_str) = shell_name.to_str() {
                    return shell_str.to_string();
                }
            }
        }

        // Fallback detection methods
        if env::var("ZSH_VERSION").is_ok() {
            return "zsh".to_string();
        }
        if env::var("BASH_VERSION").is_ok() {
            return "bash".to_string();
        }

        "unknown".to_string()
    }
}