//! AES-256-GCM encryption/decryption for credential storage.

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{AeadCore, Aes256Gcm, Key, Nonce};

use crate::config;

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("secrets not configured")]
    NotConfigured,
    #[error("invalid encryption key")]
    InvalidKey,
    #[error("encryption failed")]
    EncryptFailed,
    #[error("decryption failed")]
    DecryptFailed,
    #[error("ciphertext too short")]
    TooShort,
}

fn encryption_key() -> Result<Key<Aes256Gcm>, CryptoError> {
    let cfg = config::config();
    let secrets = cfg.secrets.as_ref().ok_or(CryptoError::NotConfigured)?;
    let key_bytes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &secrets.encryption_key,
    )
    .map_err(|_| CryptoError::InvalidKey)?;
    if key_bytes.len() != 32 {
        return Err(CryptoError::InvalidKey);
    }
    Ok(*Key::<Aes256Gcm>::from_slice(&key_bytes))
}

/// Encrypt plaintext with AES-256-GCM. Returns nonce (12 bytes) || ciphertext.
pub fn encrypt(plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let key = encryption_key()?;
    let cipher = Aes256Gcm::new(&key);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|_| CryptoError::EncryptFailed)?;
    let mut out = nonce.to_vec();
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt data produced by `encrypt()`. Input: nonce (12 bytes) || ciphertext.
pub fn decrypt(data: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if data.len() < 12 {
        return Err(CryptoError::TooShort);
    }
    let key = encryption_key()?;
    let cipher = Aes256Gcm::new(&key);
    let nonce = Nonce::from_slice(&data[..12]);
    cipher
        .decrypt(nonce, &data[12..])
        .map_err(|_| CryptoError::DecryptFailed)
}
