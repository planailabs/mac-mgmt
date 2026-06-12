use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{AeadCore, Aes256Gcm, Key, Nonce};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

fn cache_path() -> PathBuf {
    crate::config::config_dir().join(".secrets-cache")
}

fn key_file_path() -> PathBuf {
    crate::config::config_dir().join(".cache-key")
}

// ── OS-specific key storage ─────────────────────────────────────────────

/// Try to read the cache encryption key from the OS keychain.
/// Returns None if not found or on error.
#[cfg(target_os = "macos")]
fn read_key_from_keychain() -> Option<[u8; 32]> {
    let output = crate::cmd::output_with_timeout(
        Command::new("security").args([
            "find-generic-password",
            "-a",
            "mac-mgmt",
            "-s",
            "mac-mgmt-secrets-cache",
            "-w",
        ]),
        crate::cmd::DEFAULT_TIMEOUT,
    )
    .ok()?;
    if !output.status.success() {
        return None;
    }
    let hex = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let bytes = hex::decode(&hex).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Some(key)
}

/// Store the cache encryption key in the macOS Keychain.
#[cfg(target_os = "macos")]
fn write_key_to_keychain(key: &[u8; 32]) -> bool {
    let hex_key = hex::encode(key);
    crate::cmd::output_with_timeout(
        Command::new("security").args([
            "add-generic-password",
            "-a",
            "mac-mgmt",
            "-s",
            "mac-mgmt-secrets-cache",
            "-w",
            &hex_key,
            "-U",
        ]),
        crate::cmd::DEFAULT_TIMEOUT,
    )
    .map(|o| o.status.success())
    .unwrap_or(false)
}

/// Try to read the cache encryption key from the Linux kernel keyring.
#[cfg(not(target_os = "macos"))]
fn read_key_from_keychain() -> Option<[u8; 32]> {
    // keyctl request user mac-mgmt-secrets-cache
    let request = crate::cmd::output_with_timeout(
        Command::new("keyctl").args(["request", "user", "mac-mgmt-secrets-cache"]),
        crate::cmd::DEFAULT_TIMEOUT,
    )
    .ok()?;
    if !request.status.success() {
        return None;
    }
    let key_id = String::from_utf8_lossy(&request.stdout).trim().to_string();
    // keyctl pipe <key-id>
    let pipe = crate::cmd::output_with_timeout(
        Command::new("keyctl").args(["pipe", &key_id]),
        crate::cmd::DEFAULT_TIMEOUT,
    )
    .ok()?;
    if !pipe.status.success() {
        return None;
    }
    let hex = String::from_utf8_lossy(&pipe.stdout).trim().to_string();
    let bytes = hex::decode(&hex).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Some(key)
}

/// Store the cache encryption key in the Linux kernel keyring.
#[cfg(not(target_os = "macos"))]
fn write_key_to_keychain(key: &[u8; 32]) -> bool {
    let hex_key = hex::encode(key);
    crate::cmd::output_with_timeout(
        Command::new("keyctl").args(["add", "user", "mac-mgmt-secrets-cache", &hex_key, "@u"]),
        crate::cmd::DEFAULT_TIMEOUT,
    )
    .map(|o| o.status.success())
    .unwrap_or(false)
}

// ── Key management ──────────────────────────────────────────────────────

/// Read the cache encryption key from OS keychain, or file fallback.
fn load_cache_key() -> Option<[u8; 32]> {
    // Try OS keychain first
    if let Some(key) = read_key_from_keychain() {
        return Some(key);
    }
    // Fallback: file-based key
    let path = key_file_path();
    let hex = std::fs::read_to_string(&path).ok()?;
    let bytes = hex::decode(hex.trim()).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Some(key)
}

/// Generate a new cache key and store it in the OS keychain (with file fallback).
fn create_cache_key() -> Result<[u8; 32]> {
    let key: [u8; 32] = rand::random();

    // Try OS keychain
    if write_key_to_keychain(&key) {
        tracing::info!("secrets cache key stored in OS keychain");
        return Ok(key);
    }

    // Fallback: write to file with 0o600
    tracing::warn!(
        "OS keychain unavailable, storing secrets cache key in file ({})",
        key_file_path().display()
    );
    let path = key_file_path();
    std::fs::write(&path, hex::encode(key))
        .with_context(|| format!("failed to write cache key to {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(key)
}

/// Get or create the cache encryption key.
fn ensure_cache_key() -> Result<[u8; 32]> {
    if let Some(key) = load_cache_key() {
        return Ok(key);
    }
    create_cache_key()
}

// ── Public API ──────────────────────────────────────────────────────────

/// Cache secrets to disk, encrypted with AES-256-GCM.
pub fn cache_secrets(secrets: &HashMap<String, String>) -> Result<()> {
    let key_bytes = ensure_cache_key()?;
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    let plaintext = serde_json::to_vec(secrets).context("failed to serialize secrets")?;
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_ref())
        .map_err(|e| anyhow::anyhow!("encryption failed: {e}"))?;

    let mut data = nonce.to_vec();
    data.extend_from_slice(&ciphertext);

    let path = cache_path();
    std::fs::write(&path, &data)
        .with_context(|| format!("failed to write secrets cache to {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    tracing::debug!("cached {} secrets to {}", secrets.len(), path.display());
    Ok(())
}

/// Load cached secrets from disk. Returns None if cache doesn't exist or decryption fails.
pub fn load_cached_secrets() -> Option<HashMap<String, String>> {
    let key_bytes = load_cache_key()?;
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);

    let data = std::fs::read(cache_path()).ok()?;
    if data.len() < 12 {
        return None;
    }

    let nonce = Nonce::from_slice(&data[..12]);
    let plaintext = cipher.decrypt(nonce, &data[12..]).ok()?;
    serde_json::from_slice(&plaintext).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key_bytes: [u8; 32] = rand::random();
        let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
        let cipher = Aes256Gcm::new(key);
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

        let secrets = HashMap::from([
            ("KEY1".to_string(), "value1".to_string()),
            ("KEY2".to_string(), "value2".to_string()),
        ]);
        let plaintext = serde_json::to_vec(&secrets).unwrap();
        let ciphertext = cipher.encrypt(&nonce, plaintext.as_ref()).unwrap();

        let mut data = nonce.to_vec();
        data.extend_from_slice(&ciphertext);

        let dec_nonce = Nonce::from_slice(&data[..12]);
        let decrypted = cipher.decrypt(dec_nonce, &data[12..]).unwrap();
        let result: HashMap<String, String> = serde_json::from_slice(&decrypted).unwrap();
        assert_eq!(result, secrets);
    }
}
