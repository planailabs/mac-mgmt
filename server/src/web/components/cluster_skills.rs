use dioxus::prelude::*;
use dioxus_i18n::t;

use super::bundle_detail::SkillChannelDisplay;
#[cfg(feature = "server")]
use crate::web::user::current_user;

/// Direct skill assignment display.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ClusterSkillDisplay {
    pub cluster_skill_id: uuid::Uuid,
    pub skill_slug: String,
    pub channel: String,
    #[serde(default)]
    pub skill_center_name: Option<String>,
}

/// Skill coming from a bundle (read-only).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BundleSkillDisplay {
    pub skill_slug: String,
    pub channel: String,
    pub bundle_slug: String,
    pub overwritten: bool,
}

/// Bundle assignment display.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct ClusterBundleDisplay {
    pub cluster_bundle_id: uuid::Uuid,
    pub bundle_slug: String,
    pub bundle_name: String,
    #[serde(default)]
    pub skill_center_name: Option<String>,
}

#[server]
async fn list_cluster_skills(
    cluster_id: String,
) -> Result<Vec<ClusterSkillDisplay>, ServerFnError> {
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
    let skills = sqlx::query_as::<_, ClusterSkillDisplay>(
        "SELECT cs.id AS cluster_skill_id, \
                COALESCE(s.slug, cs.slug) AS skill_slug, \
                COALESCE(sc_ch.channel, cs.channel) AS channel, \
                sk_center.name AS skill_center_name \
         FROM cluster_skills cs \
         LEFT JOIN skill_channels sc_ch ON sc_ch.id = cs.skill_channel_id \
         LEFT JOIN skills s ON s.id = sc_ch.skill_id \
         LEFT JOIN skill_centers sk_center ON sk_center.id = cs.skill_center_id \
         WHERE cs.cluster_id = $1 \
         ORDER BY COALESCE(s.slug, cs.slug), COALESCE(sc_ch.channel, cs.channel)",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(skills)
}

#[server]
async fn list_cluster_bundles(
    cluster_id: String,
) -> Result<Vec<ClusterBundleDisplay>, ServerFnError> {
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
    let bundles = sqlx::query_as::<_, ClusterBundleDisplay>(
        "SELECT cb.id AS cluster_bundle_id, \
                COALESCE(b.slug, cb.slug) AS bundle_slug, \
                COALESCE(b.name, cb.bundle_name) AS bundle_name, \
                sk_center.name AS skill_center_name \
         FROM cluster_bundles cb \
         LEFT JOIN bundles b ON b.id = cb.bundle_id \
         LEFT JOIN skill_centers sk_center ON sk_center.id = cb.skill_center_id \
         WHERE cb.cluster_id = $1 \
         ORDER BY COALESCE(b.slug, cb.slug)",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(bundles)
}

#[server]
async fn list_bundle_skills(cluster_id: String) -> Result<Vec<BundleSkillDisplay>, ServerFnError> {
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
        skill_slug: String,
        channel: String,
        bundle_slug: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT DISTINCT s.slug as skill_slug, sc.channel, b.slug as bundle_slug \
         FROM cluster_bundles cb \
         JOIN bundle_items bi ON bi.bundle_id = cb.bundle_id \
         JOIN skill_channels sc ON sc.id = bi.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         JOIN bundles b ON b.id = cb.bundle_id \
         WHERE cb.cluster_id = $1 \
         ORDER BY s.slug, sc.channel",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    // A bundle skill is overwritten if a direct assignment exists for the same slug.
    let direct_slugs: std::collections::HashSet<String> = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT s.slug \
         FROM cluster_skills cs \
         JOIN skill_channels sc ON sc.id = cs.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE cs.cluster_id = $1",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .into_iter()
    .collect();

    Ok(rows
        .into_iter()
        .map(|r| BundleSkillDisplay {
            overwritten: direct_slugs.contains(&r.skill_slug),
            skill_slug: r.skill_slug,
            channel: r.channel,
            bundle_slug: r.bundle_slug,
        })
        .collect())
}

#[server]
async fn list_all_skill_channels() -> Result<Vec<SkillChannelDisplay>, ServerFnError> {
    let _user = current_user().await?;
    let pool = crate::server_pool()?;
    let channels = sqlx::query_as::<_, SkillChannelDisplay>(
        "SELECT sc.id, s.slug as skill_slug, sc.channel \
         FROM skill_channels sc \
         JOIN skills s ON s.id = sc.skill_id \
         ORDER BY s.slug, sc.channel",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(channels)
}

/// All bundles for the assignment dropdown.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct BundleOption {
    pub id: uuid::Uuid,
    pub slug: String,
    pub name: String,
}

#[server]
async fn list_all_bundles() -> Result<Vec<BundleOption>, ServerFnError> {
    let _user = current_user().await?;
    let pool = crate::server_pool()?;
    let bundles =
        sqlx::query_as::<_, BundleOption>("SELECT id, slug, name FROM bundles ORDER BY slug")
            .fetch_all(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(bundles)
}

#[server]
async fn add_cluster_skill(
    cluster_id: String,
    skill_channel_id: Option<String>,
    skill_center_id: Option<String>,
    remote_id: Option<String>,
    slug: Option<String>,
    channel: Option<String>,
    skill_name: Option<String>,
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
            .ok_or_else(|| ServerFnError::new("remote_id required for remote skill"))?
            .parse()
            .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
        let slug = slug.ok_or_else(|| ServerFnError::new("slug required for remote skill"))?;
        let channel =
            channel.ok_or_else(|| ServerFnError::new("channel required for remote skill"))?;
        sqlx::query(
            "INSERT INTO cluster_skills (cluster_id, skill_center_id, remote_id, slug, channel, skill_name) \
             VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT DO NOTHING",
        )
        .bind(cid)
        .bind(sc_id)
        .bind(r_id)
        .bind(&slug)
        .bind(&channel)
        .bind(skill_name.as_deref())
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    } else {
        let scid: uuid::Uuid = skill_channel_id
            .ok_or_else(|| ServerFnError::new("skill_channel_id required for local skill"))?
            .parse()
            .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
        sqlx::query("INSERT INTO cluster_skills (cluster_id, skill_channel_id) VALUES ($1, $2)")
            .bind(cid)
            .bind(scid)
            .execute(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    }
    crate::api::push::notify_global(cid, crate::api::push::PushMessage::SyncSkills).await;
    Ok(())
}

#[server]
async fn remove_cluster_skill(cluster_skill_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_skill_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    // Check access before deleting
    let owner_cid =
        sqlx::query_scalar::<_, uuid::Uuid>("SELECT cluster_id FROM cluster_skills WHERE id = $1")
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
        "DELETE FROM cluster_skills WHERE id = $1 RETURNING cluster_id",
    )
    .bind(uuid)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    if let Some(cid) = cid {
        crate::api::push::notify_global(cid, crate::api::push::PushMessage::SyncSkills).await;
    }
    Ok(())
}

#[server]
async fn add_cluster_bundle(
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
            .ok_or_else(|| ServerFnError::new("remote_id required for remote bundle"))?
            .parse()
            .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
        let slug = slug.ok_or_else(|| ServerFnError::new("slug required for remote bundle"))?;
        sqlx::query(
            "INSERT INTO cluster_bundles (cluster_id, skill_center_id, remote_id, slug, bundle_name) \
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
            .ok_or_else(|| ServerFnError::new("bundle_id required for local bundle"))?
            .parse()
            .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

        // Check for overlap: does the new bundle share any skill_channel_id with
        // any bundle already assigned to this cluster?
        let overlap = sqlx::query_scalar::<_, String>(
            "SELECT s.slug || '/' || sc.channel \
             FROM bundle_items new_bi \
             JOIN bundle_items existing_bi ON existing_bi.skill_channel_id = new_bi.skill_channel_id \
             JOIN cluster_bundles cb ON cb.bundle_id = existing_bi.bundle_id AND cb.cluster_id = $1 \
             JOIN skill_channels sc ON sc.id = new_bi.skill_channel_id \
             JOIN skills s ON s.id = sc.skill_id \
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
                "bundle conflicts with an already-assigned bundle on skill channel: {conflicting}"
            )));
        }

        sqlx::query("INSERT INTO cluster_bundles (cluster_id, bundle_id) VALUES ($1, $2)")
            .bind(cid)
            .bind(bid)
            .execute(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    }
    crate::api::push::notify_global(cid, crate::api::push::PushMessage::SyncSkills).await;
    Ok(())
}

#[server]
async fn remove_cluster_bundle(cluster_bundle_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_bundle_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    // Check access before deleting
    let owner_cid =
        sqlx::query_scalar::<_, uuid::Uuid>("SELECT cluster_id FROM cluster_bundles WHERE id = $1")
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
        "DELETE FROM cluster_bundles WHERE id = $1 RETURNING cluster_id",
    )
    .bind(uuid)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    if let Some(cid) = cid {
        crate::api::push::notify_global(cid, crate::api::push::PushMessage::SyncSkills).await;
    }
    Ok(())
}

/// Remote skill option for add-item dropdown (from skill center catalogs).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RemoteSkillOption {
    pub skill_center_id: String,
    pub skill_center_name: String,
    pub remote_skill_channel_id: String,
    pub skill_slug: String,
    pub skill_name: String,
    pub channel: String,
}

/// Remote bundle option for add-item dropdown (from skill center catalogs).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RemoteBundleOption {
    pub skill_center_id: String,
    pub skill_center_name: String,
    pub remote_bundle_id: String,
    pub slug: String,
    pub name: String,
}

#[server]
async fn list_remote_skill_options() -> Result<Vec<RemoteSkillOption>, ServerFnError> {
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
            continue; // skill center disabled or deleted
        }
        for sc in &cached.catalog.skill_channels {
            if sc.hidden {
                continue;
            }
            result.push(RemoteSkillOption {
                skill_center_id: sc_id.to_string(),
                skill_center_name: sc_name.clone(),
                remote_skill_channel_id: sc.id.to_string(),
                skill_slug: sc.skill_slug.clone(),
                skill_name: sc.skill_name.clone(),
                channel: sc.channel.clone(),
            });
        }
    }
    result.sort_by(|a, b| {
        a.skill_center_name
            .cmp(&b.skill_center_name)
            .then(a.skill_slug.cmp(&b.skill_slug))
            .then(a.channel.cmp(&b.channel))
    });
    Ok(result)
}

#[server]
async fn list_remote_bundle_options() -> Result<Vec<RemoteBundleOption>, ServerFnError> {
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
        for b in &cached.catalog.bundles {
            if b.hidden {
                continue;
            }
            result.push(RemoteBundleOption {
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
pub fn ClusterSkills(cluster_id: String, read_only: bool) -> Element {
    let cid_skills = cluster_id.clone();
    let mut skills = use_server_future(move || {
        let cid = cid_skills.clone();
        async move { list_cluster_skills(cid).await }
    })?;

    let cid_bundles = cluster_id.clone();
    let mut bundles = use_server_future(move || {
        let cid = cid_bundles.clone();
        async move { list_cluster_bundles(cid).await }
    })?;

    let cid_bskills = cluster_id.clone();
    let bundle_skills = use_server_future(move || {
        let cid = cid_bskills.clone();
        async move { list_bundle_skills(cid).await }
    })?;

    let available_sc = use_server_future(list_all_skill_channels)?;
    let available_bundles = use_server_future(list_all_bundles)?;
    let available_remote_sc = use_server_future(list_remote_skill_options)?;
    let available_remote_bundles = use_server_future(list_remote_bundle_options)?;

    let mut selected_sc = use_signal(String::new);
    let mut selected_bundle = use_signal(String::new);
    let mut bundle_error = use_signal(|| None::<String>);

    let cid_add_skill = cluster_id.clone();
    let cid_add_bundle = cluster_id.clone();

    rsx! {
        // Direct skill assignments
        div { class: "mb-4",
            h4 { class: "text-sm font-semibold text-gray-700 dark:text-gray-200 mb-2", {t!("cluster-skills-direct")} }
            if !read_only {
                form {
                    class: "flex gap-2 mb-3",
                    onsubmit: move |evt: FormEvent| {
                        evt.prevent_default();
                        let cid = cid_add_skill.clone();
                        let val = selected_sc.read().clone();
                        spawn(async move {
                            if val.is_empty() {
                                return;
                            }
                            if let Some(rest) = val.strip_prefix("remote|") {
                                let parts: Vec<&str> = rest.splitn(5, '|').collect();
                                if parts.len() == 5 {
                                    if add_cluster_skill(
                                        cid,
                                        None,
                                        Some(parts[0].to_string()),
                                        Some(parts[1].to_string()),
                                        Some(parts[2].to_string()),
                                        Some(parts[3].to_string()),
                                        Some(parts[4].to_string()),
                                    )
                                    .await
                                    .is_ok()
                                    {
                                        selected_sc.set(String::new());
                                        skills.restart();
                                    }
                                }
                            } else if add_cluster_skill(cid, Some(val), None, None, None, None, None).await.is_ok() {
                                selected_sc.set(String::new());
                                skills.restart();
                            }
                        });
                    },
                    select {
                        class: "flex-1 border border-gray-300 dark:border-gray-600 rounded px-2 py-1 text-sm dark:bg-gray-700 dark:text-white",
                        value: "{selected_sc}",
                        onchange: move |evt| selected_sc.set(evt.value()),
                        option { value: "", {t!("cluster-skills-select")} }
                        {match &*available_sc.read() {
                            Some(Ok(list)) if !list.is_empty() => rsx! {
                                optgroup { label: t!("cluster-skills-local"),
                                    for sc in list {
                                        {
                                            let val = sc.id.to_string();
                                            let label = format!("{} / {}", sc.skill_slug, sc.channel);
                                            rsx! { option { value: "{val}", "{label}" } }
                                        }
                                    }
                                }
                            },
                            _ => rsx! {},
                        }}
                        {match &*available_remote_sc.read() {
                            Some(Ok(list)) if !list.is_empty() => {
                                // Group by skill center name
                                let mut by_sc: std::collections::BTreeMap<String, Vec<&RemoteSkillOption>> = std::collections::BTreeMap::new();
                                for rsc in list.iter() {
                                    by_sc.entry(rsc.skill_center_name.clone()).or_default().push(rsc);
                                }
                                rsx! {
                                    for (sc_name, items) in by_sc {
                                        optgroup { label: t!("cluster-skills-from-sc", name: sc_name.clone()),
                                            for rsc in items {
                                                {
                                                    let val = format!(
                                                        "remote|{}|{}|{}|{}|{}",
                                                        rsc.skill_center_id, rsc.remote_skill_channel_id,
                                                        rsc.skill_slug, rsc.channel, rsc.skill_name
                                                    );
                                                    let label = format!("{} / {}", rsc.skill_slug, rsc.channel);
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
                    button {
                        class: "bg-blue-600 text-white px-3 py-1 rounded text-sm hover:bg-blue-700",
                        r#type: "submit",
                        {t!("add")}
                    }
                }
            }
            {match &*skills.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    p { class: "text-gray-500 dark:text-gray-400 text-sm", {t!("cluster-skills-no-direct")} }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-gray-200 dark:divide-gray-700",
                        for cs in list {
                            {
                                let csid = cs.cluster_skill_id.to_string();
                                let label = format!("{} / {}", cs.skill_slug, cs.channel);
                                let is_remote = cs.skill_center_name.is_some();
                                let via = cs.skill_center_name.clone().unwrap_or_default();
                                rsx! {
                                    li { class: "py-2 flex justify-between items-center",
                                        span { class: "flex items-center gap-2",
                                            span {
                                                class: if is_remote { "text-sm font-mono text-purple-700 dark:text-purple-400" } else { "text-sm font-mono" },
                                                "{label}"
                                            }
                                            if is_remote {
                                                span { class: "text-xs text-purple-500 dark:text-purple-500", {t!("cluster-skills-via", source: via.clone())} }
                                            }
                                        }
                                        if !read_only {
                                            button {
                                                class: "text-red-600 dark:text-red-400 hover:text-red-700 dark:hover:text-red-300 text-sm",
                                                onclick: move |_| {
                                                    let csid = csid.clone();
                                                    spawn(async move {
                                                        if remove_cluster_skill(csid).await.is_ok() {
                                                            skills.restart();
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
                Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400 text-sm", "Error: {e}" } },
                None => rsx! { p { class: "text-gray-500 dark:text-gray-400 text-sm", "Loading..." } },
            }}
        }

        // Skills from bundles (read-only, blue)
        div { class: "mb-4",
            h4 { class: "text-sm font-semibold text-blue-700 dark:text-blue-400 mb-2", {t!("cluster-skills-from-bundles")} }
            {match &*bundle_skills.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    p { class: "text-gray-500 dark:text-gray-400 text-sm", {t!("cluster-skills-no-bundle-skills")} }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-gray-200 dark:divide-gray-700",
                        for bs in list {
                            {
                                let label = format!("{} / {}", bs.skill_slug, bs.channel);
                                let via = bs.bundle_slug.clone();
                                let overwritten = bs.overwritten;
                                rsx! {
                                    li { class: "py-2 flex items-center gap-2",
                                        span {
                                            class: if overwritten { "text-sm font-mono text-blue-400 dark:text-blue-600 line-through" } else { "text-sm font-mono text-blue-700 dark:text-blue-400" },
                                            "{label}"
                                        }
                                        span { class: if overwritten { "text-xs text-blue-300" } else { "text-xs text-blue-500" }, {t!("cluster-skills-via", source: via.clone())} }
                                        if overwritten {
                                            span { class: "text-xs text-gray-400 dark:text-gray-500 italic", {t!("cluster-skills-overwritten")} }
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

        // Bundle assignments
        div { class: "mb-4",
            h4 { class: "text-sm font-semibold text-gray-700 dark:text-gray-200 mb-2", {t!("cluster-skills-bundles-title")} }
            if let Some(err) = &*bundle_error.read() {
                p { class: "text-red-600 dark:text-red-400 text-sm mb-2", "{err}" }
            }
            if !read_only {
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
                                    match add_cluster_bundle(
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
                                match add_cluster_bundle(cid, Some(val), None, None, None, None).await {
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
                        option { value: "", {t!("cluster-skills-select-bundle")} }
                        {match &*available_bundles.read() {
                            Some(Ok(list)) if !list.is_empty() => rsx! {
                                optgroup { label: t!("cluster-skills-local"),
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
                        {match &*available_remote_bundles.read() {
                            Some(Ok(list)) if !list.is_empty() => {
                                let mut by_sc: std::collections::BTreeMap<String, Vec<&RemoteBundleOption>> = std::collections::BTreeMap::new();
                                for rb in list.iter() {
                                    by_sc.entry(rb.skill_center_name.clone()).or_default().push(rb);
                                }
                                rsx! {
                                    for (sc_name, items) in by_sc {
                                        optgroup { label: t!("cluster-skills-from-sc", name: sc_name.clone()),
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
                    button {
                        class: "bg-blue-600 text-white px-3 py-1 rounded text-sm hover:bg-blue-700",
                        r#type: "submit",
                        {t!("add")}
                    }
                }
            }
            {match &*bundles.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    p { class: "text-gray-500 dark:text-gray-400 text-sm", {t!("cluster-skills-no-bundle-assign")} }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-gray-200 dark:divide-gray-700",
                        for cb in list {
                            {
                                let cbid = cb.cluster_bundle_id.to_string();
                                let label = format!("{} ({})", cb.bundle_name, cb.bundle_slug);
                                let is_remote = cb.skill_center_name.is_some();
                                let via = cb.skill_center_name.clone().unwrap_or_default();
                                rsx! {
                                    li { class: "py-2 flex justify-between items-center",
                                        span { class: "flex items-center gap-2",
                                            span {
                                                class: if is_remote { "text-sm text-purple-700 dark:text-purple-400" } else { "text-sm" },
                                                "{label}"
                                            }
                                            if is_remote {
                                                span { class: "text-xs text-purple-500 dark:text-purple-500", {t!("cluster-skills-via", source: via.clone())} }
                                            }
                                        }
                                        if !read_only {
                                            button {
                                                class: "text-red-600 dark:text-red-400 hover:text-red-700 dark:hover:text-red-300 text-sm",
                                                onclick: move |_| {
                                                    let cbid = cbid.clone();
                                                    spawn(async move {
                                                        if remove_cluster_bundle(cbid).await.is_ok() {
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
                Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400 text-sm", "Error: {e}" } },
                None => rsx! { p { class: "text-gray-500 dark:text-gray-400 text-sm", "Loading..." } },
            }}
        }
    }
}
