use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
use anyhow::{Context, Result};
use sha2::Digest;
use std::path::{Path, PathBuf};

use crate::types::SessionManifest;

const NONCE_LEN: usize = 12;

/// Manages encrypted session storage on disk.
pub struct SessionStore {
    sessions_dir: PathBuf,
    cipher: Aes256Gcm,
    ttl_seconds: u64,
}

impl SessionStore {
    pub fn new(data_dir: &Path, ttl_seconds: u64) -> Result<Self> {
        let sessions_dir = data_dir.join("sessions");
        std::fs::create_dir_all(&sessions_dir)
            .with_context(|| format!("failed to create {}", sessions_dir.display()))?;

        let key = load_or_create_key(data_dir)?;
        let cipher = Aes256Gcm::new(&key.into());

        Ok(Self {
            sessions_dir,
            cipher,
            ttl_seconds,
        })
    }

    fn session_path(&self, id: &str) -> PathBuf {
        self.sessions_dir.join(format!("{id}.enc"))
    }

    pub fn save(&self, manifest: &SessionManifest) -> Result<()> {
        let json = serde_json::to_vec(manifest)?;
        let nonce_bytes: [u8; NONCE_LEN] = rand::random();
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, json.as_ref())
            .map_err(|e| anyhow::anyhow!("encryption failed: {e}"))?;

        let mut data = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        data.extend_from_slice(&nonce_bytes);
        data.extend_from_slice(&ciphertext);

        let path = self.session_path(&manifest.id);
        std::fs::write(&path, &data)
            .with_context(|| format!("failed to write session {}", path.display()))?;

        Ok(())
    }

    pub fn load(&self, id: &str) -> Result<SessionManifest> {
        let path = self.session_path(id);
        let data = std::fs::read(&path)
            .with_context(|| format!("session not found: {}", path.display()))?;

        if data.len() < NONCE_LEN + 1 {
            anyhow::bail!("corrupt session file: too short");
        }

        let nonce = Nonce::from_slice(&data[..NONCE_LEN]);
        let plaintext = self
            .cipher
            .decrypt(nonce, &data[NONCE_LEN..])
            .map_err(|e| anyhow::anyhow!("decryption failed: {e}"))?;

        let manifest: SessionManifest = serde_json::from_slice(&plaintext)?;

        // Check TTL.
        let age = chrono::Utc::now()
            .signed_duration_since(manifest.created_at)
            .num_seconds();
        if age > self.ttl_seconds as i64 {
            // Clean up expired file.
            let _ = std::fs::remove_file(&path);
            anyhow::bail!(
                "session {id} has expired ({age}s old, TTL={}s)",
                self.ttl_seconds
            );
        }

        Ok(manifest)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let path = self.session_path(id);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<SessionManifest>> {
        let mut sessions = Vec::new();
        for entry in std::fs::read_dir(&self.sessions_dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !name_str.ends_with(".enc") {
                continue;
            }
            let id = name_str.trim_end_matches(".enc");
            match self.load(id) {
                Ok(manifest) => sessions.push(manifest),
                Err(e) => {
                    tracing::debug!("skipping session {id}: {e}");
                }
            }
        }
        sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(sessions)
    }

    /// Remove all expired session files.
    pub fn gc(&self) -> usize {
        let mut removed = 0;
        if let Ok(entries) = std::fs::read_dir(&self.sessions_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if !name_str.ends_with(".enc") {
                    continue;
                }
                let id = name_str.trim_end_matches(".enc");
                // load() will auto-remove expired sessions.
                if self.load(id).is_err() {
                    removed += 1;
                }
            }
        }
        removed
    }
}

/// Load or generate the 256-bit encryption key.
fn load_or_create_key(data_dir: &Path) -> Result<[u8; 32]> {
    let key_path = data_dir.join("key");

    // Check for env-var-supplied key.
    if let Ok(env_key) = std::env::var("CLEANER_KEY") {
        // Derive a 256-bit key from the passphrase via HKDF.
        let hk = hkdf::Hkdf::<sha2::Sha256>::new(None, env_key.as_bytes());
        let mut key = [0u8; 32];
        hk.expand(b"plan-ai-cleaner", &mut key)
            .map_err(|_| anyhow::anyhow!("HKDF expand failed"))?;
        return Ok(key);
    }

    if key_path.exists() {
        let data = std::fs::read(&key_path)?;
        if data.len() != 32 {
            anyhow::bail!(
                "key file at {} has wrong length (expected 32, got {})",
                key_path.display(),
                data.len()
            );
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&data);
        return Ok(key);
    }

    // Generate a new random key.
    let key: [u8; 32] = rand::random();
    std::fs::write(&key_path, key)?;
    // Best-effort restrictive permissions.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600));
    }
    tracing::info!("generated new encryption key at {}", key_path.display());
    Ok(key)
}

/// Compute SHA-256 hex digest of the given text.
pub fn sha256_hex(text: &str) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(text.as_bytes());
    hex::encode(hasher.finalize())
}

// Inline hex encoding (avoid adding hex crate dependency).
mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes.as_ref().iter().fold(String::new(), |mut acc, b| {
            use std::fmt::Write;
            let _ = write!(acc, "{b:02x}");
            acc
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    fn sample_manifest(id: &str) -> SessionManifest {
        SessionManifest {
            id: id.to_string(),
            name: Some("test session".into()),
            created_at: chrono::Utc::now(),
            ttl_seconds: 86400,
            status: SessionStatus::Approved,
            original_hash: sha256_hex("hello world"),
            entities: vec![
                RedactionEntity {
                    id: "PERSON_1".into(),
                    category: EntityCategory::Person,
                    original: "John Smith".into(),
                    placeholder: "[PERSON_1]".into(),
                    source: DetectionSource::Regex,
                    approved: true,
                },
                RedactionEntity {
                    id: "EMAIL_1".into(),
                    category: EntityCategory::Email,
                    original: "john@acme.com".into(),
                    placeholder: "[EMAIL_1]".into(),
                    source: DetectionSource::Regex,
                    approved: true,
                },
            ],
            redacted_text: Some("Dear [PERSON_1], your email [EMAIL_1] is on file.".into()),
            original_text: "Dear John Smith, your email john@acme.com is on file.".into(),
        }
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path(), 86400).unwrap();
        let manifest = sample_manifest("test-roundtrip");

        store.save(&manifest).unwrap();
        let loaded = store.load("test-roundtrip").unwrap();

        assert_eq!(loaded.id, "test-roundtrip");
        assert_eq!(loaded.name.as_deref(), Some("test session"));
        assert_eq!(loaded.entities.len(), 2);
        assert_eq!(loaded.entities[0].original, "John Smith");
        assert_eq!(loaded.entities[1].placeholder, "[EMAIL_1]");
        assert_eq!(
            loaded.redacted_text.as_deref(),
            Some("Dear [PERSON_1], your email [EMAIL_1] is on file.")
        );
    }

    #[test]
    fn ttl_expiry() {
        let dir = tempfile::tempdir().unwrap();
        // TTL of 0 seconds — everything is immediately expired.
        let store = SessionStore::new(dir.path(), 0).unwrap();

        let mut manifest = sample_manifest("test-expiry");
        manifest.created_at = chrono::Utc::now() - chrono::Duration::seconds(10);
        store.save(&manifest).unwrap();

        let result = store.load("test-expiry");
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("expired"),
            "error should mention expiry"
        );
    }

    #[test]
    fn delete_session() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path(), 86400).unwrap();
        let manifest = sample_manifest("test-delete");

        store.save(&manifest).unwrap();
        assert!(store.load("test-delete").is_ok());

        store.delete("test-delete").unwrap();
        assert!(store.load("test-delete").is_err());
    }

    #[test]
    fn list_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path(), 86400).unwrap();

        store.save(&sample_manifest("sess-a")).unwrap();
        store.save(&sample_manifest("sess-b")).unwrap();

        let sessions = store.list().unwrap();
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn gc_removes_expired() {
        let dir = tempfile::tempdir().unwrap();
        // Very short TTL.
        let store = SessionStore::new(dir.path(), 1).unwrap();

        let mut manifest = sample_manifest("test-gc");
        manifest.created_at = chrono::Utc::now() - chrono::Duration::seconds(10);
        // Write directly to bypass any checks.
        store.save(&manifest).unwrap();

        let removed = store.gc();
        assert!(removed >= 1);
    }

    #[test]
    fn rehydrate_replaces_placeholders() {
        let manifest = sample_manifest("test-rehydrate");
        let response = "According to [PERSON_1], the account [EMAIL_1] was created last year.";
        let mut result = response.to_string();
        for entity in &manifest.entities {
            if entity.approved {
                result = result.replace(&entity.placeholder, &entity.original);
            }
        }
        assert_eq!(
            result,
            "According to John Smith, the account john@acme.com was created last year."
        );
    }

    #[test]
    fn sha256_hex_deterministic() {
        let a = sha256_hex("hello");
        let b = sha256_hex("hello");
        assert_eq!(a, b);
        assert_eq!(a.len(), 64); // 256 bits = 64 hex chars
        assert_ne!(a, sha256_hex("world"));
    }
}
