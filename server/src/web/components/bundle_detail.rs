use dioxus::prelude::*;

use crate::anthropic::{BundleItemContext, GenerateContext, GeneratedNameDesc};
use crate::models::Bundle;
use crate::web::app::Route;
use crate::web::components::generate_button::GenerateButton;
use crate::web::components::hidden_badge::HiddenBadge;

/// A skill channel with its skill slug for display.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct SkillChannelDisplay {
    pub id: uuid::Uuid,
    pub skill_slug: String,
    pub channel: String,
}

/// A bundle item joined with skill info for display.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct BundleItemDisplay {
    pub bundle_item_id: uuid::Uuid,
    pub skill_slug: String,
    pub channel: String,
}

#[server]
async fn get_bundle(id: String) -> Result<Bundle, ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let bundle = sqlx::query_as::<_, Bundle>("SELECT * FROM bundles WHERE id = $1")
        .bind(uuid)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(bundle)
}

#[server]
async fn delete_bundle(id: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    // Notify affected clusters before the cascade removes their assignments.
    crate::api::push::notify_skill_bundle_global(uuid).await;
    sqlx::query("DELETE FROM bundles WHERE id = $1")
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn update_bundle(
    id: String,
    name: String,
    description: String,
    hide_from_public_catalog: bool,
) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("UPDATE bundles SET name = $1, description = $2, hide_from_public_catalog = $3 WHERE id = $4")
        .bind(&name)
        .bind(&description)
        .bind(hide_from_public_catalog)
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    crate::api::push::notify_skill_bundle_global(uuid).await;
    Ok(())
}

#[server]
async fn list_bundle_items(bundle_id: String) -> Result<Vec<BundleItemDisplay>, ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = bundle_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let items = sqlx::query_as::<_, BundleItemDisplay>(
        "SELECT bi.id as bundle_item_id, s.slug as skill_slug, sc.channel \
         FROM bundle_items bi \
         JOIN skill_channels sc ON sc.id = bi.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE bi.bundle_id = $1 \
         ORDER BY s.slug, sc.channel",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(items)
}

#[server]
async fn list_available_skill_channels() -> Result<Vec<SkillChannelDisplay>, ServerFnError> {
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

#[server]
async fn add_bundle_item(bundle_id: String, skill_channel_id: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let bid: uuid::Uuid = bundle_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let scid: uuid::Uuid = skill_channel_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("INSERT INTO bundle_items (bundle_id, skill_channel_id) VALUES ($1, $2)")
        .bind(bid)
        .bind(scid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    crate::api::push::notify_skill_bundle_global(bid).await;
    Ok(())
}

#[server]
async fn remove_bundle_item(bundle_item_id: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = bundle_item_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let bundle_id: Option<uuid::Uuid> = sqlx::query_scalar(
        "SELECT bundle_id FROM bundle_items WHERE id = $1",
    )
    .bind(uuid)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    sqlx::query("DELETE FROM bundle_items WHERE id = $1")
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    if let Some(bid) = bundle_id {
        crate::api::push::notify_skill_bundle_global(bid).await;
    }
    Ok(())
}

#[component]
pub fn BundleDetail(id: String) -> Element {
    let navigator = navigator();
    let id_clone = id.clone();
    let mut bundle = use_server_future(move || {
        let id = id_clone.clone();
        async move { get_bundle(id).await }
    })?;

    let id_items = id.clone();
    let mut items = use_server_future(move || {
        let id = id_items.clone();
        async move { list_bundle_items(id).await }
    })?;

    let mut available = use_server_future(list_available_skill_channels)?;

    let mut editing = use_signal(|| false);
    let mut draft_name = use_signal(String::new);
    let mut draft_desc = use_signal(String::new);
    let mut draft_hide = use_signal(|| false);
    let mut selected_sc = use_signal(String::new);

    match &*bundle.read() {
        Some(Ok(b)) => {
            let created = b.created_at.format("%Y-%m-%d %H:%M").to_string();
            let bid = b.id.to_string();
            let bid_del = bid.clone();
            let bid2 = bid.clone();
            let name = b.name.clone();
            let desc = b.description.clone();
            let slug = b.slug.clone();
            let hide_flag = b.hide_from_public_catalog;

            rsx! {
                div { class: "flex items-center gap-3 mb-1",
                    if *editing.read() {
                        form {
                            class: "space-y-2",
                            onsubmit: move |evt: FormEvent| {
                                evt.prevent_default();
                                let id = bid.clone();
                                let new_name = draft_name.read().clone();
                                let new_desc = draft_desc.read().clone();
                                let new_hide = *draft_hide.read();
                                spawn(async move {
                                    if !new_name.trim().is_empty() {
                                        let _ = update_bundle(id, new_name, new_desc, new_hide).await;
                                        bundle.restart();
                                    }
                                    editing.set(false);
                                });
                            },
                            input {
                                class: "text-2xl font-bold border border-gray-300 dark:border-gray-600 rounded px-2 py-1 w-full dark:bg-gray-700 dark:text-white",
                                r#type: "text",
                                value: "{draft_name}",
                                oninput: move |e| draft_name.set(e.value()),
                                autofocus: true,
                            }
                            textarea {
                                class: "w-full border border-gray-300 dark:border-gray-600 rounded px-2 py-1 dark:bg-gray-700 dark:text-white",
                                rows: "2",
                                value: "{draft_desc}",
                                oninput: move |e| draft_desc.set(e.value()),
                            }
                            label { class: "flex items-center gap-2 text-sm text-gray-700 dark:text-gray-200",
                                input {
                                    r#type: "checkbox",
                                    checked: "{draft_hide}",
                                    oninput: move |e| draft_hide.set(e.value() == "true"),
                                }
                                "Hide from public catalog"
                            }
                            div { class: "flex gap-2",
                                button { class: "text-green-600 dark:text-green-400 hover:text-green-800 dark:hover:text-green-300", r#type: "submit", "Save" }
                                button {
                                    class: "text-gray-500 hover:text-gray-700 dark:hover:text-gray-200",
                                    r#type: "button",
                                    onclick: move |_| editing.set(false),
                                    "Cancel"
                                }
                                {
                                    let bundle_items_ctx = match &*items.read() {
                                        Some(Ok(list)) => list.iter().map(|i| BundleItemContext {
                                            skill_slug: i.skill_slug.clone(),
                                            channel: i.channel.clone(),
                                        }).collect(),
                                        _ => vec![],
                                    };
                                    rsx! {
                                        GenerateButton {
                                            context: GenerateContext::Bundle { slug: slug.clone(), items: bundle_items_ctx },
                                            current_name: draft_name.read().clone(),
                                            current_desc: draft_desc.read().clone(),
                                            on_generated: move |result: GeneratedNameDesc| {
                                                draft_name.set(result.name);
                                                draft_desc.set(result.description);
                                            },
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        h2 { class: "text-2xl font-bold", "{name}" }
                        span { class: "text-gray-400 dark:text-gray-500 font-mono text-sm", "({slug})" }
                        HiddenBadge { hidden: hide_flag }
                        button {
                            class: "text-gray-400 hover:text-gray-600 dark:text-gray-400 dark:hover:text-gray-300",
                            onclick: move |_| {
                                draft_name.set(name.clone());
                                draft_desc.set(desc.clone());
                                draft_hide.set(hide_flag);
                                editing.set(true);
                            },
                            "Edit"
                        }
                        button {
                            class: "text-red-400 dark:text-red-500 hover:text-red-600 dark:hover:text-red-400",
                            onclick: move |_| {
                                let id = bid_del.clone();
                                let nav = navigator.clone();
                                spawn(async move {
                                    if delete_bundle(id).await.is_ok() {
                                        nav.push(Route::BundleList {});
                                    }
                                });
                            },
                            "Delete"
                        }
                    }
                }
                if !*editing.read() && !b.description.is_empty() {
                    p { class: "text-gray-600 dark:text-gray-300 mb-2", "{b.description}" }
                }
                p { class: "text-gray-500 dark:text-gray-400 text-sm mb-6", "Created: {created}" }

                // Items section
                div {
                    h3 { class: "text-lg font-semibold mb-3", "Skill Channels" }
                    form {
                        class: "flex gap-2 mb-4",
                        onsubmit: move |evt: FormEvent| {
                            evt.prevent_default();
                            let bid = bid2.clone();
                            let scid = selected_sc.read().clone();
                            spawn(async move {
                                if !scid.is_empty() {
                                    if add_bundle_item(bid, scid).await.is_ok() {
                                        selected_sc.set(String::new());
                                        items.restart();
                                        available.restart();
                                    }
                                }
                            });
                        },
                        select {
                            class: "flex-1 border border-gray-300 dark:border-gray-600 rounded px-3 py-1 text-sm dark:bg-gray-700 dark:text-white",
                            value: "{selected_sc}",
                            onchange: move |evt| selected_sc.set(evt.value()),
                            option { value: "", "Select skill/channel..." }
                            {match &*available.read() {
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
                    {match &*items.read() {
                        Some(Ok(list)) => rsx! {
                            ul { class: "divide-y divide-gray-200 dark:divide-gray-700",
                                for item in list {
                                    {
                                        let biid = item.bundle_item_id.to_string();
                                        let label = format!("{} / {}", item.skill_slug, item.channel);
                                        rsx! {
                                            li { class: "py-2 flex justify-between items-center",
                                                span { class: "text-sm font-mono", "{label}" }
                                                button {
                                                    class: "text-xs text-red-600 dark:text-red-400 hover:underline",
                                                    onclick: move |_| {
                                                        let biid = biid.clone();
                                                        spawn(async move {
                                                            if remove_bundle_item(biid).await.is_ok() {
                                                                items.restart();
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
                        None => rsx! { p { class: "text-sm", "Loading..." } },
                    }}
                }
            }
        }
        Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400", "Error: {e}" } },
        None => rsx! { p { "Loading..." } },
    }
}
