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
    struct Row { id: Uuid, name: String }

    let rows = sqlx::query_as::<_, Row>("SELECT id, name FROM rollout_groups ORDER BY name")
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows.into_iter().map(|r| GroupOption { id: r.id, name: r.name }).collect())
}

#[server]
async fn create_rollout(config_toml: String, group_ids: Vec<String>) -> Result<String, ServerFnError> {
    let pool = crate::server_pool()?;

    // Validate config
    mac_mgmt_common::CustomerConfig::from_toml(&config_toml)
        .map_err(|e| ServerFnError::new(format!("invalid config: {e}")))?;

    let group_uuids: Vec<Uuid> = group_ids.iter()
        .map(|s| s.parse::<Uuid>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let rollout_id = Uuid::new_v4();
    let mut tx = pool.begin().await.map_err(|e| ServerFnError::new(e.to_string()))?;

    sqlx::query("INSERT INTO rollouts (id, config_toml) VALUES ($1, $2)")
        .bind(rollout_id)
        .bind(&config_toml)
        .execute(&mut *tx)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    for (i, gid) in group_uuids.iter().enumerate() {
        sqlx::query("INSERT INTO rollout_stages (rollout_id, group_id, stage_order) VALUES ($1, $2, $3)")
            .bind(rollout_id)
            .bind(gid)
            .bind(i as i32)
            .execute(&mut *tx)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    }

    tx.commit().await.map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(rollout_id.to_string())
}

#[component]
pub fn RolloutForm() -> Element {
    let groups = use_server_future(move || async move { get_group_options().await })?;
    let mut config_toml = use_signal(String::new);
    let mut selected_groups = use_signal(Vec::<String>::new);
    let mut error = use_signal(|| Option::<String>::None);
    let nav = navigator();

    match &*groups.read() {
        Some(Ok(group_list)) => {
            let group_list_clone = group_list.clone();
            rsx! {
                h2 { class: "text-2xl font-bold mb-4", "New Rollout" }

                div { class: "space-y-4",
                    div {
                        label { class: "block text-sm font-medium text-gray-700 mb-1", "Config TOML" }
                        textarea {
                            class: "w-full border rounded p-2 font-mono text-sm h-48",
                            value: "{config_toml}",
                            oninput: move |e| config_toml.set(e.value()),
                        }
                    }

                    div {
                        label { class: "block text-sm font-medium text-gray-700 mb-1", "Stages (select groups in order)" }
                        for g in &group_list_clone {
                            {
                                let gid = g.id.to_string();
                                let gname = g.name.clone();
                                let is_selected = selected_groups.read().contains(&gid);
                                let order = selected_groups.read().iter().position(|x| x == &gid);
                                rsx! {
                                    div { class: "flex items-center gap-2 mb-1",
                                        input {
                                            r#type: "checkbox",
                                            checked: is_selected,
                                            onchange: {
                                                let gid = gid.clone();
                                                move |_| {
                                                    let mut groups = selected_groups.write();
                                                    if let Some(pos) = groups.iter().position(|x| x == &gid) {
                                                        groups.remove(pos);
                                                    } else {
                                                        groups.push(gid.clone());
                                                    }
                                                }
                                            },
                                        }
                                        span { "{gname}" }
                                        if let Some(idx) = order {
                                            span { class: "text-xs text-gray-400", "(stage {idx})" }
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
                            let toml = config_toml.read().clone();
                            let groups = selected_groups.read().clone();
                            async move {
                                if toml.trim().is_empty() {
                                    error.set(Some("Config TOML is required".into()));
                                    return;
                                }
                                if groups.is_empty() {
                                    error.set(Some("Select at least one group".into()));
                                    return;
                                }
                                match create_rollout(toml, groups).await {
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
        Some(Err(e)) => rsx! { p { class: "text-red-600", "Error: {e}" } },
        None => rsx! { p { "Loading groups..." } },
    }
}
