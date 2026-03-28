use dioxus::prelude::*;

use super::mcp_bundle_detail::McpServerOption;

/// Direct MCP server assignment display.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct CustomerMcpServerDisplay {
    pub customer_mcp_server_id: uuid::Uuid,
    pub server_slug: String,
    pub server_name: String,
}

/// MCP bundle assignment display.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct CustomerMcpBundleDisplay {
    pub customer_mcp_bundle_id: uuid::Uuid,
    pub bundle_slug: String,
    pub bundle_name: String,
}

/// MCP bundle option for dropdown.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct McpBundleOption {
    pub id: uuid::Uuid,
    pub slug: String,
    pub name: String,
}

#[server]
async fn list_customer_mcp_servers(customer_id: String) -> Result<Vec<CustomerMcpServerDisplay>, ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = customer_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let servers = sqlx::query_as::<_, CustomerMcpServerDisplay>(
        "SELECT cms.id as customer_mcp_server_id, ms.slug as server_slug, ms.name as server_name \
         FROM customer_mcp_servers cms \
         JOIN mcp_servers ms ON ms.id = cms.mcp_server_id \
         WHERE cms.customer_id = $1 \
         ORDER BY ms.slug",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(servers)
}

#[server]
async fn list_customer_mcp_bundles(customer_id: String) -> Result<Vec<CustomerMcpBundleDisplay>, ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = customer_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let bundles = sqlx::query_as::<_, CustomerMcpBundleDisplay>(
        "SELECT cmb.id as customer_mcp_bundle_id, msb.slug as bundle_slug, msb.name as bundle_name \
         FROM customer_mcp_bundles cmb \
         JOIN mcp_server_bundles msb ON msb.id = cmb.bundle_id \
         WHERE cmb.customer_id = $1 \
         ORDER BY msb.slug",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(bundles)
}

#[server]
async fn list_all_mcp_servers() -> Result<Vec<McpServerOption>, ServerFnError> {
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
async fn list_all_mcp_bundles() -> Result<Vec<McpBundleOption>, ServerFnError> {
    let pool = crate::server_pool()?;
    let bundles = sqlx::query_as::<_, McpBundleOption>(
        "SELECT id, slug, name FROM mcp_server_bundles ORDER BY slug",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(bundles)
}

#[server]
async fn add_customer_mcp_server(customer_id: String, mcp_server_id: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let cid: uuid::Uuid = customer_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let msid: uuid::Uuid = mcp_server_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("INSERT INTO customer_mcp_servers (customer_id, mcp_server_id) VALUES ($1, $2)")
        .bind(cid)
        .bind(msid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn remove_customer_mcp_server(customer_mcp_server_id: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = customer_mcp_server_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("DELETE FROM customer_mcp_servers WHERE id = $1")
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn add_customer_mcp_bundle(customer_id: String, bundle_id: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let cid: uuid::Uuid = customer_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let bid: uuid::Uuid = bundle_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    // Check for overlap: does the new bundle share any mcp_server with
    // any bundle already assigned to this customer?
    let overlap = sqlx::query_scalar::<_, String>(
        "SELECT ms.slug \
         FROM mcp_server_bundle_items new_bi \
         JOIN mcp_server_bundle_items existing_bi ON existing_bi.mcp_server_id = new_bi.mcp_server_id \
         JOIN customer_mcp_bundles cmb ON cmb.bundle_id = existing_bi.bundle_id AND cmb.customer_id = $1 \
         JOIN mcp_servers ms ON ms.id = new_bi.mcp_server_id \
         WHERE new_bi.bundle_id = $2 \
         LIMIT 1",
    )
    .bind(cid)
    .bind(bid)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    if let Some(conflicting) = overlap {
        return Err(ServerFnError::new(format!(
            "bundle conflicts with an already-assigned bundle on MCP server: {conflicting}"
        )));
    }

    sqlx::query("INSERT INTO customer_mcp_bundles (customer_id, bundle_id) VALUES ($1, $2)")
        .bind(cid)
        .bind(bid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn remove_customer_mcp_bundle(customer_mcp_bundle_id: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = customer_mcp_bundle_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("DELETE FROM customer_mcp_bundles WHERE id = $1")
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[component]
pub fn CustomerMcpServers(customer_id: String) -> Element {
    let cid_servers = customer_id.clone();
    let mut servers = use_server_future(move || {
        let cid = cid_servers.clone();
        async move { list_customer_mcp_servers(cid).await }
    })?;

    let cid_bundles = customer_id.clone();
    let mut bundles = use_server_future(move || {
        let cid = cid_bundles.clone();
        async move { list_customer_mcp_bundles(cid).await }
    })?;

    let available_servers = use_server_future(list_all_mcp_servers)?;
    let available_bundles = use_server_future(list_all_mcp_bundles)?;

    let mut selected_server = use_signal(String::new);
    let mut selected_bundle = use_signal(String::new);
    let mut bundle_error = use_signal(|| None::<String>);

    let cid_add_server = customer_id.clone();
    let cid_add_bundle = customer_id.clone();

    rsx! {
        // Direct MCP server assignments
        div { class: "mb-4",
            h4 { class: "text-sm font-semibold text-gray-700 mb-2", "Direct MCP Servers" }
            form {
                class: "flex gap-2 mb-3",
                onsubmit: move |evt: FormEvent| {
                    evt.prevent_default();
                    let cid = cid_add_server.clone();
                    let msid = selected_server.read().clone();
                    spawn(async move {
                        if !msid.is_empty() {
                            if add_customer_mcp_server(cid, msid).await.is_ok() {
                                selected_server.set(String::new());
                                servers.restart();
                            }
                        }
                    });
                },
                select {
                    class: "flex-1 border border-gray-300 rounded px-2 py-1 text-sm",
                    value: "{selected_server}",
                    onchange: move |evt| selected_server.set(evt.value()),
                    option { value: "", "Select MCP server..." }
                    {match &*available_servers.read() {
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
                    class: "bg-blue-600 text-white px-2 py-1 rounded text-xs hover:bg-blue-700",
                    r#type: "submit",
                    "Add"
                }
            }
            {match &*servers.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    p { class: "text-xs text-gray-400", "No direct MCP server assignments." }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-gray-200",
                        for cs in list {
                            {
                                let csid = cs.customer_mcp_server_id.to_string();
                                let label = format!("{} ({})", cs.server_name, cs.server_slug);
                                rsx! {
                                    li { class: "py-1 flex justify-between items-center",
                                        span { class: "text-sm font-mono", "{label}" }
                                        button {
                                            class: "text-xs text-red-600 hover:underline",
                                            onclick: move |_| {
                                                let csid = csid.clone();
                                                spawn(async move {
                                                    if remove_customer_mcp_server(csid).await.is_ok() {
                                                        servers.restart();
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

        // MCP bundle assignments
        div {
            h4 { class: "text-sm font-semibold text-gray-700 mb-2", "MCP Bundles" }
            if let Some(err) = &*bundle_error.read() {
                p { class: "text-red-600 text-xs mb-2", "{err}" }
            }
            form {
                class: "flex gap-2 mb-3",
                onsubmit: move |evt: FormEvent| {
                    evt.prevent_default();
                    let cid = cid_add_bundle.clone();
                    let bid = selected_bundle.read().clone();
                    spawn(async move {
                        if !bid.is_empty() {
                            match add_customer_mcp_bundle(cid, bid).await {
                                Ok(()) => {
                                    bundle_error.set(None);
                                    selected_bundle.set(String::new());
                                    bundles.restart();
                                }
                                Err(e) => {
                                    bundle_error.set(Some(e.to_string()));
                                }
                            }
                        }
                    });
                },
                select {
                    class: "flex-1 border border-gray-300 rounded px-2 py-1 text-sm",
                    value: "{selected_bundle}",
                    onchange: move |evt| selected_bundle.set(evt.value()),
                    option { value: "", "Select MCP bundle..." }
                    {match &*available_bundles.read() {
                        Some(Ok(list)) => rsx! {
                            for b in list {
                                {
                                    let val = b.id.to_string();
                                    let label = format!("{} ({})", b.name, b.slug);
                                    rsx! { option { value: "{val}", "{label}" } }
                                }
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
            {match &*bundles.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    p { class: "text-xs text-gray-400", "No MCP bundle assignments." }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-gray-200",
                        for cb in list {
                            {
                                let cbid = cb.customer_mcp_bundle_id.to_string();
                                let label = format!("{} ({})", cb.bundle_name, cb.bundle_slug);
                                rsx! {
                                    li { class: "py-1 flex justify-between items-center",
                                        span { class: "text-sm", "{label}" }
                                        button {
                                            class: "text-xs text-red-600 hover:underline",
                                            onclick: move |_| {
                                                let cbid = cbid.clone();
                                                spawn(async move {
                                                    if remove_customer_mcp_bundle(cbid).await.is_ok() {
                                                        bundles.restart();
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
