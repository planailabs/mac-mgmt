use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
use anyhow::{Context, Result};
use std::path::Path;

const NONCE_LEN: usize = 12;

/// A minimal view of a cleaner session manifest — just what we need for
/// rehydration. We deserialize only the fields we use to avoid coupling
/// tightly with the cleaner's types.
#[derive(serde::Deserialize)]
struct SessionManifest {
    #[serde(default)]
    status: String,
    #[serde(default)]
    entities: Vec<Entity>,
    #[serde(default)]
    redacted_text: Option<String>,
}

#[derive(serde::Deserialize)]
struct Entity {
    placeholder: String,
    original: String,
    #[serde(default)]
    approved: bool,
}

/// Load a cleaner session's redacted text.
pub fn load_redacted_text(cleaner_dir: &Path, session_id: &str) -> Result<String> {
    let manifest = load_manifest(cleaner_dir, session_id)?;
    if manifest.status != "approved" {
        anyhow::bail!(
            "session {session_id} is not yet approved (status={})",
            manifest.status
        );
    }
    manifest
        .redacted_text
        .ok_or_else(|| anyhow::anyhow!("session {session_id} has no redacted text"))
}

/// Replace placeholders in text with originals from a cleaner session.
pub fn rehydrate(cleaner_dir: &Path, session_id: &str, text: &str) -> Result<String> {
    let manifest = load_manifest(cleaner_dir, session_id)?;
    let mut result = text.to_string();
    for entity in &manifest.entities {
        if entity.approved {
            result = result.replace(&entity.placeholder, &entity.original);
        }
    }
    Ok(result)
}

fn load_manifest(cleaner_dir: &Path, session_id: &str) -> Result<SessionManifest> {
    let path = cleaner_dir
        .join("sessions")
        .join(format!("{session_id}.enc"));
    let data = std::fs::read(&path)
        .with_context(|| format!("cleaner session not found: {}", path.display()))?;

    if data.len() < NONCE_LEN + 1 {
        anyhow::bail!("corrupt cleaner session file");
    }

    let key = load_key(cleaner_dir)?;
    let cipher = Aes256Gcm::new(&key.into());
    let nonce = Nonce::from_slice(&data[..NONCE_LEN]);
    let plaintext = cipher
        .decrypt(nonce, &data[NONCE_LEN..])
        .map_err(|e| anyhow::anyhow!("failed to decrypt cleaner session: {e}"))?;

    serde_json::from_slice(&plaintext).context("failed to parse cleaner session manifest")
}

/// Check whether a cleaner session exists (without decrypting).
pub fn session_exists(cleaner_dir: &Path, session_id: &str) -> bool {
    cleaner_dir
        .join("sessions")
        .join(format!("{session_id}.enc"))
        .exists()
}

fn load_key(cleaner_dir: &Path) -> Result<[u8; 32]> {
    // Check env var first (same logic as the cleaner).
    if let Ok(env_key) = std::env::var("CLEANER_KEY") {
        let hk = hkdf::Hkdf::<sha2::Sha256>::new(None, env_key.as_bytes());
        let mut key = [0u8; 32];
        hk.expand(b"plan-ai-cleaner", &mut key)
            .map_err(|_| anyhow::anyhow!("HKDF expand failed"))?;
        return Ok(key);
    }

    let key_path = cleaner_dir.join("key");
    let data = std::fs::read(&key_path)
        .with_context(|| format!("cleaner key not found at {}", key_path.display()))?;
    if data.len() != 32 {
        anyhow::bail!("cleaner key has wrong length");
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&data);
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};

    /// Create a fake cleaner session directory with an encrypted manifest.
    fn create_test_session(dir: &Path, session_id: &str, status: &str) {
        let sessions_dir = dir.join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();

        // Write a key file.
        let key: [u8; 32] = [42u8; 32];
        std::fs::write(dir.join("key"), key).unwrap();

        // Build a manifest JSON.
        let manifest = serde_json::json!({
            "id": session_id,
            "status": status,
            "entities": [
                {
                    "placeholder": "[PERSON_1]",
                    "original": "Alice Johnson",
                    "approved": true,
                },
                {
                    "placeholder": "[COMPANY_1]",
                    "original": "Acme Corp",
                    "approved": true,
                },
                {
                    "placeholder": "[EMAIL_1]",
                    "original": "alice@acme.com",
                    "approved": false,
                },
            ],
            "redacted_text": "Dear [PERSON_1] at [COMPANY_1], your email [EMAIL_1] is on file.",
        });

        let json = serde_json::to_vec(&manifest).unwrap();
        let cipher = Aes256Gcm::new(&key.into());
        let nonce_bytes: [u8; 12] = [1u8; 12];
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher.encrypt(nonce, json.as_ref()).unwrap();

        let mut data = Vec::with_capacity(12 + ciphertext.len());
        data.extend_from_slice(&nonce_bytes);
        data.extend_from_slice(&ciphertext);

        std::fs::write(sessions_dir.join(format!("{session_id}.enc")), &data).unwrap();
    }

    #[test]
    fn rehydrate_approved_entities_only() {
        let dir = tempfile::tempdir().unwrap();
        create_test_session(dir.path(), "sess-1", "approved");

        let text = "[PERSON_1] works at [COMPANY_1]. Contact [EMAIL_1].";
        let result = rehydrate(dir.path(), "sess-1", text).unwrap();

        // PERSON_1 and COMPANY_1 are approved, EMAIL_1 is not.
        assert_eq!(
            result,
            "Alice Johnson works at Acme Corp. Contact [EMAIL_1]."
        );
    }

    #[test]
    fn load_redacted_text_from_approved_session() {
        let dir = tempfile::tempdir().unwrap();
        create_test_session(dir.path(), "sess-2", "approved");

        let text = load_redacted_text(dir.path(), "sess-2").unwrap();
        assert_eq!(
            text,
            "Dear [PERSON_1] at [COMPANY_1], your email [EMAIL_1] is on file."
        );
    }

    #[test]
    fn load_redacted_text_rejects_unapproved() {
        let dir = tempfile::tempdir().unwrap();
        create_test_session(dir.path(), "sess-3", "scanned");

        let result = load_redacted_text(dir.path(), "sess-3");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not yet approved"));
    }

    #[test]
    fn session_not_found() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("sessions")).unwrap();
        std::fs::write(dir.path().join("key"), [0u8; 32]).unwrap();

        let result = load_redacted_text(dir.path(), "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn session_exists_check() {
        let dir = tempfile::tempdir().unwrap();
        create_test_session(dir.path(), "exists-1", "approved");

        assert!(session_exists(dir.path(), "exists-1"));
        assert!(!session_exists(dir.path(), "nope"));
    }
}
