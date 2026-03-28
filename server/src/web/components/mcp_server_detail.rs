use dioxus::prelude::*;

use crate::models::McpServer;

#[server]
async fn get_mcp_server(id: String) -> Result<McpServer, ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let server = sqlx::query_as::<_, McpServer>("SELECT * FROM mcp_servers WHERE id = $1")
        .bind(uuid)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(server)
}

#[server]
async fn update_mcp_server(id: String, name: String, description: String, config_json: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let parsed: serde_json::Value = serde_json::from_str(&config_json)
        .map_err(|e| ServerFnError::new(format!("invalid JSON: {e}")))?;
    crate::mcp_schema::validate_mcp_server_config(&parsed)
        .map_err(|e| ServerFnError::new(format!("schema validation failed: {e}")))?;
    sqlx::query("UPDATE mcp_servers SET name = $1, description = $2, config_json = $3 WHERE id = $4")
        .bind(&name)
        .bind(&description)
        .bind(&parsed)
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn create_mcp_server(slug: String, name: String, description: String, config_json: String) -> Result<McpServer, ServerFnError> {
    let pool = crate::server_pool()?;
    let parsed: serde_json::Value = serde_json::from_str(&config_json)
        .map_err(|e| ServerFnError::new(format!("invalid JSON: {e}")))?;
    crate::mcp_schema::validate_mcp_server_config(&parsed)
        .map_err(|e| ServerFnError::new(format!("schema validation failed: {e}")))?;
    let server = sqlx::query_as::<_, McpServer>(
        "INSERT INTO mcp_servers (slug, name, description, config_json) VALUES ($1, $2, $3, $4) RETURNING *",
    )
    .bind(&slug)
    .bind(&name)
    .bind(&description)
    .bind(&parsed)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(server)
}

#[server]
async fn delete_mcp_server(id: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("DELETE FROM mcp_servers WHERE id = $1")
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[component]
pub fn McpServerDetail(id: String) -> Element {
    let id_clone = id.clone();
    let mut server = use_server_future(move || {
        let id = id_clone.clone();
        async move { get_mcp_server(id).await }
    })?;

    let mut editing = use_signal(|| false);
    let mut draft_name = use_signal(String::new);
    let mut draft_desc = use_signal(String::new);
    let mut draft_config = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);

    match &*server.read() {
        Some(Ok(s)) => {
            let created = s.created_at.format("%Y-%m-%d %H:%M").to_string();
            let sid = s.id.to_string();
            let name = s.name.clone();
            let desc = s.description.clone();
            let slug = s.slug.clone();
            let config_str_display = serde_json::to_string_pretty(&s.config_json).unwrap_or_default();
            let config_str_edit = config_str_display.clone();

            rsx! {
                div { class: "flex items-center gap-3 mb-1",
                    if *editing.read() {
                        form {
                            class: "space-y-2 w-full",
                            onsubmit: move |_| {
                                let id = sid.clone();
                                let new_name = draft_name.read().clone();
                                let new_desc = draft_desc.read().clone();
                                let new_config = draft_config.read().clone();
                                async move {
                                    if !new_name.trim().is_empty() {
                                        match update_mcp_server(id, new_name, new_desc, new_config).await {
                                            Ok(()) => {
                                                error.set(None);
                                                server.restart();
                                            }
                                            Err(e) => {
                                                error.set(Some(e.to_string()));
                                                return;
                                            }
                                        }
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
                            label { class: "block text-sm font-medium text-gray-700", "Config JSON" }
                            textarea {
                                class: "w-full border border-gray-300 rounded px-2 py-1 font-mono text-sm",
                                rows: "10",
                                value: "{draft_config}",
                                oninput: move |e| draft_config.set(e.value()),
                            }
                            if let Some(err) = &*error.read() {
                                p { class: "text-red-600 text-sm", "{err}" }
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
                                draft_config.set(config_str_edit.clone());
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

                if !*editing.read() {
                    div {
                        h3 { class: "text-lg font-semibold mb-3", "Config JSON" }
                        pre { class: "bg-gray-100 p-4 rounded text-sm font-mono overflow-x-auto whitespace-pre-wrap",
                            "{config_str_display}"
                        }
                    }
                }
            }
        }
        Some(Err(e)) => rsx! { p { class: "text-red-600", "Error: {e}" } },
        None => rsx! { p { "Loading..." } },
    }
}

#[component]
pub fn McpServerForm() -> Element {
    let navigator = navigator();
    let mut slug = use_signal(String::new);
    let mut name = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut config_json = use_signal(|| r#"{"command": "", "args": []}"#.to_string());
    let mut error = use_signal(|| None::<String>);

    let on_submit = move |evt: FormEvent| {
        evt.prevent_default();
        let nav = navigator.clone();
        let slug_val = slug.read().clone();
        let name_val = name.read().clone();
        let desc_val = description.read().clone();
        let config_val = config_json.read().clone();
        spawn(async move {
            match create_mcp_server(slug_val, name_val, desc_val, config_val).await {
                Ok(server) => {
                    nav.push(crate::web::app::Route::McpServerDetail { id: server.id.to_string() });
                }
                Err(e) => {
                    error.set(Some(e.to_string()));
                }
            }
        });
    };

    rsx! {
        h2 { class: "text-2xl font-bold mb-4", "New MCP Server" }
        if let Some(err) = &*error.read() {
            p { class: "text-red-600 mb-4", "{err}" }
        }
        form { onsubmit: on_submit,
            div { class: "mb-4",
                label { class: "block text-sm font-medium text-gray-700 mb-1", "Slug" }
                input {
                    class: "w-full border border-gray-300 rounded px-3 py-2 font-mono",
                    r#type: "text",
                    required: true,
                    placeholder: "my-server",
                    value: "{slug}",
                    oninput: move |evt| slug.set(evt.value()),
                }
            }
            div { class: "mb-4",
                label { class: "block text-sm font-medium text-gray-700 mb-1", "Name" }
                input {
                    class: "w-full border border-gray-300 rounded px-3 py-2",
                    r#type: "text",
                    required: true,
                    value: "{name}",
                    oninput: move |evt| name.set(evt.value()),
                }
            }
            div { class: "mb-4",
                label { class: "block text-sm font-medium text-gray-700 mb-1", "Description" }
                textarea {
                    class: "w-full border border-gray-300 rounded px-3 py-2",
                    rows: "2",
                    value: "{description}",
                    oninput: move |evt| description.set(evt.value()),
                }
            }
            div { class: "mb-4",
                label { class: "block text-sm font-medium text-gray-700 mb-1", "Config JSON" }
                textarea {
                    class: "w-full border border-gray-300 rounded px-3 py-2 font-mono text-sm",
                    rows: "8",
                    required: true,
                    value: "{config_json}",
                    oninput: move |evt| config_json.set(evt.value()),
                }
            }
            button {
                class: "bg-blue-600 text-white px-4 py-2 rounded hover:bg-blue-700",
                r#type: "submit",
                "Create"
            }
        }
    }
}
