use dioxus::prelude::*;

use crate::models::Cluster;
use crate::web::app::Route;
#[cfg(feature = "server")]
use crate::web::user::current_user;

use super::config_editor::ConfigEditor;
use super::config_history::ConfigHistory;
use super::cluster_mcp_servers::ClusterMcpServers;
use super::cluster_skills::ClusterSkills;
use super::cluster_ssh_keys::ClusterSshKeys;
use super::setting_token_list::SettingTokenList;
use super::token_list::SyncTokenList;

#[server]
async fn can_write_cluster(cluster_id: String) -> Result<bool, ServerFnError> {
    let user = current_user().await?;
    if user.is_admin {
        return Ok(true);
    }
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    match user
        .writable_cluster_ids(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    {
        Some(ids) => Ok(ids.contains(&uuid)),
        None => Ok(true),
    }
}

#[server]
async fn is_global_admin() -> Result<bool, ServerFnError> {
    let user = current_user().await?;
    Ok(user.is_admin)
}

#[server]
async fn get_cluster(id: String) -> Result<Cluster, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if let Some(ids) = user.accessible_cluster_ids(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))? {
        if !ids.contains(&uuid) {
            return Err(ServerFnError::new("access denied"));
        }
    }
    let cluster = sqlx::query_as::<_, Cluster>("SELECT * FROM clusters WHERE id = $1")
        .bind(uuid)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(cluster)
}

#[server]
async fn get_pinned_rollout(version: String) -> Result<Option<String>, ServerFnError> {
    let _user = current_user().await?;
    let pool = crate::server_pool()?;
    let rollout_id: Option<uuid::Uuid> = sqlx::query_scalar(
        "SELECT id FROM rollouts \
         WHERE target_version = $1 \
         ORDER BY updated_at DESC LIMIT 1",
    )
    .bind(&version)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(rollout_id.map(|id| id.to_string()))
}

#[server]
async fn rename_cluster(id: String, name: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("UPDATE clusters SET name = $1 WHERE id = $2")
        .bind(&name)
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn set_pinned_version(id: String, version: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    user.require_cluster_write(&pool, uuid).await?;
    let ver = version.trim().to_string();
    if ver.is_empty() {
        sqlx::query("UPDATE clusters SET pinned_version = NULL WHERE id = $1")
            .bind(uuid)
            .execute(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    } else {
        sqlx::query("UPDATE clusters SET pinned_version = $1 WHERE id = $2")
            .bind(&ver)
            .bind(uuid)
            .execute(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    }
    Ok(())
}

#[server]
async fn set_nixpkgs_commit(id: String, commit: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    user.require_cluster_write(&pool, uuid).await?;
    let c = commit.trim().to_string();
    if c.is_empty() {
        sqlx::query("UPDATE clusters SET nixpkgs_commit = NULL WHERE id = $1")
            .bind(uuid)
            .execute(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    } else {
        let valid = (7..=40).contains(&c.len()) && c.chars().all(|ch| ch.is_ascii_hexdigit());
        if !valid {
            return Err(ServerFnError::new("commit must be 7-40 hex chars"));
        }
        sqlx::query("UPDATE clusters SET nixpkgs_commit = $1 WHERE id = $2")
            .bind(&c)
            .bind(uuid)
            .execute(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    }
    crate::api::push::notify_global(uuid, crate::api::push::PushMessage::SyncNixpkgs).await;
    Ok(())
}

#[server]
async fn delete_cluster(id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("DELETE FROM clusters WHERE id = $1")
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ActiveRolloutEntry {
    id: String,
    target_version: Option<String>,
    status: String,
}

#[server]
async fn get_active_rollouts(cluster_id: String) -> Result<Vec<ActiveRolloutEntry>, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let cid: uuid::Uuid = cluster_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if let Some(ids) = user.accessible_cluster_ids(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))? {
        if !ids.contains(&cid) {
            return Err(ServerFnError::new("access denied"));
        }
    }

    #[derive(sqlx::FromRow)]
    struct Row {
        id: uuid::Uuid,
        target_version: Option<String>,
        status: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT DISTINCT r.id, r.target_version, r.status FROM rollouts r \
         JOIN rollout_stages rs ON rs.rollout_id = r.id \
         JOIN rollout_group_members rgm ON rgm.group_id = rs.group_id \
         WHERE rgm.cluster_id = $1 AND r.status IN ('rolling', 'paused') \
         ORDER BY r.target_version DESC",
    )
    .bind(cid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows.into_iter().map(|r| ActiveRolloutEntry {
        id: r.id.to_string(),
        target_version: r.target_version,
        status: r.status,
    }).collect())
}

#[component]
pub fn ClusterDetail(id: String) -> Element {
    let id_clone = id.clone();
    let mut cluster = use_server_future(move || {
        let id = id_clone.clone();
        async move { get_cluster(id).await }
    })?;

    let cid_for_write = id.clone();
    let write_check = use_server_future(move || {
        let cid = cid_for_write.clone();
        async move { can_write_cluster(cid).await }
    })?;
    let admin_check = use_server_future(is_global_admin)?;

    let can_write = matches!(&*write_check.read(), Some(Ok(true)));
    let is_admin = matches!(&*admin_check.read(), Some(Ok(true)));
    let read_only = !can_write;

    let mut editing = use_signal(|| false);
    let mut draft_name = use_signal(String::new);
    let mut confirm_delete = use_signal(|| false);
    let nav = navigator();

    match &*cluster.read() {
        Some(Ok(c)) => {
            let created = c.created_at.format("%Y-%m-%d %H:%M").to_string();
            let pinned = c.pinned_version.clone();
            let nix_commit = c.nixpkgs_commit.clone();
            let cid = c.id.to_string();
            let cid2 = cid.clone();
            let name = c.name.clone();
            rsx! {
                div { class: "flex items-center gap-3 mb-2",
                    if *editing.read() {
                        form {
                            class: "flex items-center gap-2",
                            onsubmit: move |evt: FormEvent| {
                                evt.prevent_default();
                                let id = cid.clone();
                                let new_name = draft_name.read().clone();
                                async move {
                                    if !new_name.trim().is_empty() {
                                        let _ = rename_cluster(id, new_name).await;
                                        cluster.restart();
                                    }
                                    editing.set(false);
                                }
                            },
                            input {
                                class: "text-2xl font-bold border border-gray-300 dark:border-gray-600 rounded px-2 py-1 dark:bg-gray-700 dark:text-white",
                                r#type: "text",
                                value: "{draft_name}",
                                oninput: move |e| draft_name.set(e.value()),
                                autofocus: true,
                            }
                            button {
                                class: "text-green-600 dark:text-green-400 hover:text-green-800 dark:hover:text-green-300",
                                r#type: "submit",
                                "Save"
                            }
                            button {
                                class: "text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200",
                                r#type: "button",
                                onclick: move |_| editing.set(false),
                                "Cancel"
                            }
                        }
                    } else {
                        h2 { class: "text-2xl font-bold", "{name}" }
                        if is_admin {
                            button {
                                class: "text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300",
                                onclick: move |_| {
                                    draft_name.set(name.clone());
                                    editing.set(true);
                                },
                                "Edit"
                            }
                            if *confirm_delete.read() {
                                span { class: "text-red-600 dark:text-red-400 text-sm", "Delete this cluster?" }
                                button {
                                    class: "bg-red-600 text-white px-3 py-1 rounded text-sm hover:bg-red-700",
                                    onclick: {
                                        let cid = cid.clone();
                                        move |_| {
                                            let cid = cid.clone();
                                            async move {
                                                let _ = delete_cluster(cid).await;
                                                nav.push(Route::ClusterList {});
                                            }
                                        }
                                    },
                                    "Confirm"
                                }
                                button {
                                    class: "text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 text-sm",
                                    onclick: move |_| confirm_delete.set(false),
                                    "Cancel"
                                }
                            } else {
                                button {
                                    class: "text-red-400 dark:text-red-500 hover:text-red-600 dark:hover:text-red-400 text-sm",
                                    onclick: move |_| confirm_delete.set(true),
                                    "Delete"
                                }
                            }
                        }
                    }
                }
                div { class: "text-gray-500 dark:text-gray-400 mb-6 flex items-center gap-4 flex-wrap",
                    span { "Created: {created}" }
                    PinnedVersion { cluster_id: cid2.clone(), version: pinned.clone(), read_only, on_change: move |_| cluster.restart() }
                    NixpkgsCommit { cluster_id: cid2.clone(), commit: nix_commit.clone(), read_only, on_change: move |_| cluster.restart() }
                    ActiveRollouts { cluster_id: cid2.clone() }
                }

                div { class: "grid grid-cols-1 lg:grid-cols-2 gap-6",
                    div {
                        h3 { class: "text-lg font-semibold mb-3", "Sync Tokens" }
                        SyncTokenList { cluster_id: cid2.clone(), read_only }
                    }
                    div {
                        h3 { class: "text-lg font-semibold mb-3", "Setting / Cluster Tokens" }
                        SettingTokenList { cluster_id: cid2.clone(), read_only }
                    }
                    div {
                        h3 { class: "text-lg font-semibold mb-3", "Config" }
                        ConfigEditor { cluster_id: cid2.clone(), read_only }
                    }
                    div {
                        h3 { class: "text-lg font-semibold mb-3", "Config History" }
                        ConfigHistory { cluster_id: cid2.clone() }
                    }
                    div {
                        h3 { class: "text-lg font-semibold mb-3", "Skills" }
                        ClusterSkills { cluster_id: cid2.clone(), read_only }
                    }
                    div {
                        h3 { class: "text-lg font-semibold mb-3", "MCP Servers" }
                        ClusterMcpServers { cluster_id: cid2.clone(), read_only }
                    }
                    div {
                        h3 { class: "text-lg font-semibold mb-3", "SSH Keys" }
                        ClusterSshKeys { cluster_id: cid2.clone(), read_only }
                    }
                }
            }
        }
        Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400", "Error: {e}" } },
        None => rsx! { p { "Loading..." } },
    }
}

#[component]
fn ActiveRollouts(cluster_id: String) -> Element {
    let cid = cluster_id.clone();
    let rollouts = use_server_future(move || {
        let cid = cid.clone();
        async move { get_active_rollouts(cid).await }
    })?;

    let entries = match &*rollouts.read() {
        Some(Ok(list)) => list.clone(),
        _ => vec![],
    };

    if entries.is_empty() {
        return rsx! {};
    }

    rsx! {
        for entry in &entries {
            {
                let badge_class = match entry.status.as_str() {
                    "rolling" => "bg-blue-100 dark:bg-blue-900 text-blue-800 dark:text-blue-200",
                    "paused" => "bg-yellow-100 dark:bg-yellow-900 text-yellow-800 dark:text-yellow-200",
                    _ => "bg-gray-100 dark:bg-gray-700 text-gray-800 dark:text-gray-200",
                };
                let rid = entry.id.clone();
                let label = match &entry.target_version {
                    Some(v) => format!("{} v{v}", entry.status),
                    None => format!("{} (nixpkgs)", entry.status),
                };
                rsx! {
                    Link {
                        to: Route::RolloutDetail { id: rid },
                        class: "px-2 py-0.5 rounded text-xs font-medium {badge_class} hover:opacity-80",
                        "{label}"
                    }
                }
            }
        }
    }
}

#[component]
fn PinnedVersion(cluster_id: String, version: Option<String>, read_only: bool, on_change: EventHandler) -> Element {
    let mut editing = use_signal(|| false);
    let mut draft = use_signal(String::new);

    // Fetch rollout link if we have a version
    let ver_for_query = version.clone().unwrap_or_default();
    let has_version = version.is_some();
    let rollout = use_server_future(move || {
        let ver = ver_for_query.clone();
        async move {
            if ver.is_empty() {
                Ok(None)
            } else {
                get_pinned_rollout(ver).await
            }
        }
    })?;

    let rollout_id = match &*rollout.read() {
        Some(Ok(id)) => id.clone(),
        _ => None,
    };

    if !read_only && *editing.read() {
        let cid = cluster_id.clone();
        rsx! {
            form {
                class: "flex items-center gap-1",
                onsubmit: move |evt: FormEvent| {
                    evt.prevent_default();
                    let cid = cid.clone();
                    let ver = draft.read().clone();
                    async move {
                        let _ = set_pinned_version(cid, ver).await;
                        editing.set(false);
                        on_change.call(());
                    }
                },
                span { "Version: " }
                input {
                    class: "border border-gray-300 dark:border-gray-600 rounded px-2 dark:bg-gray-700 dark:text-white py-0.5 text-sm font-mono w-24",
                    r#type: "text",
                    placeholder: "0.1.6",
                    value: "{draft}",
                    oninput: move |e| draft.set(e.value()),
                    autofocus: true,
                }
                button { class: "text-green-600 dark:text-green-400 hover:text-green-800 dark:hover:text-green-300 text-sm", r#type: "submit", "Save" }
                button {
                    class: "text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 text-sm",
                    r#type: "button",
                    onclick: move |_| editing.set(false),
                    "Cancel"
                }
            }
        }
    } else if has_version {
        let ver_display = version.clone().unwrap_or_default();
        rsx! {
            span { class: "flex items-center gap-1",
                span { "Version: " }
                span { class: "font-mono font-medium text-gray-700 dark:text-gray-200", "v{ver_display}" }
                if let Some(rid) = rollout_id {
                    Link {
                        to: Route::RolloutDetail { id: rid },
                        class: "text-blue-600 dark:text-blue-400 hover:underline text-sm",
                        "(rollout)"
                    }
                }
                if !read_only {
                    button {
                        class: "text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 text-sm",
                        onclick: move |_| {
                            draft.set(ver_display.clone());
                            editing.set(true);
                        },
                        "Edit"
                    }
                }
            }
        }
    } else {
        rsx! {
            span { class: "flex items-center gap-1",
                span { class: "text-gray-400 dark:text-gray-500", "No version pinned" }
                if !read_only {
                    button {
                        class: "text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 text-sm",
                        onclick: move |_| {
                            draft.set(String::new());
                            editing.set(true);
                        },
                        "Set"
                    }
                }
            }
        }
    }
}

#[component]
fn NixpkgsCommit(cluster_id: String, commit: Option<String>, read_only: bool, on_change: EventHandler) -> Element {
    let mut editing = use_signal(|| false);
    let mut draft = use_signal(String::new);

    if !read_only && *editing.read() {
        let cid = cluster_id.clone();
        rsx! {
            form {
                class: "flex items-center gap-1",
                onsubmit: move |evt: FormEvent| {
                    evt.prevent_default();
                    let cid = cid.clone();
                    let val = draft.read().clone();
                    async move {
                        let _ = set_nixpkgs_commit(cid, val).await;
                        editing.set(false);
                        on_change.call(());
                    }
                },
                span { "Nixpkgs: " }
                input {
                    class: "border border-gray-300 dark:border-gray-600 rounded px-2 dark:bg-gray-700 dark:text-white py-0.5 text-sm font-mono w-64",
                    r#type: "text",
                    placeholder: "commit sha",
                    value: "{draft}",
                    oninput: move |e| draft.set(e.value()),
                    autofocus: true,
                }
                button { class: "text-green-600 dark:text-green-400 hover:text-green-800 dark:hover:text-green-300 text-sm", r#type: "submit", "Save" }
                button {
                    class: "text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 text-sm",
                    r#type: "button",
                    onclick: move |_| editing.set(false),
                    "Cancel"
                }
            }
        }
    } else if let Some(c) = commit {
        let display = c.clone();
        let display_for_edit = display.clone();
        let short: String = display.chars().take(12).collect();
        rsx! {
            span { class: "flex items-center gap-1",
                span { "Nixpkgs: " }
                span { class: "font-mono font-medium text-gray-700 dark:text-gray-200", title: "{display}", "{short}" }
                if !read_only {
                    button {
                        class: "text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 text-sm",
                        onclick: move |_| {
                            draft.set(display_for_edit.clone());
                            editing.set(true);
                        },
                        "Edit"
                    }
                }
            }
        }
    } else {
        rsx! {
            span { class: "flex items-center gap-1",
                span { class: "text-gray-400 dark:text-gray-500", "No nixpkgs pin" }
                if !read_only {
                    button {
                        class: "text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 text-sm",
                        onclick: move |_| {
                            draft.set(String::new());
                            editing.set(true);
                        },
                        "Set"
                    }
                }
            }
        }
    }
}
