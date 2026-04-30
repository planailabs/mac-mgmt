use std::collections::HashMap;

use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};

use crate::anthropic::{GenerateContext, GeneratedNameDesc};
use crate::models::{Skill, SkillChannel};
use crate::web::components::generate_button::GenerateButton;
use crate::web::components::hidden_badge::HiddenBadge;
use crate::web::components::ui::{
    Button, ButtonKind, ButtonSize, ErrorText, HelpText, SectionHeading,
};
#[cfg(feature = "server")]
use crate::web::user::current_user;

#[server]
async fn get_skill(id: String) -> Result<Skill, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let skill = sqlx::query_as::<_, Skill>("SELECT * FROM skills WHERE id = $1")
        .bind(uuid)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(skill)
}

#[server]
async fn update_skill(
    id: String,
    name: String,
    description: String,
    hide_from_public_catalog: bool,
) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("UPDATE skills SET name = $1, description = $2, hide_from_public_catalog = $3 WHERE id = $4")
        .bind(&name)
        .bind(&description)
        .bind(hide_from_public_catalog)
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    crate::api::push::notify_federation_global();
    Ok(())
}

#[server]
async fn list_channels(skill_id: String) -> Result<Vec<SkillChannel>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = skill_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let channels = sqlx::query_as::<_, SkillChannel>(
        "SELECT * FROM skill_channels WHERE skill_id = $1 ORDER BY channel",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(channels)
}

/// Resolve store paths for all channels of a skill from xzar.
#[server]
async fn resolve_channel_paths(
    skill_id: String,
) -> Result<HashMap<String, Vec<(String, String)>>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = skill_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    let slug = sqlx::query_scalar::<_, String>("SELECT slug FROM skills WHERE id = $1")
        .bind(uuid)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let channels =
        sqlx::query_scalar::<_, String>("SELECT channel FROM skill_channels WHERE skill_id = $1")
            .bind(uuid)
            .fetch_all(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

    if channels.is_empty() {
        return Ok(HashMap::new());
    }

    let cfg = crate::config::config();
    let xzar = cfg
        .xzar
        .as_ref()
        .ok_or_else(|| ServerFnError::new("xzar not configured".to_string()))?;
    let pins = crate::xzar::fetch_pins(&xzar.url, &xzar.token)
        .await
        .map_err(|e| ServerFnError::new(format!("xzar error: {e}")))?;

    let mut result = HashMap::new();
    let prefix = format!("skill/{slug}/");
    for pin in &pins {
        if pin.abandoned || pin.roots.is_empty() {
            continue;
        }
        if let Some(rest) = pin.name.strip_prefix(&prefix) {
            if let Some((channel, arch)) = rest.split_once('/') {
                if channels.contains(&channel.to_string()) {
                    let path = crate::xzar::store_path_for_pin(&pins, &pin.name);
                    if let Some(path) = path {
                        let entry: &mut Vec<(String, String)> =
                            result.entry(channel.to_string()).or_insert_with(Vec::new);
                        entry.push((arch.to_string(), path));
                    }
                }
            }
        }
    }

    Ok(result)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChannelMcpDep {
    dep_id: String,
    mcp_server_id: String,
    mcp_server_slug: String,
}

#[server]
async fn list_channel_mcp_deps(
    skill_channel_id: String,
) -> Result<Vec<ChannelMcpDep>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = skill_channel_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct Row {
        dep_id: uuid::Uuid,
        mcp_server_id: uuid::Uuid,
        mcp_server_slug: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT smd.id as dep_id, smd.mcp_server_id, ms.slug as mcp_server_slug \
         FROM skill_mcp_dependencies smd \
         JOIN mcp_servers ms ON ms.id = smd.mcp_server_id \
         WHERE smd.skill_channel_id = $1 \
         ORDER BY ms.slug",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| ChannelMcpDep {
            dep_id: r.dep_id.to_string(),
            mcp_server_id: r.mcp_server_id.to_string(),
            mcp_server_slug: r.mcp_server_slug,
        })
        .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct McpServerOption {
    id: String,
    slug: String,
    name: String,
}

#[server]
async fn list_all_mcp_servers() -> Result<Vec<McpServerOption>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: uuid::Uuid,
        slug: String,
        name: String,
    }

    let rows = sqlx::query_as::<_, Row>("SELECT id, slug, name FROM mcp_servers ORDER BY slug")
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| McpServerOption {
            id: r.id.to_string(),
            slug: r.slug,
            name: r.name,
        })
        .collect())
}

#[server]
async fn add_channel_mcp_dep(
    skill_channel_id: String,
    mcp_server_id: String,
) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let sc_id: uuid::Uuid = skill_channel_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let ms_id: uuid::Uuid = mcp_server_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("INSERT INTO skill_mcp_dependencies (skill_channel_id, mcp_server_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(sc_id)
        .bind(ms_id)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    crate::api::push::notify_federation_global();
    Ok(())
}

#[server]
async fn remove_channel_mcp_dep(dep_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = dep_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("DELETE FROM skill_mcp_dependencies WHERE id = $1")
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    crate::api::push::notify_federation_global();
    Ok(())
}

#[component]
pub fn SkillDetail(id: String) -> Element {
    let id_clone = id.clone();
    let mut skill = use_server_future(move || {
        let id = id_clone.clone();
        async move { get_skill(id).await }
    })?;

    let id_channels = id.clone();
    let channels = use_server_future(move || {
        let id = id_channels.clone();
        async move { list_channels(id).await }
    })?;

    let id_paths = id.clone();
    let paths = use_server_future(move || {
        let id = id_paths.clone();
        async move { resolve_channel_paths(id).await }
    })?;

    let mut editing = use_signal(|| false);
    let mut draft_name = use_signal(String::new);
    let mut draft_desc = use_signal(String::new);
    let mut draft_hide = use_signal(|| false);

    match &*skill.read() {
        Some(Ok(s)) => {
            let created = s.created_at.format("%Y-%m-%d %H:%M").to_string();
            let sid = s.id.to_string();
            let name = s.name.clone();
            let desc = s.description.clone();
            let slug = s.slug.clone();
            let hide_flag = s.hide_from_public_catalog;

            rsx! {
                div { class: "flex items-center gap-3 mb-1",
                    if *editing.read() {
                        form { class: "space-y-2",
                            onsubmit: move |evt: FormEvent| {
                                evt.prevent_default();
                                let id = sid.clone();
                                let new_name = draft_name.read().clone();
                                let new_desc = draft_desc.read().clone();
                                let new_hide = *draft_hide.read();
                                spawn(async move {
                                    if !new_name.trim().is_empty() {
                                        let _ = update_skill(id, new_name, new_desc, new_hide).await;
                                        skill.restart();
                                    }
                                    editing.set(false);
                                });
                            },
                            input {
                                class: "input text-2xl font-bold py-1",
                                r#type: "text",
                                value: "{draft_name}",
                                oninput: move |e| draft_name.set(e.value()),
                                autofocus: true,
                            }
                            textarea {
                                class: "input py-1",
                                rows: "2",
                                value: "{draft_desc}",
                                oninput: move |e| draft_desc.set(e.value()),
                            }
                            label { class: "flex items-center gap-2 text-sm text-fg-strong",
                                input {
                                    r#type: "checkbox",
                                    checked: "{draft_hide}",
                                    class: "rounded border-line",
                                    oninput: move |e| draft_hide.set(e.value() == "true"),
                                }
                                {t!("skill-detail-hide")}
                            }
                            div { class: "flex gap-2",
                                button { class: "text-success hover:opacity-80", r#type: "submit",
                                    {t!("save")}
                                }
                                button { class: "text-fg-muted hover:text-fg-strong", r#type: "button",
                                    onclick: move |_| editing.set(false),
                                    {t!("cancel")}
                                }
                                GenerateButton {
                                    context: GenerateContext::Skill { skill_id: sid.clone() },
                                    current_name: draft_name.read().clone(),
                                    current_desc: draft_desc.read().clone(),
                                    on_generated: move |result: GeneratedNameDesc| {
                                        draft_name.set(result.name);
                                        draft_desc.set(result.description);
                                    },
                                }
                            }
                        }
                    } else {
                        h2 { class: "h-page mb-0", "{name}" }
                        span { class: "text-fg-faint font-mono text-sm", "({slug})" }
                        HiddenBadge { hidden: hide_flag }
                        button { class: "text-fg-faint hover:text-fg-muted",
                            onclick: move |_| {
                                draft_name.set(name.clone());
                                draft_desc.set(desc.clone());
                                draft_hide.set(hide_flag);
                                editing.set(true);
                            },
                            {t!("edit")}
                        }
                    }
                }
                if !*editing.read() && !s.description.is_empty() {
                    p { class: "text-fg mb-2", "{s.description}" }
                }
                p { class: "help mb-6", {t!("cluster-detail-created", date: created)} }

                // Channels section (read-only, synced from xzar)
                div {
                    SectionHeading { {t!("skill-detail-channels")} }
                    p { class: "help-xs mb-3", {t!("skill-detail-channels-synced")} }
                    {match &*channels.read() {
                        Some(Ok(list)) if list.is_empty() => rsx! {
                            HelpText { {t!("skill-detail-no-channels")} }
                        },
                        Some(Ok(list)) => {
                            let path_map = match &*paths.read() {
                                Some(Ok(m)) => m.clone(),
                                _ => HashMap::new(),
                            };
                            rsx! {
                                ul { class: "divide-y divide-line-soft",
                                    for ch in list {
                                        {
                                            let ch_name = ch.channel.clone();
                                            let ch_created = ch.created_at.format("%Y-%m-%d").to_string();
                                            let arch_paths = path_map.get(&ch.channel).cloned().unwrap_or_default();
                                            let ch_id = ch.id.to_string();
                                            rsx! {
                                                li { class: "py-2",
                                                    div {
                                                        span { class: "text-sm font-mono font-medium", "{ch_name}" }
                                                        span { class: "text-xs text-fg-faint ml-2", "{ch_created}" }
                                                    }
                                                    for (arch, path) in &arch_paths {
                                                        p { class: "text-xs text-fg-faint font-mono mt-0.5 truncate",
                                                            span { class: "text-fg-muted", "{arch}" }
                                                            " {path}"
                                                        }
                                                    }
                                                    ChannelNixPackages { channel_id: ch_id.clone() }
                                                    ChannelMcpDeps { channel_id: ch_id }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
                        None => rsx! { HelpText { {t!("loading")} } },
                    }}
                }
            }
        }
        Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
        None => rsx! { HelpText { {t!("loading")} } },
    }
}

#[component]
fn ChannelMcpDeps(channel_id: String) -> Element {
    let cid = channel_id.clone();
    let mut deps = use_server_future(move || {
        let id = cid.clone();
        async move { list_channel_mcp_deps(id).await }
    })?;

    let cid_add = channel_id.clone();
    let all_servers = use_server_future(move || async move { list_all_mcp_servers().await })?;

    let mut selected_server = use_signal(String::new);

    rsx! {
        div { class: "mt-3",
            h4 { class: "text-sm font-semibold text-fg-strong mb-2", {t!("skill-detail-mcp-deps")} }
            form { class: "flex gap-2 mb-3",
                onsubmit: move |evt: FormEvent| {
                    evt.prevent_default();
                    let sid = selected_server.read().clone();
                    let channel = cid_add.clone();
                    spawn(async move {
                        if !sid.is_empty()
                            && add_channel_mcp_dep(channel, sid).await.is_ok()
                        {
                            selected_server.set(String::new());
                            deps.restart();
                        }
                    });
                },
                select { class: "input flex-1 w-auto py-1 text-sm",
                    value: "{selected_server}",
                    onchange: move |e| selected_server.set(e.value()),
                    option { value: "", {t!("skill-detail-select-mcp")} }
                    {match &*all_servers.read() {
                        Some(Ok(servers)) => rsx! {
                            for s in servers {
                                option { value: "{s.id}", "{s.slug} — {s.name}" }
                            }
                        },
                        _ => rsx! {},
                    }}
                }
                Button { kind: ButtonKind::Submit, size: ButtonSize::Xs,
                    {t!("add")}
                }
            }
            {match &*deps.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    HelpText { xs: true, {t!("skill-detail-no-mcp-deps")} }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-line-soft",
                        for dep in list {
                            {
                                let dep_id = dep.dep_id.clone();
                                let slug = dep.mcp_server_slug.clone();
                                rsx! {
                                    li { class: "py-1 flex justify-between items-center",
                                        span { class: "text-sm font-mono", "{slug}" }
                                        button { class: "link-danger text-xs",
                                            onclick: move |_| {
                                                let did = dep_id.clone();
                                                spawn(async move {
                                                    if remove_channel_mcp_dep(did).await.is_ok() {
                                                        deps.restart();
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
                },
                Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
                None => rsx! { HelpText { xs: true, {t!("loading")} } },
            }}
        }
    }
}

// ── Channel nix packages sub-component ────────────────────────────────

#[server]
async fn get_channel_nix_packages(channel_id: String) -> Result<Vec<String>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = channel_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let pkgs: Vec<String> = sqlx::query_scalar(
        "SELECT unnest(nix_packages) FROM skill_channels WHERE id = $1",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(pkgs)
}

#[server]
async fn add_channel_nix_package(
    channel_id: String,
    package: String,
) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = channel_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query(
        "UPDATE skill_channels SET nix_packages = array_append(nix_packages, $1) \
         WHERE id = $2 AND NOT ($1 = ANY(nix_packages))",
    )
    .bind(&package)
    .bind(uuid)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    crate::api::push::notify_federation_global();
    crate::api::push::notify_skill_channels_global(&[uuid]).await;
    Ok(())
}

#[server]
async fn remove_channel_nix_package(
    channel_id: String,
    package: String,
) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = channel_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query(
        "UPDATE skill_channels SET nix_packages = array_remove(nix_packages, $1) WHERE id = $2",
    )
    .bind(&package)
    .bind(uuid)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    crate::api::push::notify_federation_global();
    crate::api::push::notify_skill_channels_global(&[uuid]).await;
    Ok(())
}

#[component]
fn ChannelNixPackages(channel_id: String) -> Element {
    let cid = channel_id.clone();
    let mut pkgs = use_server_future(move || {
        let id = cid.clone();
        async move { get_channel_nix_packages(id).await }
    })?;

    let mut new_pkg = use_signal(String::new);

    rsx! {
        div { class: "mt-3",
            h4 { class: "text-sm font-semibold text-fg-strong mb-2", {t!("skill-detail-nix-deps")} }
            form { class: "flex gap-2 mb-3",
                onsubmit: move |evt: FormEvent| {
                    evt.prevent_default();
                    let pkg = new_pkg.read().clone();
                    let ch = channel_id.clone();
                    spawn(async move {
                        if !pkg.trim().is_empty()
                            && add_channel_nix_package(ch, pkg).await.is_ok()
                        {
                            new_pkg.set(String::new());
                            pkgs.restart();
                        }
                    });
                },
                input { class: "input flex-1 w-auto py-1 text-sm font-mono",
                    r#type: "text",
                    placeholder: t!("skill-detail-nix-placeholder"),
                    value: "{new_pkg}",
                    oninput: move |e| new_pkg.set(e.value()),
                }
                Button { kind: ButtonKind::Submit, size: ButtonSize::Xs,
                    {t!("add")}
                }
            }
            {match &*pkgs.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    HelpText { xs: true, {t!("skill-detail-no-nix-deps")} }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-line-soft",
                        for pkg in list {
                            {
                                let pkg_name = pkg.clone();
                                let ch = channel_id.clone();
                                rsx! {
                                    li { class: "py-1 flex justify-between items-center",
                                        span { class: "text-sm font-mono", "{pkg_name}" }
                                        button { class: "link-danger text-xs",
                                            onclick: move |_| {
                                                let p = pkg_name.clone();
                                                let c = ch.clone();
                                                spawn(async move {
                                                    if remove_channel_nix_package(c, p).await.is_ok() {
                                                        pkgs.restart();
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
                },
                Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
                None => rsx! { HelpText { xs: true, {t!("loading")} } },
            }}
        }
    }
}
