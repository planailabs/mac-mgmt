use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::models::Cluster;
use crate::web::app::Route;
use crate::web::components::ui::{
    Badge, BadgeVariant, Button, ButtonKind, ButtonSize, ButtonVariant, ErrorText, HelpText,
    SectionHeading,
};
#[cfg(feature = "server")]
use crate::web::user::current_user;

use super::cluster_healer_settings::ClusterHealerSettings;
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
async fn can_admin_cluster(cluster_id: String) -> Result<bool, ServerFnError> {
    let user = current_user().await?;
    if user.is_admin {
        return Ok(true);
    }
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let org_ids = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT organization_id FROM organization_clusters WHERE cluster_id = $1",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(org_ids.iter().any(|oid| user.is_org_admin(oid)))
}

#[server]
async fn get_cluster(id: String) -> Result<Cluster, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id
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
    let uuid: uuid::Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
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
    let uuid: uuid::Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
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
    let uuid: uuid::Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
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

        let current_commit: Option<String> =
            sqlx::query_scalar("SELECT nixpkgs_commit FROM clusters WHERE id = $1")
                .bind(uuid)
                .fetch_one(&pool)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
        let mut shas: std::collections::HashSet<String> = [c.clone()].into_iter().collect();
        if let Some(ref current) = current_commit {
            shas.insert(current.clone());
        }
        let counts = crate::commit_count::nixpkgs_commit_counts(&shas).await;
        let new_count = counts.get(&c).ok_or_else(|| {
            ServerFnError::new(format!("unknown nixpkgs commit {c}"))
        })?;
        if let Some(ref current) = current_commit {
            let cur_count = counts.get(current).ok_or_else(|| {
                ServerFnError::new(format!("cannot resolve commit count for current {current}"))
            })?;
            if new_count < cur_count {
                let short_new: String = c.chars().take(12).collect();
                let short_cur: String = current.chars().take(12).collect();
                return Err(ServerFnError::new(format!(
                    "nixpkgs {short_new} (#{new_count}) is older than current {short_cur} (#{cur_count}); use rollback to downgrade"
                )));
            }
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
    let uuid: uuid::Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
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
async fn get_cloud_init(cluster_id: String) -> Result<String, ServerFnError> {
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

    let rollout_version: Option<Option<String>> = sqlx::query_scalar(
        "SELECT r.target_version FROM rollouts r \
         JOIN rollout_stages rs ON rs.rollout_id = r.id \
         WHERE (rs.group_id = '00000000-0000-0000-0000-000000000000'::uuid \
                OR rs.group_id IN (SELECT group_id FROM rollout_group_members WHERE cluster_id = $1)) \
           AND r.status = 'rolling' AND rs.status = 'rolling' \
         ORDER BY r.created_at DESC LIMIT 1",
    )
    .bind(cid)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    let pinned: Option<String> =
        sqlx::query_scalar("SELECT pinned_version FROM clusters WHERE id = $1")
            .bind(cid)
            .fetch_optional(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .flatten();
    let latest: Option<String> = sqlx::query_scalar(
        "SELECT version FROM daemon_versions ORDER BY \
         string_to_array(version, '.')::int[] DESC LIMIT 1",
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    let version = rollout_version
        .and_then(|v| v)
        .or(pinned)
        .or(latest)
        .ok_or_else(|| {
            ServerFnError::new(
                "no daemon version available (no rollout, no pinned_version, no daemon_versions rows)",
            )
        })?;

    let server_url = crate::config::config()
        .api
        .external_url
        .trim_end_matches('/')
        .to_string();

    use rand::Rng;
    use sha2::{Digest, Sha256};
    let raw_token = hex::encode(rand::rng().random::<[u8; 32]>());
    let hash = hex::encode(Sha256::digest(raw_token.as_bytes()));
    let label = format!(
        "cloud-init-webui-{}",
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ")
    );
    sqlx::query(
        "INSERT INTO tokens (cluster_id, token_hash, label, kind) VALUES ($1, $2, $3, 'sync')",
    )
    .bind(cid)
    .bind(&hash)
    .bind(&label)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(crate::api::routes::render_cloud_init(
        &server_url,
        &raw_token,
        &version,
        "x86_64-linux",
        None,
    ))
}

#[server]
async fn get_active_rollouts(cluster_id: String) -> Result<Vec<ActiveRolloutEntry>, ServerFnError> {
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

    Ok(rows
        .into_iter()
        .map(|r| ActiveRolloutEntry {
            id: r.id.to_string(),
            target_version: r.target_version,
            status: r.status,
        })
        .collect())
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
    let cid_for_admin = id.clone();
    let cluster_admin_check = use_server_future(move || {
        let cid = cid_for_admin.clone();
        async move { can_admin_cluster(cid).await }
    })?;
    let admin_check = use_server_future(is_global_admin)?;

    let can_write = matches!(&*write_check.read(), Some(Ok(true)));
    let is_admin = matches!(&*admin_check.read(), Some(Ok(true)));
    let can_admin = matches!(&*cluster_admin_check.read(), Some(Ok(true)));
    let read_only = !can_write;

    let mut editing = use_signal(|| false);
    let mut draft_name = use_signal(String::new);
    let mut confirm_delete = use_signal(|| false);
    let mut cloud_init_open = use_signal(|| false);
    let nav = navigator();

    match &*cluster.read() {
        Some(Ok(c)) => {
            let created = c.created_at.format("%Y-%m-%d %H:%M").to_string();
            let pinned = c.pinned_version.clone();
            let nix_commit = c.nixpkgs_commit.clone();
            let cid = c.id.to_string();
            let cid2 = cid.clone();
            let name = c.name.clone();
            let name_for_modal = name.clone();
            rsx! {
                div { class: "flex items-center gap-3 mb-2",
                    if *editing.read() {
                        form { class: "flex items-center gap-2",
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
                                class: "input text-2xl font-bold w-auto py-1",
                                r#type: "text",
                                value: "{draft_name}",
                                oninput: move |e| draft_name.set(e.value()),
                                autofocus: true,
                            }
                            button { class: "text-success hover:opacity-80", r#type: "submit",
                                {t!("save")}
                            }
                            button { class: "text-fg-muted hover:text-fg-strong", r#type: "button",
                                onclick: move |_| editing.set(false),
                                {t!("cancel")}
                            }
                        }
                    } else {
                        h2 { class: "h-page mb-0", "{name}" }
                        if is_admin {
                            button { class: "text-fg-faint hover:text-fg-muted",
                                onclick: move |_| {
                                    draft_name.set(name.clone());
                                    editing.set(true);
                                },
                                {t!("edit")}
                            }
                            if *confirm_delete.read() {
                                span { class: "text-danger text-sm", {t!("cluster-detail-delete-confirm")} }
                                Button { variant: ButtonVariant::Danger, size: ButtonSize::Sm,
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
                                    {t!("cluster-detail-confirm-delete")}
                                }
                                button { class: "text-fg-muted hover:text-fg-strong text-sm",
                                    onclick: move |_| confirm_delete.set(false),
                                    {t!("cancel")}
                                }
                            } else {
                                button { class: "link-danger text-sm",
                                    onclick: move |_| confirm_delete.set(true),
                                    {t!("delete")}
                                }
                            }
                        }
                    }
                }
                div { class: "text-fg-muted mb-6 flex items-center gap-4 flex-wrap",
                    span { {t!("cluster-detail-created", date: created)} }
                    PinnedVersion { cluster_id: cid2.clone(), version: pinned.clone(), read_only, on_change: move |_| cluster.restart() }
                    NixpkgsCommit { cluster_id: cid2.clone(), commit: nix_commit.clone(), read_only, on_change: move |_| cluster.restart() }
                    ActiveRollouts { cluster_id: cid2.clone() }
                    if can_write {
                        Button { size: ButtonSize::Sm,
                            onclick: move |_| cloud_init_open.set(true),
                            {t!("cluster-detail-cloud-init")}
                        }
                        super::push_menu::PushMenu { cluster_id: cid2.clone() }
                    }
                }

                if can_write {
                    CloudInitModal {
                        cluster_id: cid2.clone(),
                        cluster_name: name_for_modal,
                        open: cloud_init_open,
                    }
                }

                div { class: "grid grid-cols-1 lg:grid-cols-2 gap-6",
                    div {
                        SectionHeading { {t!("cluster-detail-tab-sync-tokens")} }
                        SyncTokenList { cluster_id: cid2.clone(), read_only }
                    }
                    div {
                        SectionHeading { {t!("cluster-detail-tab-setting-tokens")} }
                        SettingTokenList { cluster_id: cid2.clone(), read_only }
                    }
                    div {
                        SectionHeading { {t!("cluster-detail-tab-config")} }
                        Link { to: Route::ClusterConfigPage { id: cid2.clone() },
                            class: "btn btn-lg btn-primary",
                            {t!("cluster-detail-open-config")}
                        }
                    }
                    div {
                        SectionHeading { {t!("cluster-detail-tab-skills")} }
                        ClusterSkills { cluster_id: cid2.clone(), read_only }
                    }
                    div {
                        SectionHeading { {t!("cluster-detail-tab-mcp-servers")} }
                        ClusterMcpServers { cluster_id: cid2.clone(), read_only }
                    }
                    div {
                        SectionHeading { {t!("cluster-detail-tab-packages")} }
                        Link { to: Route::ClusterPackagesPage { id: cid2.clone() },
                            class: "btn btn-lg btn-primary",
                            {t!("cluster-detail-open-packages")}
                        }
                    }
                    div {
                        SectionHeading { {t!("cluster-detail-tab-ssh-keys")} }
                        ClusterSshKeys { cluster_id: cid2.clone(), read_only: !can_admin }
                    }
                    div {
                        SectionHeading { {t!("cluster-detail-tab-healer")} }
                        ClusterHealerSettings { cluster_id: cid2.clone(), read_only }
                    }
                }
            }
        }
        Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
        None => rsx! { HelpText { {t!("loading")} } },
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
                let variant = match entry.status.as_str() {
                    "rolling" => BadgeVariant::Info,
                    "paused"  => BadgeVariant::Warn,
                    _         => BadgeVariant::Neutral,
                };
                let rid = entry.id.clone();
                let label = match &entry.target_version {
                    Some(v) => format!("{} v{v}", entry.status),
                    None => format!("{} (nixpkgs)", entry.status),
                };
                rsx! {
                    Link { to: Route::RolloutDetail { id: rid }, class: "no-underline hover:opacity-80",
                        Badge { variant, "{label}" }
                    }
                }
            }
        }
    }
}

#[component]
fn PinnedVersion(
    cluster_id: String,
    version: Option<String>,
    read_only: bool,
    on_change: EventHandler,
) -> Element {
    let mut editing = use_signal(|| false);
    let mut draft = use_signal(String::new);

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
            form { class: "flex items-center gap-1",
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
                span { {t!("cluster-detail-version-label")} }
                input { class: "input input-sm w-24 font-mono",
                    r#type: "text",
                    placeholder: "0.1.6",
                    value: "{draft}",
                    oninput: move |e| draft.set(e.value()),
                    autofocus: true,
                }
                button { class: "text-success hover:opacity-80 text-sm", r#type: "submit",
                    {t!("save")}
                }
                button { class: "text-fg-muted hover:text-fg-strong text-sm", r#type: "button",
                    onclick: move |_| editing.set(false),
                    {t!("cancel")}
                }
            }
        }
    } else if has_version {
        let ver_display = version.clone().unwrap_or_default();
        rsx! {
            span { class: "flex items-center gap-1",
                span { {t!("cluster-detail-version-label")} }
                span { class: "font-mono font-medium text-fg",
                    {t!("cluster-detail-version-value", version: ver_display.clone())}
                }
                if let Some(rid) = rollout_id {
                    Link { to: Route::RolloutDetail { id: rid }, class: "link text-sm",
                        {t!("cluster-detail-version-rollout")}
                    }
                }
                if !read_only {
                    button { class: "text-fg-faint hover:text-fg-muted text-sm",
                        onclick: move |_| {
                            draft.set(ver_display.clone());
                            editing.set(true);
                        },
                        {t!("edit")}
                    }
                }
            }
        }
    } else {
        rsx! {
            span { class: "flex items-center gap-1",
                span { class: "text-fg-faint", {t!("cluster-detail-no-version")} }
                if !read_only {
                    button { class: "text-fg-faint hover:text-fg-muted text-sm",
                        onclick: move |_| {
                            draft.set(String::new());
                            editing.set(true);
                        },
                        {t!("cluster-detail-set")}
                    }
                }
            }
        }
    }
}

#[server]
async fn get_nixpkgs_commit_count(sha: String) -> Result<Option<u64>, ServerFnError> {
    let shas = std::collections::HashSet::from([sha.clone()]);
    let counts = crate::commit_count::nixpkgs_commit_counts(&shas).await;
    Ok(counts.get(&sha).copied())
}

#[component]
fn NixpkgsCommit(
    cluster_id: String,
    commit: Option<String>,
    read_only: bool,
    on_change: EventHandler,
) -> Element {
    let mut editing = use_signal(|| false);
    let mut draft = use_signal(String::new);

    if !read_only && *editing.read() {
        let cid = cluster_id.clone();
        rsx! {
            form { class: "flex items-center gap-1",
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
                span { {t!("cluster-detail-nixpkgs-label")} }
                input { class: "input input-sm w-64 font-mono",
                    r#type: "text",
                    placeholder: t!("cluster-detail-nixpkgs-placeholder"),
                    value: "{draft}",
                    oninput: move |e| draft.set(e.value()),
                    autofocus: true,
                }
                button { class: "text-success hover:opacity-80 text-sm", r#type: "submit",
                    {t!("save")}
                }
                button { class: "text-fg-muted hover:text-fg-strong text-sm", r#type: "button",
                    onclick: move |_| editing.set(false),
                    {t!("cancel")}
                }
            }
        }
    } else if let Some(c) = commit {
        let display = c.clone();
        let display_for_edit = display.clone();
        let short: String = display.chars().take(12).collect();
        let url = format!("https://git.plan.ai/plan-ai/nixpkgs/-/commit/{display}");

        let sha_for_count = display.clone();
        let count_future = use_resource(move || {
            let sha = sha_for_count.clone();
            async move { get_nixpkgs_commit_count(sha).await.ok().flatten() }
        });
        let count_label = count_future
            .read()
            .as_ref()
            .and_then(|n| n.as_ref())
            .map(|n| format!(" #{n}"))
            .unwrap_or_default();

        rsx! {
            span { class: "flex items-center gap-1",
                span { {t!("cluster-detail-nixpkgs-label")} }
                a { class: "font-mono font-medium text-fg hover:text-brand",
                    href: "{url}",
                    target: "_blank",
                    title: "{display}",
                    "{short}{count_label}"
                }
                if !read_only {
                    button { class: "text-fg-faint hover:text-fg-muted text-sm",
                        onclick: move |_| {
                            draft.set(display_for_edit.clone());
                            editing.set(true);
                        },
                        {t!("edit")}
                    }
                }
            }
        }
    } else {
        rsx! {
            span { class: "flex items-center gap-1",
                span { class: "text-fg-faint", {t!("cluster-detail-no-nixpkgs")} }
                if !read_only {
                    button { class: "text-fg-faint hover:text-fg-muted text-sm",
                        onclick: move |_| {
                            draft.set(String::new());
                            editing.set(true);
                        },
                        {t!("cluster-detail-set")}
                    }
                }
            }
        }
    }
}

#[component]
fn CloudInitModal(cluster_id: String, cluster_name: String, mut open: Signal<bool>) -> Element {
    let mut yaml = use_signal(|| None::<String>);
    let mut loading = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut copied = use_signal(|| false);

    let cid_for_fetch = cluster_id.clone();
    use_effect(move || {
        if *open.read() && yaml.read().is_none() && !*loading.read() {
            let cid = cid_for_fetch.clone();
            loading.set(true);
            error.set(None);
            spawn(async move {
                match get_cloud_init(cid).await {
                    Ok(y) => yaml.set(Some(y)),
                    Err(e) => error.set(Some(e.to_string())),
                }
                loading.set(false);
            });
        }
    });

    if !*open.read() {
        return rsx! {};
    }

    let current_yaml = yaml.read().clone().unwrap_or_default();
    let current_error = error.read().clone();
    let is_loading = *loading.read();
    let filename = format!(
        "cloud-init-{}.yaml",
        cluster_name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            })
            .collect::<String>()
    );

    rsx! {
        div { class: "fixed inset-0 z-50 flex items-center justify-center bg-black/50",
            onclick: move |_| open.set(false),
            div { class: "bg-surface rounded-lg shadow-xl max-w-3xl w-full max-h-[85vh] flex flex-col",
                onclick: move |e| e.stop_propagation(),

                div { class: "px-4 py-3 border-b border-line-soft flex items-center justify-between",
                    div {
                        h2 { class: "font-semibold text-base text-fg-strong",
                            {t!("cluster-detail-cloud-init-title", cluster: cluster_name.clone())}
                        }
                        p { class: "help-xs mt-0.5", {t!("cluster-detail-cloud-init-desc")} }
                    }
                    button { r#type: "button",
                        class: "text-fg-muted hover:text-fg-strong text-xl leading-none",
                        onclick: move |_| open.set(false),
                        "×"
                    }
                }

                div { class: "px-4 py-3 overflow-y-auto flex-1",
                    if is_loading {
                        HelpText { {t!("cluster-detail-generating")} }
                    } else if let Some(e) = &current_error {
                        ErrorText { {t!("error-message", message: e.to_string())} }
                    } else {
                        textarea { class: "input h-80 font-mono text-xs",
                            readonly: true,
                            value: "{current_yaml}",
                        }
                    }
                }

                div { class: "px-4 py-3 border-t border-line-soft flex justify-end gap-2",
                    if !is_loading && current_error.is_none() {
                        button { r#type: "button",
                            class: "btn btn-md btn-secondary",
                            onclick: {
                                let yaml_text = current_yaml.clone();
                                move |_| {
                                    let encoded = serde_json::to_string(&yaml_text).unwrap_or_default();
                                    let js = format!(
                                        "navigator.clipboard.writeText({encoded}).then(() => {{}}, () => {{}});"
                                    );
                                    let _ = document::eval(&js);
                                    copied.set(true);
                                    let _ = document::eval(
                                        "setTimeout(() => { \
                                             const el = document.querySelector('[data-copy-ack]'); \
                                             if (el) el.textContent = 'Copy'; \
                                         }, 1500);",
                                    );
                                }
                            },
                            "data-copy-ack": "1",
                            if *copied.read() { {t!("cluster-detail-copied")} } else { {t!("cluster-detail-copy")} }
                        }
                        Button { kind: ButtonKind::Button, size: ButtonSize::Md,
                            onclick: {
                                let yaml_text = current_yaml.clone();
                                let filename = filename.clone();
                                move |_| {
                                    let encoded_text = serde_json::to_string(&yaml_text).unwrap_or_default();
                                    let encoded_name = serde_json::to_string(&filename).unwrap_or_default();
                                    let js = format!(
                                        "{{ \
                                            const blob = new Blob([{encoded_text}], {{ type: 'text/yaml' }}); \
                                            const url = URL.createObjectURL(blob); \
                                            const a = document.createElement('a'); \
                                            a.href = url; \
                                            a.download = {encoded_name}; \
                                            document.body.appendChild(a); \
                                            a.click(); \
                                            document.body.removeChild(a); \
                                            URL.revokeObjectURL(url); \
                                        }}"
                                    );
                                    let _ = document::eval(&js);
                                }
                            },
                            {t!("cluster-detail-download")}
                        }
                    }
                    Button { variant: ButtonVariant::Secondary, size: ButtonSize::Md,
                        onclick: move |_| open.set(false),
                        {t!("close")}
                    }
                }
            }
        }
    }
}
