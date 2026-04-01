use std::collections::HashMap;

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::models::{Skill, SkillChannel};

#[server]
async fn get_skill(id: String) -> Result<Skill, ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let skill = sqlx::query_as::<_, Skill>("SELECT * FROM skills WHERE id = $1")
        .bind(uuid)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(skill)
}

#[server]
async fn update_skill(id: String, name: String, description: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("UPDATE skills SET name = $1, description = $2 WHERE id = $3")
        .bind(&name)
        .bind(&description)
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn list_channels(skill_id: String) -> Result<Vec<SkillChannel>, ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = skill_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
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
async fn resolve_channel_paths(skill_id: String) -> Result<HashMap<String, Vec<(String, String)>>, ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = skill_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    let slug = sqlx::query_scalar::<_, String>("SELECT slug FROM skills WHERE id = $1")
        .bind(uuid)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let channels = sqlx::query_scalar::<_, String>(
        "SELECT channel FROM skill_channels WHERE skill_id = $1",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    if channels.is_empty() {
        return Ok(HashMap::new());
    }

    let cfg = crate::config::config();
    let pins = crate::xzar::fetch_pins(&cfg.xzar.url, &cfg.xzar.token)
        .await
        .map_err(|e| ServerFnError::new(format!("xzar error: {e}")))?;

    // Map channel → [(arch, store_path), ...]
    let mut result = HashMap::new();
    let prefix = format!("skill/{slug}/");
    for pin in &pins {
        if pin.abandoned || pin.roots.is_empty() {
            continue;
        }
        if let Some(rest) = pin.name.strip_prefix(&prefix) {
            // rest = "{channel}/{arch}"
            if let Some((channel, arch)) = rest.split_once('/') {
                if channels.contains(&channel.to_string()) {
                    let path = crate::xzar::store_path_for_pin(&pins, &pin.name);
                    if let Some(path) = path {
                        let entry: &mut Vec<(String, String)> = result
                            .entry(channel.to_string())
                            .or_insert_with(Vec::new);
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
async fn list_channel_mcp_deps(skill_channel_id: String) -> Result<Vec<ChannelMcpDep>, ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = skill_channel_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

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

    Ok(rows.into_iter().map(|r| ChannelMcpDep {
        dep_id: r.dep_id.to_string(),
        mcp_server_id: r.mcp_server_id.to_string(),
        mcp_server_slug: r.mcp_server_slug,
    }).collect())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct McpServerOption {
    id: String,
    slug: String,
    name: String,
}

#[server]
async fn list_all_mcp_servers() -> Result<Vec<McpServerOption>, ServerFnError> {
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

    Ok(rows.into_iter().map(|r| McpServerOption {
        id: r.id.to_string(),
        slug: r.slug,
        name: r.name,
    }).collect())
}

#[server]
async fn add_channel_mcp_dep(skill_channel_id: String, mcp_server_id: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let sc_id: uuid::Uuid = skill_channel_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let ms_id: uuid::Uuid = mcp_server_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("INSERT INTO skill_mcp_dependencies (skill_channel_id, mcp_server_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(sc_id)
        .bind(ms_id)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn remove_channel_mcp_dep(dep_id: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = dep_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("DELETE FROM skill_mcp_dependencies WHERE id = $1")
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
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

    match &*skill.read() {
        Some(Ok(s)) => {
            let created = s.created_at.format("%Y-%m-%d %H:%M").to_string();
            let sid = s.id.to_string();
            let name = s.name.clone();
            let desc = s.description.clone();
            let slug = s.slug.clone();

            rsx! {
                div { class: "flex items-center gap-3 mb-1",
                    if *editing.read() {
                        form {
                            class: "space-y-2",
                            onsubmit: move |_| {
                                let id = sid.clone();
                                let new_name = draft_name.read().clone();
                                let new_desc = draft_desc.read().clone();
                                async move {
                                    if !new_name.trim().is_empty() {
                                        let _ = update_skill(id, new_name, new_desc).await;
                                        skill.restart();
                                    }
                                    editing.set(false);
                                }
                            },
                            input {
                                class: "text-2xl font-bold border border-gray-300 rounded px-2 py-1 w-full",
                                r#type: "text",
                                value: "{draft_name}",
                                oninput: move |e| draft_name.set(e.value()),
                                autofocus: true,
                            }
                            textarea {
                                class: "w-full border border-gray-300 rounded px-2 py-1",
                                rows: "2",
                                value: "{draft_desc}",
                                oninput: move |e| draft_desc.set(e.value()),
                            }
                            div { class: "flex gap-2",
                                button { class: "text-green-600 hover:text-green-800", r#type: "submit", "Save" }
                                button {
                                    class: "text-gray-500 hover:text-gray-700",
                                    r#type: "button",
                                    onclick: move |_| editing.set(false),
                                    "Cancel"
                                }
                            }
                        }
                    } else {
                        h2 { class: "text-2xl font-bold", "{name}" }
                        span { class: "text-gray-400 font-mono text-sm", "({slug})" }
                        button {
                            class: "text-gray-400 hover:text-gray-600",
                            onclick: move |_| {
                                draft_name.set(name.clone());
                                draft_desc.set(desc.clone());
                                editing.set(true);
                            },
                            "Edit"
                        }
                    }
                }
                if !*editing.read() && !s.description.is_empty() {
                    p { class: "text-gray-600 mb-2", "{s.description}" }
                }
                p { class: "text-gray-500 text-sm mb-6", "Created: {created}" }

                // Channels section (read-only, synced from xzar)
                div {
                    h3 { class: "text-lg font-semibold mb-3", "Channels" }
                    p { class: "text-xs text-gray-400 mb-3", "Channels are synced from xzar." }
                    {match &*channels.read() {
                        Some(Ok(list)) if list.is_empty() => rsx! {
                            p { class: "text-sm text-gray-500", "No channels synced yet." }
                        },
                        Some(Ok(list)) => {
                            let path_map = match &*paths.read() {
                                Some(Ok(m)) => m.clone(),
                                _ => HashMap::new(),
                            };
                            rsx! {
                                ul { class: "divide-y divide-gray-200",
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
                                                        span { class: "text-xs text-gray-400 ml-2", "{ch_created}" }
                                                    }
                                                    for (arch, path) in &arch_paths {
                                                        p { class: "text-xs text-gray-400 font-mono mt-0.5 truncate",
                                                            span { class: "text-gray-500", "{arch}" }
                                                            " {path}"
                                                        }
                                                    }
                                                    ChannelMcpDeps { channel_id: ch_id }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        Some(Err(e)) => rsx! { p { class: "text-red-600 text-sm", "Error: {e}" } },
                        None => rsx! { p { class: "text-sm", "Loading..." } },
                    }}
                }
            }
        }
        Some(Err(e)) => rsx! { p { class: "text-red-600", "Error: {e}" } },
        None => rsx! { p { "Loading..." } },
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
            h4 { class: "text-sm font-semibold text-gray-700 mb-2", "MCP Dependencies" }
            form {
                class: "flex gap-2 mb-3",
                onsubmit: move |evt: FormEvent| {
                    evt.prevent_default();
                    let sid = selected_server.read().clone();
                    let channel = cid_add.clone();
                    spawn(async move {
                        if !sid.is_empty() {
                            if add_channel_mcp_dep(channel, sid).await.is_ok() {
                                selected_server.set(String::new());
                                deps.restart();
                            }
                        }
                    });
                },
                select {
                    class: "flex-1 border border-gray-300 rounded px-2 py-1 text-sm",
                    value: "{selected_server}",
                    onchange: move |e| selected_server.set(e.value()),
                    option { value: "", "Select MCP server..." }
                    {match &*all_servers.read() {
                        Some(Ok(servers)) => rsx! {
                            for s in servers {
                                option { value: "{s.id}", "{s.slug} — {s.name}" }
                            }
                        },
                        _ => rsx! {},
                    }}
                }
                button {
                    class: "bg-blue-600 text-white px-2 py-1 rounded text-xs hover:bg-blue-700",
                    r#type: "submit",
                    "Add"
                }
            }
            {match &*deps.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    p { class: "text-xs text-gray-400", "No MCP dependencies." }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-gray-200",
                        for dep in list {
                            {
                                let dep_id = dep.dep_id.clone();
                                let slug = dep.mcp_server_slug.clone();
                                rsx! {
                                    li { class: "py-1 flex justify-between items-center",
                                        span { class: "text-sm font-mono", "{slug}" }
                                        button {
                                            class: "text-xs text-red-600 hover:underline",
                                            onclick: move |_| {
                                                let did = dep_id.clone();
                                                spawn(async move {
                                                    if remove_channel_mcp_dep(did).await.is_ok() {
                                                        deps.restart();
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
                Some(Err(e)) => rsx! { p { class: "text-red-600 text-xs", "Error: {e}" } },
                None => rsx! { p { class: "text-xs", "Loading..." } },
            }}
        }
    }
}
