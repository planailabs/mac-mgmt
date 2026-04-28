use serde::Deserialize;
use std::collections::HashMap;

#[cfg(any(feature = "server", feature = "server-api-only"))]
use std::collections::HashSet;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XzarPin {
    pub name: String,
    pub abandoned: bool,
    pub roots: Vec<XzarPinRoot>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XzarPinRoot {
    pub drv_full: String,
}

/// Fetch all pins from xzar.
pub async fn fetch_pins(url: &str, token: &str) -> Result<Vec<XzarPin>, String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{url}/pins"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("xzar request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("xzar returned {}", resp.status()));
    }

    resp.json::<Vec<XzarPin>>()
        .await
        .map_err(|e| format!("failed to parse xzar response: {e}"))
}

/// Resolve skill store paths from a list of pins for a given architecture.
/// For each (slug, channel), looks for pin named `skill/{slug}/{channel}/{arch}`.
/// Returns a map of slug → store path. Skills without a matching pin are skipped.
pub fn resolve_store_paths(
    pins: &[XzarPin],
    skills: &[(String, String)],
    arch: &str,
) -> HashMap<String, String> {
    let mut result = HashMap::new();

    for (slug, channel) in skills {
        let pin_name = format!("skill/{slug}/{channel}/{arch}");
        let noarch_name = format!("skill/{slug}/{channel}/noarch");
        let pin = pins
            .iter()
            .find(|p| p.name == pin_name && !p.abandoned)
            .or_else(|| pins.iter().find(|p| p.name == noarch_name && !p.abandoned));
        if let Some(pin) = pin {
            if let Some(root) = pin.roots.first() {
                let path = if root.drv_full.starts_with("/nix/store/") {
                    root.drv_full.clone()
                } else {
                    format!("/nix/store/{}", root.drv_full)
                };
                result.insert(slug.clone(), path);
            }
        }
    }

    result
}

// ── Shared skill sync logic ─────────────────────────────────────────────
//
// Used by both the web UI "Sync from xzar" button (Dioxus server function)
// and the skill-importer background jobs.

/// Result of syncing skills from xzar pins into the database.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SyncResult {
    pub created_skills: u32,
    pub created_channels: u32,
    pub removed_channels: u32,
    pub removed_skills: u32,
}

/// Fetch all pins from xzar and sync skills into the database.
///
/// Upserts skill/channel rows for every `skill/{slug}/{channel}/{arch}` pin
/// found in xzar, then removes channels and skills that no longer have a
/// matching pin. Notifies federation and affected clusters of deletions.
#[cfg(any(feature = "server", feature = "server-api-only"))]
pub async fn sync_skills_db(pool: &sqlx::PgPool) -> Result<SyncResult, String> {
    let cfg = crate::config::config();
    let xzar = cfg.xzar.as_ref().ok_or("xzar not configured")?;
    let pins = fetch_pins(&xzar.url, &xzar.token).await?;
    sync_skills_from_pins(pool, &pins).await
}

/// Sync skills from a pre-fetched list of pins into the database.
#[cfg(any(feature = "server", feature = "server-api-only"))]
pub async fn sync_skills_from_pins(
    pool: &sqlx::PgPool,
    pins: &[XzarPin],
) -> Result<SyncResult, String> {
    let mut created_skills: u32 = 0;
    let mut created_channels: u32 = 0;

    for pin in pins {
        if pin.abandoned || pin.roots.is_empty() {
            continue;
        }

        let parts: Vec<&str> = pin.name.splitn(4, '/').collect();
        if parts.len() != 4 || parts[0] != "skill" {
            continue;
        }
        let slug = parts[1];
        let channel = parts[2];

        let inserted = sqlx::query_scalar::<_, bool>(
            "INSERT INTO skills (slug, name) VALUES ($1, $1) \
             ON CONFLICT (slug) DO NOTHING \
             RETURNING true",
        )
        .bind(slug)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;

        if inserted.is_some() {
            created_skills += 1;
        }

        let skill_id = sqlx::query_scalar::<_, uuid::Uuid>(
            "SELECT id FROM skills WHERE slug = $1",
        )
        .bind(slug)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;

        let ch_inserted = sqlx::query_scalar::<_, bool>(
            "INSERT INTO skill_channels (skill_id, channel) VALUES ($1, $2) \
             ON CONFLICT (skill_id, channel) DO NOTHING \
             RETURNING true",
        )
        .bind(skill_id)
        .bind(channel)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;

        if ch_inserted.is_some() {
            created_channels += 1;
        }
    }

    // Collect valid (slug, channel) pairs
    let mut valid_pairs: HashSet<(String, String)> = HashSet::new();
    for pin in pins {
        if pin.abandoned || pin.roots.is_empty() {
            continue;
        }
        let parts: Vec<&str> = pin.name.splitn(4, '/').collect();
        if parts.len() == 4 && parts[0] == "skill" {
            valid_pairs.insert((parts[1].to_string(), parts[2].to_string()));
        }
    }

    let slugs_vec: Vec<String> = valid_pairs.iter().map(|(s, _)| s.clone()).collect();
    let channels_vec: Vec<String> = valid_pairs.iter().map(|(_, c)| c.clone()).collect();
    let to_remove_channel_ids: Vec<uuid::Uuid> = sqlx::query_scalar(
        "SELECT sc.id FROM skill_channels sc \
         JOIN skills s ON sc.skill_id = s.id \
         WHERE NOT EXISTS ( \
             SELECT 1 FROM unnest($1::text[], $2::text[]) AS v(slug, channel) \
             WHERE v.slug = s.slug AND v.channel = sc.channel \
         )",
    )
    .bind(&slugs_vec)
    .bind(&channels_vec)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    if !to_remove_channel_ids.is_empty() {
        crate::api::push::notify_federation_global();
        crate::api::push::notify_skill_channels_global(&to_remove_channel_ids).await;
    }

    let removed_channels = sqlx::query_scalar::<_, i64>(
        "WITH deleted AS ( \
             DELETE FROM skill_channels WHERE id = ANY($1) RETURNING id \
         ) SELECT count(*) FROM deleted",
    )
    .bind(&to_remove_channel_ids)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    let removed_skills = sqlx::query_scalar::<_, i64>(
        "WITH deleted AS ( \
             DELETE FROM skills s \
             WHERE NOT EXISTS ( \
                 SELECT 1 FROM skill_channels sc WHERE sc.skill_id = s.id \
             ) \
             RETURNING s.id \
         ) SELECT count(*) FROM deleted",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(SyncResult {
        created_skills,
        created_channels,
        removed_channels: removed_channels as u32,
        removed_skills: removed_skills as u32,
    })
}

/// Extract the store path for a specific pin name from a list of pins.
pub fn store_path_for_pin(pins: &[XzarPin], pin_name: &str) -> Option<String> {
    pins.iter()
        .find(|p| p.name == pin_name && !p.abandoned)
        .and_then(|p| p.roots.first())
        .map(|root| {
            if root.drv_full.starts_with("/nix/store/") {
                root.drv_full.clone()
            } else {
                format!("/nix/store/{}", root.drv_full)
            }
        })
}
