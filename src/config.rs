use crate::data_paths::DataPaths;
use aes_gcm::{
    aead::{
        rand_core::{OsRng, RngCore},
        Aead, KeyInit,
    },
    Aes256Gcm, Key, Nonce,
};
use anyhow::{anyhow, Result};
use argon2::Argon2;
use polymarket_rs_client::ApiCreds;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
struct StoredCredentials {
    api_key: String,
    api_secret: String,
    api_passphrase: String,
    private_key: Option<String>,
}

/// Get the path to the credentials file
fn get_creds_path(data_paths: &DataPaths) -> Result<PathBuf> {
    let auth_dir = data_paths.auth();
    std::fs::create_dir_all(&auth_dir)?;
    Ok(auth_dir.join("creds.json.enc"))
}

/// Legacy path for backward compatibility
fn get_legacy_creds_path() -> Result<PathBuf> {
    let config_dir = directories::ProjectDirs::from("com", "polybot", "polybot")
        .ok_or_else(|| anyhow!("Could not determine config directory"))?
        .config_dir()
        .to_path_buf();

    Ok(config_dir.join("creds.json.enc"))
}

/// Get or prompt for passphrase
async fn get_passphrase() -> Result<String> {
    // First check environment variable
    if let Ok(passphrase) = std::env::var("POLYBOT_PASSPHRASE") {
        return Ok(passphrase);
    }

    // Otherwise prompt
    let passphrase = rpassword::prompt_password("Enter passphrase for credential encryption: ")?;
    if passphrase.is_empty() {
        return Err(anyhow!("Passphrase cannot be empty"));
    }
    Ok(passphrase)
}

/// Derive encryption key from passphrase
fn derive_key(passphrase: &str, salt: &[u8]) -> Result<Key<Aes256Gcm>> {
    let mut key_bytes = [0u8; 32];
    Argon2::default()
        .hash_password_into(passphrase.as_bytes(), salt, &mut key_bytes)
        .map_err(|e| anyhow!("Failed to derive key: {}", e))?;
    Ok(Key::<Aes256Gcm>::from_slice(&key_bytes).clone())
}

/// Load stored credentials helper
async fn load_stored_credentials(
    creds_path: &PathBuf,
    passphrase: &str,
) -> Result<StoredCredentials> {
    // Read encrypted file
    let encrypted = std::fs::read(creds_path)?;

    if encrypted.len() < 28 {
        // 16 (salt) + 12 (nonce) = 28
        return Err(anyhow!("Invalid encrypted file format"));
    }

    // Extract components
    let salt = &encrypted[..16];
    let nonce_bytes = &encrypted[16..28];
    let ciphertext = &encrypted[28..];

    // Derive key and create cipher
    let key = derive_key(passphrase, salt)?;
    let cipher = Aes256Gcm::new(&key);
    let nonce = Nonce::from_slice(nonce_bytes);

    // Decrypt
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| anyhow!("Decryption failed. Wrong passphrase?"))?;

    // Deserialize
    let stored: StoredCredentials = serde_json::from_slice(&plaintext)?;
    Ok(stored)
}

/// Save stored credentials helper
async fn save_stored_credentials(
    creds_path: &PathBuf,
    passphrase: &str,
    stored: &StoredCredentials,
) -> Result<()> {
    // Serialize credentials
    let json = serde_json::to_string(&stored)?;

    // Generate salt and nonce
    let mut salt = [0u8; 16];
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce_bytes);

    // Derive key and create cipher
    let key = derive_key(&passphrase, &salt)?;
    let cipher = Aes256Gcm::new(&key);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt
    let ciphertext = cipher
        .encrypt(nonce, json.as_bytes())
        .map_err(|e| anyhow!("Encryption failed: {}", e))?;

    // Write salt + nonce + ciphertext
    let mut output = Vec::new();
    output.extend_from_slice(&salt);
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);

    std::fs::write(creds_path, output)?;
    Ok(())
}

/// Save credentials to encrypted file
pub async fn save_credentials(data_paths: &DataPaths, api_creds: &ApiCreds) -> Result<()> {
    let creds_path = get_creds_path(data_paths)?;
    let passphrase = get_passphrase().await?;

    // Load existing credentials if they exist to preserve private key
    let mut stored = if creds_path.exists() {
        match load_stored_credentials(&creds_path, &passphrase).await {
            Ok(existing) => existing,
            Err(_) => StoredCredentials {
                api_key: api_creds.api_key.clone(),
                api_secret: api_creds.secret.clone(),
                api_passphrase: api_creds.passphrase.clone(),
                private_key: None,
            },
        }
    } else {
        StoredCredentials {
            api_key: api_creds.api_key.clone(),
            api_secret: api_creds.secret.clone(),
            api_passphrase: api_creds.passphrase.clone(),
            private_key: None,
        }
    };

    // Update API credentials
    stored.api_key = api_creds.api_key.clone();
    stored.api_secret = api_creds.secret.clone();
    stored.api_passphrase = api_creds.passphrase.clone();

    save_stored_credentials(&creds_path, &passphrase, &stored).await
}

/// Save private key to encrypted file
pub async fn save_private_key(data_paths: &DataPaths, private_key: &str) -> Result<()> {
    let creds_path = get_creds_path(data_paths)?;
    let passphrase = get_passphrase().await?;

    // Load existing credentials
    let mut stored = if creds_path.exists() {
        load_stored_credentials(&creds_path, &passphrase).await?
    } else {
        return Err(anyhow!("No credentials found. Run 'polybot init' first"));
    };

    // Update private key
    stored.private_key = Some(private_key.to_string());

    save_stored_credentials(&creds_path, &passphrase, &stored).await
}

/// Load credentials from encrypted file (with legacy support)
pub async fn load_credentials(data_paths: &DataPaths) -> Result<ApiCreds> {
    let creds_path = get_creds_path(data_paths)?;

    // Check if credentials exist in new location
    let (path_to_use, needs_migration) = if creds_path.exists() {
        (creds_path, false)
    } else {
        // Check legacy location
        if let Ok(legacy_path) = get_legacy_creds_path() {
            if legacy_path.exists() {
                (legacy_path, true)
            } else {
                return Err(anyhow!("No credentials found. Run 'polybot init' first"));
            }
        } else {
            return Err(anyhow!("No credentials found. Run 'polybot init' first"));
        }
    };

    let passphrase = get_passphrase().await?;
    let stored = load_stored_credentials(&path_to_use, &passphrase).await?;

    // Migrate if needed
    if needs_migration {
        save_stored_credentials(&get_creds_path(data_paths)?, &passphrase, &stored).await?;
        // Optionally delete old file
        // std::fs::remove_file(&path_to_use).ok();
    }

    Ok(ApiCreds {
        api_key: stored.api_key,
        secret: stored.api_secret,
        passphrase: stored.api_passphrase,
    })
}

/// Load private key from encrypted file
pub async fn load_private_key(data_paths: &DataPaths) -> Result<String> {
    let creds_path = get_creds_path(data_paths)?;

    // Check if credentials exist in new location
    let path_to_use = if creds_path.exists() {
        creds_path
    } else {
        // Check legacy location
        if let Ok(legacy_path) = get_legacy_creds_path() {
            if legacy_path.exists() {
                legacy_path
            } else {
                return Err(anyhow!("No credentials found. Run 'polybot init' first"));
            }
        } else {
            return Err(anyhow!("No credentials found. Run 'polybot init' first"));
        }
    };

    let passphrase = get_passphrase().await?;
    let stored = load_stored_credentials(&path_to_use, &passphrase).await?;

    stored
        .private_key
        .ok_or_else(|| anyhow!("Private key not found. Please run 'polybot init' again"))
}
