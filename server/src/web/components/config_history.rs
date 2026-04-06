use dioxus::prelude::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
async fn get_config_history(customer_id: String) -> Result<Vec<ConfigVersion>, ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: Uuid = customer_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        created_at: DateTime<Utc>,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, created_at FROM customer_configs WHERE customer_id = $1 ORDER BY created_at DESC LIMIT 50",
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

    let pool = crate::server_pool()?;
    let left_uuid: Uuid = left_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let right_uuid: Uuid = right_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    let left_json: serde_json::Value =
        sqlx::query_scalar("SELECT config_json FROM customer_configs WHERE id = $1")
            .bind(left_uuid)
            .fetch_one(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    let left_text = serde_json::to_string_pretty(&left_json).unwrap_or_default();

    let right_json: serde_json::Value =
        sqlx::query_scalar("SELECT config_json FROM customer_configs WHERE id = $1")
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
pub fn ConfigHistory(customer_id: String) -> Element {
    let cid = customer_id.clone();
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
                return rsx! {
                    p { class: "text-gray-500 text-sm", "No config history yet." }
                };
            }

            let versions_left = versions.clone();
            let versions_right = versions.clone();

            rsx! {
                div { class: "mt-4",
                    div { class: "flex gap-4 mb-3 items-end",
                        div {
                            label { class: "block text-sm font-medium text-gray-700 mb-1",
                                "Left (older)"
                            }
                            select {
                                class: "border border-gray-300 rounded px-2 py-1 text-sm",
                                onchange: move |e| {
                                    let val = e.value();
                                    if val.is_empty() {
                                        left_id.set(None);
                                    } else {
                                        left_id.set(Some(val));
                                    }
                                },
                                option { value: "", "Select version..." }
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
                            label { class: "block text-sm font-medium text-gray-700 mb-1",
                                "Right (newer)"
                            }
                            select {
                                class: "border border-gray-300 rounded px-2 py-1 text-sm",
                                onchange: move |e| {
                                    let val = e.value();
                                    if val.is_empty() {
                                        right_id.set(None);
                                    } else {
                                        right_id.set(Some(val));
                                    }
                                },
                                option { value: "", "Select version..." }
                                for v in &versions_right {
                                    {
                                        let vid = v.id.to_string();
                                        let label = v.created_at.format("%Y-%m-%d %H:%M:%S").to_string();
                                        rsx! { option { value: "{vid}", "{label}" } }
                                    }
                                }
                            }
                        }
                        button {
                            class: "bg-blue-600 text-white px-3 py-1 rounded text-sm hover:bg-blue-700 disabled:opacity-50",
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
                                            Ok(lines) => {
                                                diff_lines.set(Some(lines));
                                            }
                                            Err(e) => {
                                                diff_error.set(Some(e.to_string()));
                                            }
                                        }
                                        diff_loading.set(false);
                                    }
                                }
                            },
                            if *diff_loading.read() {
                                "Loading..."
                            } else {
                                "Compare"
                            }
                        }
                    }

                    if let Some(error) = &*diff_error.read() {
                        p { class: "text-red-600 text-sm", "{error}" }
                    }

                    if let Some(lines) = &*diff_lines.read() {
                        div { class: "border rounded overflow-auto max-h-96",
                            pre { class: "text-xs font-mono p-2",
                                for line in lines {
                                    {
                                        let (bg, prefix) = match line.tag.as_str() {
                                            "insert" => ("bg-green-50 text-green-800", "+ "),
                                            "delete" => ("bg-red-50 text-red-800", "- "),
                                            _ => ("", "  "),
                                        };
                                        rsx! {
                                            span { class: "block {bg}", "{prefix}{line.content}" }
                                        }
                                    }
                                }
                            }
                        }
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
