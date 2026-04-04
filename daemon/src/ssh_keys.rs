use anyhow::{Context, Result};
use russh::keys::{parse_public_key_base64, PublicKey};

pub async fn sync(server_url: &str, token: &str) -> Result<Vec<PublicKey>> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{server_url}/api/ssh-keys"))
        .bearer_auth(token)
        .send()
        .await
        .context("failed to reach SSH keys API")?;

    let status = resp.status();
    if !status.is_success() {
        anyhow::bail!("SSH keys API returned {status}");
    }

    #[derive(serde::Deserialize)]
    struct KeyEntry {
        public_key: String,
    }

    let entries: Vec<KeyEntry> = resp
        .json()
        .await
        .context("failed to parse SSH keys response")?;

    let mut keys = Vec::new();
    for entry in &entries {
        let b64 = match entry.public_key.split_whitespace().nth(1) {
            Some(b) => b,
            None => {
                tracing::warn!("skipping SSH key with unexpected format");
                continue;
            }
        };
        match parse_public_key_base64(b64) {
            Ok(key) => keys.push(key),
            Err(e) => tracing::warn!("skipping invalid SSH key: {e}"),
        }
    }

    tracing::info!("synced {} SSH keys from server", keys.len());
    Ok(keys)
}
