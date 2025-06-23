//! Environment-based authentication helpers for direct API access

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use hmac::{Hmac, Mac};
use polymarket_rs_client::ApiCreds;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Generate L2 signature for Polymarket API authentication
pub fn generate_l2_signature(
    api_secret: &str,
    timestamp: u64,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<String> {
    // Create the message to sign
    // Format: timestamp + method + path + body (if present)
    let mut message = format!("{}{}{}", timestamp, method, path);
    if let Some(body_str) = body {
        message.push_str(body_str);
    }

    // Decode the API secret from base64 (using URL_SAFE like polymarket-rs-client)
    let secret_bytes = general_purpose::URL_SAFE
        .decode(api_secret)
        .map_err(|e| anyhow!("Failed to decode API secret: {}", e))?;

    // Create HMAC
    let mut mac = HmacSha256::new_from_slice(&secret_bytes)
        .map_err(|e| anyhow!("Invalid key length: {}", e))?;

    // Sign the message
    mac.update(message.as_bytes());

    // Get the result and encode as base64 (using URL_SAFE like polymarket-rs-client)
    let result = mac.finalize();
    let signature = general_purpose::URL_SAFE.encode(result.into_bytes());

    Ok(signature)
}

/// Build L2 authentication headers for Polymarket API
pub fn build_l2_headers(
    api_creds: &ApiCreds,
    address: &str,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<Vec<(String, String)>> {
    // Use seconds timestamp like polymarket-rs-client
    let timestamp = Utc::now().timestamp() as u64;

    // Generate signature
    let signature = generate_l2_signature(&api_creds.secret, timestamp, method, path, body)?;

    // Use lowercase header names like polymarket-rs-client
    Ok(vec![
        ("poly_address".to_string(), address.to_string()),
        ("poly_api_key".to_string(), api_creds.api_key.clone()),
        ("poly_passphrase".to_string(), api_creds.passphrase.clone()),
        ("poly_timestamp".to_string(), timestamp.to_string()),
        ("poly_signature".to_string(), signature),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_l2_signature_generation() {
        // Test with the same parameters as the polymarket-rs-client test
        let api_secret = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
        let timestamp = 1000000;
        let method = "test-sign";
        let path = "/orders";

        // Create a test body similar to the library's test
        let body = r#"{"hash": "0x123"}"#;

        let signature =
            generate_l2_signature(api_secret, timestamp, method, path, Some(body)).unwrap();

        // The signature should be a valid base64 string
        assert!(!signature.is_empty());
        assert!(general_purpose::URL_SAFE.decode(&signature).is_ok());

        // Test that the signature is consistent
        let signature2 =
            generate_l2_signature(api_secret, timestamp, method, path, Some(body)).unwrap();

        assert_eq!(signature, signature2);
    }

    #[test]
    fn test_build_l2_headers() {
        let api_creds = ApiCreds {
            api_key: "test_key".to_string(),
            secret: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string(),
            passphrase: "test_pass".to_string(),
        };

        let headers = build_l2_headers(
            &api_creds,
            "0x1234567890123456789012345678901234567890",
            "GET",
            "/test",
            None,
        )
        .unwrap();

        // Check that all required headers are present
        let header_map: HashMap<String, String> = headers.into_iter().collect();

        assert!(header_map.contains_key("poly_address"));
        assert!(header_map.contains_key("poly_api_key"));
        assert!(header_map.contains_key("poly_passphrase"));
        assert!(header_map.contains_key("poly_timestamp"));
        assert!(header_map.contains_key("poly_signature"));

        assert_eq!(header_map["poly_api_key"], "test_key");
        assert_eq!(header_map["poly_passphrase"], "test_pass");
        assert_eq!(
            header_map["poly_address"],
            "0x1234567890123456789012345678901234567890"
        );
    }
}
