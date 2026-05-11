//! Periodic background sync: Cloudflare DNS records and domain renewal dates.

use sqlx::PgPool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use uuid::Uuid;

const SYNC_INTERVAL: Duration = Duration::from_secs(6 * 3600); // every 6 hours

/// Spawn the periodic sync background task.
pub fn spawn(pool: PgPool) {
    tokio::spawn(async move {
        // Small delay to let the server finish starting.
        tokio::time::sleep(Duration::from_secs(10)).await;
        loop {
            if let Err(e) = run_sync(&pool).await {
                tracing::error!("periodic sync failed: {e}");
            }
            tokio::time::sleep(SYNC_INTERVAL).await;
        }
    });
}

async fn run_sync(pool: &PgPool) -> anyhow::Result<()> {
    tracing::info!("starting periodic sync");
    super::counters::COUNTERS.reset_sync_gauges();
    sync_cloudflare_dns(pool).await;
    sync_domain_expiry(pool).await;
    sync_nameserver_status(pool).await;
    sync_bot_protection(pool).await;
    tracing::info!("periodic sync complete");
    Ok(())
}

// ── Cloudflare DNS sync ──────────────────────────────────────────────

async fn sync_cloudflare_dns(pool: &PgPool) {
    let domains = match sqlx::query_as::<_, (Uuid, String, String, Uuid)>(
        "SELECT id, name, cloudflare_zone_id, cloudflare_credential_id \
         FROM domains WHERE cloudflare_zone_id IS NOT NULL AND cloudflare_credential_id IS NOT NULL",
    )
    .fetch_all(pool)
    .await
    {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("failed to list CF domains for sync: {e}");
            return;
        }
    };

    for (domain_id, domain_name, zone_id, cred_id) in &domains {
        if let Err(e) = sync_dns_for_domain(pool, *domain_id, domain_name, zone_id, *cred_id).await
        {
            tracing::warn!("DNS sync failed for {domain_name}: {e}");
            super::counters::COUNTERS.sync_dns_errors.fetch_add(1, Ordering::Relaxed);
        }
    }
    tracing::info!("synced DNS for {} domains", domains.len());
}

async fn sync_dns_for_domain(
    pool: &PgPool,
    domain_id: Uuid,
    domain_name: &str,
    zone_id: &str,
    cred_id: Uuid,
) -> anyhow::Result<()> {
    let client = crate::credentials::cf_client(pool, cred_id).await?;
    let cf_records = client.list_dns_records(zone_id).await?;

    // Collect CF record IDs we've seen so we can remove stale local records.
    let mut seen_cf_ids = Vec::with_capacity(cf_records.len());

    for rec in &cf_records {
        // Delete leftover ACME challenge records and skip them
        if rec.name.starts_with("_acme-challenge") {
            if let Err(e) = client.delete_dns_record(zone_id, &rec.id).await {
                tracing::debug!("failed to delete _acme-challenge record {}: {e}", rec.id);
            } else {
                tracing::info!(domain = domain_name, "removed stale _acme-challenge record");
            }
            continue;
        }

        let sub_name = if rec.name == domain_name {
            "@".to_string()
        } else if let Some(stripped) = rec.name.strip_suffix(&format!(".{domain_name}")) {
            stripped.to_string()
        } else {
            rec.name.clone()
        };

        let sub_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO subdomains (domain_id, name) VALUES ($1, $2) \
             ON CONFLICT (domain_id, name) DO UPDATE SET updated_at = now() RETURNING id",
        )
        .bind(domain_id)
        .bind(&sub_name)
        .fetch_one(pool)
        .await?;

        let content = rec.content.as_deref().unwrap_or("");
        let proxied = rec.proxied.unwrap_or(false);

        // Upsert: update if CF record ID exists, insert otherwise.
        let existing = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM dns_records WHERE domain_id = $1 AND cloudflare_record_id = $2",
        )
        .bind(domain_id)
        .bind(&rec.id)
        .fetch_optional(pool)
        .await?;

        if let Some(local_id) = existing {
            sqlx::query(
                "UPDATE dns_records SET subdomain_id = $1, name = $2, record_type = $3, \
                 record_value = $4, proxied = $5, updated_at = now() WHERE id = $6",
            )
            .bind(sub_id)
            .bind(&sub_name)
            .bind(&rec.record_type)
            .bind(content)
            .bind(proxied)
            .bind(local_id)
            .execute(pool)
            .await?;
        } else {
            sqlx::query(
                "INSERT INTO dns_records (subdomain_id, domain_id, name, record_type, record_value, proxied, cloudflare_record_id) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(sub_id)
            .bind(domain_id)
            .bind(&sub_name)
            .bind(&rec.record_type)
            .bind(content)
            .bind(proxied)
            .bind(&rec.id)
            .execute(pool)
            .await?;
        }

        seen_cf_ids.push(rec.id.clone());
    }

    // Remove local records whose CF record ID is no longer present.
    if !seen_cf_ids.is_empty() {
        sqlx::query(
            "DELETE FROM dns_records WHERE domain_id = $1 \
             AND cloudflare_record_id IS NOT NULL \
             AND cloudflare_record_id != ALL($2)",
        )
        .bind(domain_id)
        .bind(&seen_cf_ids)
        .execute(pool)
        .await?;
    }

    // Clean up any local _acme-challenge subdomain and dns_record entries
    let acme_sub_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM subdomains WHERE domain_id = $1 AND name LIKE '_acme-challenge%'",
    )
    .bind(domain_id)
    .fetch_all(pool)
    .await?;
    if !acme_sub_ids.is_empty() {
        sqlx::query("DELETE FROM dns_records WHERE subdomain_id = ANY($1)")
            .bind(&acme_sub_ids).execute(pool).await?;
        sqlx::query("DELETE FROM subdomains WHERE id = ANY($1)")
            .bind(&acme_sub_ids).execute(pool).await?;
    }

    Ok(())
}

// ── Domain expiry sync ───────────────────────────────────────────────

async fn sync_domain_expiry(pool: &PgPool) {
    let domains = match sqlx::query_as::<_, (Uuid, String, Option<String>, Option<Uuid>, Option<Uuid>)>(
        "SELECT id, name, registrar_type, registrar_credential_id, cloudflare_credential_id \
         FROM domains WHERE registrar_type IS NOT NULL",
    )
    .fetch_all(pool)
    .await
    {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("failed to list domains for expiry sync: {e}");
            return;
        }
    };

    let mut updated = 0usize;
    for (domain_id, domain_name, registrar_type, reg_cred_id, cf_cred_id) in &domains {
        let result = match registrar_type.as_deref() {
            Some("spaceship") => {
                if let Some(cred_id) = reg_cred_id {
                    sync_spaceship_expiry(pool, *domain_id, domain_name, *cred_id).await
                } else {
                    continue;
                }
            }
            Some("cloudflare") => {
                if let Some(cred_id) = cf_cred_id {
                    sync_cloudflare_expiry(pool, *domain_id, domain_name, *cred_id).await
                } else {
                    continue;
                }
            }
            _ => continue,
        };
        match result {
            Ok(true) => updated += 1,
            Ok(false) => {}
            Err(e) => {
                tracing::warn!("expiry sync failed for {domain_name}: {e}");
                super::counters::COUNTERS.sync_expiry_errors.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    tracing::info!("updated expiry for {updated}/{} domains", domains.len());
}

async fn sync_spaceship_expiry(
    pool: &PgPool,
    domain_id: Uuid,
    domain_name: &str,
    cred_id: Uuid,
) -> anyhow::Result<bool> {
    let client = crate::credentials::spaceship_client(pool, cred_id).await?;
    let info = client.get_domain_info(domain_name).await?;

    let expiry = info
        .expiration_date
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&chrono::Utc));

    let registered = info
        .registration_date
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&chrono::Utc));

    if expiry.is_none() && registered.is_none() {
        return Ok(false);
    }

    sqlx::query(
        "UPDATE domains SET expires_at = COALESCE($1, expires_at), \
         registered_at = COALESCE($2, registered_at), updated_at = now() WHERE id = $3",
    )
    .bind(expiry)
    .bind(registered)
    .bind(domain_id)
    .execute(pool)
    .await?;

    Ok(true)
}

async fn sync_cloudflare_expiry(
    pool: &PgPool,
    domain_id: Uuid,
    domain_name: &str,
    cred_id: Uuid,
) -> anyhow::Result<bool> {
    let (client, account_id) = crate::credentials::cf_client_with_account(pool, cred_id).await?;
    let reg = client
        .get_domain_registration(&account_id, domain_name)
        .await?;

    let expiry = reg
        .expires_at
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&chrono::Utc));

    let registered = reg
        .created_at
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&chrono::Utc));

    if expiry.is_none() && registered.is_none() {
        return Ok(false);
    }

    sqlx::query(
        "UPDATE domains SET expires_at = COALESCE($1, expires_at), \
         registered_at = COALESCE($2, registered_at), updated_at = now() WHERE id = $3",
    )
    .bind(expiry)
    .bind(registered)
    .bind(domain_id)
    .execute(pool)
    .await?;

    Ok(true)
}

// ── Nameserver status sync ───────────────────────────────────────────

async fn sync_nameserver_status(pool: &PgPool) {
    // For domains that have a CF zone and a spaceship registrar, compare NS.
    let domains = match sqlx::query_as::<_, (Uuid, String, String, Uuid, Uuid)>(
        "SELECT d.id, d.name, d.cloudflare_zone_id, d.cloudflare_credential_id, d.registrar_credential_id \
         FROM domains d \
         WHERE d.cloudflare_zone_id IS NOT NULL \
           AND d.cloudflare_credential_id IS NOT NULL \
           AND d.registrar_type = 'spaceship' \
           AND d.registrar_credential_id IS NOT NULL",
    )
    .fetch_all(pool)
    .await
    {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("failed to list domains for NS status sync: {e}");
            return;
        }
    };

    let mut checked = 0usize;
    for (domain_id, domain_name, zone_id, cf_cred_id, ss_cred_id) in &domains {
        match check_ns_match(pool, *domain_id, &domain_name, zone_id, *cf_cred_id, *ss_cred_id).await {
            Ok(()) => checked += 1,
            Err(e) => {
                tracing::warn!("NS status check failed for {domain_name}: {e}");
                super::counters::COUNTERS.sync_ns_errors.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    tracing::info!("checked NS status for {checked}/{} domains", domains.len());
}

async fn check_ns_match(
    pool: &PgPool,
    domain_id: Uuid,
    domain_name: &str,
    zone_id: &str,
    cf_cred_id: Uuid,
    ss_cred_id: Uuid,
) -> anyhow::Result<()> {
    // Get expected NS from Cloudflare zone
    let cf_client = crate::credentials::cf_client(pool, cf_cred_id).await?;
    let zone = cf_client.get_zone(zone_id).await?;
    let mut expected: Vec<String> = zone.name_servers.unwrap_or_default();
    expected.sort();

    if expected.is_empty() {
        // Can't determine — leave ns_ok as NULL
        return Ok(());
    }

    // Get current NS from Spaceship
    let ss_client = crate::credentials::spaceship_client(pool, ss_cred_id).await?;
    let info = ss_client.get_domain_info(domain_name).await?;
    let mut actual: Vec<String> = info
        .nameservers
        .and_then(|ns| ns.hosts)
        .unwrap_or_default();
    actual.sort();

    let ns_ok = actual == expected;

    sqlx::query("UPDATE domains SET ns_ok = $1, updated_at = now() WHERE id = $2")
        .bind(ns_ok)
        .bind(domain_id)
        .execute(pool)
        .await?;

    Ok(())
}

// ── Bot protection sync ──────────────────────────────────────────────

async fn sync_bot_protection(pool: &PgPool) {
    let domains = match sqlx::query_as::<_, (Uuid, String, String, Uuid)>(
        "SELECT id, name, cloudflare_zone_id, cloudflare_credential_id \
         FROM domains WHERE cloudflare_zone_id IS NOT NULL AND cloudflare_credential_id IS NOT NULL",
    )
    .fetch_all(pool)
    .await
    {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("failed to list domains for bot protection sync: {e}");
            return;
        }
    };

    let mut updated = 0usize;
    for (domain_id, domain_name, zone_id, cred_id) in &domains {
        match sync_bot_for_domain(pool, *domain_id, zone_id, *cred_id).await {
            Ok(()) => updated += 1,
            Err(e) => {
                tracing::warn!("bot protection sync failed for {domain_name}: {e}");
                super::counters::COUNTERS.sync_bot_errors.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    tracing::info!("synced bot protection for {updated}/{} domains", domains.len());
}

async fn sync_bot_for_domain(
    pool: &PgPool,
    domain_id: Uuid,
    zone_id: &str,
    cred_id: Uuid,
) -> anyhow::Result<()> {
    let client = crate::credentials::cf_client(pool, cred_id).await?;
    let bot_config = client.get_bot_management(zone_id).await?;
    let ai_bots = bot_config["ai_bots_protection"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();

    sqlx::query("UPDATE domains SET ai_bots_protection = $1, updated_at = now() WHERE id = $2")
        .bind(&ai_bots)
        .bind(domain_id)
        .execute(pool)
        .await?;

    Ok(())
}

