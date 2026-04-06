use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::web::app::Route;

const ALL_CUSTOMERS_SENTINEL: &str = "__all__";

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

/// `stage_ids` is an ordered list of group UUIDs or `"__all__"` sentinel.
/// `"__all__"` creates an ad-hoc group containing every customer.
#[server]
async fn create_rollout(
    target_version: String,
    stage_ids: Vec<String>,
) -> Result<String, ServerFnError> {
    let pool = crate::server_pool()?;

    if target_version.trim().is_empty() {
        return Err(ServerFnError::new("target version is required"));
    }

    let parts: Vec<&str> = target_version.trim().split('.').collect();
    if parts.len() < 3 || parts.iter().any(|p| p.parse::<u64>().is_err()) {
        return Err(ServerFnError::new(
            "version must be semver (e.g., 0.1.6)",
        ));
    }

    if stage_ids.is_empty() {
        return Err(ServerFnError::new("select at least one stage"));
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Resolve each stage_id to a real group UUID
    let mut resolved: Vec<Uuid> = Vec::with_capacity(stage_ids.len());
    for sid in &stage_ids {
        if sid == "__all__" {
            let group_id = Uuid::new_v4();
            let group_name = format!("all (v{})", target_version.trim());
            sqlx::query(
                "INSERT INTO rollout_groups (id, name, description) VALUES ($1, $2, 'Auto-created for rollout') \
                 ON CONFLICT (name) DO NOTHING",
            )
            .bind(group_id)
            .bind(&group_name)
            .execute(&mut *tx)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
            let actual_id: Uuid =
                sqlx::query_scalar("SELECT id FROM rollout_groups WHERE name = $1")
                    .bind(&group_name)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|e| ServerFnError::new(e.to_string()))?;
            sqlx::query(
                "INSERT INTO rollout_group_members (group_id, customer_id) \
                 SELECT $1, id FROM customers \
                 WHERE id NOT IN (SELECT customer_id FROM rollout_group_members WHERE group_id = $1) \
                 ON CONFLICT DO NOTHING",
            )
            .bind(actual_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
            resolved.push(actual_id);
        } else {
            let gid: Uuid = sid
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            resolved.push(gid);
        }
    }

    // Downgrade check: list customers that would be downgraded, with their groups
    #[derive(sqlx::FromRow)]
    struct DowngradeRow {
        customer_name: String,
        pinned_version: String,
        group_name: String,
    }

    let downgrades = sqlx::query_as::<_, DowngradeRow>(
        "SELECT DISTINCT c.name AS customer_name, c.pinned_version, rg.name AS group_name \
         FROM customers c \
         JOIN rollout_group_members rgm ON rgm.customer_id = c.id \
         JOIN rollout_groups rg ON rg.id = rgm.group_id \
         WHERE rgm.group_id = ANY($1) \
           AND c.pinned_version IS NOT NULL \
           AND c.pinned_version > $2 \
         ORDER BY c.name, rg.name",
    )
    .bind(&resolved)
    .bind(target_version.trim())
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    if !downgrades.is_empty() {
        // Group by customer
        let mut by_customer: std::collections::BTreeMap<String, (String, Vec<String>)> =
            std::collections::BTreeMap::new();
        for d in &downgrades {
            by_customer
                .entry(d.customer_name.clone())
                .or_insert_with(|| (d.pinned_version.clone(), Vec::new()))
                .1
                .push(d.group_name.clone());
        }
        let details: Vec<String> = by_customer
            .into_iter()
            .map(|(name, (ver, groups))| format!("{name} (v{ver}, in: {})", groups.join(", ")))
            .collect();
        return Err(ServerFnError::new(format!(
            "Would downgrade to {}: {}",
            target_version.trim(),
            details.join("; ")
        )));
    }

    let rollout_id = Uuid::new_v4();
    sqlx::query("INSERT INTO rollouts (id, target_version) VALUES ($1, $2)")
        .bind(rollout_id)
        .bind(&target_version)
        .execute(&mut *tx)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    for (i, gid) in resolved.iter().enumerate() {
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
    let mut selected_stages = use_signal(Vec::<String>::new);
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
                            "Stages (select in order)"
                        }

                        // "All Customers" as a selectable stage
                        {
                            let key = ALL_CUSTOMERS_SENTINEL.to_string();
                            let is_selected = selected_stages.read().contains(&key);
                            let order = selected_stages.read().iter().position(|x| x == &key);
                            rsx! {
                                div { class: "flex items-center gap-2 mb-1",
                                    input {
                                        r#type: "checkbox",
                                        checked: is_selected,
                                        onchange: {
                                            let key = key.clone();
                                            move |_| {
                                                let mut stages = selected_stages.write();
                                                if let Some(pos) = stages.iter().position(|x| x == &key) {
                                                    stages.remove(pos);
                                                } else {
                                                    stages.push(key.clone());
                                                }
                                            }
                                        },
                                    }
                                    span { class: "font-semibold", "All Customers" }
                                    if let Some(idx) = order {
                                        span { class: "text-xs text-gray-400",
                                            "(stage {idx})"
                                        }
                                    }
                                }
                            }
                        }

                        // Regular groups
                        for g in &group_list_clone {
                            {
                                let gid = g.id.to_string();
                                let gname = g.name.clone();
                                let is_selected = selected_stages.read().contains(&gid);
                                let order = selected_stages
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
                                                    let mut stages =
                                                        selected_stages.write();
                                                    if let Some(pos) =
                                                        stages.iter().position(|x| x == &gid)
                                                    {
                                                        stages.remove(pos);
                                                    } else {
                                                        stages.push(gid.clone());
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

                        if group_list_clone.is_empty() {
                            p { class: "text-gray-500 text-xs mt-1",
                                Link {
                                    to: Route::RolloutGroupList {},
                                    class: "text-blue-600 hover:underline",
                                    "Create groups"
                                }
                                " to roll out in stages."
                            }
                        }
                    }

                    if let Some(err) = &*error.read() {
                        p { class: "text-red-600 text-sm", "{err}" }
                    }

                    button {
                        class: "bg-blue-600 text-white px-4 py-2 rounded hover:bg-blue-700",
                        onclick: move |_| {
                            let ver = target_version.read().clone();
                            let stages = selected_stages.read().clone();
                            async move {
                                if ver.trim().is_empty() {
                                    error.set(Some("Target version is required".into()));
                                    return;
                                }
                                if stages.is_empty() {
                                    error.set(Some("Select at least one stage".into()));
                                    return;
                                }
                                match create_rollout(ver, stages).await {
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
            p { class: "text-red-600 text-sm", "Error: {e}" }
        },
        None => rsx! {
            p { class: "text-gray-500 text-sm", "Loading..." }
        },
    }
}
