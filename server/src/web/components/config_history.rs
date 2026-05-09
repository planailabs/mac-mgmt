use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::web::components::ui::{Button, ButtonSize, ErrorText, HelpText};
#[cfg(feature = "server")]
use crate::web::user::{current_user, WebUserExt};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConfigVersion {
    id: Uuid,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffLine {
    pub tag: String,
    pub content: String,
}

#[server]
async fn get_config_history(cluster_id: String) -> Result<Vec<ConfigVersion>, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if let Some(ids) = user
        .accessible_cluster_ids(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    {
        if !ids.contains(&uuid) {
            return Err(ServerFnError::new("access denied"));
        }
    }

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        created_at: DateTime<Utc>,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, created_at FROM cluster_configs WHERE cluster_id = $1 ORDER BY created_at DESC LIMIT 50",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| ConfigVersion {
            id: r.id,
            created_at: r.created_at,
        })
        .collect())
}

#[server]
async fn get_config_diff(
    left_id: String,
    right_id: String,
) -> Result<Vec<DiffLine>, ServerFnError> {
    use similar::{ChangeTag, TextDiff};

    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let left_uuid: Uuid = left_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let right_uuid: Uuid = right_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if let Some(ids) = user
        .accessible_cluster_ids(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    {
        let cluster_id: Uuid =
            sqlx::query_scalar("SELECT cluster_id FROM cluster_configs WHERE id = $1")
                .bind(left_uuid)
                .fetch_one(&pool)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
        if !ids.contains(&cluster_id) {
            return Err(ServerFnError::new("access denied"));
        }
    }

    let left_json: serde_json::Value =
        sqlx::query_scalar("SELECT config_json FROM cluster_configs WHERE id = $1")
            .bind(left_uuid)
            .fetch_one(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    let left_text = serde_json::to_string_pretty(&left_json).unwrap_or_default();

    let right_json: serde_json::Value =
        sqlx::query_scalar("SELECT config_json FROM cluster_configs WHERE id = $1")
            .bind(right_uuid)
            .fetch_one(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    let right_text = serde_json::to_string_pretty(&right_json).unwrap_or_default();

    let diff = TextDiff::from_lines(&left_text, &right_text);
    let lines: Vec<DiffLine> = diff
        .iter_all_changes()
        .map(|change| {
            let tag = match change.tag() {
                ChangeTag::Equal => "equal",
                ChangeTag::Insert => "insert",
                ChangeTag::Delete => "delete",
            };
            DiffLine {
                tag: tag.to_string(),
                content: change.value().to_string(),
            }
        })
        .collect();

    Ok(lines)
}

#[component]
pub fn ConfigHistory(cluster_id: String) -> Element {
    let cid = cluster_id.clone();
    let history = use_server_future(move || {
        let id = cid.clone();
        async move { get_config_history(id).await }
    })?;

    let mut left_id = use_signal(|| Option::<String>::None);
    let mut right_id = use_signal(|| Option::<String>::None);
    let mut diff_lines = use_signal(|| Option::<Vec<DiffLine>>::None);
    let mut diff_loading = use_signal(|| false);
    let mut diff_error = use_signal(|| Option::<String>::None);

    match &*history.read() {
        Some(Ok(versions)) => {
            if versions.is_empty() {
                return rsx! { HelpText { {t!("config-history-no-history")} } };
            }

            let versions_left = versions.clone();
            let versions_right = versions.clone();

            rsx! {
                div { class: "mt-4",
                    div { class: "flex gap-4 mb-3 items-end",
                        div {
                            label { class: "label", {t!("config-history-left")} }
                            select { class: "input input-sm",
                                onchange: move |e| {
                                    let val = e.value();
                                    if val.is_empty() {
                                        left_id.set(None);
                                    } else {
                                        left_id.set(Some(val));
                                    }
                                },
                                option { value: "", {t!("config-history-select")} }
                                for v in &versions_left {
                                    {
                                        let vid = v.id.to_string();
                                        let label = v.created_at.format("%Y-%m-%d %H:%M:%S").to_string();
                                        rsx! { option { value: "{vid}", "{label}" } }
                                    }
                                }
                            }
                        }
                        div {
                            label { class: "label", {t!("config-history-right")} }
                            select { class: "input input-sm",
                                onchange: move |e| {
                                    let val = e.value();
                                    if val.is_empty() {
                                        right_id.set(None);
                                    } else {
                                        right_id.set(Some(val));
                                    }
                                },
                                option { value: "", {t!("config-history-select")} }
                                for v in &versions_right {
                                    {
                                        let vid = v.id.to_string();
                                        let label = v.created_at.format("%Y-%m-%d %H:%M:%S").to_string();
                                        rsx! { option { value: "{vid}", "{label}" } }
                                    }
                                }
                            }
                        }
                        Button { size: ButtonSize::Sm,
                            disabled: left_id.read().is_none()
                                || right_id.read().is_none()
                                || *diff_loading.read(),
                            onclick: move |_| {
                                let l = left_id.read().clone();
                                let r = right_id.read().clone();
                                async move {
                                    if let (Some(l), Some(r)) = (l, r) {
                                        diff_loading.set(true);
                                        diff_error.set(None);
                                        match get_config_diff(l, r).await {
                                            Ok(lines) => { diff_lines.set(Some(lines)); }
                                            Err(e) => { diff_error.set(Some(e.to_string())); }
                                        }
                                        diff_loading.set(false);
                                    }
                                }
                            },
                            if *diff_loading.read() {
                                {t!("loading")}
                            } else {
                                {t!("config-history-compare")}
                            }
                        }
                    }

                    if let Some(error) = &*diff_error.read() {
                        ErrorText { "{error}" }
                    }

                    if let Some(lines) = &*diff_lines.read() {
                        div { class: "border border-line-soft rounded overflow-auto max-h-96",
                            pre { class: "text-xs font-mono p-2",
                                for line in lines {
                                    {
                                        let (cls, prefix) = match line.tag.as_str() {
                                            "insert" => ("bg-success-soft text-success", "+ "),
                                            "delete" => ("bg-danger-soft text-danger", "- "),
                                            _ => ("", "  "),
                                        };
                                        rsx! {
                                            span { class: "block {cls}", "{prefix}{line.content}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
        None => rsx! { HelpText { {t!("loading")} } },
    }
}
