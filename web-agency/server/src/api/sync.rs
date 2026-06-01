//! Periodic background sync: Cloudflare DNS records and domain renewal dates.

use sqlx::PgPool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use uuid::Uuid;

const SYNC_INTERVAL: Duration = Duration::from_secs(6 * 3600); // every 6 hours

/// Lock to ensure only one sync runs at a time.
static SYNC_RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Spawn the periodic sync background task.
pub fn spawn(pool: PgPool) {
    tokio::spawn(async move {
        // Small delay to let the server finish starting.
        tokio::time::sleep(Duration::from_secs(10)).await;
        loop {
            trigger_sync(&pool).await;
            tokio::time::sleep(SYNC_INTERVAL).await;
        }
    });
}

/// Run a sync if one isn't already in progress. Returns false if skipped.
pub async fn trigger_sync(pool: &PgPool) -> bool {
    if SYNC_RUNNING.swap(true, Ordering::AcqRel) {
        tracing::info!("sync already running, skipping");
        return false;
    }
    let result = run_sync(pool).await;
    SYNC_RUNNING.store(false, Ordering::Release);
    if let Err(e) = result {
        tracing::error!("periodic sync failed: {e}");
    }
    true
}

async fn run_sync(pool: &PgPool) -> anyhow::Result<()> {
    tracing::info!("starting periodic sync");
    super::counters::COUNTERS.reset_sync_gauges();
    sync_cloudflare_dns(pool).await;
    sync_domain_expiry(pool).await;
    sync_nameserver_status(pool).await;
    sync_bot_protection(pool).await;
    sync_changedetection(pool).await;
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
            super::counters::COUNTERS
                .sync_dns_errors
                .fetch_add(1, Ordering::Relaxed);
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
            .bind(&acme_sub_ids)
            .execute(pool)
            .await?;
        sqlx::query("DELETE FROM subdomains WHERE id = ANY($1)")
            .bind(&acme_sub_ids)
            .execute(pool)
            .await?;
    }

    Ok(())
}

// ── Domain expiry sync ───────────────────────────────────────────────

async fn sync_domain_expiry(pool: &PgPool) {
    let domains =
        match sqlx::query_as::<_, (Uuid, String, Option<String>, Option<Uuid>, Option<Uuid>)>(
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
                super::counters::COUNTERS
                    .sync_expiry_errors
                    .fetch_add(1, Ordering::Relaxed);
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
        match check_ns_match(
            pool,
            *domain_id,
            &domain_name,
            zone_id,
            *cf_cred_id,
            *ss_cred_id,
        )
        .await
        {
            Ok(()) => checked += 1,
            Err(e) => {
                tracing::warn!("NS status check failed for {domain_name}: {e}");
                super::counters::COUNTERS
                    .sync_ns_errors
                    .fetch_add(1, Ordering::Relaxed);
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
    let mut actual: Vec<String> = info.nameservers.and_then(|ns| ns.hosts).unwrap_or_default();
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
                super::counters::COUNTERS
                    .sync_bot_errors
                    .fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    tracing::info!(
        "synced bot protection for {updated}/{} domains",
        domains.len()
    );
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

// ── ChangeDetection.io sync ─────────────────────────────────────────

async fn sync_changedetection(pool: &PgPool) {
    // Fetch webspaces that have a changedetection credential assigned.
    let webspaces = match sqlx::query_as::<_, (Uuid, String, Uuid)>(
        "SELECT w.id, w.name, w.changedetection_credential_id \
         FROM webspace_hosts w \
         WHERE w.changedetection_credential_id IS NOT NULL",
    )
    .fetch_all(pool)
    .await
    {
        Ok(w) => w,
        Err(e) => {
            tracing::error!("failed to list webspaces for changedetection sync: {e}");
            return;
        }
    };

    let mut synced = 0usize;
    // Cache clients per credential to avoid re-creating them.
    let mut client_cache: std::collections::HashMap<Uuid, (changedetection_api::Client, String)> =
        std::collections::HashMap::new();

    for (ws_id, ws_name, cred_id) in &webspaces {
        // Get or create the client for this credential.
        let (client, group_name) = match client_cache.get(cred_id) {
            Some(entry) => (entry.0.clone(), entry.1.clone()),
            None => match crate::credentials::changedetection_client(pool, *cred_id).await {
                Ok(entry) => {
                    client_cache.insert(*cred_id, entry.clone());
                    entry
                }
                Err(e) => {
                    tracing::warn!(
                        webspace = ws_name.as_str(),
                        "failed to get changedetection client: {e}"
                    );
                    continue;
                }
            },
        };

        match sync_changedetection_for_webspace(pool, *ws_id, ws_name, &client, &group_name).await {
            Ok(()) => synced += 1,
            Err(e) => {
                tracing::warn!("changedetection sync failed for webspace {ws_name}: {e}");
                super::counters::COUNTERS
                    .sync_changedetection_errors
                    .fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    tracing::info!(
        "synced changedetection for {synced}/{} webspaces",
        webspaces.len()
    );

    // Garbage-collect orphaned watches and tags.
    gc_changedetection(pool).await;
}

/// Sync all sub-URLs for a single webspace.
async fn sync_changedetection_for_webspace(
    pool: &PgPool,
    ws_id: Uuid,
    ws_name: &str,
    client: &changedetection_api::Client,
    group_name: &str,
) -> anyhow::Result<()> {
    // 0. Ensure at least a "/" sub-URL exists.
    let suburls = sqlx::query_as::<_, (Uuid, String, Option<Uuid>, String, serde_json::Value)>(
        "SELECT id, path, tag_id, secret, tag_settings \
         FROM changedetection_suburls WHERE webspace_host_id = $1 ORDER BY path",
    )
    .bind(ws_id)
    .fetch_all(pool)
    .await?;

    let suburls = if suburls.is_empty() {
        // Auto-create "/" sub-URL.
        let secret = generate_secret();
        sqlx::query(
            "INSERT INTO changedetection_suburls (webspace_host_id, path, secret) VALUES ($1, '/', $2)",
        )
        .bind(ws_id)
        .bind(&secret)
        .execute(pool)
        .await?;
        tracing::info!(
            webspace = ws_name,
            "auto-created / sub-URL for changedetection"
        );
        sqlx::query_as::<_, (Uuid, String, Option<Uuid>, String, serde_json::Value)>(
            "SELECT id, path, tag_id, secret, tag_settings \
             FROM changedetection_suburls WHERE webspace_host_id = $1 ORDER BY path",
        )
        .bind(ws_id)
        .fetch_all(pool)
        .await?
    } else {
        suburls
    };

    // 1. Resolve the credential-level group tag.
    let group_tag_uuid = find_or_create_tag(client, group_name).await?;
    let group_tag_str = group_tag_uuid.to_string();

    // 2. Resolve the per-webspace tag (format: "group:webspace") — kept for human filtering.
    let ws_tag_title = format!("{group_name}:{ws_name}");
    let ws_tag_uuid = find_or_create_tag(client, &ws_tag_title).await?;
    let ws_tag_str = ws_tag_uuid.to_string();

    // 3. Fetch domain bindings once for the webspace.
    let bindings = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT d.name, s.name \
         FROM webspace_host_domains wd \
         JOIN domains d ON d.id = wd.domain_id \
         LEFT JOIN subdomains s ON s.id = wd.subdomain_id \
         WHERE wd.webspace_host_id = $1",
    )
    .bind(ws_id)
    .fetch_all(pool)
    .await?;

    let hostnames: Vec<String> = bindings
        .iter()
        .map(|(domain, sub)| match sub.as_deref() {
            Some(s) if s != "@" => format!("{s}.{domain}"),
            _ => domain.clone(),
        })
        .collect();

    tracing::debug!(
        webspace = ws_name,
        group_tag = %group_tag_uuid,
        ws_tag = %ws_tag_uuid,
        suburls = suburls.len(),
        hostnames = hostnames.len(),
        "resolved changedetection tags for webspace"
    );

    // 4. Sync each sub-URL.
    for (suburl_id, path, cached_tag_id, secret, tag_settings_json) in &suburls {
        if let Err(e) = sync_changedetection_for_suburl(
            pool,
            client,
            ws_name,
            group_name,
            &group_tag_str,
            &ws_tag_str,
            &hostnames,
            *suburl_id,
            path,
            *cached_tag_id,
            secret,
            tag_settings_json,
        )
        .await
        {
            tracing::warn!(
                webspace = ws_name,
                path,
                "changedetection sub-URL sync failed: {e}"
            );
        }
    }

    Ok(())
}

/// Sync a single sub-URL: resolve its tag, apply settings, create/update/delete watches.
#[allow(clippy::too_many_arguments)]
async fn sync_changedetection_for_suburl(
    pool: &PgPool,
    client: &changedetection_api::Client,
    ws_name: &str,
    group_name: &str,
    group_tag_str: &str,
    ws_tag_str: &str,
    hostnames: &[String],
    suburl_id: Uuid,
    path: &str,
    cached_tag_id: Option<Uuid>,
    secret: &str,
    tag_settings_json: &serde_json::Value,
) -> anyhow::Result<()> {
    // 1. Resolve the sub-URL tag (format: "group:webspace:path").
    let suburl_tag_title = format!("{group_name}:{ws_name}:{path}");
    let suburl_tag_uuid =
        resolve_or_create_suburl_tag(client, pool, suburl_id, cached_tag_id, &suburl_tag_title)
            .await?;
    let suburl_tag_str = suburl_tag_uuid.to_string();

    // 2. Ensure the sub-URL tag has notification_urls pointing to our webhook.
    sync_tag_notifications(client, suburl_tag_uuid, secret).await?;

    // 3. Apply tag_settings from DB to the changedetection tag.
    sync_tag_settings(client, suburl_tag_uuid, tag_settings_json).await?;

    let watch_tags = vec![group_tag_str, ws_tag_str, suburl_tag_str.as_str()];
    let watch_tags_json: Vec<&str> = watch_tags.clone();

    // 4. Build expected watch URLs for this sub-URL.
    let mut expected_urls: Vec<String> = hostnames
        .iter()
        .map(|hostname| format!("https://{hostname}{path}"))
        .collect();
    expected_urls.sort();
    expected_urls.dedup();

    // 5. List existing watches tagged with this sub-URL tag.
    let existing = client
        .list_watches(None, Some(&suburl_tag_title))
        .await
        .map_err(|e| anyhow::anyhow!("list_watches failed: {e}"))?;
    let watches = existing.into_inner();

    tracing::debug!(
        webspace = ws_name,
        path,
        expected = expected_urls.len(),
        existing = watches.len(),
        "changedetection sub-URL sync state"
    );

    // 6. Create missing watches or update existing ones.
    for url in &expected_urls {
        let title_str = url.strip_prefix("https://").unwrap_or(url);

        let existing_entry = watches
            .iter()
            .find(|(_, w)| w.url.as_deref() == Some(url.as_str()));

        if let Some((uuid_str, watch)) = existing_entry {
            let title_ok = watch
                .title
                .as_deref()
                .map(|t| t.as_str() == title_str)
                .unwrap_or(false);
            let tags_ok = watch_tags_json
                .iter()
                .all(|t| watch.tags.iter().any(|wt| wt == *t));

            if !title_ok || !tags_ok {
                let watch_uuid: uuid::Uuid = uuid_str
                    .parse()
                    .map_err(|e| anyhow::anyhow!("invalid watch UUID {uuid_str}: {e}"))?;
                let body = serde_json::json!({
                    "title": title_str,
                    "tags": &watch_tags_json,
                });
                let update: changedetection_api::types::UpdateWatch = serde_json::from_value(body)
                    .map_err(|e| anyhow::anyhow!("failed to build UpdateWatch: {e}"))?;
                client
                    .update_watch(&watch_uuid, &update)
                    .await
                    .map_err(|e| anyhow::anyhow!("update_watch for {url} failed: {e}"))?;
                tracing::info!(
                    webspace = ws_name,
                    path,
                    url,
                    "updated changedetection watch"
                );
            }
        } else {
            let body = serde_json::json!({
                "url": url,
                "title": title_str,
                "tags": &watch_tags_json,
            });
            let create: changedetection_api::types::CreateWatch = serde_json::from_value(body)
                .map_err(|e| anyhow::anyhow!("failed to build CreateWatch: {e}"))?;
            client
                .create_watch(&create)
                .await
                .map_err(|e| anyhow::anyhow!("create_watch for {url} failed: {e}"))?;
            tracing::info!(
                webspace = ws_name,
                path,
                url,
                "created changedetection watch"
            );
        }
    }

    // 7. Delete stale watches (only within the sub-URL tag).
    for (uuid_str, watch) in &watches {
        let watch_url = watch.url.as_deref().unwrap_or("");
        if !expected_urls.iter().any(|u| u == watch_url) {
            let watch_uuid: uuid::Uuid = match uuid_str.parse() {
                Ok(u) => u,
                Err(_) => continue,
            };
            if let Err(e) =
                safe_delete_watch(client, &watch_uuid, watch_url, &suburl_tag_str, watch).await
            {
                tracing::warn!(
                    webspace = ws_name,
                    path,
                    url = watch_url,
                    "failed to delete stale watch: {e}"
                );
            } else {
                tracing::info!(
                    webspace = ws_name,
                    path,
                    url = watch_url,
                    "deleted stale changedetection watch"
                );
            }
        }
    }

    Ok(())
}

/// Find an existing tag by title or create a new one (no DB caching).
async fn find_or_create_tag(
    client: &changedetection_api::Client,
    title: &str,
) -> anyhow::Result<uuid::Uuid> {
    let tags = client
        .list_tags()
        .await
        .map_err(|e| anyhow::anyhow!("list_tags failed: {e}"))?;

    for (uuid_str, tag) in tags.into_inner().iter() {
        if tag.title.as_deref().map(|t| t.as_str()) == Some(title) {
            let uuid: uuid::Uuid = uuid_str
                .parse()
                .map_err(|e| anyhow::anyhow!("invalid tag UUID {uuid_str}: {e}"))?;
            return Ok(uuid);
        }
    }

    let body = serde_json::json!({ "title": title });
    let create: changedetection_api::types::CreateTag = serde_json::from_value(body)
        .map_err(|e| anyhow::anyhow!("failed to build CreateTag: {e}"))?;
    let resp = client
        .create_tag(&create)
        .await
        .map_err(|e| anyhow::anyhow!("create_tag failed: {e}"))?;
    let tag_resp = resp.into_inner();
    let uuid = tag_resp
        .uuid
        .ok_or_else(|| anyhow::anyhow!("create_tag returned no UUID"))?;
    tracing::info!(tag = %uuid, title, "created new changedetection tag");
    Ok(uuid)
}

/// Resolve a per-sub-URL tag, creating it if needed and caching the UUID
/// in the `changedetection_suburls` table.
async fn resolve_or_create_suburl_tag(
    client: &changedetection_api::Client,
    pool: &PgPool,
    suburl_id: Uuid,
    cached_tag_id: Option<Uuid>,
    tag_title: &str,
) -> anyhow::Result<uuid::Uuid> {
    // Fast path: we have a cached UUID — verify it exists.
    if let Some(id) = cached_tag_id {
        match client.get_tag(&id, None, None).await {
            Ok(resp) => {
                let tag = resp.into_inner();
                let current_title = tag.title.as_deref().map(|t| t.as_str());
                if current_title != Some(tag_title) {
                    let body = serde_json::json!({ "title": tag_title });
                    let update: changedetection_api::types::Tag = serde_json::from_value(body)
                        .map_err(|e| anyhow::anyhow!("failed to build Tag update: {e}"))?;
                    client
                        .update_tag(&id, &update)
                        .await
                        .map_err(|e| anyhow::anyhow!("update_tag title failed: {e}"))?;
                    tracing::info!(tag = %id, title = tag_title, "updated changedetection tag title");
                }
                return Ok(id);
            }
            Err(_) => {
                tracing::warn!(tag = %id, "cached changedetection tag not found, re-resolving");
            }
        }
    }

    // Search existing tags by title.
    let tags = client
        .list_tags()
        .await
        .map_err(|e| anyhow::anyhow!("list_tags failed: {e}"))?;

    for (uuid_str, tag) in tags.into_inner().iter() {
        if tag.title.as_deref().map(|t| t.as_str()) == Some(tag_title) {
            let uuid: uuid::Uuid = uuid_str
                .parse()
                .map_err(|e| anyhow::anyhow!("invalid tag UUID {uuid_str}: {e}"))?;
            cache_suburl_tag_id(pool, suburl_id, uuid).await;
            return Ok(uuid);
        }
    }

    // Create new tag.
    let body = serde_json::json!({ "title": tag_title });
    let create: changedetection_api::types::CreateTag = serde_json::from_value(body)
        .map_err(|e| anyhow::anyhow!("failed to build CreateTag: {e}"))?;
    let resp = client
        .create_tag(&create)
        .await
        .map_err(|e| anyhow::anyhow!("create_tag failed: {e}"))?;
    let tag_resp = resp.into_inner();
    let uuid = tag_resp
        .uuid
        .ok_or_else(|| anyhow::anyhow!("create_tag returned no UUID"))?;
    cache_suburl_tag_id(pool, suburl_id, uuid).await;
    tracing::info!(tag = %uuid, title = tag_title, "created new changedetection tag");
    Ok(uuid)
}

/// Persist the resolved tag UUID in the changedetection_suburls table.
async fn cache_suburl_tag_id(pool: &PgPool, suburl_id: Uuid, tag_id: Uuid) {
    if let Err(e) = sqlx::query("UPDATE changedetection_suburls SET tag_id = $1 WHERE id = $2")
        .bind(tag_id)
        .bind(suburl_id)
        .execute(pool)
        .await
    {
        tracing::warn!(%suburl_id, %tag_id, "failed to cache changedetection suburl tag id: {e}");
    }
}

/// Apply tag_settings from the database to the changedetection.io tag.
///
/// This merges the user-configured settings with the tag's existing state,
/// preserving notification_urls (the webhook URL is set separately).
async fn sync_tag_settings(
    client: &changedetection_api::Client,
    tag_uuid: Uuid,
    tag_settings_json: &serde_json::Value,
) -> anyhow::Result<()> {
    // Skip if settings are empty/default.
    if tag_settings_json.is_null()
        || (tag_settings_json.is_object()
            && tag_settings_json.as_object().map_or(true, |o| o.is_empty()))
    {
        return Ok(());
    }

    // Build the update body from the stored settings.
    // We use serde_json::Value directly since the fields map 1:1 to the Tag API.
    let update: changedetection_api::types::Tag = serde_json::from_value(tag_settings_json.clone())
        .map_err(|e| anyhow::anyhow!("failed to build Tag from tag_settings: {e}"))?;
    client
        .update_tag(&tag_uuid, &update)
        .await
        .map_err(|e| anyhow::anyhow!("update_tag settings failed: {e}"))?;
    tracing::debug!(tag = %tag_uuid, "applied tag_settings to changedetection tag");
    Ok(())
}

/// Delete a watch only if it belongs to the expected tag/group.
///
/// This prevents accidentally removing watches that a user created outside
/// the managed group.
async fn safe_delete_watch(
    client: &changedetection_api::Client,
    watch_uuid: &uuid::Uuid,
    watch_url: &str,
    expected_tag_uuid: &str,
    watch: &changedetection_api::types::Watch,
) -> anyhow::Result<()> {
    // Verify the watch belongs to the expected group.
    let belongs = watch
        .tag
        .as_deref()
        .map(|t| t.as_str() == expected_tag_uuid)
        .unwrap_or(false)
        || watch.tags.iter().any(|t| t == expected_tag_uuid);

    if !belongs {
        tracing::warn!(
            uuid = %watch_uuid,
            url = watch_url,
            "skipping delete: watch does not belong to expected tag {expected_tag_uuid}"
        );
        return Ok(());
    }

    client
        .delete_watch(watch_uuid)
        .await
        .map_err(|e| anyhow::anyhow!("delete_watch {watch_uuid} failed: {e}"))?;
    Ok(())
}

/// Generate a random hex secret for webhook URLs.
fn generate_secret() -> String {
    use rand::Rng;
    let bytes: [u8; 32] = rand::rng().random();
    hex::encode(bytes)
}

/// Ensure the webspace tag has notification_urls pointing to our webhook.
async fn sync_tag_notifications(
    client: &changedetection_api::Client,
    tag_uuid: Uuid,
    secret: &str,
) -> anyhow::Result<()> {
    let agency_domain = crate::config::config()
        .proxy
        .as_ref()
        .map(|p| p.agency_domain.as_str())
        .unwrap_or("localhost");
    let webhook_url = format!("json://https://{agency_domain}/api/changedetection/{secret}");

    // Check current tag to see if notification_urls already includes our webhook.
    let tag = client
        .get_tag(&tag_uuid, None, None)
        .await
        .map_err(|e| anyhow::anyhow!("get_tag for notification sync failed: {e}"))?
        .into_inner();

    let already_set = tag
        .notification_urls
        .iter()
        .any(|u| u.as_str() == webhook_url);
    if already_set {
        return Ok(());
    }

    let body = serde_json::json!({
        "notification_urls": [&webhook_url],
        "notification_format": "markdown",
    });
    let update: changedetection_api::types::Tag = serde_json::from_value(body)
        .map_err(|e| anyhow::anyhow!("failed to build Tag notification update: {e}"))?;
    client
        .update_tag(&tag_uuid, &update)
        .await
        .map_err(|e| anyhow::anyhow!("update_tag notification_urls failed: {e}"))?;
    tracing::info!(tag = %tag_uuid, "set changedetection tag notification_urls");
    Ok(())
}

/// Garbage-collect orphaned watches and tags across all changedetection credentials.
async fn gc_changedetection(pool: &PgPool) {
    // Get distinct credentials used by webspaces.
    let creds = match sqlx::query_as::<_, (Uuid,)>(
        "SELECT DISTINCT changedetection_credential_id \
         FROM webspace_hosts WHERE changedetection_credential_id IS NOT NULL",
    )
    .fetch_all(pool)
    .await
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("gc_changedetection: failed to list credentials: {e}");
            return;
        }
    };

    for (cred_id,) in &creds {
        if let Err(e) = gc_for_credential(pool, *cred_id).await {
            tracing::warn!(%cred_id, "changedetection GC failed: {e}");
        }
    }
}

async fn gc_for_credential(pool: &PgPool, cred_id: Uuid) -> anyhow::Result<()> {
    let (client, group_name) = crate::credentials::changedetection_client(pool, cred_id).await?;

    // Build valid watch URLs from suburls joined with domain bindings.
    let valid_urls: std::collections::HashSet<String> =
        sqlx::query_as::<_, (String, Option<String>, String)>(
            "SELECT d.name, s.name, cs.path \
         FROM changedetection_suburls cs \
         JOIN webspace_hosts h ON h.id = cs.webspace_host_id \
         JOIN webspace_host_domains wd ON wd.webspace_host_id = h.id \
         JOIN domains d ON d.id = wd.domain_id \
         LEFT JOIN subdomains s ON s.id = wd.subdomain_id \
         WHERE h.changedetection_credential_id = $1",
        )
        .bind(cred_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|(domain, sub, path)| {
            let hostname = match sub.as_deref() {
                Some(s) if s != "@" => format!("{s}.{domain}"),
                _ => domain,
            };
            format!("https://{hostname}{path}")
        })
        .collect();

    // Collect valid webspace names and their sub-URL paths.
    let valid_ws_names: std::collections::HashSet<String> = sqlx::query_scalar::<_, String>(
        "SELECT name FROM webspace_hosts WHERE changedetection_credential_id = $1",
    )
    .bind(cred_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .collect();

    // Collect valid (webspace_name, path) pairs for sub-URL tag validation.
    let valid_suburl_pairs: std::collections::HashSet<(String, String)> =
        sqlx::query_as::<_, (String, String)>(
            "SELECT h.name, cs.path \
             FROM changedetection_suburls cs \
             JOIN webspace_hosts h ON h.id = cs.webspace_host_id \
             WHERE h.changedetection_credential_id = $1",
        )
        .bind(cred_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .collect();

    // Resolve the group tag.
    let group_tag_uuid = find_or_create_tag(&client, &group_name).await?;

    // GC orphaned watches: list all watches with the group tag.
    let watches = client
        .list_watches(None, Some(&group_name))
        .await
        .map_err(|e| anyhow::anyhow!("gc list_watches failed: {e}"))?
        .into_inner();

    for (uuid_str, watch) in &watches {
        let watch_url = watch.url.as_deref().unwrap_or("");
        if !watch_url.is_empty() && !valid_urls.contains(watch_url) {
            if let Ok(uuid) = uuid_str.parse::<Uuid>() {
                if let Err(e) = client.delete_watch(&uuid).await {
                    tracing::warn!(url = watch_url, "gc: failed to delete orphaned watch: {e}");
                } else {
                    tracing::info!(
                        url = watch_url,
                        "gc: deleted orphaned changedetection watch"
                    );
                }
            }
        }
    }

    // GC orphaned tags: handles both old "group:ws" and new "group:ws:path" formats.
    let tags = client
        .list_tags()
        .await
        .map_err(|e| anyhow::anyhow!("gc list_tags failed: {e}"))?
        .into_inner();

    let prefix = format!("{group_name}:");
    for (uuid_str, tag) in tags.iter() {
        let title = match tag.title.as_deref().map(|t| t.as_str()) {
            Some(t) => t,
            None => continue,
        };
        if !title.starts_with(&prefix) {
            continue;
        }
        let rest = &title[prefix.len()..];
        if rest.is_empty() {
            continue;
        }

        let uuid = match uuid_str.parse::<Uuid>() {
            Ok(u) => u,
            Err(_) => continue,
        };
        if uuid == group_tag_uuid {
            continue; // Never delete the group tag itself.
        }

        let orphaned = if let Some(colon_pos) = rest.find(":/") {
            // New format: "group:ws_name:/path"
            let ws_name = &rest[..colon_pos];
            let path = &rest[colon_pos + 1..]; // includes leading /
            !valid_suburl_pairs.contains(&(ws_name.to_string(), path.to_string()))
        } else {
            // Old format "group:ws_name" or webspace-level tag "group:ws_name"
            // Keep if the webspace still exists (it's the human-friendly grouping tag).
            !valid_ws_names.contains(rest)
        };

        if orphaned {
            if let Err(e) = client.delete_tag(&uuid).await {
                tracing::warn!(title, "gc: failed to delete orphaned tag: {e}");
            } else {
                tracing::info!(title, "gc: deleted orphaned changedetection tag");
            }
        }
    }

    Ok(())
}
