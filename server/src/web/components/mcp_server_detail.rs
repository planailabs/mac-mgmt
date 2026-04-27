use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::anthropic::{GenerateContext, GeneratedNameDesc};
use crate::models::McpServer;
use crate::web::app::Route;
use crate::web::components::generate_button::GenerateButton;
use crate::web::components::hidden_badge::HiddenBadge;
#[cfg(feature = "server")]
use crate::web::user::current_user;

// ── Server functions ─────────────────────────────────────────────────

#[server]
async fn get_mcp_server(id: String) -> Result<McpServer, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query_as::<_, McpServer>("SELECT * FROM mcp_servers WHERE id = $1")
        .bind(uuid)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
async fn upsert_mcp_server(
    id: Option<String>,
    slug: String,
    name: String,
    description: String,
    config_json: String,
    hide_from_public_catalog: bool,
) -> Result<McpServer, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let parsed: serde_json::Value = serde_json::from_str(&config_json)
        .map_err(|e| ServerFnError::new(format!("invalid JSON: {e}")))?;
    crate::mcp_schema::validate_mcp_server_config(&parsed)
        .map_err(|e| ServerFnError::new(format!("schema validation failed: {e}")))?;

    if let Some(id) = id {
        let uuid: uuid::Uuid = id
            .parse()
            .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
        sqlx::query_as::<_, McpServer>(
            "UPDATE mcp_servers SET name = $1, description = $2, config_json = $3, hide_from_public_catalog = $4 WHERE id = $5 RETURNING *",
        )
        .bind(&name)
        .bind(&description)
        .bind(&parsed)
        .bind(hide_from_public_catalog)
        .bind(uuid)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
    } else {
        sqlx::query_as::<_, McpServer>(
            "INSERT INTO mcp_servers (slug, name, description, config_json, hide_from_public_catalog) VALUES ($1, $2, $3, $4, $5) RETURNING *",
        )
        .bind(&slug)
        .bind(&name)
        .bind(&description)
        .bind(&parsed)
        .bind(hide_from_public_catalog)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
    }
}

#[server]
async fn add_nix_package(id: String, package: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let pkg = package.trim().to_string();
    if pkg.is_empty() {
        return Err(ServerFnError::new("package name cannot be empty"));
    }
    sqlx::query(
        "UPDATE mcp_servers SET nix_packages = array_append(nix_packages, $1) \
         WHERE id = $2 AND NOT ($1 = ANY(nix_packages))",
    )
    .bind(&pkg)
    .bind(uuid)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn remove_nix_package(id: String, package: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query(
        "UPDATE mcp_servers SET nix_packages = array_remove(nix_packages, $1) WHERE id = $2",
    )
    .bind(&package)
    .bind(uuid)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn delete_mcp_server(id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    // Resolve affected clusters BEFORE the delete — the cascade will wipe
    // both direct assignments and bundle memberships, so a post-delete query
    // would find nothing.
    crate::api::push::notify_federation_global();
    crate::api::push::notify_mcp_server_global(uuid).await;
    sqlx::query("DELETE FROM mcp_servers WHERE id = $1")
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SkillDepRow {
    skill_slug: String,
    channel: String,
    skill_id: String,
}

#[server]
async fn list_dependent_skills(mcp_server_id: String) -> Result<Vec<SkillDepRow>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = mcp_server_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct Row {
        skill_slug: String,
        channel: String,
        skill_id: uuid::Uuid,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT s.slug as skill_slug, sc.channel, s.id as skill_id \
         FROM skill_mcp_dependencies smd \
         JOIN skill_channels sc ON sc.id = smd.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE smd.mcp_server_id = $1 \
         ORDER BY s.slug, sc.channel",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| SkillDepRow {
            skill_slug: r.skill_slug,
            channel: r.channel,
            skill_id: r.skill_id.to_string(),
        })
        .collect())
}

// ── Shared form fields component ─────────────────────────────────────

#[component]
fn McpServerFormFields(
    slug: Signal<String>,
    name: Signal<String>,
    description: Signal<String>,
    config_json: Signal<String>,
    hide_from_public_catalog: Signal<bool>,
    slug_readonly: bool,
) -> Element {
    rsx! {
        div { class: "mb-4",
            label { class: "block text-sm font-medium text-gray-700 dark:text-gray-200 mb-1", "Slug" }
            input {
                class: "w-full border border-gray-300 dark:border-gray-600 rounded px-3 py-2 font-mono dark:bg-gray-700 dark:text-white",
                class: if slug_readonly { "bg-gray-100 dark:bg-gray-700 text-gray-500 dark:text-gray-400" } else { "" },
                r#type: "text",
                required: true,
                readonly: slug_readonly,
                placeholder: "my-server",
                value: "{slug}",
                oninput: move |evt| slug.set(evt.value()),
            }
        }
        div { class: "mb-4",
            label { class: "block text-sm font-medium text-gray-700 dark:text-gray-200 mb-1", "Name" }
            input {
                class: "w-full border border-gray-300 dark:border-gray-600 rounded px-3 py-2 dark:bg-gray-700 dark:text-white",
                r#type: "text",
                required: true,
                value: "{name}",
                oninput: move |evt| name.set(evt.value()),
            }
        }
        div { class: "mb-4",
            label { class: "block text-sm font-medium text-gray-700 dark:text-gray-200 mb-1", "Description" }
            textarea {
                class: "w-full border border-gray-300 dark:border-gray-600 rounded px-3 py-2 dark:bg-gray-700 dark:text-white",
                rows: "2",
                value: "{description}",
                oninput: move |evt| description.set(evt.value()),
            }
        }
        div { class: "mb-4",
            label { class: "block text-sm font-medium text-gray-700 dark:text-gray-200 mb-1", "Config JSON" }
            textarea {
                class: "w-full border border-gray-300 dark:border-gray-600 rounded px-3 py-2 font-mono text-sm dark:bg-gray-700 dark:text-white",
                rows: "10",
                required: true,
                value: "{config_json}",
                oninput: move |evt| config_json.set(evt.value()),
            }
        }
        div { class: "mb-4",
            label { class: "flex items-center gap-2 text-sm text-gray-700 dark:text-gray-200",
                input {
                    r#type: "checkbox",
                    checked: "{hide_from_public_catalog}",
                    oninput: move |evt| hide_from_public_catalog.set(evt.value() == "true"),
                }
                "Hide from public catalog"
            }
        }
    }
}

// ── Detail page (read-only + nix packages) ───────────────────────────

#[component]
pub fn McpServerDetail(id: String) -> Element {
    let navigator = navigator();
    let id_clone = id.clone();
    let mut server = use_server_future(move || {
        let id = id_clone.clone();
        async move { get_mcp_server(id).await }
    })?;

    let mut new_pkg = use_signal(String::new);

    let id_deps = id.clone();
    let dep_skills = use_server_future(move || {
        let id = id_deps.clone();
        async move { list_dependent_skills(id).await }
    })?;

    match &*server.read() {
        Some(Ok(s)) => {
            let created = s.created_at.format("%Y-%m-%d %H:%M").to_string();
            let sid = s.id.to_string();
            let sid_del = sid.clone();
            let sid_pkg = sid.clone();
            let name = s.name.clone();
            let slug = s.slug.clone();
            let config_str = serde_json::to_string_pretty(&s.config_json).unwrap_or_default();
            let packages = s.nix_packages.clone();
            let hide_flag = s.hide_from_public_catalog;

            rsx! {
                div { class: "flex items-center gap-3 mb-1",
                    h2 { class: "text-2xl font-bold", "{name}" }
                    span { class: "text-gray-400 dark:text-gray-500 font-mono text-sm", "({slug})" }
                    HiddenBadge { hidden: hide_flag }
                    Link {
                        to: Route::McpServerEdit { id: sid },
                        class: "text-gray-400 hover:text-gray-600 dark:text-gray-400 dark:hover:text-gray-300",
                        "Edit"
                    }
                    button {
                        class: "text-red-400 dark:text-red-500 hover:text-red-600 dark:hover:text-red-400",
                        onclick: move |_| {
                            let id = sid_del.clone();
                            let nav = navigator.clone();
                            spawn(async move {
                                if delete_mcp_server(id).await.is_ok() {
                                    nav.push(Route::McpServerList {});
                                }
                            });
                        },
                        "Delete"
                    }
                }
                if !s.description.is_empty() {
                    p { class: "text-gray-600 dark:text-gray-300 mb-2", "{s.description}" }
                }
                p { class: "text-gray-500 dark:text-gray-400 text-sm mb-6", "Created: {created}" }

                div { class: "mb-6",
                    h3 { class: "text-lg font-semibold mb-3", "Config JSON" }
                    pre { class: "bg-gray-100 dark:bg-gray-700 p-4 rounded text-sm font-mono overflow-x-auto whitespace-pre-wrap",
                        "{config_str}"
                    }
                }

                // Nix packages section
                div {
                    h3 { class: "text-lg font-semibold mb-3", "Nix Dependencies" }
                    form {
                        class: "flex gap-2 mb-4",
                        onsubmit: move |evt: FormEvent| {
                            evt.prevent_default();
                            let id = sid_pkg.clone();
                            let pkg = new_pkg.read().clone();
                            spawn(async move {
                                if !pkg.trim().is_empty() {
                                    if add_nix_package(id, pkg).await.is_ok() {
                                        new_pkg.set(String::new());
                                        server.restart();
                                    }
                                }
                            });
                        },
                        input {
                            class: "flex-1 border border-gray-300 dark:border-gray-600 rounded px-3 py-1 text-sm font-mono dark:bg-gray-700 dark:text-white",
                            r#type: "text",
                            placeholder: "package-name",
                            value: "{new_pkg}",
                            oninput: move |e| new_pkg.set(e.value()),
                        }
                        button {
                            class: "bg-blue-600 text-white px-3 py-1 rounded text-sm hover:bg-blue-700",
                            r#type: "submit",
                            "Add"
                        }
                    }
                    if packages.is_empty() {
                        p { class: "text-sm text-gray-400 dark:text-gray-500", "No nix dependencies." }
                    } else {
                        ul { class: "divide-y divide-gray-200 dark:divide-gray-700",
                            for pkg in &packages {
                                {
                                    let pkg_display = pkg.clone();
                                    let pkg_remove = pkg.clone();
                                    let id_rm = id.clone();
                                    rsx! {
                                        li { class: "py-2 flex justify-between items-center",
                                            span { class: "text-sm font-mono", "{pkg_display}" }
                                            button {
                                                class: "text-xs text-red-600 dark:text-red-400 hover:underline",
                                                onclick: move |_| {
                                                    let id = id_rm.clone();
                                                    let pkg = pkg_remove.clone();
                                                    spawn(async move {
                                                        if remove_nix_package(id, pkg).await.is_ok() {
                                                            server.restart();
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
                }

                // Required by Skills section
                div { class: "mt-6",
                    h3 { class: "text-lg font-semibold mb-3", "Required by Skills" }
                    {match &*dep_skills.read() {
                        Some(Ok(list)) if list.is_empty() => rsx! {
                            p { class: "text-sm text-gray-400 dark:text-gray-500", "No skills depend on this MCP server." }
                        },
                        Some(Ok(list)) => rsx! {
                            ul { class: "divide-y divide-gray-200 dark:divide-gray-700",
                                for dep in list {
                                    {
                                        let skill_slug = dep.skill_slug.clone();
                                        let channel = dep.channel.clone();
                                        let skill_id = dep.skill_id.clone();
                                        rsx! {
                                            li { class: "py-2",
                                                Link {
                                                    to: Route::SkillDetail { id: skill_id },
                                                    class: "text-sm text-blue-600 dark:text-blue-400 hover:underline font-mono",
                                                    "{skill_slug}"
                                                }
                                                span { class: "text-xs text-gray-400 dark:text-gray-500 ml-2", "({channel})" }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        Some(Err(e)) => rsx! { p { class: "text-sm text-red-600 dark:text-red-400", "Error: {e}" } },
                        None => rsx! { p { class: "text-sm", "Loading..." } },
                    }}
                }
            }
        }
        Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400", "Error: {e}" } },
        None => rsx! { p { "Loading..." } },
    }
}

// ── Create form ──────────────────────────────────────────────────────

#[component]
pub fn McpServerForm() -> Element {
    let navigator = navigator();
    let slug = use_signal(String::new);
    let mut name = use_signal(String::new);
    let mut description = use_signal(String::new);
    let config_json = use_signal(|| r#"{"command": "", "args": []}"#.to_string());
    let hide_from_public_catalog = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    rsx! {
        h2 { class: "text-2xl font-bold mb-4", "New MCP Server" }
        if let Some(err) = &*error.read() {
            p { class: "text-red-600 dark:text-red-400 mb-4", "{err}" }
        }
        form {
            onsubmit: move |evt: FormEvent| {
                evt.prevent_default();
                let nav = navigator.clone();
                let s = slug.read().clone();
                let n = name.read().clone();
                let d = description.read().clone();
                let c = config_json.read().clone();
                let h = *hide_from_public_catalog.read();
                spawn(async move {
                    match upsert_mcp_server(None, s, n, d, c, h).await {
                        Ok(server) => { nav.push(Route::McpServerDetail { id: server.id.to_string() }); }
                        Err(e) => error.set(Some(e.to_string())),
                    }
                });
            },
            McpServerFormFields {
                slug,
                name,
                description,
                config_json,
                hide_from_public_catalog,
                slug_readonly: false,
            }
            div { class: "flex gap-3 items-center",
                button {
                    class: "bg-blue-600 text-white px-4 py-2 rounded hover:bg-blue-700",
                    r#type: "submit",
                    "Create"
                }
                GenerateButton {
                    context: GenerateContext::McpServer { slug: slug.read().clone(), config_json: config_json.read().clone() },
                    current_name: name.read().clone(),
                    current_desc: description.read().clone(),
                    on_generated: move |result: GeneratedNameDesc| {
                        name.set(result.name);
                        description.set(result.description);
                    },
                }
            }
        }
    }
}

// ── Edit form ────────────────────────────────────────────────────────

#[component]
pub fn McpServerEdit(id: String) -> Element {
    let id_load = id.clone();
    let existing = use_server_future(move || {
        let id = id_load.clone();
        async move { get_mcp_server(id).await }
    })?;

    let navigator = navigator();
    let mut slug = use_signal(String::new);
    let mut name = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut config_json = use_signal(String::new);
    let mut hide_from_public_catalog = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut loaded = use_signal(|| false);

    // Pre-fill signals once when data arrives
    if let Some(Ok(s)) = &*existing.read() {
        if !*loaded.read() {
            slug.set(s.slug.clone());
            name.set(s.name.clone());
            description.set(s.description.clone());
            config_json.set(serde_json::to_string_pretty(&s.config_json).unwrap_or_default());
            hide_from_public_catalog.set(s.hide_from_public_catalog);
            loaded.set(true);
        }
    }

    match &*existing.read() {
        Some(Ok(_)) => {
            let edit_id = id.clone();
            let nav_id = id.clone();
            rsx! {
                h2 { class: "text-2xl font-bold mb-4", "Edit MCP Server" }
                if let Some(err) = &*error.read() {
                    p { class: "text-red-600 dark:text-red-400 mb-4", "{err}" }
                }
                form {
                    onsubmit: move |evt: FormEvent| {
                        evt.prevent_default();
                        let nav = navigator.clone();
                        let eid = edit_id.clone();
                        let nid = nav_id.clone();
                        let s = slug.read().clone();
                        let n = name.read().clone();
                        let d = description.read().clone();
                        let c = config_json.read().clone();
                        let h = *hide_from_public_catalog.read();
                        spawn(async move {
                            match upsert_mcp_server(Some(eid), s, n, d, c, h).await {
                                Ok(_) => { nav.push(Route::McpServerDetail { id: nid }); }
                                Err(e) => error.set(Some(e.to_string())),
                            }
                        });
                    },
                    McpServerFormFields {
                        slug,
                        name,
                        description,
                        config_json,
                        hide_from_public_catalog,
                        slug_readonly: true,
                    }
                    div { class: "flex gap-3 items-center",
                        button {
                            class: "bg-blue-600 text-white px-4 py-2 rounded hover:bg-blue-700",
                            r#type: "submit",
                            "Save"
                        }
                        Link {
                            to: Route::McpServerDetail { id: id.clone() },
                            class: "px-4 py-2 text-gray-600 dark:text-gray-300 hover:text-gray-900 dark:hover:text-white",
                            "Cancel"
                        }
                        GenerateButton {
                            context: GenerateContext::McpServer { slug: slug.read().clone(), config_json: config_json.read().clone() },
                            current_name: name.read().clone(),
                            current_desc: description.read().clone(),
                            on_generated: move |result: GeneratedNameDesc| {
                                name.set(result.name);
                                description.set(result.description);
                            },
                        }
                    }
                }
            }
        }
        Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400", "Error: {e}" } },
        None => rsx! { p { "Loading..." } },
    }
}
