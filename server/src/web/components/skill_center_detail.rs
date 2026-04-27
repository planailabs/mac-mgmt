use dioxus::prelude::*;

use super::skill_center_list::SkillCenterRow;
use crate::web::app::Route;

/// Summary of a skill center's cached catalog.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct CatalogSummary {
    pub skill_channels: usize,
    pub bundles: usize,
    pub mcp_servers: usize,
    pub mcp_bundles: usize,
    pub fetched_at: Option<String>,
}

#[server]
async fn get_catalog_summary(id: String) -> Result<CatalogSummary, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;

    let uuid: uuid::Uuid = id
        .parse()
        .map_err(|_| ServerFnError::new("invalid UUID"))?;

    // Access the skill center cache from global state
    let cache = crate::skill_center_cache::SkillCenterCache::global()
        .ok_or_else(|| ServerFnError::new("cache not available"))?;

    if let Some(cached) = cache.get(&uuid).await {
        Ok(CatalogSummary {
            skill_channels: cached.catalog.skill_channels.len(),
            bundles: cached.catalog.bundles.len(),
            mcp_servers: cached.catalog.mcp_servers.len(),
            mcp_bundles: cached.catalog.mcp_bundles.len(),
            fetched_at: Some(cached.fetched_at.to_rfc3339()),
        })
    } else {
        Ok(CatalogSummary::default())
    }
}

#[server]
async fn get_skill_center(id: String) -> Result<Option<SkillCenterRow>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    let uuid: uuid::Uuid = id
        .parse()
        .map_err(|_| ServerFnError::new("invalid UUID"))?;

    let row = sqlx::query_as::<_, SkillCenterRow>(
        "SELECT id, name, url, priority, enabled, created_at, updated_at \
         FROM skill_centers WHERE id = $1",
    )
    .bind(uuid)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(format!("query failed: {e}")))?;

    Ok(row)
}

#[server]
async fn update_skill_center(
    id: String,
    name: String,
    url: String,
    federation_token: String,
    priority: i32,
    enabled: bool,
) -> Result<SkillCenterRow, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    let uuid: uuid::Uuid = id
        .parse()
        .map_err(|_| ServerFnError::new("invalid UUID"))?;

    // Only update federation_token if non-empty (allows keeping existing token)
    let row = if federation_token.trim().is_empty() {
        sqlx::query_as::<_, SkillCenterRow>(
            "UPDATE skill_centers SET name = $1, url = $2, \
             priority = $3, enabled = $4, updated_at = now() \
             WHERE id = $5 \
             RETURNING id, name, url, priority, enabled, created_at, updated_at",
        )
        .bind(&name)
        .bind(&url)
        .bind(priority)
        .bind(enabled)
        .bind(uuid)
        .fetch_one(&pool)
        .await
    } else {
        sqlx::query_as::<_, SkillCenterRow>(
            "UPDATE skill_centers SET name = $1, url = $2, federation_token = $3, \
             priority = $4, enabled = $5, updated_at = now() \
             WHERE id = $6 \
             RETURNING id, name, url, priority, enabled, created_at, updated_at",
        )
        .bind(&name)
        .bind(&url)
        .bind(&federation_token)
        .bind(priority)
        .bind(enabled)
        .bind(uuid)
        .fetch_one(&pool)
        .await
    };

    row.map_err(|e| ServerFnError::new(format!("update failed: {e}")))
}

#[server]
async fn delete_skill_center(id: String) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    let uuid: uuid::Uuid = id
        .parse()
        .map_err(|_| ServerFnError::new("invalid UUID"))?;

    sqlx::query("DELETE FROM skill_centers WHERE id = $1")
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(format!("delete failed: {e}")))?;

    Ok(())
}

#[component]
pub fn SkillCenterDetail(id: String) -> Element {
    let id2 = id.clone();
    let mut center_future = use_server_future(move || {
        let id = id.clone();
        async move { get_skill_center(id).await }
    })?;

    let catalog_summary = use_server_future(move || {
        let id = id2.clone();
        async move { get_catalog_summary(id).await }
    })?;

    let mut editing = use_signal(|| false);
    let mut draft_name = use_signal(String::new);
    let mut draft_url = use_signal(String::new);
    let mut draft_token = use_signal(String::new);
    let mut draft_priority = use_signal(|| "0".to_string());
    let mut draft_enabled = use_signal(|| true);
    let mut error = use_signal(|| None::<String>);
    let nav = navigator();

    rsx! {
        div { class: "px-6 py-8 max-w-3xl mx-auto",
            match &*center_future.read() {
                Some(Ok(Some(center))) => {
                    let center_id = center.id.to_string();
                    let center_name = center.name.clone();
                    let center_url = center.url.clone();
                    let center_url_display = center.url.clone();
                    let center_priority = center.priority;
                    let center_enabled = center.enabled;

                    rsx! {
                        div { class: "flex items-center justify-between mb-6",
                            h1 { class: "text-2xl font-bold dark:text-white", "{center_name}" }
                            div { class: "flex gap-2",
                                if !*editing.read() {
                                    button {
                                        onclick: move |_| {
                                            draft_name.set(center_name.clone());
                                            draft_url.set(center_url.clone());
                                            draft_token.set(String::new());
                                            draft_priority.set(center_priority.to_string());
                                            draft_enabled.set(center_enabled);
                                            editing.set(true);
                                        },
                                        class: "bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 px-3 py-1.5 rounded text-sm hover:bg-gray-200 dark:hover:bg-gray-600",
                                        "Edit"
                                    }
                                    button {
                                        onclick: {
                                            let id = center_id.clone();
                                            move |_| {
                                                let id = id.clone();
                                                let nav = nav.clone();
                                                spawn(async move {
                                                    if let Err(e) = delete_skill_center(id).await {
                                                        tracing::error!("delete failed: {e}");
                                                    } else {
                                                        nav.push(Route::SkillCenterList {});
                                                    }
                                                });
                                            }
                                        },
                                        class: "bg-red-100 dark:bg-red-900 text-red-700 dark:text-red-300 px-3 py-1.5 rounded text-sm hover:bg-red-200 dark:hover:bg-red-800",
                                        "Delete"
                                    }
                                }
                            }
                        }

                        if *editing.read() {
                            form {
                                onsubmit: {
                                    let id = center_id.clone();
                                    move |e: Event<FormData>| {
                                        e.prevent_default();
                                        let id = id.clone();
                                        let name = draft_name.read().clone();
                                        let url = draft_url.read().clone();
                                        let token = draft_token.read().clone();
                                        let priority: i32 = draft_priority.read().parse().unwrap_or(0);
                                        let enabled = *draft_enabled.read();
                                        spawn(async move {
                                            match update_skill_center(id, name, url, token, priority, enabled).await {
                                                Ok(_) => {
                                                    editing.set(false);
                                                    error.set(None);
                                                    center_future.restart();
                                                }
                                                Err(e) => error.set(Some(e.to_string())),
                                            }
                                        });
                                    }
                                },
                                class: "space-y-4",
                                if let Some(err) = &*error.read() {
                                    p { class: "text-red-600 text-sm", "{err}" }
                                }
                                div {
                                    label { class: "block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1", "Name" }
                                    input {
                                        r#type: "text",
                                        value: "{draft_name}",
                                        oninput: move |e| draft_name.set(e.value()),
                                        class: "w-full border border-gray-300 dark:border-gray-600 dark:bg-gray-700 dark:text-white rounded px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500",
                                    }
                                }
                                div {
                                    label { class: "block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1", "URL" }
                                    input {
                                        r#type: "text",
                                        value: "{draft_url}",
                                        oninput: move |e| draft_url.set(e.value()),
                                        class: "w-full border border-gray-300 dark:border-gray-600 dark:bg-gray-700 dark:text-white rounded px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500",
                                    }
                                }
                                div {
                                    label { class: "block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1", "Federation Token" }
                                    input {
                                        r#type: "password",
                                        value: "{draft_token}",
                                        oninput: move |e| draft_token.set(e.value()),
                                        class: "w-full border border-gray-300 dark:border-gray-600 dark:bg-gray-700 dark:text-white rounded px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500",
                                        placeholder: "Leave empty to keep current token",
                                    }
                                }
                                div {
                                    label { class: "block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1", "Priority" }
                                    input {
                                        r#type: "number",
                                        value: "{draft_priority}",
                                        oninput: move |e| draft_priority.set(e.value()),
                                        class: "w-full border border-gray-300 dark:border-gray-600 dark:bg-gray-700 dark:text-white rounded px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500",
                                    }
                                }
                                div { class: "flex items-center gap-2",
                                    input {
                                        r#type: "checkbox",
                                        checked: "{draft_enabled}",
                                        oninput: move |e| draft_enabled.set(e.value() == "true"),
                                        class: "rounded border-gray-300 dark:border-gray-600",
                                        id: "edit-enabled",
                                    }
                                    label { r#for: "edit-enabled", class: "text-sm text-gray-700 dark:text-gray-300", "Enabled" }
                                }
                                div { class: "flex gap-2",
                                    button {
                                        r#type: "submit",
                                        class: "bg-blue-600 text-white px-4 py-2 rounded text-sm hover:bg-blue-700",
                                        "Save"
                                    }
                                    button {
                                        r#type: "button",
                                        onclick: move |_| { editing.set(false); error.set(None); },
                                        class: "bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 px-4 py-2 rounded text-sm hover:bg-gray-200 dark:hover:bg-gray-600",
                                        "Cancel"
                                    }
                                }
                            }
                        } else {
                            dl { class: "grid grid-cols-2 gap-x-4 gap-y-2 text-sm mt-4",
                                dt { class: "font-medium dark:text-gray-300", "URL" }
                                dd { class: "dark:text-gray-400", "{center_url_display}" }
                                dt { class: "font-medium dark:text-gray-300", "Priority" }
                                dd { class: "dark:text-gray-400", "{center_priority}" }
                                dt { class: "font-medium dark:text-gray-300", "Enabled" }
                                dd { class: "dark:text-gray-400",
                                    if center_enabled { "Yes" } else { "No" }
                                }
                                dt { class: "font-medium dark:text-gray-300", "Created" }
                                dd { class: "dark:text-gray-400", "{center.created_at}" }
                                dt { class: "font-medium dark:text-gray-300", "Updated" }
                                dd { class: "dark:text-gray-400", "{center.updated_at}" }
                            }

                            // Cached catalog summary
                            h3 { class: "text-lg font-semibold mt-6 mb-3 dark:text-white", "Cached Catalog" }
                            {match &*catalog_summary.read() {
                                Some(Ok(summary)) => {
                                    if summary.skill_channels == 0 && summary.bundles == 0 && summary.mcp_servers == 0 && summary.mcp_bundles == 0 {
                                        rsx! { p { class: "text-gray-500 dark:text-gray-400 text-sm", "No catalog data cached yet. The catalog will be fetched automatically." } }
                                    } else {
                                        rsx! {
                                            div { class: "grid grid-cols-2 sm:grid-cols-4 gap-4",
                                                div { class: "bg-gray-50 dark:bg-gray-800 rounded p-3",
                                                    p { class: "text-2xl font-bold dark:text-white", "{summary.skill_channels}" }
                                                    p { class: "text-xs text-gray-500 dark:text-gray-400", "Skill Channels" }
                                                }
                                                div { class: "bg-gray-50 dark:bg-gray-800 rounded p-3",
                                                    p { class: "text-2xl font-bold dark:text-white", "{summary.bundles}" }
                                                    p { class: "text-xs text-gray-500 dark:text-gray-400", "Bundles" }
                                                }
                                                div { class: "bg-gray-50 dark:bg-gray-800 rounded p-3",
                                                    p { class: "text-2xl font-bold dark:text-white", "{summary.mcp_servers}" }
                                                    p { class: "text-xs text-gray-500 dark:text-gray-400", "MCP Servers" }
                                                }
                                                div { class: "bg-gray-50 dark:bg-gray-800 rounded p-3",
                                                    p { class: "text-2xl font-bold dark:text-white", "{summary.mcp_bundles}" }
                                                    p { class: "text-xs text-gray-500 dark:text-gray-400", "MCP Bundles" }
                                                }
                                            }
                                            if let Some(ref ts) = summary.fetched_at {
                                                p { class: "text-xs text-gray-500 dark:text-gray-400 mt-2", "Last synced: {ts}" }
                                            }
                                        }
                                    }
                                },
                                Some(Err(e)) => rsx! { p { class: "text-red-500 text-sm", "Failed to load catalog summary: {e}" } },
                                None => rsx! { p { class: "text-gray-500 text-sm", "Loading catalog..." } },
                            }}
                        }
                    }
                },
                Some(Ok(None)) => rsx! { p { class: "text-red-500", "Skill center not found." } },
                Some(Err(e)) => rsx! { p { class: "text-red-500", "Error: {e}" } },
                None => rsx! { p { class: "text-gray-500", "Loading..." } },
            }
        }
    }
}
