use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::web::app::Route;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GroupOption {
    id: Uuid,
    name: String,
}

#[server]
async fn get_group_options() -> Result<Vec<GroupOption>, ServerFnError> {
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        name: String,
    }

    let rows = sqlx::query_as::<_, Row>("SELECT id, name FROM rollout_groups ORDER BY name")
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| GroupOption {
            id: r.id,
            name: r.name,
        })
        .collect())
}

#[server]
async fn create_rollout(
    target_version: String,
    target_environment: String,
    group_ids: Vec<String>,
) -> Result<String, ServerFnError> {
    let pool = crate::server_pool()?;

    if target_version.trim().is_empty() {
        return Err(ServerFnError::new("target version is required"));
    }

    // Validate semver
    let parts: Vec<&str> = target_version.trim().split('.').collect();
    if parts.len() < 3 || parts.iter().any(|p| p.parse::<u64>().is_err()) {
        return Err(ServerFnError::new(
            "version must be semver (e.g., 0.1.6)",
        ));
    }

    // Downgrade check
    let max_version: Option<String> = {
        let gids: Vec<Uuid> = group_ids.iter()
            .filter_map(|s| s.parse::<Uuid>().ok())
            .collect();
        sqlx::query_scalar(
            "SELECT MAX(c.pinned_version) FROM customers c \
             JOIN rollout_group_members rgm ON rgm.customer_id = c.id \
             WHERE rgm.group_id = ANY($1) AND c.pinned_version IS NOT NULL",
        )
        .bind(&gids)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    };
    if let Some(ref max_ver) = max_version {
        if max_ver > &target_version {
            return Err(ServerFnError::new(format!(
                "would downgrade from {max_ver} to {target_version}"
            )));
        }
    }

    let group_uuids: Vec<Uuid> = group_ids
        .iter()
        .map(|s| s.parse::<Uuid>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let rollout_id = Uuid::new_v4();
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    sqlx::query("INSERT INTO rollouts (id, target_version, target_environment) VALUES ($1, $2, $3)")
        .bind(rollout_id)
        .bind(&target_version)
        .bind(&target_environment)
        .execute(&mut *tx)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    for (i, gid) in group_uuids.iter().enumerate() {
        sqlx::query(
            "INSERT INTO rollout_stages (rollout_id, group_id, stage_order) VALUES ($1, $2, $3)",
        )
        .bind(rollout_id)
        .bind(gid)
        .bind(i as i32)
        .execute(&mut *tx)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    }

    tx.commit()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(rollout_id.to_string())
}

#[component]
pub fn RolloutForm() -> Element {
    let groups = use_server_future(move || async move { get_group_options().await })?;
    let mut target_version = use_signal(String::new);
    let mut target_env = use_signal(|| "stable".to_string());
    let mut selected_groups = use_signal(Vec::<String>::new);
    let mut error = use_signal(|| Option::<String>::None);
    let nav = navigator();

    match &*groups.read() {
        Some(Ok(group_list)) => {
            let group_list_clone = group_list.clone();
            rsx! {
                div { class: "flex justify-between items-center mb-4",
                    h2 { class: "text-2xl font-bold", "New Version Rollout" }
                    Link {
                        to: Route::RolloutGroupList {},
                        class: "text-blue-600 hover:underline text-sm",
                        "Manage Groups"
                    }
                }

                div { class: "space-y-4",
                    div {
                        label { class: "block text-sm font-medium text-gray-700 mb-1",
                            "Target Version"
                        }
                        input {
                            class: "w-full border rounded px-3 py-2 text-sm font-mono",
                            placeholder: "e.g. 0.1.6",
                            value: "{target_version}",
                            oninput: move |e| target_version.set(e.value()),
                        }
                        p { class: "text-xs text-gray-400 mt-1",
                            "Semver version to roll out. Downgrades are blocked."
                        }
                    }
                    div {
                        label { class: "block text-sm font-medium text-gray-700 mb-1",
                            "Environment"
                        }
                        input {
                            class: "w-full border rounded px-3 py-2 text-sm font-mono",
                            placeholder: "e.g. stable, beta, canary",
                            value: "{target_env}",
                            oninput: move |e| target_env.set(e.value()),
                        }
                        p { class: "text-xs text-gray-400 mt-1",
                            "Update channel. Maps to {{UPDATE_BASE}}/{{environment}}/mac-mgmt.tar.gz"
                        }
                    }

                    div {
                        label { class: "block text-sm font-medium text-gray-700 mb-1",
                            "Stages (select groups in order)"
                        }
                        if group_list_clone.is_empty() {
                            p { class: "text-gray-500 text-sm",
                                "No groups yet. "
                                Link {
                                    to: Route::RolloutGroupList {},
                                    class: "text-blue-600 hover:underline",
                                    "Create one first."
                                }
                            }
                        } else {
                            for g in &group_list_clone {
                                {
                                    let gid = g.id.to_string();
                                    let gname = g.name.clone();
                                    let is_selected = selected_groups.read().contains(&gid);
                                    let order = selected_groups
                                        .read()
                                        .iter()
                                        .position(|x| x == &gid);
                                    rsx! {
                                        div { class: "flex items-center gap-2 mb-1",
                                            input {
                                                r#type: "checkbox",
                                                checked: is_selected,
                                                onchange: {
                                                    let gid = gid.clone();
                                                    move |_| {
                                                        let mut groups =
                                                            selected_groups.write();
                                                        if let Some(pos) =
                                                            groups.iter().position(|x| x == &gid)
                                                        {
                                                            groups.remove(pos);
                                                        } else {
                                                            groups.push(gid.clone());
                                                        }
                                                    }
                                                },
                                            }
                                            span { "{gname}" }
                                            if let Some(idx) = order {
                                                span { class: "text-xs text-gray-400",
                                                    "(stage {idx})"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if let Some(err) = &*error.read() {
                        p { class: "text-red-600 text-sm", "{err}" }
                    }

                    button {
                        class: "bg-blue-500 text-white px-4 py-2 rounded hover:bg-blue-600",
                        onclick: move |_| {
                            let ver = target_version.read().clone();
                            let env = target_env.read().clone();
                            let groups = selected_groups.read().clone();
                            async move {
                                if ver.trim().is_empty() {
                                    error.set(Some("Target version is required".into()));
                                    return;
                                }
                                if groups.is_empty() {
                                    error.set(Some("Select at least one group".into()));
                                    return;
                                }
                                match create_rollout(ver, env, groups).await {
                                    Ok(id) => {
                                        nav.push(Route::RolloutDetail { id });
                                    }
                                    Err(e) => error.set(Some(e.to_string())),
                                }
                            }
                        },
                        "Create Rollout"
                    }
                }
            }
        }
        Some(Err(e)) => rsx! {
            p { class: "text-red-600", "Error: {e}" }
        },
        None => rsx! {
            p { "Loading groups..." }
        },
    }
}
