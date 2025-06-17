use anyhow::{Result, anyhow};
use polymarket_rs_client::ClobClient;
use tracing::info;

use crate::config;
use crate::data_paths::DataPaths;

/// Initialize authentication with L1 wallet and save L2 API credentials
pub async fn init_auth(host: &str, data_paths: &DataPaths, private_key_hex: &str, nonce: u64) -> Result<()> {
    // Remove 0x prefix if present
    let private_key_hex = private_key_hex.trim_start_matches("0x");
    
    // Create client with L1 headers (this automatically adds POLY_* headers)
    let client = ClobClient::with_l1_headers(host, private_key_hex, 137);
    
    info!("📡 Performing L1 authentication...");
    
    // Create or derive API key (L2 credentials)
    // Convert nonce to U256
    let nonce_u256 = if nonce == 0 {
        None
    } else {
        Some(polymarket_rs_client::U256::from(nonce))
    };
    
    let api_creds = client.create_or_derive_api_key(nonce_u256).await
        .map_err(|e| anyhow!("Failed to create/derive API key: {}", e))?;
    
    info!("🔑 API credentials obtained successfully");
    
    // Save credentials to encrypted file
    config::save_credentials(data_paths, &api_creds).await?;
    
    // Also save the private key for future use
    config::save_private_key(data_paths, private_key_hex).await?;
    
    info!("💾 Credentials saved to encrypted storage");
    
    Ok(())
}

/// Get an authenticated client with saved credentials
pub async fn get_authenticated_client(host: &str, data_paths: &DataPaths) -> Result<ClobClient> {
    info!("Creating authenticated client for host: {}", host);
    
    // Load saved credentials
    let api_creds = config::load_credentials(data_paths).await
        .map_err(|e| anyhow!("Failed to load credentials. Run 'polybot init' first: {}", e))?;
    
    info!("Loaded API credentials successfully");
    
    // Load private key
    let private_key = config::load_private_key(data_paths).await
        .map_err(|e| anyhow!("Failed to load private key. Run 'polybot init' first: {}", e))?;
    
    info!("Loaded private key successfully");
    
    // Determine chain ID based on host
    let chain_id = if host.contains("mumbai") { 80001 } else { 137 };
    info!("Using chain ID: {}", chain_id);
    
    // Create client with L1 headers (this sets up the signer)
    let mut client = ClobClient::with_l1_headers(host, &private_key, chain_id);
    
    // Set the API credentials for L2 authentication
    client.set_api_creds(api_creds);
    
    info!("Client created successfully");
    
    Ok(client)
}
