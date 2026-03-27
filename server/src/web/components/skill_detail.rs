use std::collections::HashMap;

use dioxus::prelude::*;

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

#[server]
async fn add_channel(skill_id: String, channel: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = skill_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    // Look up the skill slug to check xzar
    let slug = sqlx::query_scalar::<_, String>("SELECT slug FROM skills WHERE id = $1")
        .bind(uuid)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let cfg = crate::config::config();
    let pins = crate::xzar::fetch_pins(&cfg.xzar.url, &cfg.xzar.token)
        .await
        .map_err(|e| ServerFnError::new(format!("xzar error: {e}")))?;

    let pin_name = format!("skill/{slug}/{channel}");
    let pin = pins.iter().find(|p| p.name == pin_name && !p.abandoned);
    match pin {
        None => return Err(ServerFnError::new(format!("no xzar pin found for {pin_name}"))),
        Some(p) if p.roots.is_empty() => {
            return Err(ServerFnError::new(format!("xzar pin {pin_name} has no roots")))
        }
        _ => {}
    }

    sqlx::query("INSERT INTO skill_channels (skill_id, channel) VALUES ($1, $2)")
        .bind(uuid)
        .bind(&channel)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

/// Resolve store paths for all channels of a skill from xzar.
#[server]
async fn resolve_channel_paths(skill_id: String) -> Result<HashMap<String, String>, ServerFnError> {
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

    let skills: Vec<(String, String)> = channels.into_iter().map(|ch| (slug.clone(), ch)).collect();
    // resolve_store_paths returns slug→path, but we need channel→path
    // so we call it per-channel
    let mut result = HashMap::new();
    for (_, channel) in &skills {
        let pin_name = format!("skill/{slug}/{channel}");
        if let Some(pin) = pins.iter().find(|p| p.name == pin_name && !p.abandoned) {
            if let Some(root) = pin.roots.first() {
                result.insert(channel.clone(), root.drv_full.clone());
            }
        }
    }

    Ok(result)
}

#[server]
async fn remove_channel(channel_id: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = channel_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("DELETE FROM skill_channels WHERE id = $1")
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
    let mut channels = use_server_future(move || {
        let id = id_channels.clone();
        async move { list_channels(id).await }
    })?;

    let id_paths = id.clone();
    let mut paths = use_server_future(move || {
        let id = id_paths.clone();
        async move { resolve_channel_paths(id).await }
    })?;

    let mut editing = use_signal(|| false);
    let mut draft_name = use_signal(String::new);
    let mut draft_desc = use_signal(String::new);
    let mut new_channel = use_signal(String::new);
    let mut channel_error = use_signal(|| None::<String>);

    match &*skill.read() {
        Some(Ok(s)) => {
            let created = s.created_at.format("%Y-%m-%d %H:%M").to_string();
            let sid = s.id.to_string();
            let sid2 = sid.clone();
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

                // Channels section
                div {
                    h3 { class: "text-lg font-semibold mb-3", "Channels" }
                    if let Some(err) = &*channel_error.read() {
                        p { class: "text-red-600 text-sm mb-2", "{err}" }
                    }
                    form {
                        class: "flex gap-2 mb-4",
                        onsubmit: move |evt: FormEvent| {
                            evt.prevent_default();
                            let sid = sid2.clone();
                            let ch = new_channel.read().clone();
                            spawn(async move {
                                if !ch.trim().is_empty() {
                                    match add_channel(sid, ch).await {
                                        Ok(()) => {
                                            channel_error.set(None);
                                            new_channel.set(String::new());
                                            channels.restart();
                                            paths.restart();
                                        }
                                        Err(e) => {
                                            channel_error.set(Some(e.to_string()));
                                        }
                                    }
                                }
                            });
                        },
                        input {
                            class: "flex-1 border border-gray-300 rounded px-3 py-1 text-sm font-mono",
                            r#type: "text",
                            placeholder: "Channel name (e.g. stable)",
                            value: "{new_channel}",
                            oninput: move |evt| new_channel.set(evt.value()),
                        }
                        button {
                            class: "bg-blue-600 text-white px-3 py-1 rounded text-sm hover:bg-blue-700",
                            r#type: "submit",
                            "Add Channel"
                        }
                    }
                    {match &*channels.read() {
                        Some(Ok(list)) => {
                            let path_map = match &*paths.read() {
                                Some(Ok(m)) => m.clone(),
                                _ => HashMap::new(),
                            };
                            rsx! {
                                ul { class: "divide-y divide-gray-200",
                                    for ch in list {
                                        {
                                            let chid = ch.id.to_string();
                                            let ch_name = ch.channel.clone();
                                            let ch_created = ch.created_at.format("%Y-%m-%d").to_string();
                                            let store_path = path_map.get(&ch.channel).cloned();
                                            rsx! {
                                                li { class: "py-2",
                                                    div { class: "flex justify-between items-center",
                                                        div {
                                                            span { class: "text-sm font-mono font-medium", "{ch_name}" }
                                                            span { class: "text-xs text-gray-400 ml-2", "{ch_created}" }
                                                        }
                                                        button {
                                                            class: "text-xs text-red-600 hover:underline",
                                                            onclick: move |_| {
                                                                let chid = chid.clone();
                                                                spawn(async move {
                                                                    if remove_channel(chid).await.is_ok() {
                                                                        channels.restart();
                                                                        paths.restart();
                                                                    }
                                                                });
                                                            },
                                                            "Remove"
                                                        }
                                                    }
                                                    if let Some(sp) = &store_path {
                                                        p { class: "text-xs text-gray-400 font-mono mt-0.5 truncate", "{sp}" }
                                                    }
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
