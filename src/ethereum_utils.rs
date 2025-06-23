//! Ethereum utilities for address derivation and key management

use anyhow::{anyhow, Result};
use ethers_core::k256::ecdsa::SigningKey;
use ethers_core::utils::secret_key_to_address;

/// Derive Ethereum address from private key hex string
pub fn derive_address_from_private_key(private_key_hex: &str) -> Result<String> {
    // Remove 0x prefix if present
    let private_key_hex = private_key_hex.trim_start_matches("0x");

    // Decode hex to bytes
    let private_key_bytes =
        hex::decode(private_key_hex).map_err(|e| anyhow!("Invalid private key hex: {}", e))?;

    // Create signing key
    let signing_key = SigningKey::from_slice(&private_key_bytes)
        .map_err(|e| anyhow!("Invalid private key: {}", e))?;

    // Derive address
    let address = secret_key_to_address(&signing_key);

    // Return as 0x-prefixed hex string
    Ok(format!("{:?}", address))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_address() {
        // Test with a known private key (DO NOT USE IN PRODUCTION)
        let private_key = "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let result = derive_address_from_private_key(private_key);
        assert!(result.is_ok());
        let address = result.unwrap();
        assert!(address.starts_with("0x"));
        assert_eq!(address.len(), 42); // 0x + 40 hex chars
    }

    #[test]
    fn test_derive_address_no_prefix() {
        // Test without 0x prefix
        let private_key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let result = derive_address_from_private_key(private_key);
        assert!(result.is_ok());
    }
}
