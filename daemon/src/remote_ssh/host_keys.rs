use anyhow::{Context, Result};
use russh::keys::{decode_secret_key, encode_pkcs8_pem, Algorithm, PrivateKey};
use std::fs;
use std::path::PathBuf;

fn ssh_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/root"))
        .join(".config/mac-mgmt/ssh")
}

fn host_key_path() -> PathBuf {
    ssh_dir().join("host_ed25519_key")
}

pub fn authorized_keys_path() -> PathBuf {
    ssh_dir().join("authorized_keys")
}

pub fn load_or_generate() -> Result<PrivateKey> {
    let path = host_key_path();

    if path.exists() {
        tracing::info!("loading host key from {}", path.display());
        let pem = fs::read_to_string(&path)
            .with_context(|| format!("failed to read host key from {}", path.display()))?;
        let key = decode_secret_key(&pem, None)
            .context("failed to decode host key")?;
        return Ok(key);
    }

    tracing::info!("generating new Ed25519 host key");
    let dir = ssh_dir();
    fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create {}", dir.display()))?;

    let mut rng = rand::rngs::OsRng;
    let key = PrivateKey::random(&mut rng, Algorithm::Ed25519)
        .context("failed to generate Ed25519 key")?;

    let mut pem_buf = Vec::new();
    encode_pkcs8_pem(&key, &mut pem_buf)
        .context("failed to encode host key as PKCS8 PEM")?;
    fs::write(&path, &pem_buf)
        .with_context(|| format!("failed to write host key to {}", path.display()))?;

    // Restrict permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    }

    tracing::info!("host key saved to {}", path.display());
    Ok(key)
}
