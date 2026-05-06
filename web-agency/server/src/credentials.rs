//! Shared credential helpers for building API clients.
//!
//! Consolidates the repeated "fetch encrypted credential → decrypt → parse JSON → build client"
//! pattern that was duplicated across domain_detail, webspace_detail, domain_list, domain_import,
//! domain_add, domain_register, credential_form, credential_edit, api/sync, and api/deploy.

use sqlx::PgPool;
use uuid::Uuid;

/// Fetch and decrypt a credential's JSON data from the database.
pub async fn credential_json(
    pool: &PgPool,
    cred_id: Uuid,
    cred_type: &str,
) -> anyhow::Result<serde_json::Value> {
    let encrypted = sqlx::query_scalar::<_, Vec<u8>>(
        "SELECT encrypted_data FROM credentials WHERE id = $1 AND credential_type = $2",
    )
    .bind(cred_id)
    .bind(cred_type)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow::anyhow!("{cred_type} credential {cred_id} not found"))?;
    let decrypted = crate::crypto::decrypt(&encrypted)?;
    Ok(serde_json::from_slice(&decrypted)?)
}

/// Build a Cloudflare API client from a stored credential.
pub async fn cf_client(
    pool: &PgPool,
    cred_id: Uuid,
) -> anyhow::Result<cloudflare_api::compat::SimpleClient> {
    let data = credential_json(pool, cred_id, "cloudflare").await?;
    let token = data["api_token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing api_token"))?;
    Ok(cloudflare_api::compat::SimpleClient::new(token))
}

/// Build a Cloudflare API client and resolve the account ID.
/// If the credential has no account_id stored, resolves it via the API
/// and backfills it into the encrypted credential data so future calls are instant.
pub async fn cf_client_with_account(
    pool: &PgPool,
    cred_id: Uuid,
) -> anyhow::Result<(cloudflare_api::compat::SimpleClient, String)> {
    let data = credential_json(pool, cred_id, "cloudflare").await?;
    let token = data["api_token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing api_token"))?;
    let configured = data["account_id"].as_str().unwrap_or("");
    let client = cloudflare_api::compat::SimpleClient::new(token);
    let account_id = client.resolve_account_id(configured).await?;

    // Backfill account_id if it was empty
    if configured.is_empty() && !account_id.is_empty() {
        let mut updated = data.clone();
        updated["account_id"] = serde_json::Value::String(account_id.clone());
        if let Ok(encrypted) = crate::crypto::encrypt(
            serde_json::to_vec(&updated)
                .unwrap_or_default()
                .as_slice(),
        ) {
            let _ = sqlx::query(
                "UPDATE credentials SET encrypted_data = $1, updated_at = now() WHERE id = $2",
            )
            .bind(&encrypted)
            .bind(cred_id)
            .execute(pool)
            .await;
            tracing::info!(cred_id = %cred_id, account_id = %account_id, "backfilled account_id into credential");
        }
    }

    Ok((client, account_id))
}

/// Fetch the server URL and token from a mac-mgmt credential.
pub async fn mac_mgmt_credential(
    pool: &PgPool,
    cred_id: Uuid,
) -> anyhow::Result<(String, String)> {
    let data = credential_json(pool, cred_id, "mac-mgmt").await?;
    let server_url = data["server_url"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing server_url"))?
        .to_string();
    let token = data["token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing token"))?
        .to_string();
    Ok((server_url, token))
}

/// Build a Spaceship API client from a stored credential.
pub async fn spaceship_client(
    pool: &PgPool,
    cred_id: Uuid,
) -> anyhow::Result<spaceship_api::compat::SimpleClient> {
    let data = credential_json(pool, cred_id, "spaceship").await?;
    let key = data["api_key"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing api_key"))?;
    let secret = data["api_secret"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing api_secret"))?;
    Ok(spaceship_api::compat::SimpleClient::new(key, secret))
}
