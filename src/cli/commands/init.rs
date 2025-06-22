use anyhow::Result;
use clap::Args;
use tracing::info;
use crate::data_paths::DataPaths;

#[derive(Args)]
pub struct InitArgs {
    /// Private key in hex format (without 0x prefix)
    #[arg(long = "pk")]
    pub private_key: String,
    
    /// Nonce for API key derivation (default: 0)
    #[arg(long, default_value = "0")]
    pub nonce: u64,
}

pub struct InitCommand {
    args: InitArgs,
}

impl InitCommand {
    pub fn new(args: InitArgs) -> Self {
        Self { args }
    }

    pub async fn execute(&self, host: &str, data_paths: DataPaths) -> Result<()> {
        info!("🔐 Initializing Polymarket authentication...");
        crate::auth::init_auth(host, &data_paths, &self.args.private_key, self.args.nonce).await?;
        info!("✅ Authentication successful! Credentials saved.");
        
        // Auto-add user's address to address book
        if let Err(e) = self.init_address_book(host, &data_paths).await {
            info!("Note: Could not initialize address book: {}", e);
        }
        
        Ok(())
    }
    
    async fn init_address_book(&self, _host: &str, data_paths: &DataPaths) -> Result<()> {
        use crate::ethereum_utils::derive_address_from_private_key;
        use crate::address_book::{AddressBookCommand, types::AddressType};
        use tokio::sync::oneshot;
        
        // Derive address from private key
        let address = derive_address_from_private_key(&self.args.private_key)?;
        
        // Get address service without portfolio service to avoid circular dependency
        let address_service = crate::address_book::service::get_address_book_service(
            data_paths.clone(),
            None,
            None, // No gamma tracker needed for init
        ).await?;
        
        // Check if address already exists
        let (tx, rx) = oneshot::channel();
        address_service.send(AddressBookCommand::GetAddress {
            address_or_label: address.clone(),
            response: tx,
        }).await?;
        
        if rx.await??.is_none() {
            // Add the address
            let (tx, rx) = oneshot::channel();
            address_service.send(AddressBookCommand::AddAddress {
                address: address.clone(),
                label: Some("my-wallet".to_string()),
                description: Some("Primary trading wallet".to_string()),
                address_type: AddressType::Own,
                tags: vec!["primary".to_string()],
                response: tx,
            }).await?;
            
            if let Ok(_entry) = rx.await? {
                info!("📖 Added your wallet to address book as 'my-wallet'");
                
                // Set as current
                let (tx, rx) = oneshot::channel();
                address_service.send(AddressBookCommand::SetCurrentAddress {
                    address,
                    response: tx,
                }).await?;
                let _ = rx.await?;
            }
        } else {
            info!("📖 Your wallet is already in the address book");
        }
        
        Ok(())
    }
} 