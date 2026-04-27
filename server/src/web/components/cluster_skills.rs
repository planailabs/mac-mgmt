use dioxus::prelude::*;

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
        "SELECT cs.id as cluster_skill_id, s.slug as skill_slug, sc.channel \
         FROM cluster_skills cs \
         JOIN skill_channels sc ON sc.id = cs.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE cs.cluster_id = $1 \
         ORDER BY s.slug, sc.channel",
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
        "SELECT cb.id as cluster_bundle_id, b.slug as bundle_slug, b.name as bundle_name \
         FROM cluster_bundles cb \
         JOIN bundles b ON b.id = cb.bundle_id \
         WHERE cb.cluster_id = $1 \
         ORDER BY b.slug",
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
    skill_channel_id: String,
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
    let scid: uuid::Uuid = skill_channel_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("INSERT INTO cluster_skills (cluster_id, skill_channel_id) VALUES ($1, $2)")
        .bind(cid)
        .bind(scid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
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
async fn add_cluster_bundle(cluster_id: String, bundle_id: String) -> Result<(), ServerFnError> {
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
    let bid: uuid::Uuid = bundle_id
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

/// Remote skill assignment from a skill center.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct RemoteSkillDisplay {
    pub id: uuid::Uuid,
    pub slug: String,
    pub channel: String,
    pub skill_name: String,
    pub skill_center_name: String,
}

/// Remote bundle assignment from a skill center.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct RemoteBundleDisplay {
    pub id: uuid::Uuid,
    pub slug: String,
    pub bundle_name: String,
    pub skill_center_name: String,
}

#[server]
async fn list_remote_skills(cluster_id: String) -> Result<Vec<RemoteSkillDisplay>, ServerFnError> {
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
    let rows = sqlx::query_as::<_, RemoteSkillDisplay>(
        "SELECT crs.id, crs.slug, crs.channel, crs.skill_name, sc.name AS skill_center_name \
         FROM cluster_remote_skills crs \
         JOIN skill_centers sc ON sc.id = crs.skill_center_id \
         WHERE crs.cluster_id = $1 \
         ORDER BY crs.slug, crs.channel",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(rows)
}

#[server]
async fn list_remote_bundles(
    cluster_id: String,
) -> Result<Vec<RemoteBundleDisplay>, ServerFnError> {
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
    let rows = sqlx::query_as::<_, RemoteBundleDisplay>(
        "SELECT crb.id, crb.slug, crb.bundle_name, sc.name AS skill_center_name \
         FROM cluster_remote_bundles crb \
         JOIN skill_centers sc ON sc.id = crb.skill_center_id \
         WHERE crb.cluster_id = $1 \
         ORDER BY crb.slug",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(rows)
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

    let cid_remote_skills = cluster_id.clone();
    let remote_skills = use_server_future(move || {
        let cid = cid_remote_skills.clone();
        async move { list_remote_skills(cid).await }
    })?;

    let cid_remote_bundles = cluster_id.clone();
    let remote_bundles = use_server_future(move || {
        let cid = cid_remote_bundles.clone();
        async move { list_remote_bundles(cid).await }
    })?;

    let available_sc = use_server_future(list_all_skill_channels)?;
    let available_bundles = use_server_future(list_all_bundles)?;

    let mut selected_sc = use_signal(String::new);
    let mut selected_bundle = use_signal(String::new);
    let mut bundle_error = use_signal(|| None::<String>);

    let cid_add_skill = cluster_id.clone();
    let cid_add_bundle = cluster_id.clone();

    rsx! {
        // Direct skill assignments
        div { class: "mb-4",
            h4 { class: "text-sm font-semibold text-gray-700 dark:text-gray-200 mb-2", "Direct Skills" }
            if !read_only {
                form {
                    class: "flex gap-2 mb-3",
                    onsubmit: move |evt: FormEvent| {
                        evt.prevent_default();
                        let cid = cid_add_skill.clone();
                        let scid = selected_sc.read().clone();
                        spawn(async move {
                            if !scid.is_empty() {
                                if add_cluster_skill(cid, scid).await.is_ok() {
                                    selected_sc.set(String::new());
                                    skills.restart();
                                }
                            }
                        });
                    },
                    select {
                        class: "flex-1 border border-gray-300 dark:border-gray-600 rounded px-2 py-1 text-sm dark:bg-gray-700 dark:text-white",
                        value: "{selected_sc}",
                        onchange: move |evt| selected_sc.set(evt.value()),
                        option { value: "", "Select skill/channel..." }
                        {match &*available_sc.read() {
                            Some(Ok(list)) => rsx! {
                                for sc in list {
                                    {
                                        let val = sc.id.to_string();
                                        let label = format!("{} / {}", sc.skill_slug, sc.channel);
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
            }
            {match &*skills.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    p { class: "text-gray-500 dark:text-gray-400 text-sm", "No direct skill assignments." }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-gray-200 dark:divide-gray-700",
                        for cs in list {
                            {
                                let csid = cs.cluster_skill_id.to_string();
                                let label = format!("{} / {}", cs.skill_slug, cs.channel);
                                rsx! {
                                    li { class: "py-2 flex justify-between items-center",
                                        span { class: "text-sm font-mono", "{label}" }
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
                                                "Remove"
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
            h4 { class: "text-sm font-semibold text-blue-700 dark:text-blue-400 mb-2", "From Bundles" }
            {match &*bundle_skills.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    p { class: "text-gray-500 dark:text-gray-400 text-sm", "No skills from bundles." }
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

        // Remote skill assignments (from skill centers, purple)
        div { class: "mb-4",
            h4 { class: "text-sm font-semibold text-purple-700 dark:text-purple-400 mb-2", "From Skill Centers" }
            {match &*remote_skills.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    p { class: "text-gray-500 dark:text-gray-400 text-sm", "No remote skill assignments." }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-gray-200 dark:divide-gray-700",
                        for rs in list {
                            {
                                let label = format!("{} / {}", rs.slug, rs.channel);
                                let via = rs.skill_center_name.clone();
                                rsx! {
                                    li { class: "py-2 flex items-center gap-2",
                                        span { class: "text-sm font-mono text-purple-700 dark:text-purple-400", "{label}" }
                                        span { class: "text-xs text-purple-500 dark:text-purple-500", "via {via}" }
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
            h4 { class: "text-sm font-semibold text-gray-700 dark:text-gray-200 mb-2", "Bundles" }
            if let Some(err) = &*bundle_error.read() {
                p { class: "text-red-600 dark:text-red-400 text-sm mb-2", "{err}" }
            }
            if !read_only {
                form {
                    class: "flex gap-2 mb-3",
                    onsubmit: move |evt: FormEvent| {
                        evt.prevent_default();
                        let cid = cid_add_bundle.clone();
                        let bid = selected_bundle.read().clone();
                        spawn(async move {
                            if !bid.is_empty() {
                                match add_cluster_bundle(cid, bid).await {
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
                        option { value: "", "Select bundle..." }
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
            }
            {match &*bundles.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    p { class: "text-gray-500 dark:text-gray-400 text-sm", "No bundle assignments." }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-gray-200 dark:divide-gray-700",
                        for cb in list {
                            {
                                let cbid = cb.cluster_bundle_id.to_string();
                                let label = format!("{} ({})", cb.bundle_name, cb.bundle_slug);
                                rsx! {
                                    li { class: "py-2 flex justify-between items-center",
                                        span { class: "text-sm", "{label}" }
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
                                                "Remove"
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

        // Remote bundle assignments (from skill centers, purple)
        div {
            h4 { class: "text-sm font-semibold text-purple-700 dark:text-purple-400 mb-2", "Bundles from Skill Centers" }
            {match &*remote_bundles.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    p { class: "text-gray-500 dark:text-gray-400 text-sm", "No remote bundle assignments." }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-gray-200 dark:divide-gray-700",
                        for rb in list {
                            {
                                let label = if rb.bundle_name.is_empty() {
                                    rb.slug.clone()
                                } else {
                                    format!("{} ({})", rb.bundle_name, rb.slug)
                                };
                                let via = rb.skill_center_name.clone();
                                rsx! {
                                    li { class: "py-2 flex items-center gap-2",
                                        span { class: "text-sm text-purple-700 dark:text-purple-400", "{label}" }
                                        span { class: "text-xs text-purple-500 dark:text-purple-500", "via {via}" }
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
