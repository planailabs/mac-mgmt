use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{
    Badge, BadgeVariant, Button, ButtonVariant, Card, FormField, PageHeader, SectionHeading, Td,
    TdMuted, Th,
};

// ── Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredOption {
    id: Uuid,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CdConfig {
    webspace_name: String,
    credential_id: Option<Uuid>,
    credential_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NotificationRow {
    id: Uuid,
    title: String,
    body_preview: String,
    created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NotificationDetail {
    id: Uuid,
    title: String,
    body: String,
    created_at: String,
    webspace_name: String,
}

// ── Server Functions ──────────────────────────────────────────────────

#[server]
async fn get_cd_config(webspace_id: Uuid) -> Result<CdConfig, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let (name, cred_id) = sqlx::query_as::<_, (String, Option<Uuid>)>(
        "SELECT w.name, w.changedetection_credential_id FROM webspaces w WHERE w.id = $1",
    )
    .bind(webspace_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Verify read access.
    let org_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT organization_id FROM webspaces WHERE id = $1",
    )
    .bind(webspace_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    use crate::web::user::WebUserExt;
    user.require_org_read(&org_id)?;

    let cred_name = if let Some(cid) = cred_id {
        sqlx::query_scalar::<_, String>("SELECT name FROM credentials WHERE id = $1")
            .bind(cid)
            .fetch_optional(&pool)
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    Ok(CdConfig {
        webspace_name: name,
        credential_id: cred_id,
        credential_name: cred_name,
    })
}

#[server]
async fn list_cd_creds() -> Result<Vec<CredOption>, ServerFnError> {
    let _user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let rows = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, name FROM credentials WHERE credential_type = 'changedetection' ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|(id, name)| CredOption { id, name })
        .collect())
}

#[server]
async fn set_webspace_changedetection(
    webspace_id: Uuid,
    credential_id: Option<Uuid>,
) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT organization_id FROM webspaces WHERE id = $1",
    )
    .bind(webspace_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    sqlx::query(
        "UPDATE webspaces SET changedetection_credential_id = $1, updated_at = now() WHERE id = $2",
    )
    .bind(credential_id)
    .bind(webspace_id)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server]
async fn list_notifications(webspace_id: Uuid) -> Result<Vec<NotificationRow>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT organization_id FROM webspaces WHERE id = $1",
    )
    .bind(webspace_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    use crate::web::user::WebUserExt;
    user.require_org_read(&org_id)?;

    let rows = sqlx::query_as::<_, (Uuid, Option<String>, Option<String>, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, title, body, created_at \
         FROM changedetection_notifications \
         WHERE webspace_id = $1 \
         ORDER BY created_at DESC \
         LIMIT 100",
    )
    .bind(webspace_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|(id, title, body, created_at)| {
            let body_preview = body
                .as_deref()
                .unwrap_or("")
                .chars()
                .take(120)
                .collect::<String>();
            NotificationRow {
                id,
                title: title.unwrap_or_default(),
                body_preview,
                created_at: created_at.format("%Y-%m-%d %H:%M").to_string(),
            }
        })
        .collect())
}

#[server]
async fn get_notification(
    webspace_id: Uuid,
    notification_id: Uuid,
) -> Result<NotificationDetail, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT organization_id FROM webspaces WHERE id = $1",
    )
    .bind(webspace_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    use crate::web::user::WebUserExt;
    user.require_org_read(&org_id)?;

    let (title, body, created_at, ws_name) = sqlx::query_as::<_, (Option<String>, Option<String>, chrono::DateTime<chrono::Utc>, String)>(
        "SELECT n.title, n.body, n.created_at, w.name \
         FROM changedetection_notifications n \
         JOIN webspaces w ON w.id = n.webspace_id \
         WHERE n.id = $1 AND n.webspace_id = $2",
    )
    .bind(notification_id)
    .bind(webspace_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(NotificationDetail {
        id: notification_id,
        title: title.unwrap_or_default(),
        body: body.unwrap_or_default(),
        created_at: created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        webspace_name: ws_name,
    })
}

// ── Components ────────────────────────────────────────────────────────

#[component]
pub fn WebspaceChangedetection(id: String) -> Element {
    let wid = match Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => return rsx! { div { class: "text-danger", "Invalid webspace ID" } },
    };

    let config = use_server_future(move || async move { get_cd_config(wid).await })?;
    let cfg = match &*config.read() {
        Some(Ok(c)) => c.clone(),
        Some(Err(e)) => return rsx! { div { class: "text-danger", "Error: {e}" } },
        None => return rsx! { div { class: "text-fg-muted", "Loading..." } },
    };

    rsx! {
        PageHeader {
            Link {
                to: crate::web::app::Route::WebspaceDetail { id: id.clone() },
                class: "text-brand underline",
                "{cfg.webspace_name}"
            }
            " / Change Detection"
        }

        SectionHeading { "Credential" }
        CredentialSection { webspace_id: wid, config: config, current_credential_id: cfg.credential_id, current_credential_name: cfg.credential_name.clone() }

        SectionHeading { class: "mt-6", "Notifications" }
        NotificationsInbox { webspace_id: wid, ws_id_str: id.clone() }
    }
}

#[component]
fn CredentialSection(
    webspace_id: Uuid,
    config: Resource<Result<CdConfig, ServerFnError>>,
    current_credential_id: Option<Uuid>,
    current_credential_name: Option<String>,
) -> Element {
    let cd_creds = use_server_future(list_cd_creds)?;
    let cred_list: Vec<CredOption> = match &*cd_creds.read() {
        Some(Ok(c)) => c.clone(),
        _ => vec![],
    };

    let mut selected_cred = use_signal(move || {
        current_credential_id
            .map(|id| id.to_string())
            .unwrap_or_default()
    });
    let mut saving = use_signal(|| false);
    let mut result_msg = use_signal(|| None::<String>);

    rsx! {
        Card {
            div { class: "p-6 space-y-4",
                div { class: "flex items-center gap-3",
                    span { class: "text-sm text-fg-muted", "Current:" }
                    if let Some(ref name) = current_credential_name {
                        Badge { variant: BadgeVariant::Info, "{name}" }
                    } else {
                        span { class: "text-fg-muted text-sm", "Not configured" }
                    }
                }

                if cred_list.is_empty() {
                    div { class: "text-sm text-fg-muted",
                        "No ChangeDetection.io credentials available. "
                        Link {
                            to: crate::web::app::Route::CredentialForm {},
                            class: "text-brand underline",
                            "Add one"
                        }
                    }
                } else {
                    div { class: "flex items-end gap-3",
                        FormField { label: "Credential",
                            select {
                                class: "input w-64",
                                value: "{selected_cred}",
                                oninput: move |evt| selected_cred.set(evt.value()),
                                option { value: "", "None" }
                                for c in &cred_list {
                                    option { value: "{c.id}", "{c.name}" }
                                }
                            }
                        }
                        Button {
                            variant: ButtonVariant::Primary,
                            disabled: *saving.read(),
                            onclick: {
                                let wid = webspace_id;
                                move |_| {
                                    let cred_str = selected_cred.read().clone();
                                    saving.set(true);
                                    result_msg.set(None);
                                    spawn(async move {
                                        let cid = uuid::Uuid::parse_str(&cred_str).ok();
                                        match set_webspace_changedetection(wid, cid).await {
                                            Ok(()) => {
                                                result_msg.set(Some("Saved".into()));
                                                config.restart();
                                            }
                                            Err(e) => result_msg.set(Some(format!("Error: {e}"))),
                                        }
                                        saving.set(false);
                                    });
                                }
                            },
                            if *saving.read() { "Saving..." } else { "Save" }
                        }
                    }
                }

                if let Some(ref msg) = *result_msg.read() {
                    div { class: "text-sm text-fg-muted", "{msg}" }
                }
            }
        }
    }
}

#[component]
fn NotificationsInbox(webspace_id: Uuid, ws_id_str: String) -> Element {
    let notifications =
        use_server_future(move || async move { list_notifications(webspace_id).await })?;
    let rows = match &*notifications.read() {
        Some(Ok(r)) => r.clone(),
        Some(Err(e)) => return rsx! { div { class: "text-danger", "Error: {e}" } },
        None => return rsx! { div { class: "text-fg-muted", "Loading..." } },
    };

    rsx! {
        Card {
            if rows.is_empty() {
                div { class: "p-6 text-fg-muted text-sm", "No notifications yet." }
            } else {
                table { class: "w-full",
                    thead {
                        tr {
                            Th { "Title" }
                            Th { "Preview" }
                            Th { "Date" }
                        }
                    }
                    tbody {
                        for row in &rows {
                            tr {
                                class: "cursor-pointer hover:bg-bg-hover",
                                onclick: {
                                    let nid = row.id.to_string();
                                    let wsid = ws_id_str.clone();
                                    move |_| {
                                        navigator().push(
                                            crate::web::app::Route::WebspaceChangedetectionNotification {
                                                id: wsid.clone(),
                                                notification_id: nid.clone(),
                                            },
                                        );
                                    }
                                },
                                Td { "{row.title}" }
                                TdMuted { "{row.body_preview}" }
                                TdMuted { "{row.created_at}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Notification Detail ───────────────────────────────────────────────

#[component]
pub fn WebspaceChangedetectionNotification(id: String, notification_id: String) -> Element {
    let ws_id = match Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => return rsx! { div { class: "text-danger", "Invalid webspace ID" } },
    };
    let notif_id = match Uuid::parse_str(&notification_id) {
        Ok(id) => id,
        Err(_) => return rsx! { div { class: "text-danger", "Invalid notification ID" } },
    };

    let detail =
        use_server_future(move || async move { get_notification(ws_id, notif_id).await })?;
    let d = match &*detail.read() {
        Some(Ok(d)) => d.clone(),
        Some(Err(e)) => return rsx! { div { class: "text-danger", "Error: {e}" } },
        None => return rsx! { div { class: "text-fg-muted", "Loading..." } },
    };

    rsx! {
        PageHeader {
            Link {
                to: crate::web::app::Route::WebspaceChangedetection { id: id.clone() },
                class: "text-brand underline",
                "{d.webspace_name} / Change Detection"
            }
            " / Notification"
        }

        Card {
            div { class: "p-6 space-y-4",
                div { class: "flex items-center gap-3",
                    h2 { class: "text-lg font-semibold text-fg", "{d.title}" }
                    Badge { variant: BadgeVariant::Neutral, "{d.created_at}" }
                }
                pre { class: "whitespace-pre-wrap text-sm text-fg font-mono bg-bg-inset p-4 rounded overflow-x-auto",
                    "{d.body}"
                }
            }
        }
    }
}
