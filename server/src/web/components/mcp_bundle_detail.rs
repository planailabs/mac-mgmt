use dioxus::prelude::*;

use crate::anthropic::{GenerateContext, GeneratedNameDesc, McpBundleItemContext};
use crate::models::McpServerBundle;
use crate::web::app::Route;
use crate::web::components::generate_button::GenerateButton;

/// An MCP server for display in the bundle items list.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct McpBundleItemDisplay {
    pub bundle_item_id: uuid::Uuid,
    pub server_slug: String,
    pub server_name: String,
}

/// Available MCP server for the add dropdown.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct McpServerOption {
    pub id: uuid::Uuid,
    pub slug: String,
    pub name: String,
}

#[server]
async fn get_mcp_bundle(id: String) -> Result<McpServerBundle, ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let bundle = sqlx::query_as::<_, McpServerBundle>("SELECT * FROM mcp_server_bundles WHERE id = $1")
        .bind(uuid)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(bundle)
}

#[server]
async fn delete_mcp_bundle(id: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    crate::api::push::notify_mcp_bundle_global(uuid).await;
    sqlx::query("DELETE FROM mcp_server_bundles WHERE id = $1")
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn update_mcp_bundle(id: String, name: String, description: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("UPDATE mcp_server_bundles SET name = $1, description = $2 WHERE id = $3")
        .bind(&name)
        .bind(&description)
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    crate::api::push::notify_mcp_bundle_global(uuid).await;
    Ok(())
}

#[server]
async fn list_mcp_bundle_items(bundle_id: String) -> Result<Vec<McpBundleItemDisplay>, ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = bundle_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let items = sqlx::query_as::<_, McpBundleItemDisplay>(
        "SELECT msbi.id as bundle_item_id, ms.slug as server_slug, ms.name as server_name \
         FROM mcp_server_bundle_items msbi \
         JOIN mcp_servers ms ON ms.id = msbi.mcp_server_id \
         WHERE msbi.bundle_id = $1 \
         ORDER BY ms.slug",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(items)
}

#[server]
async fn list_available_mcp_servers() -> Result<Vec<McpServerOption>, ServerFnError> {
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
async fn add_mcp_bundle_item(bundle_id: String, mcp_server_id: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let bid: uuid::Uuid = bundle_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let msid: uuid::Uuid = mcp_server_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("INSERT INTO mcp_server_bundle_items (bundle_id, mcp_server_id) VALUES ($1, $2)")
        .bind(bid)
        .bind(msid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    crate::api::push::notify_mcp_bundle_global(bid).await;
    Ok(())
}

#[server]
async fn remove_mcp_bundle_item(bundle_item_id: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = bundle_item_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let bundle_id: Option<uuid::Uuid> = sqlx::query_scalar(
        "SELECT bundle_id FROM mcp_server_bundle_items WHERE id = $1",
    )
    .bind(uuid)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    sqlx::query("DELETE FROM mcp_server_bundle_items WHERE id = $1")
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    if let Some(bid) = bundle_id {
        crate::api::push::notify_mcp_bundle_global(bid).await;
    }
    Ok(())
}

#[component]
pub fn McpBundleDetail(id: String) -> Element {
    let navigator = navigator();
    let id_clone = id.clone();
    let mut bundle = use_server_future(move || {
        let id = id_clone.clone();
        async move { get_mcp_bundle(id).await }
    })?;

    let id_items = id.clone();
    let mut items = use_server_future(move || {
        let id = id_items.clone();
        async move { list_mcp_bundle_items(id).await }
    })?;

    let mut available = use_server_future(list_available_mcp_servers)?;

    let mut editing = use_signal(|| false);
    let mut draft_name = use_signal(String::new);
    let mut draft_desc = use_signal(String::new);
    let mut selected_server = use_signal(String::new);

    match &*bundle.read() {
        Some(Ok(b)) => {
            let created = b.created_at.format("%Y-%m-%d %H:%M").to_string();
            let bid = b.id.to_string();
            let bid_del = bid.clone();
            let bid2 = bid.clone();
            let name = b.name.clone();
            let desc = b.description.clone();
            let slug = b.slug.clone();

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
                                spawn(async move {
                                    if !new_name.trim().is_empty() {
                                        let _ = update_mcp_bundle(id, new_name, new_desc).await;
                                        bundle.restart();
                                    }
                                    editing.set(false);
                                });
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
                                {
                                    let mcp_items_ctx = match &*items.read() {
                                        Some(Ok(list)) => list.iter().map(|i| McpBundleItemContext {
                                            server_slug: i.server_slug.clone(),
                                            server_name: i.server_name.clone(),
                                        }).collect(),
                                        _ => vec![],
                                    };
                                    rsx! {
                                        GenerateButton {
                                            context: GenerateContext::McpBundle { slug: slug.clone(), items: mcp_items_ctx },
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
                        button {
                            class: "text-red-400 hover:text-red-600",
                            onclick: move |_| {
                                let id = bid_del.clone();
                                let nav = navigator.clone();
                                spawn(async move {
                                    if delete_mcp_bundle(id).await.is_ok() {
                                        nav.push(Route::McpBundleList {});
                                    }
                                });
                            },
                            "Delete"
                        }
                    }
                }
                if !*editing.read() && !b.description.is_empty() {
                    p { class: "text-gray-600 mb-2", "{b.description}" }
                }
                p { class: "text-gray-500 text-sm mb-6", "Created: {created}" }

                // Items section
                div {
                    h3 { class: "text-lg font-semibold mb-3", "MCP Servers" }
                    form {
                        class: "flex gap-2 mb-4",
                        onsubmit: move |evt: FormEvent| {
                            evt.prevent_default();
                            let bid = bid2.clone();
                            let msid = selected_server.read().clone();
                            spawn(async move {
                                if !msid.is_empty() {
                                    if add_mcp_bundle_item(bid, msid).await.is_ok() {
                                        selected_server.set(String::new());
                                        items.restart();
                                        available.restart();
                                    }
                                }
                            });
                        },
                        select {
                            class: "flex-1 border border-gray-300 rounded px-3 py-1 text-sm",
                            value: "{selected_server}",
                            onchange: move |evt| selected_server.set(evt.value()),
                            option { value: "", "Select MCP server..." }
                            {match &*available.read() {
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
                    {match &*items.read() {
                        Some(Ok(list)) => rsx! {
                            ul { class: "divide-y divide-gray-200",
                                for item in list {
                                    {
                                        let biid = item.bundle_item_id.to_string();
                                        let label = format!("{} ({})", item.server_name, item.server_slug);
                                        rsx! {
                                            li { class: "py-2 flex justify-between items-center",
                                                span { class: "text-sm font-mono", "{label}" }
                                                button {
                                                    class: "text-xs text-red-600 hover:underline",
                                                    onclick: move |_| {
                                                        let biid = biid.clone();
                                                        spawn(async move {
                                                            if remove_mcp_bundle_item(biid).await.is_ok() {
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
