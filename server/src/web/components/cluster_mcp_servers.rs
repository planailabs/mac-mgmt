use dioxus::prelude::*;
use dioxus_i18n::t;

use super::mcp_bundle_detail::McpServerOption;
use crate::web::components::ui::{Button, ButtonKind, ButtonSize, ErrorText, HelpText};
#[cfg(feature = "server")]
use crate::web::user::{current_user, WebUserExt};

/// Direct MCP server assignment display.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ClusterMcpServerDisplay {
    pub cluster_mcp_server_id: uuid::Uuid,
    pub server_slug: String,
    pub server_name: String,
    #[serde(default)]
    pub skill_center_name: Option<String>,
}

/// MCP server coming from a bundle (read-only).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BundleMcpServerDisplay {
    pub server_slug: String,
    pub server_name: String,
    pub bundle_slug: String,
    pub overwritten: bool,
}

/// MCP server coming transitively from a skill dependency (read-only).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TransitiveMcpServerDisplay {
    pub server_slug: String,
    pub server_name: String,
    pub skill_slug: String,
    pub channel: String,
    pub overwritten: bool,
}

/// MCP bundle assignment display.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ClusterMcpBundleDisplay {
    pub cluster_mcp_bundle_id: uuid::Uuid,
    pub bundle_slug: String,
    pub bundle_name: String,
    #[serde(default)]
    pub skill_center_name: Option<String>,
}

/// MCP bundle option for dropdown.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct McpBundleOption {
    pub id: uuid::Uuid,
    pub slug: String,
    pub name: String,
}

#[server]
async fn list_cluster_mcp_servers(
    cluster_id: String,
) -> Result<Vec<ClusterMcpServerDisplay>, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if let Some(ids) = user
        .accessible_cluster_ids(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    {
        if !ids.contains(&uuid) {
            return Err(ServerFnError::new("access denied"));
        }
    }
    let servers = sqlx::query_as::<_, ClusterMcpServerDisplay>(
        "SELECT cms.id AS cluster_mcp_server_id, \
                COALESCE(ms.slug, cms.slug) AS server_slug, \
                COALESCE(ms.name, cms.mcp_name) AS server_name, \
                sk_center.name AS skill_center_name \
         FROM cluster_mcp_servers cms \
         LEFT JOIN mcp_servers ms ON ms.id = cms.mcp_server_id \
         LEFT JOIN skill_centers sk_center ON sk_center.id = cms.skill_center_id \
         WHERE cms.cluster_id = $1 \
         ORDER BY COALESCE(ms.slug, cms.slug)",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(servers)
}

#[server]
async fn list_cluster_mcp_bundles(
    cluster_id: String,
) -> Result<Vec<ClusterMcpBundleDisplay>, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if let Some(ids) = user
        .accessible_cluster_ids(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    {
        if !ids.contains(&uuid) {
            return Err(ServerFnError::new("access denied"));
        }
    }
    let bundles = sqlx::query_as::<_, ClusterMcpBundleDisplay>(
        "SELECT cmb.id AS cluster_mcp_bundle_id, \
                COALESCE(msb.slug, cmb.slug) AS bundle_slug, \
                COALESCE(msb.name, cmb.bundle_name) AS bundle_name, \
                sk_center.name AS skill_center_name \
         FROM cluster_mcp_bundles cmb \
         LEFT JOIN mcp_server_bundles msb ON msb.id = cmb.bundle_id \
         LEFT JOIN skill_centers sk_center ON sk_center.id = cmb.skill_center_id \
         WHERE cmb.cluster_id = $1 \
         ORDER BY COALESCE(msb.slug, cmb.slug)",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(bundles)
}

#[server]
async fn list_bundle_mcp_servers(
    cluster_id: String,
) -> Result<Vec<BundleMcpServerDisplay>, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if let Some(ids) = user
        .accessible_cluster_ids(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    {
        if !ids.contains(&uuid) {
            return Err(ServerFnError::new("access denied"));
        }
    }

    #[derive(sqlx::FromRow)]
    struct Row {
        server_slug: String,
        server_name: String,
        bundle_slug: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT DISTINCT ms.slug as server_slug, ms.name as server_name, msb.slug as bundle_slug \
         FROM cluster_mcp_bundles cmb \
         JOIN mcp_server_bundle_items msbi ON msbi.bundle_id = cmb.bundle_id \
         JOIN mcp_servers ms ON ms.id = msbi.mcp_server_id \
         JOIN mcp_server_bundles msb ON msb.id = cmb.bundle_id \
         WHERE cmb.cluster_id = $1 \
         ORDER BY ms.slug",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Overwritten if a direct assignment exists for the same server slug.
    let direct_slugs: std::collections::HashSet<String> = sqlx::query_scalar::<_, String>(
        "SELECT ms.slug \
         FROM cluster_mcp_servers cms \
         JOIN mcp_servers ms ON ms.id = cms.mcp_server_id \
         WHERE cms.cluster_id = $1",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .into_iter()
    .collect();

    Ok(rows
        .into_iter()
        .map(|r| BundleMcpServerDisplay {
            overwritten: direct_slugs.contains(&r.server_slug),
            server_slug: r.server_slug,
            server_name: r.server_name,
            bundle_slug: r.bundle_slug,
        })
        .collect())
}

#[server]
async fn list_transitive_mcp_servers(
    cluster_id: String,
) -> Result<Vec<TransitiveMcpServerDisplay>, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let cid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if let Some(ids) = user
        .accessible_cluster_ids(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    {
        if !ids.contains(&cid) {
            return Err(ServerFnError::new("access denied"));
        }
    }

    // Resolve winning skill channels for this cluster (direct wins over bundle).
    #[derive(sqlx::FromRow)]
    struct WinRow {
        skill_channel_id: uuid::Uuid,
        slug: String,
        channel: String,
        is_direct: bool,
    }

    let skill_rows = sqlx::query_as::<_, WinRow>(
        "SELECT sc.id AS skill_channel_id, s.slug, sc.channel, true AS is_direct \
         FROM cluster_skills cs \
         JOIN skill_channels sc ON sc.id = cs.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE cs.cluster_id = $1 \
         UNION ALL \
         SELECT sc.id AS skill_channel_id, s.slug, sc.channel, false AS is_direct \
         FROM cluster_bundles cb \
         JOIN bundle_items bi ON bi.bundle_id = cb.bundle_id \
         JOIN skill_channels sc ON sc.id = bi.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE cb.cluster_id = $1",
    )
    .bind(cid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Dedup: direct wins per slug.
    let mut winners: std::collections::HashMap<String, (uuid::Uuid, String, String, bool)> =
        std::collections::HashMap::new();
    for r in &skill_rows {
        match winners.get(&r.slug) {
            Some((_, _, _, true)) => {}
            _ => {
                winners.insert(
                    r.slug.clone(),
                    (
                        r.skill_channel_id,
                        r.slug.clone(),
                        r.channel.clone(),
                        r.is_direct,
                    ),
                );
            }
        }
    }

    let channel_ids: Vec<uuid::Uuid> = winners.values().map(|(id, _, _, _)| *id).collect();
    if channel_ids.is_empty() {
        return Ok(Vec::new());
    }

    // Build a map from channel_id -> (slug, channel) for labeling.
    let channel_info: std::collections::HashMap<uuid::Uuid, (String, String)> = winners
        .into_values()
        .map(|(id, slug, channel, _)| (id, (slug, channel)))
        .collect();

    #[derive(sqlx::FromRow)]
    struct DepRow {
        skill_channel_id: uuid::Uuid,
        server_slug: String,
        server_name: String,
    }

    let dep_rows = sqlx::query_as::<_, DepRow>(
        "SELECT smd.skill_channel_id, ms.slug as server_slug, ms.name as server_name \
         FROM skill_mcp_dependencies smd \
         JOIN mcp_servers ms ON ms.id = smd.mcp_server_id \
         WHERE smd.skill_channel_id = ANY($1) \
         ORDER BY ms.slug",
    )
    .bind(&channel_ids)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Overwritten if a direct or bundle assignment exists for the same server slug.
    let higher_slugs: std::collections::HashSet<String> = sqlx::query_scalar::<_, String>(
        "SELECT ms.slug \
         FROM cluster_mcp_servers cms \
         JOIN mcp_servers ms ON ms.id = cms.mcp_server_id \
         WHERE cms.cluster_id = $1 \
         UNION \
         SELECT ms.slug \
         FROM cluster_mcp_bundles cmb \
         JOIN mcp_server_bundle_items msbi ON msbi.bundle_id = cmb.bundle_id \
         JOIN mcp_servers ms ON ms.id = msbi.mcp_server_id \
         WHERE cmb.cluster_id = $1",
    )
    .bind(cid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .into_iter()
    .collect();

    let result = dep_rows
        .into_iter()
        .filter_map(|r| {
            let (skill_slug, channel) = channel_info.get(&r.skill_channel_id)?;
            Some(TransitiveMcpServerDisplay {
                overwritten: higher_slugs.contains(&r.server_slug),
                server_slug: r.server_slug,
                server_name: r.server_name,
                skill_slug: skill_slug.clone(),
                channel: channel.clone(),
            })
        })
        .collect();

    Ok(result)
}

#[server]
async fn list_all_mcp_servers() -> Result<Vec<McpServerOption>, ServerFnError> {
    let _user = current_user().await?;
    let pool = crate::server_pool()?;
    let servers = sqlx::query_as::<_, McpServerOption>(
        "SELECT id, slug, name FROM mcp_servers ORDER BY slug",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(servers)
}

#[server]
async fn list_all_mcp_bundles() -> Result<Vec<McpBundleOption>, ServerFnError> {
    let _user = current_user().await?;
    let pool = crate::server_pool()?;
    let bundles = sqlx::query_as::<_, McpBundleOption>(
        "SELECT id, slug, name FROM mcp_server_bundles ORDER BY slug",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(bundles)
}

#[server]
async fn add_cluster_mcp_server(
    cluster_id: String,
    mcp_server_id: Option<String>,
    skill_center_id: Option<String>,
    remote_id: Option<String>,
    slug: Option<String>,
    mcp_name: Option<String>,
) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let cid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if let Some(ids) = user
        .writable_cluster_ids(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    {
        if !ids.contains(&cid) {
            return Err(ServerFnError::new("access denied"));
        }
    }
    if let Some(sc_id_str) = skill_center_id {
        let sc_id: uuid::Uuid = sc_id_str
            .parse()
            .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
        let r_id: uuid::Uuid = remote_id
            .ok_or_else(|| ServerFnError::new("remote_id required for remote MCP server"))?
            .parse()
            .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
        let slug =
            slug.ok_or_else(|| ServerFnError::new("slug required for remote MCP server"))?;
        sqlx::query(
            "INSERT INTO cluster_mcp_servers (cluster_id, skill_center_id, remote_id, slug, mcp_name) \
             VALUES ($1, $2, $3, $4, $5) ON CONFLICT DO NOTHING",
        )
        .bind(cid)
        .bind(sc_id)
        .bind(r_id)
        .bind(&slug)
        .bind(mcp_name.as_deref())
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    } else {
        let msid: uuid::Uuid = mcp_server_id
            .ok_or_else(|| ServerFnError::new("mcp_server_id required for local MCP server"))?
            .parse()
            .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
        sqlx::query(
            "INSERT INTO cluster_mcp_servers (cluster_id, mcp_server_id) VALUES ($1, $2)",
        )
        .bind(cid)
        .bind(msid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    }
    crate::api::push::notify_global(cid, crate::api::push::PushMessage::SyncMcpServers).await;
    Ok(())
}

#[server]
async fn remove_cluster_mcp_server(cluster_mcp_server_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_mcp_server_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    // Check access before deleting
    let owner_cid = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT cluster_id FROM cluster_mcp_servers WHERE id = $1",
    )
    .bind(uuid)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    if let Some(owner_cid) = owner_cid {
        if let Some(ids) = user
            .writable_cluster_ids(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
        {
            if !ids.contains(&owner_cid) {
                return Err(ServerFnError::new("access denied"));
            }
        }
    }
    let cid = sqlx::query_scalar::<_, uuid::Uuid>(
        "DELETE FROM cluster_mcp_servers WHERE id = $1 RETURNING cluster_id",
    )
    .bind(uuid)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    if let Some(cid) = cid {
        crate::api::push::notify_global(cid, crate::api::push::PushMessage::SyncMcpServers).await;
    }
    Ok(())
}

#[server]
async fn add_cluster_mcp_bundle(
    cluster_id: String,
    bundle_id: Option<String>,
    skill_center_id: Option<String>,
    remote_id: Option<String>,
    slug: Option<String>,
    bundle_name: Option<String>,
) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let cid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if let Some(ids) = user
        .writable_cluster_ids(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    {
        if !ids.contains(&cid) {
            return Err(ServerFnError::new("access denied"));
        }
    }
    if let Some(sc_id_str) = skill_center_id {
        let sc_id: uuid::Uuid = sc_id_str
            .parse()
            .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
        let r_id: uuid::Uuid = remote_id
            .ok_or_else(|| ServerFnError::new("remote_id required for remote MCP bundle"))?
            .parse()
            .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
        let slug =
            slug.ok_or_else(|| ServerFnError::new("slug required for remote MCP bundle"))?;
        sqlx::query(
            "INSERT INTO cluster_mcp_bundles (cluster_id, skill_center_id, remote_id, slug, bundle_name) \
             VALUES ($1, $2, $3, $4, $5) ON CONFLICT DO NOTHING",
        )
        .bind(cid)
        .bind(sc_id)
        .bind(r_id)
        .bind(&slug)
        .bind(bundle_name.as_deref())
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    } else {
        let bid: uuid::Uuid = bundle_id
            .ok_or_else(|| ServerFnError::new("bundle_id required for local MCP bundle"))?
            .parse()
            .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

        // Check for overlap: does the new bundle share any mcp_server with
        // any bundle already assigned to this cluster?
        let overlap = sqlx::query_scalar::<_, String>(
            "SELECT ms.slug \
             FROM mcp_server_bundle_items new_bi \
             JOIN mcp_server_bundle_items existing_bi ON existing_bi.mcp_server_id = new_bi.mcp_server_id \
             JOIN cluster_mcp_bundles cmb ON cmb.bundle_id = existing_bi.bundle_id AND cmb.cluster_id = $1 \
             JOIN mcp_servers ms ON ms.id = new_bi.mcp_server_id \
             WHERE new_bi.bundle_id = $2 \
             LIMIT 1",
        )
        .bind(cid)
        .bind(bid)
        .fetch_optional(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

        if let Some(conflicting) = overlap {
            return Err(ServerFnError::new(format!(
                "bundle conflicts with an already-assigned bundle on MCP server: {conflicting}"
            )));
        }

        sqlx::query("INSERT INTO cluster_mcp_bundles (cluster_id, bundle_id) VALUES ($1, $2)")
            .bind(cid)
            .bind(bid)
            .execute(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    }
    crate::api::push::notify_global(cid, crate::api::push::PushMessage::SyncMcpServers).await;
    Ok(())
}

#[server]
async fn remove_cluster_mcp_bundle(cluster_mcp_bundle_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_mcp_bundle_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    // Check access before deleting
    let owner_cid = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT cluster_id FROM cluster_mcp_bundles WHERE id = $1",
    )
    .bind(uuid)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    if let Some(owner_cid) = owner_cid {
        if let Some(ids) = user
            .writable_cluster_ids(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
        {
            if !ids.contains(&owner_cid) {
                return Err(ServerFnError::new("access denied"));
            }
        }
    }
    let cid = sqlx::query_scalar::<_, uuid::Uuid>(
        "DELETE FROM cluster_mcp_bundles WHERE id = $1 RETURNING cluster_id",
    )
    .bind(uuid)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    if let Some(cid) = cid {
        crate::api::push::notify_global(cid, crate::api::push::PushMessage::SyncMcpServers).await;
    }
    Ok(())
}

/// Remote MCP server option for add-item dropdown (from skill center catalogs).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RemoteMcpServerOption {
    pub skill_center_id: String,
    pub skill_center_name: String,
    pub remote_mcp_server_id: String,
    pub slug: String,
    pub name: String,
}

/// Remote MCP bundle option for add-item dropdown (from skill center catalogs).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RemoteMcpBundleOption {
    pub skill_center_id: String,
    pub skill_center_name: String,
    pub remote_bundle_id: String,
    pub slug: String,
    pub name: String,
}

#[server]
async fn list_remote_mcp_server_options() -> Result<Vec<RemoteMcpServerOption>, ServerFnError> {
    let _user = current_user().await?;
    let pool = crate::server_pool()?;
    let cache = crate::skill_center_cache::SkillCenterCache::global()
        .ok_or_else(|| ServerFnError::new("skill center cache not initialized"))?;

    #[derive(sqlx::FromRow)]
    struct ScName {
        id: uuid::Uuid,
        name: String,
    }
    let sc_rows: Vec<ScName> =
        sqlx::query_as("SELECT id, name FROM skill_centers WHERE enabled = true")
            .fetch_all(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    let sc_names: std::collections::HashMap<uuid::Uuid, String> =
        sc_rows.into_iter().map(|r| (r.id, r.name)).collect();

    let catalogs = cache.get_all().await;
    let mut result = Vec::new();
    for (sc_id, cached) in &catalogs {
        let sc_name = sc_names.get(sc_id).cloned().unwrap_or_default();
        if sc_name.is_empty() {
            continue;
        }
        for m in &cached.catalog.mcp_servers {
            if m.hidden {
                continue;
            }
            result.push(RemoteMcpServerOption {
                skill_center_id: sc_id.to_string(),
                skill_center_name: sc_name.clone(),
                remote_mcp_server_id: m.id.to_string(),
                slug: m.slug.clone(),
                name: m.name.clone(),
            });
        }
    }
    result.sort_by(|a, b| {
        a.skill_center_name
            .cmp(&b.skill_center_name)
            .then(a.slug.cmp(&b.slug))
    });
    Ok(result)
}

#[server]
async fn list_remote_mcp_bundle_options() -> Result<Vec<RemoteMcpBundleOption>, ServerFnError> {
    let _user = current_user().await?;
    let pool = crate::server_pool()?;
    let cache = crate::skill_center_cache::SkillCenterCache::global()
        .ok_or_else(|| ServerFnError::new("skill center cache not initialized"))?;

    #[derive(sqlx::FromRow)]
    struct ScName {
        id: uuid::Uuid,
        name: String,
    }
    let sc_rows: Vec<ScName> =
        sqlx::query_as("SELECT id, name FROM skill_centers WHERE enabled = true")
            .fetch_all(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    let sc_names: std::collections::HashMap<uuid::Uuid, String> =
        sc_rows.into_iter().map(|r| (r.id, r.name)).collect();

    let catalogs = cache.get_all().await;
    let mut result = Vec::new();
    for (sc_id, cached) in &catalogs {
        let sc_name = sc_names.get(sc_id).cloned().unwrap_or_default();
        if sc_name.is_empty() {
            continue;
        }
        for b in &cached.catalog.mcp_bundles {
            if b.hidden {
                continue;
            }
            result.push(RemoteMcpBundleOption {
                skill_center_id: sc_id.to_string(),
                skill_center_name: sc_name.clone(),
                remote_bundle_id: b.id.to_string(),
                slug: b.slug.clone(),
                name: b.name.clone(),
            });
        }
    }
    result.sort_by(|a, b| {
        a.skill_center_name
            .cmp(&b.skill_center_name)
            .then(a.slug.cmp(&b.slug))
    });
    Ok(result)
}

#[component]
pub fn ClusterMcpServers(cluster_id: String, read_only: bool) -> Element {
    let cid_servers = cluster_id.clone();
    let mut servers = use_server_future(move || {
        let cid = cid_servers.clone();
        async move { list_cluster_mcp_servers(cid).await }
    })?;

    let cid_bundles = cluster_id.clone();
    let mut bundles = use_server_future(move || {
        let cid = cid_bundles.clone();
        async move { list_cluster_mcp_bundles(cid).await }
    })?;

    let cid_bmcps = cluster_id.clone();
    let bundle_mcps = use_server_future(move || {
        let cid = cid_bmcps.clone();
        async move { list_bundle_mcp_servers(cid).await }
    })?;

    let cid_tmcps = cluster_id.clone();
    let transitive_mcps = use_server_future(move || {
        let cid = cid_tmcps.clone();
        async move { list_transitive_mcp_servers(cid).await }
    })?;

    let available_servers = use_server_future(list_all_mcp_servers)?;
    let available_bundles = use_server_future(list_all_mcp_bundles)?;
    let available_remote_servers = use_server_future(list_remote_mcp_server_options)?;
    let available_remote_mcp_bundles = use_server_future(list_remote_mcp_bundle_options)?;

    let mut selected_server = use_signal(String::new);
    let mut selected_bundle = use_signal(String::new);
    let mut bundle_error = use_signal(|| None::<String>);

    let cid_add_server = cluster_id.clone();
    let cid_add_bundle = cluster_id.clone();

    rsx! {
        // Direct MCP server assignments
        div { class: "mb-4",
            h4 { class: "text-sm font-semibold text-fg-strong mb-2", {t!("cluster-mcp-direct")} }
            if !read_only {
                form {
                    class: "flex gap-2 mb-3",
                    onsubmit: move |evt: FormEvent| {
                        evt.prevent_default();
                        let cid = cid_add_server.clone();
                        let val = selected_server.read().clone();
                        spawn(async move {
                            if val.is_empty() {
                                return;
                            }
                            if let Some(rest) = val.strip_prefix("remote|") {
                                let parts: Vec<&str> = rest.splitn(4, '|').collect();
                                if parts.len() == 4 {
                                    if add_cluster_mcp_server(
                                        cid,
                                        None,
                                        Some(parts[0].to_string()),
                                        Some(parts[1].to_string()),
                                        Some(parts[2].to_string()),
                                        Some(parts[3].to_string()),
                                    )
                                    .await
                                    .is_ok()
                                    {
                                        selected_server.set(String::new());
                                        servers.restart();
                                    }
                                }
                            } else if add_cluster_mcp_server(cid, Some(val), None, None, None, None).await.is_ok() {
                                selected_server.set(String::new());
                                servers.restart();
                            }
                        });
                    },
                    select { class: "input flex-1 w-auto py-1 text-sm",
                        value: "{selected_server}",
                        onchange: move |evt| selected_server.set(evt.value()),
                        option { value: "", {t!("cluster-mcp-select")} }
                        {match &*available_servers.read() {
                            Some(Ok(list)) if !list.is_empty() => rsx! {
                                optgroup { label: t!("cluster-mcp-local"),
                                    for s in list {
                                        {
                                            let val = s.id.to_string();
                                            let label = format!("{} ({})", s.name, s.slug);
                                            rsx! { option { value: "{val}", "{label}" } }
                                        }
                                    }
                                }
                            },
                            _ => rsx! {},
                        }}
                        {match &*available_remote_servers.read() {
                            Some(Ok(list)) if !list.is_empty() => {
                                let mut by_sc: std::collections::BTreeMap<String, Vec<&RemoteMcpServerOption>> = std::collections::BTreeMap::new();
                                for rm in list.iter() {
                                    by_sc.entry(rm.skill_center_name.clone()).or_default().push(rm);
                                }
                                rsx! {
                                    for (sc_name, items) in by_sc {
                                        optgroup { label: t!("cluster-mcp-from-sc", name: sc_name.clone()),
                                            for rm in items {
                                                {
                                                    let val = format!(
                                                        "remote|{}|{}|{}|{}",
                                                        rm.skill_center_id, rm.remote_mcp_server_id,
                                                        rm.slug, rm.name
                                                    );
                                                    let label = format!("{} ({})", rm.name, rm.slug);
                                                    rsx! { option { value: "{val}", "{label}" } }
                                                }
                                            }
                                        }
                                    }
                                }
                            },
                            _ => rsx! {},
                        }}
                    }
                    Button { kind: ButtonKind::Submit, size: ButtonSize::Sm,
                        {t!("add")}
                    }
                }
            }
            {match &*servers.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    HelpText { {t!("cluster-mcp-no-direct")} }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-line-soft",
                        for cs in list {
                            {
                                let csid = cs.cluster_mcp_server_id.to_string();
                                let label = format!("{} ({})", cs.server_name, cs.server_slug);
                                let is_remote = cs.skill_center_name.is_some();
                                let via = cs.skill_center_name.clone().unwrap_or_default();
                                rsx! {
                                    li { class: "py-2 flex justify-between items-center",
                                        span { class: "flex items-center gap-2",
                                            span {
                                                class: if is_remote { "text-sm font-mono text-fg-muted" } else { "text-sm font-mono" },
                                                "{label}"
                                            }
                                            if is_remote {
                                                span { class: "text-xs text-fg-faint", {t!("cluster-mcp-via", source: via.clone())} }
                                            }
                                        }
                                        if !read_only {
                                            button { class: "link-danger text-sm",
                                                onclick: move |_| {
                                                    let csid = csid.clone();
                                                    spawn(async move {
                                                        if remove_cluster_mcp_server(csid).await.is_ok() {
                                                            servers.restart();
                                                        }
                                                    });
                                                },
                                                {t!("remove")}
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! { ErrorText { "Error: {e}" } },
                None => rsx! { HelpText { "Loading..." } },
            }}
        }

        // MCP servers from bundles (read-only, blue)
        div { class: "mb-4",
            h4 { class: "text-sm font-semibold text-info mb-2", {t!("cluster-mcp-from-bundles")} }
            {match &*bundle_mcps.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    HelpText { {t!("cluster-mcp-no-bundle-mcp")} }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-line-soft",
                        for bm in list {
                            {
                                let label = format!("{} ({})", bm.server_name, bm.server_slug);
                                let via = bm.bundle_slug.clone();
                                let overwritten = bm.overwritten;
                                rsx! {
                                    li { class: "py-2 flex items-center gap-2",
                                        span {
                                            class: if overwritten { "text-sm font-mono text-info opacity-50 line-through" } else { "text-sm font-mono text-info" },
                                            "{label}"
                                        }
                                        span { class: if overwritten { "text-xs text-info opacity-50" } else { "text-xs text-info" }, {t!("cluster-mcp-via", source: via.clone())} }
                                        if overwritten {
                                            span { class: "text-xs text-fg-faint italic", {t!("cluster-mcp-overwritten")} }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! { ErrorText { "Error: {e}" } },
                None => rsx! { HelpText { "Loading..." } },
            }}
        }

        // MCP servers from skills (transitive, read-only, grey)
        div { class: "mb-4",
            h4 { class: "text-sm font-semibold text-fg-muted mb-2", {t!("cluster-mcp-from-skills")} }
            {match &*transitive_mcps.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    HelpText { {t!("cluster-mcp-no-transitive")} }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-line-soft",
                        for tm in list {
                            {
                                let label = format!("{} ({})", tm.server_name, tm.server_slug);
                                let via = format!("{} / {}", tm.skill_slug, tm.channel);
                                let overwritten = tm.overwritten;
                                rsx! {
                                    li { class: "py-2 flex items-center gap-2",
                                        span {
                                            class: if overwritten { "text-sm font-mono text-fg-muted opacity-50 line-through" } else { "text-sm font-mono text-fg-muted" },
                                            "{label}"
                                        }
                                        span { class: if overwritten { "text-xs text-fg-faint opacity-50" } else { "text-xs text-fg-faint" }, {t!("cluster-mcp-via", source: via.clone())} }
                                        if overwritten {
                                            span { class: "text-xs text-fg-faint italic", {t!("cluster-mcp-overwritten")} }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! { ErrorText { "Error: {e}" } },
                None => rsx! { HelpText { "Loading..." } },
            }}
        }

        // MCP bundle assignments
        div { class: "mb-4",
            h4 { class: "text-sm font-semibold text-fg-strong mb-2", {t!("cluster-mcp-bundles-title")} }
            if !read_only {
                if let Some(err) = &*bundle_error.read() {
                    ErrorText { class: "mb-2", "{err}" }
                }
                form {
                    class: "flex gap-2 mb-3",
                    onsubmit: move |evt: FormEvent| {
                        evt.prevent_default();
                        let cid = cid_add_bundle.clone();
                        let val = selected_bundle.read().clone();
                        spawn(async move {
                            if val.is_empty() {
                                return;
                            }
                            if let Some(rest) = val.strip_prefix("remote|") {
                                let parts: Vec<&str> = rest.splitn(4, '|').collect();
                                if parts.len() == 4 {
                                    match add_cluster_mcp_bundle(
                                        cid,
                                        None,
                                        Some(parts[0].to_string()),
                                        Some(parts[1].to_string()),
                                        Some(parts[2].to_string()),
                                        Some(parts[3].to_string()),
                                    )
                                    .await
                                    {
                                        Ok(()) => {
                                            bundle_error.set(None);
                                            selected_bundle.set(String::new());
                                            bundles.restart();
                                        }
                                        Err(e) => {
                                            bundle_error.set(Some(e.to_string()));
                                        }
                                    }
                                }
                            } else {
                                match add_cluster_mcp_bundle(cid, Some(val), None, None, None, None).await {
                                    Ok(()) => {
                                        bundle_error.set(None);
                                        selected_bundle.set(String::new());
                                        bundles.restart();
                                    }
                                    Err(e) => {
                                        bundle_error.set(Some(e.to_string()));
                                    }
                                }
                            }
                        });
                    },
                    select { class: "input flex-1 w-auto py-1 text-sm",
                        value: "{selected_bundle}",
                        onchange: move |evt| selected_bundle.set(evt.value()),
                        option { value: "", {t!("cluster-mcp-select-bundle")} }
                        {match &*available_bundles.read() {
                            Some(Ok(list)) if !list.is_empty() => rsx! {
                                optgroup { label: t!("cluster-mcp-local"),
                                    for b in list {
                                        {
                                            let val = b.id.to_string();
                                            let label = format!("{} ({})", b.name, b.slug);
                                            rsx! { option { value: "{val}", "{label}" } }
                                        }
                                    }
                                }
                            },
                            _ => rsx! {},
                        }}
                        {match &*available_remote_mcp_bundles.read() {
                            Some(Ok(list)) if !list.is_empty() => {
                                let mut by_sc: std::collections::BTreeMap<String, Vec<&RemoteMcpBundleOption>> = std::collections::BTreeMap::new();
                                for rb in list.iter() {
                                    by_sc.entry(rb.skill_center_name.clone()).or_default().push(rb);
                                }
                                rsx! {
                                    for (sc_name, items) in by_sc {
                                        optgroup { label: t!("cluster-mcp-from-sc", name: sc_name.clone()),
                                            for rb in items {
                                                {
                                                    let val = format!(
                                                        "remote|{}|{}|{}|{}",
                                                        rb.skill_center_id, rb.remote_bundle_id,
                                                        rb.slug, rb.name
                                                    );
                                                    let label = format!("{} ({})", rb.name, rb.slug);
                                                    rsx! { option { value: "{val}", "{label}" } }
                                                }
                                            }
                                        }
                                    }
                                }
                            },
                            _ => rsx! {},
                        }}
                    }
                    Button { kind: ButtonKind::Submit, size: ButtonSize::Sm,
                        {t!("add")}
                    }
                }
            }
            {match &*bundles.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    HelpText { {t!("cluster-mcp-no-bundle-assign")} }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-line-soft",
                        for cb in list {
                            {
                                let cbid = cb.cluster_mcp_bundle_id.to_string();
                                let label = format!("{} ({})", cb.bundle_name, cb.bundle_slug);
                                let is_remote = cb.skill_center_name.is_some();
                                let via = cb.skill_center_name.clone().unwrap_or_default();
                                rsx! {
                                    li { class: "py-2 flex justify-between items-center",
                                        span { class: "flex items-center gap-2",
                                            span {
                                                class: if is_remote { "text-sm text-fg-muted" } else { "text-sm" },
                                                "{label}"
                                            }
                                            if is_remote {
                                                span { class: "text-xs text-fg-faint", {t!("cluster-mcp-via", source: via.clone())} }
                                            }
                                        }
                                        if !read_only {
                                            button { class: "link-danger text-sm",
                                                onclick: move |_| {
                                                    let cbid = cbid.clone();
                                                    spawn(async move {
                                                        if remove_cluster_mcp_bundle(cbid).await.is_ok() {
                                                            bundles.restart();
                                                        }
                                                    });
                                                },
                                                {t!("remove")}
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! { ErrorText { "Error: {e}" } },
                None => rsx! { HelpText { "Loading..." } },
            }}
        }
    }
}
