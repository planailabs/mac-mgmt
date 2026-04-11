use dioxus::prelude::*;

#[cfg(feature = "server")]
use crate::web::user::current_user;
use super::mcp_bundle_detail::McpServerOption;

/// Direct MCP server assignment display.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ClusterMcpServerDisplay {
    pub cluster_mcp_server_id: uuid::Uuid,
    pub server_slug: String,
    pub server_name: String,
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
async fn list_cluster_mcp_servers(cluster_id: String) -> Result<Vec<ClusterMcpServerDisplay>, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if let Some(ids) = user.accessible_cluster_ids(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))? {
        if !ids.contains(&uuid) {
            return Err(ServerFnError::new("access denied"));
        }
    }
    let servers = sqlx::query_as::<_, ClusterMcpServerDisplay>(
        "SELECT cms.id as cluster_mcp_server_id, ms.slug as server_slug, ms.name as server_name \
         FROM cluster_mcp_servers cms \
         JOIN mcp_servers ms ON ms.id = cms.mcp_server_id \
         WHERE cms.cluster_id = $1 \
         ORDER BY ms.slug",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(servers)
}

#[server]
async fn list_cluster_mcp_bundles(cluster_id: String) -> Result<Vec<ClusterMcpBundleDisplay>, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if let Some(ids) = user.accessible_cluster_ids(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))? {
        if !ids.contains(&uuid) {
            return Err(ServerFnError::new("access denied"));
        }
    }
    let bundles = sqlx::query_as::<_, ClusterMcpBundleDisplay>(
        "SELECT cmb.id as cluster_mcp_bundle_id, msb.slug as bundle_slug, msb.name as bundle_name \
         FROM cluster_mcp_bundles cmb \
         JOIN mcp_server_bundles msb ON msb.id = cmb.bundle_id \
         WHERE cmb.cluster_id = $1 \
         ORDER BY msb.slug",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(bundles)
}

#[server]
async fn list_bundle_mcp_servers(cluster_id: String) -> Result<Vec<BundleMcpServerDisplay>, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if let Some(ids) = user.accessible_cluster_ids(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))? {
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
async fn list_transitive_mcp_servers(cluster_id: String) -> Result<Vec<TransitiveMcpServerDisplay>, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let cid: uuid::Uuid = cluster_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if let Some(ids) = user.accessible_cluster_ids(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))? {
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
                    (r.skill_channel_id, r.slug.clone(), r.channel.clone(), r.is_direct),
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
async fn add_cluster_mcp_server(cluster_id: String, mcp_server_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let cid: uuid::Uuid = cluster_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if let Some(ids) = user.accessible_cluster_ids(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))? {
        if !ids.contains(&cid) {
            return Err(ServerFnError::new("access denied"));
        }
    }
    let msid: uuid::Uuid = mcp_server_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("INSERT INTO cluster_mcp_servers (cluster_id, mcp_server_id) VALUES ($1, $2)")
        .bind(cid)
        .bind(msid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    crate::api::push::notify_global(cid, crate::api::push::PushMessage::SyncMcpServers).await;
    Ok(())
}

#[server]
async fn remove_cluster_mcp_server(cluster_mcp_server_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_mcp_server_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    // Check access before deleting
    let owner_cid = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT cluster_id FROM cluster_mcp_servers WHERE id = $1",
    )
    .bind(uuid)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    if let Some(owner_cid) = owner_cid {
        if let Some(ids) = user.accessible_cluster_ids(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))? {
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
async fn add_cluster_mcp_bundle(cluster_id: String, bundle_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let cid: uuid::Uuid = cluster_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if let Some(ids) = user.accessible_cluster_ids(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))? {
        if !ids.contains(&cid) {
            return Err(ServerFnError::new("access denied"));
        }
    }
    let bid: uuid::Uuid = bundle_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

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
    crate::api::push::notify_global(cid, crate::api::push::PushMessage::SyncMcpServers).await;
    Ok(())
}

#[server]
async fn remove_cluster_mcp_bundle(cluster_mcp_bundle_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_mcp_bundle_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    // Check access before deleting
    let owner_cid = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT cluster_id FROM cluster_mcp_bundles WHERE id = $1",
    )
    .bind(uuid)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    if let Some(owner_cid) = owner_cid {
        if let Some(ids) = user.accessible_cluster_ids(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))? {
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

#[component]
pub fn ClusterMcpServers(cluster_id: String) -> Element {
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

    let mut selected_server = use_signal(String::new);
    let mut selected_bundle = use_signal(String::new);
    let mut bundle_error = use_signal(|| None::<String>);

    let cid_add_server = cluster_id.clone();
    let cid_add_bundle = cluster_id.clone();

    rsx! {
        // Direct MCP server assignments
        div { class: "mb-4",
            h4 { class: "text-sm font-semibold text-gray-700 dark:text-gray-200 mb-2", "Direct MCP Servers" }
            form {
                class: "flex gap-2 mb-3",
                onsubmit: move |evt: FormEvent| {
                    evt.prevent_default();
                    let cid = cid_add_server.clone();
                    let msid = selected_server.read().clone();
                    spawn(async move {
                        if !msid.is_empty() {
                            if add_cluster_mcp_server(cid, msid).await.is_ok() {
                                selected_server.set(String::new());
                                servers.restart();
                            }
                        }
                    });
                },
                select {
                    class: "flex-1 border border-gray-300 dark:border-gray-600 rounded px-2 py-1 text-sm dark:bg-gray-700 dark:text-white",
                    value: "{selected_server}",
                    onchange: move |evt| selected_server.set(evt.value()),
                    option { value: "", "Select MCP server..." }
                    {match &*available_servers.read() {
                        Some(Ok(list)) => rsx! {
                            for s in list {
                                {
                                    let val = s.id.to_string();
                                    let label = format!("{} ({})", s.name, s.slug);
                                    rsx! { option { value: "{val}", "{label}" } }
                                }
                            }
                        },
                        _ => rsx! {},
                    }}
                }
                button {
                    class: "bg-blue-600 text-white px-3 py-1 rounded text-sm hover:bg-blue-700",
                    r#type: "submit",
                    "Add"
                }
            }
            {match &*servers.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    p { class: "text-gray-500 dark:text-gray-400 text-sm", "No direct MCP server assignments." }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-gray-200 dark:divide-gray-700",
                        for cs in list {
                            {
                                let csid = cs.cluster_mcp_server_id.to_string();
                                let label = format!("{} ({})", cs.server_name, cs.server_slug);
                                rsx! {
                                    li { class: "py-2 flex justify-between items-center",
                                        span { class: "text-sm font-mono", "{label}" }
                                        button {
                                            class: "text-red-600 dark:text-red-400 hover:text-red-700 dark:hover:text-red-300 text-sm",
                                            onclick: move |_| {
                                                let csid = csid.clone();
                                                spawn(async move {
                                                    if remove_cluster_mcp_server(csid).await.is_ok() {
                                                        servers.restart();
                                                    }
                                                });
                                            },
                                            "Remove"
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400 text-sm", "Error: {e}" } },
                None => rsx! { p { class: "text-gray-500 dark:text-gray-400 text-sm", "Loading..." } },
            }}
        }

        // MCP servers from bundles (read-only, blue)
        div { class: "mb-4",
            h4 { class: "text-sm font-semibold text-blue-700 dark:text-blue-400 mb-2", "From Bundles" }
            {match &*bundle_mcps.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    p { class: "text-gray-500 dark:text-gray-400 text-sm", "No MCP servers from bundles." }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-gray-200 dark:divide-gray-700",
                        for bm in list {
                            {
                                let label = format!("{} ({})", bm.server_name, bm.server_slug);
                                let via = bm.bundle_slug.clone();
                                let overwritten = bm.overwritten;
                                rsx! {
                                    li { class: "py-2 flex items-center gap-2",
                                        span {
                                            class: if overwritten { "text-sm font-mono text-blue-400 dark:text-blue-600 line-through" } else { "text-sm font-mono text-blue-700 dark:text-blue-400" },
                                            "{label}"
                                        }
                                        span { class: if overwritten { "text-xs text-blue-300" } else { "text-xs text-blue-500" }, "via {via}" }
                                        if overwritten {
                                            span { class: "text-xs text-gray-400 dark:text-gray-500 italic", "overwritten" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400 text-sm", "Error: {e}" } },
                None => rsx! { p { class: "text-gray-500 dark:text-gray-400 text-sm", "Loading..." } },
            }}
        }

        // MCP servers from skills (transitive, read-only, grey)
        div { class: "mb-4",
            h4 { class: "text-sm font-semibold text-gray-500 dark:text-gray-400 mb-2", "From Skills (transitive)" }
            {match &*transitive_mcps.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    p { class: "text-gray-500 dark:text-gray-400 text-sm", "No transitive MCP dependencies." }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-gray-200 dark:divide-gray-700",
                        for tm in list {
                            {
                                let label = format!("{} ({})", tm.server_name, tm.server_slug);
                                let via = format!("{} / {}", tm.skill_slug, tm.channel);
                                let overwritten = tm.overwritten;
                                rsx! {
                                    li { class: "py-2 flex items-center gap-2",
                                        span {
                                            class: if overwritten { "text-sm font-mono text-gray-400 dark:text-gray-500 line-through" } else { "text-sm font-mono text-gray-500 dark:text-gray-400" },
                                            "{label}"
                                        }
                                        span { class: if overwritten { "text-xs text-gray-300 dark:text-gray-600" } else { "text-xs text-gray-400 dark:text-gray-500" }, "via {via}" }
                                        if overwritten {
                                            span { class: "text-xs text-gray-400 dark:text-gray-500 italic", "overwritten" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400 text-sm", "Error: {e}" } },
                None => rsx! { p { class: "text-gray-500 dark:text-gray-400 text-sm", "Loading..." } },
            }}
        }

        // MCP bundle assignments
        div {
            h4 { class: "text-sm font-semibold text-gray-700 dark:text-gray-200 mb-2", "MCP Bundles" }
            if let Some(err) = &*bundle_error.read() {
                p { class: "text-red-600 dark:text-red-400 text-sm mb-2", "{err}" }
            }
            form {
                class: "flex gap-2 mb-3",
                onsubmit: move |evt: FormEvent| {
                    evt.prevent_default();
                    let cid = cid_add_bundle.clone();
                    let bid = selected_bundle.read().clone();
                    spawn(async move {
                        if !bid.is_empty() {
                            match add_cluster_mcp_bundle(cid, bid).await {
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
                select {
                    class: "flex-1 border border-gray-300 dark:border-gray-600 rounded px-2 py-1 text-sm dark:bg-gray-700 dark:text-white",
                    value: "{selected_bundle}",
                    onchange: move |evt| selected_bundle.set(evt.value()),
                    option { value: "", "Select MCP bundle..." }
                    {match &*available_bundles.read() {
                        Some(Ok(list)) => rsx! {
                            for b in list {
                                {
                                    let val = b.id.to_string();
                                    let label = format!("{} ({})", b.name, b.slug);
                                    rsx! { option { value: "{val}", "{label}" } }
                                }
                            }
                        },
                        _ => rsx! {},
                    }}
                }
                button {
                    class: "bg-blue-600 text-white px-3 py-1 rounded text-sm hover:bg-blue-700",
                    r#type: "submit",
                    "Add"
                }
            }
            {match &*bundles.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    p { class: "text-gray-500 dark:text-gray-400 text-sm", "No MCP bundle assignments." }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-gray-200 dark:divide-gray-700",
                        for cb in list {
                            {
                                let cbid = cb.cluster_mcp_bundle_id.to_string();
                                let label = format!("{} ({})", cb.bundle_name, cb.bundle_slug);
                                rsx! {
                                    li { class: "py-2 flex justify-between items-center",
                                        span { class: "text-sm", "{label}" }
                                        button {
                                            class: "text-red-600 dark:text-red-400 hover:text-red-700 dark:hover:text-red-300 text-sm",
                                            onclick: move |_| {
                                                let cbid = cbid.clone();
                                                spawn(async move {
                                                    if remove_cluster_mcp_bundle(cbid).await.is_ok() {
                                                        bundles.restart();
                                                    }
                                                });
                                            },
                                            "Remove"
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400 text-sm", "Error: {e}" } },
                None => rsx! { p { class: "text-gray-500 dark:text-gray-400 text-sm", "Loading..." } },
            }}
        }
    }
}
