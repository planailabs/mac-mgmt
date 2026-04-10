use dioxus::prelude::*;

use crate::models::Customer;
use crate::web::app::Route;

use super::config_editor::ConfigEditor;
use super::config_history::ConfigHistory;
use super::customer_mcp_servers::CustomerMcpServers;
use super::customer_skills::CustomerSkills;
use super::customer_ssh_keys::CustomerSshKeys;
use super::setting_token_list::SettingTokenList;
use super::token_list::SyncTokenList;

#[server]
async fn get_customer(id: String) -> Result<Customer, ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let customer = sqlx::query_as::<_, Customer>("SELECT * FROM customers WHERE id = $1")
        .bind(uuid)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(customer)
}

#[server]
async fn get_pinned_rollout(version: String) -> Result<Option<String>, ServerFnError> {
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
async fn rename_customer(id: String, name: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("UPDATE customers SET name = $1 WHERE id = $2")
        .bind(&name)
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn set_pinned_version(id: String, version: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let ver = version.trim().to_string();
    if ver.is_empty() {
        sqlx::query("UPDATE customers SET pinned_version = NULL WHERE id = $1")
            .bind(uuid)
            .execute(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    } else {
        sqlx::query("UPDATE customers SET pinned_version = $1 WHERE id = $2")
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
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let c = commit.trim().to_string();
    if c.is_empty() {
        sqlx::query("UPDATE customers SET nixpkgs_commit = NULL WHERE id = $1")
            .bind(uuid)
            .execute(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    } else {
        let valid = (7..=40).contains(&c.len()) && c.chars().all(|ch| ch.is_ascii_hexdigit());
        if !valid {
            return Err(ServerFnError::new("commit must be 7-40 hex chars"));
        }
        sqlx::query("UPDATE customers SET nixpkgs_commit = $1 WHERE id = $2")
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
async fn delete_customer(id: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("DELETE FROM customers WHERE id = $1")
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
async fn get_active_rollouts(customer_id: String) -> Result<Vec<ActiveRolloutEntry>, ServerFnError> {
    let pool = crate::server_pool()?;
    let cid: uuid::Uuid = customer_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

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
         WHERE rgm.customer_id = $1 AND r.status IN ('rolling', 'paused') \
         ORDER BY r.target_version DESC",
    )
    .bind(cid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows.into_iter().map(|r| ActiveRolloutEntry {
        id: r.id.to_string(),
        target_version: r.target_version,
        status: r.status,
    }).collect())
}

#[component]
pub fn CustomerDetail(id: String) -> Element {
    let id_clone = id.clone();
    let mut customer = use_server_future(move || {
        let id = id_clone.clone();
        async move { get_customer(id).await }
    })?;

    let mut editing = use_signal(|| false);
    let mut draft_name = use_signal(String::new);
    let mut confirm_delete = use_signal(|| false);
    let nav = navigator();

    match &*customer.read() {
        Some(Ok(c)) => {
            let created = c.created_at.format("%Y-%m-%d %H:%M").to_string();
            let pinned = c.pinned_version.clone();
            let nix_commit = c.nixpkgs_commit.clone();
            let cid = c.id.to_string();
            let cid2 = cid.clone();
            let name = c.name.clone();
            rsx! {
                div { class: "flex items-center gap-3 mb-2",
                    if *editing.read() {
                        form {
                            class: "flex items-center gap-2",
                            onsubmit: move |evt: FormEvent| {
                                evt.prevent_default();
                                let id = cid.clone();
                                let new_name = draft_name.read().clone();
                                async move {
                                    if !new_name.trim().is_empty() {
                                        let _ = rename_customer(id, new_name).await;
                                        customer.restart();
                                    }
                                    editing.set(false);
                                }
                            },
                            input {
                                class: "text-2xl font-bold border border-gray-300 dark:border-gray-600 rounded px-2 py-1 dark:bg-gray-700 dark:text-white",
                                r#type: "text",
                                value: "{draft_name}",
                                oninput: move |e| draft_name.set(e.value()),
                                autofocus: true,
                            }
                            button {
                                class: "text-green-600 dark:text-green-400 hover:text-green-800 dark:hover:text-green-300",
                                r#type: "submit",
                                "Save"
                            }
                            button {
                                class: "text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200",
                                r#type: "button",
                                onclick: move |_| editing.set(false),
                                "Cancel"
                            }
                        }
                    } else {
                        h2 { class: "text-2xl font-bold", "{name}" }
                        button {
                            class: "text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300",
                            onclick: move |_| {
                                draft_name.set(name.clone());
                                editing.set(true);
                            },
                            "Edit"
                        }
                        if *confirm_delete.read() {
                            span { class: "text-red-600 dark:text-red-400 text-sm", "Delete this customer?" }
                            button {
                                class: "bg-red-600 text-white px-3 py-1 rounded text-sm hover:bg-red-700",
                                onclick: {
                                    let cid = cid.clone();
                                    move |_| {
                                        let cid = cid.clone();
                                        async move {
                                            let _ = delete_customer(cid).await;
                                            nav.push(Route::CustomerList {});
                                        }
                                    }
                                },
                                "Confirm"
                            }
                            button {
                                class: "text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 text-sm",
                                onclick: move |_| confirm_delete.set(false),
                                "Cancel"
                            }
                        } else {
                            button {
                                class: "text-red-400 dark:text-red-500 hover:text-red-600 dark:hover:text-red-400 text-sm",
                                onclick: move |_| confirm_delete.set(true),
                                "Delete"
                            }
                        }
                    }
                }
                div { class: "text-gray-500 dark:text-gray-400 mb-6 flex items-center gap-4 flex-wrap",
                    span { "Created: {created}" }
                    PinnedVersion { customer_id: cid2.clone(), version: pinned.clone(), on_change: move |_| customer.restart() }
                    NixpkgsCommit { customer_id: cid2.clone(), commit: nix_commit.clone(), on_change: move |_| customer.restart() }
                    ActiveRollouts { customer_id: cid2.clone() }
                }

                div { class: "grid grid-cols-1 lg:grid-cols-2 gap-6",
                    div {
                        h3 { class: "text-lg font-semibold mb-3", "Sync Tokens" }
                        SyncTokenList { customer_id: cid2.clone() }
                    }
                    div {
                        h3 { class: "text-lg font-semibold mb-3", "Setting / Customer Tokens" }
                        SettingTokenList { customer_id: cid2.clone() }
                    }
                    div {
                        h3 { class: "text-lg font-semibold mb-3", "Config" }
                        ConfigEditor { customer_id: cid2.clone() }
                    }
                    div {
                        h3 { class: "text-lg font-semibold mb-3", "Config History" }
                        ConfigHistory { customer_id: cid2.clone() }
                    }
                    div {
                        h3 { class: "text-lg font-semibold mb-3", "Skills" }
                        CustomerSkills { customer_id: cid2.clone() }
                    }
                    div {
                        h3 { class: "text-lg font-semibold mb-3", "MCP Servers" }
                        CustomerMcpServers { customer_id: cid2.clone() }
                    }
                    div {
                        h3 { class: "text-lg font-semibold mb-3", "SSH Keys" }
                        CustomerSshKeys { customer_id: cid2.clone() }
                    }
                }
            }
        }
        Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400", "Error: {e}" } },
        None => rsx! { p { "Loading..." } },
    }
}

#[component]
fn ActiveRollouts(customer_id: String) -> Element {
    let cid = customer_id.clone();
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
                let badge_class = match entry.status.as_str() {
                    "rolling" => "bg-blue-100 dark:bg-blue-900 text-blue-800 dark:text-blue-200",
                    "paused" => "bg-yellow-100 dark:bg-yellow-900 text-yellow-800 dark:text-yellow-200",
                    _ => "bg-gray-100 dark:bg-gray-700 text-gray-800 dark:text-gray-200",
                };
                let rid = entry.id.clone();
                let label = match &entry.target_version {
                    Some(v) => format!("{} v{v}", entry.status),
                    None => format!("{} (nixpkgs)", entry.status),
                };
                rsx! {
                    Link {
                        to: Route::RolloutDetail { id: rid },
                        class: "px-2 py-0.5 rounded text-xs font-medium {badge_class} hover:opacity-80",
                        "{label}"
                    }
                }
            }
        }
    }
}

#[component]
fn PinnedVersion(customer_id: String, version: Option<String>, on_change: EventHandler) -> Element {
    let mut editing = use_signal(|| false);
    let mut draft = use_signal(String::new);

    // Fetch rollout link if we have a version
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

    if *editing.read() {
        let cid = customer_id.clone();
        rsx! {
            form {
                class: "flex items-center gap-1",
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
                span { "Version: " }
                input {
                    class: "border border-gray-300 dark:border-gray-600 rounded px-2 dark:bg-gray-700 dark:text-white py-0.5 text-sm font-mono w-24",
                    r#type: "text",
                    placeholder: "0.1.6",
                    value: "{draft}",
                    oninput: move |e| draft.set(e.value()),
                    autofocus: true,
                }
                button { class: "text-green-600 dark:text-green-400 hover:text-green-800 dark:hover:text-green-300 text-sm", r#type: "submit", "Save" }
                button {
                    class: "text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 text-sm",
                    r#type: "button",
                    onclick: move |_| editing.set(false),
                    "Cancel"
                }
            }
        }
    } else if has_version {
        let ver_display = version.clone().unwrap_or_default();
        rsx! {
            span { class: "flex items-center gap-1",
                span { "Version: " }
                span { class: "font-mono font-medium text-gray-700 dark:text-gray-200", "v{ver_display}" }
                if let Some(rid) = rollout_id {
                    Link {
                        to: Route::RolloutDetail { id: rid },
                        class: "text-blue-600 dark:text-blue-400 hover:underline text-sm",
                        "(rollout)"
                    }
                }
                button {
                    class: "text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 text-sm",
                    onclick: move |_| {
                        draft.set(ver_display.clone());
                        editing.set(true);
                    },
                    "Edit"
                }
            }
        }
    } else {
        rsx! {
            span { class: "flex items-center gap-1",
                span { class: "text-gray-400 dark:text-gray-500", "No version pinned" }
                button {
                    class: "text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 text-sm",
                    onclick: move |_| {
                        draft.set(String::new());
                        editing.set(true);
                    },
                    "Set"
                }
            }
        }
    }
}

#[component]
fn NixpkgsCommit(customer_id: String, commit: Option<String>, on_change: EventHandler) -> Element {
    let mut editing = use_signal(|| false);
    let mut draft = use_signal(String::new);

    if *editing.read() {
        let cid = customer_id.clone();
        rsx! {
            form {
                class: "flex items-center gap-1",
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
                span { "Nixpkgs: " }
                input {
                    class: "border border-gray-300 dark:border-gray-600 rounded px-2 dark:bg-gray-700 dark:text-white py-0.5 text-sm font-mono w-64",
                    r#type: "text",
                    placeholder: "commit sha",
                    value: "{draft}",
                    oninput: move |e| draft.set(e.value()),
                    autofocus: true,
                }
                button { class: "text-green-600 dark:text-green-400 hover:text-green-800 dark:hover:text-green-300 text-sm", r#type: "submit", "Save" }
                button {
                    class: "text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 text-sm",
                    r#type: "button",
                    onclick: move |_| editing.set(false),
                    "Cancel"
                }
            }
        }
    } else if let Some(c) = commit {
        let display = c.clone();
        let short: String = display.chars().take(12).collect();
        rsx! {
            span { class: "flex items-center gap-1",
                span { "Nixpkgs: " }
                span { class: "font-mono font-medium text-gray-700 dark:text-gray-200", title: "{display}", "{short}" }
                button {
                    class: "text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 text-sm",
                    onclick: move |_| {
                        draft.set(display.clone());
                        editing.set(true);
                    },
                    "Edit"
                }
            }
        }
    } else {
        rsx! {
            span { class: "flex items-center gap-1",
                span { class: "text-gray-400 dark:text-gray-500", "No nixpkgs pin" }
                button {
                    class: "text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 text-sm",
                    onclick: move |_| {
                        draft.set(String::new());
                        editing.set(true);
                    },
                    "Set"
                }
            }
        }
    }
}
