//! CLI command for managing token selections

use anyhow::Result;
use clap::{Args, Subcommand};

use crate::data_paths::DataPaths;
use crate::markets::datasets::SelectionManager;
use crate::tui::SelectionBuilder;

#[derive(Args, Clone)]
pub struct SelectionsArgs {
    #[command(subcommand)]
    pub command: Option<SelectionsSubcommand>,
}

#[derive(Subcommand, Clone)]
pub enum SelectionsSubcommand {
    /// List all available selections
    List,
    
    /// Create a new selection interactively
    Create {
        /// Optional name for the selection (prompts if not provided)
        #[arg(long)]
        name: Option<String>,
    },
    
    /// Show details of a specific selection
    Show {
        /// Name of the selection
        name: String,
    },
    
    /// Delete a selection
    Delete {
        /// Name of the selection
        name: String,
        
        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },
    
    /// Add tokens to an existing selection
    Add {
        /// Name of the selection
        name: String,
        
        /// Token IDs to add (comma-separated)
        #[arg(value_delimiter = ',')]
        tokens: Vec<String>,
    },
    
    /// Remove tokens from a selection
    Remove {
        /// Name of the selection
        name: String,
        
        /// Token IDs to remove (comma-separated)
        #[arg(value_delimiter = ',')]
        tokens: Vec<String>,
    },
    
    /// Export selection tokens (for use with stream command)
    Export {
        /// Name of the selection
        name: String,
        
        /// Output format
        #[arg(long, default_value = "list")]
        format: ExportFormat,
    },
}

#[derive(Clone, Debug)]
pub enum ExportFormat {
    List,
    CommaSeparated,
    Json,
}

impl std::str::FromStr for ExportFormat {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "list" => Ok(ExportFormat::List),
            "comma" | "csv" => Ok(ExportFormat::CommaSeparated),
            "json" => Ok(ExportFormat::Json),
            _ => Err(format!("Unknown format: {}", s)),
        }
    }
}

pub struct SelectionsCommand {
    args: SelectionsArgs,
}

impl SelectionsCommand {
    pub fn new(args: SelectionsArgs) -> Self {
        Self { args }
    }
    
    pub async fn execute(&self, _host: &str, data_paths: DataPaths) -> Result<()> {
        match &self.args.command {
            None => {
                // No subcommand - show interactive menu
                self.show_interactive_menu(&data_paths).await
            }
            Some(command) => {
                // Execute specific subcommand
                let manager = SelectionManager::new(&data_paths.data());
                match command {
                    SelectionsSubcommand::List => self.list_selections_interactive(&data_paths).await,
                    SelectionsSubcommand::Create { name } => self.create_selection(&manager, name.clone()).await,
                    SelectionsSubcommand::Show { name } => self.show_selection(&manager, name).await,
                    SelectionsSubcommand::Delete { name, force } => self.delete_selection(&manager, name, *force).await,
                    SelectionsSubcommand::Add { name, tokens } => self.add_tokens(&manager, name, tokens).await,
                    SelectionsSubcommand::Remove { name, tokens } => self.remove_tokens(&manager, name, tokens).await,
                    SelectionsSubcommand::Export { name, format } => self.export_selection(&manager, name, format).await,
                }
            }
        }
    }
    
    
    async fn create_selection(&self, manager: &SelectionManager, name: Option<String>) -> Result<()> {
        // Launch the TUI selection builder
        match SelectionBuilder::run(name.clone()).await {
            Ok(result) => {
                if result.cancelled {
                    println!("Selection creation cancelled.");
                    return Ok(());
                }
                
                let selection = manager.create_selection(
                    result.name.clone(),
                    result.description,
                    result.tokens,
                )?;
                
                manager.save_selection(&selection)?;
                
                println!("✅ Created selection '{}' with {} tokens", result.name, selection.tokens.len());
                println!("\nStream this selection with: polybot stream --selection {}", result.name);
                
                Ok(())
            }
            Err(_) => {
                // TUI not available
                if let Some(selection_name) = name {
                    // Create an empty selection with the given name
                    let selection = manager.create_selection(
                        selection_name.clone(),
                        Some("Created via CLI".to_string()),
                        Vec::new(),
                    )?;
                    
                    manager.save_selection(&selection)?;
                    
                    println!("✅ Created empty selection '{}'", selection_name);
                    println!("\nUse 'polybot selections add {} <token1,token2,...>' to add tokens", selection_name);
                    
                    Ok(())
                } else {
                    println!("❌ Error: Interactive mode not available.");
                    println!("Please provide a name with --name to create a selection in non-interactive mode.");
                    Err(anyhow::anyhow!("Interactive mode not available and no name provided"))
                }
            }
        }
    }
    
    async fn show_selection(&self, manager: &SelectionManager, name: &str) -> Result<()> {
        let selection = manager.load_selection(name)?;
        
        println!("Token Selection: {}", selection.name);
        println!("================");
        
        if let Some(desc) = &selection.description {
            println!("Description: {}", desc);
        }
        
        println!("Created: {}", selection.created_at.format("%Y-%m-%d %H:%M:%S UTC"));
        println!("Modified: {}", selection.modified_at.format("%Y-%m-%d %H:%M:%S UTC"));
        
        if !selection.tags.is_empty() {
            println!("Tags: {}", selection.tags.join(", "));
        }
        
        println!("\nTokens ({}):", selection.tokens.len());
        println!("-----------");
        
        for (i, token_info) in selection.tokens.iter().enumerate() {
            print!("{:3}. {}", i + 1, token_info.token_id);
            
            if let Some(name) = &token_info.name {
                print!(" - {}", name);
            }
            
            if let Some(market) = &token_info.market {
                print!(" ({})", market);
            }
            
            println!();
        }
        
        if let Some(notes) = &selection.metadata.notes {
            println!("\nNotes: {}", notes);
        }
        
        Ok(())
    }
    
    async fn delete_selection(&self, manager: &SelectionManager, name: &str, force: bool) -> Result<()> {
        if !force {
            println!("Are you sure you want to delete selection '{}'? (y/N)", name);
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            
            if !input.trim().to_lowercase().starts_with('y') {
                println!("Deletion cancelled.");
                return Ok(());
            }
        }
        
        manager.delete_selection(name)?;
        println!("✅ Deleted selection '{}'", name);
        
        Ok(())
    }
    
    async fn add_tokens(&self, manager: &SelectionManager, name: &str, tokens: &[String]) -> Result<()> {
        manager.add_tokens(name, tokens.to_vec())?;
        
        let selection = manager.load_selection(name)?;
        println!("✅ Added {} tokens to '{}' (now {} total)", 
                 tokens.len(), name, selection.tokens.len());
        
        Ok(())
    }
    
    async fn remove_tokens(&self, manager: &SelectionManager, name: &str, tokens: &[String]) -> Result<()> {
        manager.remove_tokens(name, tokens)?;
        
        let selection = manager.load_selection(name)?;
        println!("✅ Removed tokens from '{}' (now {} total)", 
                 name, selection.tokens.len());
        
        Ok(())
    }
    
    async fn export_selection(&self, manager: &SelectionManager, name: &str, format: &ExportFormat) -> Result<()> {
        let tokens = manager.get_tokens(name)?;
        
        match format {
            ExportFormat::List => {
                for token in tokens {
                    println!("{}", token);
                }
            },
            ExportFormat::CommaSeparated => {
                println!("{}", tokens.join(","));
            },
            ExportFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&tokens)?);
            },
        }
        
        Ok(())
    }
    
    async fn show_interactive_menu(&self, data_paths: &DataPaths) -> Result<()> {
        use crate::tui::{SelectionsMenu, SelectionsMenuResult};
        
        // Try to run interactive menu
        loop {
            match SelectionsMenu::run(data_paths).await {
                Ok(result) => {
                    match result {
                        SelectionsMenuResult::Exit => {
                            println!("Exiting selections manager.");
                            return Ok(());
                        }
                        SelectionsMenuResult::CreateNew(result) => {
                            self.handle_selection_creation(result, data_paths)?;
                            // Continue showing the menu
                        }
                    }
                }
                Err(_) => {
                    // Fall back to non-interactive list
                    println!("Interactive mode not available. Showing list of selections:\n");
                    return self.list_selections_non_interactive(data_paths).await;
                }
            }
        }
    }
    
    async fn list_selections_interactive(&self, data_paths: &DataPaths) -> Result<()> {
        use crate::tui::{SelectionsMenu, SelectionsMenuResult};
        
        // Try to use interactive menu, fall back to non-interactive list if TUI fails
        match SelectionsMenu::run(data_paths).await {
            Ok(result) => {
                match result {
                    SelectionsMenuResult::CreateNew(result) => {
                        self.handle_selection_creation(result, data_paths)?;
                    }
                    _ => {}
                }
                Ok(())
            }
            Err(_) => {
                // Fall back to non-interactive list
                self.list_selections_non_interactive(data_paths).await
            }
        }
    }
    
    async fn list_selections_non_interactive(&self, data_paths: &DataPaths) -> Result<()> {
        let manager = SelectionManager::new(&data_paths.data());
        let selections = manager.list_all_selections()?;
        
        if selections.is_empty() {
            println!("No selections found.");
            println!("\nCreate a new selection with: polybot selections create");
            return Ok(());
        }
        
        println!("Available Token Selections:");
        println!("==========================");
        
        // Get detailed info to distinguish explicit vs implicit
        let selections_info = manager.get_all_selections_info()?;
        
        for (selection, is_explicit) in selections_info {
            let indicator = if is_explicit { "⭐" } else { "🔍" };
            let source = if is_explicit { "User-created" } else { "Auto-discovered" };
            
            println!("\n{} {}", indicator, selection.name);
            println!("   Tokens: {} ({})", selection.tokens.len(), source);
            if let Some(desc) = &selection.description {
                println!("   Description: {}", desc);
            }
            println!("   Created: {}", selection.created_at.format("%Y-%m-%d %H:%M"));
            if !selection.tags.is_empty() {
                println!("   Tags: {}", selection.tags.join(", "));
            }
        }
        
        println!("\nUse 'polybot selections show <name>' for details");
        println!("Use 'polybot stream --selection <name>' to stream a selection");
        
        Ok(())
    }
    
    fn handle_selection_creation(&self, result: crate::tui::SelectionBuilderResult, data_paths: &DataPaths) -> Result<()> {
        if !result.cancelled {
            let manager = SelectionManager::new(&data_paths.data());
            let selection = manager.create_selection(
                result.name.clone(),
                result.description,
                result.tokens,
            )?;
            
            manager.save_selection(&selection)?;
            println!("✅ Created selection '{}' with {} tokens", result.name, selection.tokens.len());
            println!("\nStream this selection with: polybot stream --selection {}", result.name);
        }
        Ok(())
    }
}