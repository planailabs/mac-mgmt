//! Shared credential helpers for building API clients.
//!
//! Consolidates the repeated "fetch encrypted credential → decrypt → parse JSON → build client"
//! pattern that was duplicated across domain_detail, webspace_detail, domain_list, domain_import,
//! domain_add, domain_register, credential_form, credential_edit, api/sync, and api/deploy.

use sqlx::PgPool;
use std::net::IpAddr;
use uuid::Uuid;

/// Reject outbound URLs that point at the loopback interface, private/link-local
/// ranges, or cloud metadata endpoints. Stored credential URLs are used verbatim
/// as request targets, so without this an operator who can create a credential
/// could turn a server-side fetch into an SSRF against internal services (e.g.
/// http://169.254.169.254/ or http://localhost:2375/). Call at credential
/// creation and again before each outbound request.
pub fn validate_outbound_url(url: &str) -> anyhow::Result<()> {
    let parsed = reqwest::Url::parse(url).map_err(|e| anyhow::anyhow!("invalid url: {e}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        anyhow::bail!("url scheme must be http or https");
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("url has no host"))?;
    // `host_str` keeps IPv6 literals bracketed; strip for parsing.
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    if let Ok(ip) = bare.parse::<IpAddr>() {
        reject_private_ip(ip)?;
    } else {
        let d = host.trim_end_matches('.').to_ascii_lowercase();
        if d == "localhost" || d.ends_with(".localhost") || d == "metadata" {
            anyhow::bail!("url host is not permitted");
        }
    }
    Ok(())
}

fn reject_private_ip(ip: IpAddr) -> anyhow::Result<()> {
    let blocked = match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                // CGNAT 100.64.0.0/10
                || (v4.octets()[0] == 100 && (v4.octets()[1] & 0xc0) == 64)
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                // unique-local fc00::/7 and link-local fe80::/10
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80
                // IPv4-mapped — re-check the embedded v4
                || v6.to_ipv4_mapped().is_some_and(|m| reject_private_ip(IpAddr::V4(m)).is_err())
        }
    };
    if blocked {
        anyhow::bail!("url resolves to a non-public address");
    }
    Ok(())
}

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
        if let Ok(encrypted) =
            crate::crypto::encrypt(serde_json::to_vec(&updated).unwrap_or_default().as_slice())
        {
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
pub async fn mac_mgmt_credential(pool: &PgPool, cred_id: Uuid) -> anyhow::Result<(String, String)> {
    let data = credential_json(pool, cred_id, "mac-mgmt").await?;
    let server_url = data["server_url"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing server_url"))?
        .to_string();
    validate_outbound_url(&server_url)?;
    let token = data["token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing token"))?
        .to_string();
    Ok((server_url, token))
}

/// Build a ChangeDetection.io API client from a stored credential.
///
/// Returns the client (with `x-api-key` injected as a default header) and the
/// group name that watches should be organized under.
pub async fn changedetection_client(
    pool: &PgPool,
    cred_id: Uuid,
) -> anyhow::Result<(changedetection_api::Client, String)> {
    let data = credential_json(pool, cred_id, "changedetection").await?;
    let api_url = data["api_url"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing api_url"))?;
    validate_outbound_url(api_url)?;
    let api_key = data["api_key"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing api_key"))?;
    let group = data["group"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing group"))?
        .to_string();

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::HeaderName::from_static("x-api-key"),
        reqwest::header::HeaderValue::from_str(api_key)
            .map_err(|e| anyhow::anyhow!("invalid api_key header value: {e}"))?,
    );
    let http = reqwest::ClientBuilder::new()
        .default_headers(headers)
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    Ok((
        changedetection_api::Client::new_with_client(api_url, http),
        group,
    ))
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

#[cfg(test)]
mod ssrf_tests {
    use super::validate_outbound_url;

    #[test]
    fn blocks_metadata_and_loopback_and_private() {
        for bad in [
            "http://169.254.169.254/latest/meta-data/",
            "http://localhost:2375/",
            "http://127.0.0.1/",
            "http://[::1]/",
            "http://10.0.0.5/",
            "http://192.168.1.1/",
            "http://100.64.0.1/",
            "http://metadata/computeMetadata/v1/",
            "ftp://example.com/",
            "file:///etc/passwd",
        ] {
            assert!(validate_outbound_url(bad).is_err(), "{bad} should be blocked");
        }
    }

    #[test]
    fn allows_public_hosts() {
        for ok in ["https://api.example.com/", "http://203.0.113.10:8080/x"] {
            assert!(validate_outbound_url(ok).is_ok(), "{ok} should be allowed");
        }
    }
}
