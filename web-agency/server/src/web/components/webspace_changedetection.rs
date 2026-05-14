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
    path: String,
    created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NotificationDetail {
    id: Uuid,
    title: String,
    body: String,
    created_at: String,
    webspace_name: String,
    path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SubUrlRow {
    id: Uuid,
    path: String,
    created_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct SubUrlDetail {
    id: Uuid,
    path: String,
    tag_settings: TagSettings,
}

/// Tag settings stored in changedetection_suburls.tag_settings JSONB.
/// Maps 1:1 to ChangeDetection.io tag update fields.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TagSettings {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<TagCondition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conditions_match_logic: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extract_text: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extract_lines_containing: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub text_should_not_be_present: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include_filters: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subtractive_selectors: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignore_text: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trigger_text: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trim_text_whitespace: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort_text_alphabetically: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remove_duplicate_lines: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_unique_lines: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notification_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notification_body: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notification_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notification_muted: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notification_screenshot: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TagCondition {
    #[serde(default)]
    pub field: String,
    #[serde(default)]
    pub operator: String,
    #[serde(default)]
    pub value: String,
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

    let (org_id, old_cred_id, ws_name) =
        sqlx::query_as::<_, (Uuid, Option<Uuid>, String)>(
            "SELECT organization_id, changedetection_credential_id, name \
             FROM webspaces WHERE id = $1",
        )
        .bind(webspace_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    // Undeploy old credential: clean up tags, watches, and notifications so
    // nothing from the old credential instance is left behind.
    if let Some(old_cred) = old_cred_id {
        if credential_id != Some(old_cred) {
            if let Ok((client, group_name)) =
                crate::credentials::changedetection_client(&pool, old_cred).await
            {
                // Delete all sub-URL tags from the old CD instance.
                let suburl_tags = sqlx::query_scalar::<_, Uuid>(
                    "SELECT tag_id FROM changedetection_suburls \
                     WHERE webspace_id = $1 AND tag_id IS NOT NULL",
                )
                .bind(webspace_id)
                .fetch_all(&pool)
                .await
                .unwrap_or_default();
                for tag_id in &suburl_tags {
                    let _ = client.delete_tag(tag_id).await;
                }

                // Delete the webspace-level tag.
                let ws_tag_title = format!("{group_name}:{ws_name}");
                if let Ok(tags) = client.list_tags().await {
                    for (uuid_str, tag) in tags.into_inner().iter() {
                        if tag.title.as_deref().map(|t| t.as_str()) == Some(&ws_tag_title) {
                            if let Ok(uuid) = uuid_str.parse::<Uuid>() {
                                let _ = client.delete_tag(&uuid).await;
                            }
                        }
                    }
                }
            }

            // Clear cached tag_ids on sub-URLs and delete notifications (they belong
            // to the old credential).
            sqlx::query(
                "UPDATE changedetection_suburls SET tag_id = NULL WHERE webspace_id = $1",
            )
            .bind(webspace_id)
            .execute(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

            sqlx::query(
                "DELETE FROM changedetection_notifications WHERE webspace_id = $1",
            )
            .bind(webspace_id)
            .execute(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        }
    }

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

// ── Sub-URL CRUD ─────────────────────────────────────────────────────

#[server]
async fn list_suburls(webspace_id: Uuid) -> Result<Vec<SubUrlRow>, ServerFnError> {
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

    let rows = sqlx::query_as::<_, (Uuid, String, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, path, created_at FROM changedetection_suburls \
         WHERE webspace_id = $1 ORDER BY path",
    )
    .bind(webspace_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|(id, path, created_at)| SubUrlRow {
            id,
            path,
            created_at: created_at.format("%Y-%m-%d %H:%M").to_string(),
        })
        .collect())
}

#[server]
async fn create_suburl(webspace_id: Uuid, path: String) -> Result<Uuid, ServerFnError> {
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

    // Normalize path: must start with /, no trailing / (unless root), no double slashes.
    let path = path.trim().to_string();
    let path = if path.is_empty() || path == "/" {
        "/".to_string()
    } else {
        let mut p = path;
        if !p.starts_with('/') {
            p = format!("/{p}");
        }
        while p.len() > 1 && p.ends_with('/') {
            p.pop();
        }
        p = p.replace("//", "/");
        p
    };

    let secret = {
        use rand::Rng;
        let bytes: [u8; 32] = rand::rng().random();
        hex::encode(bytes)
    };

    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO changedetection_suburls (webspace_id, path, secret) \
         VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(webspace_id)
    .bind(&path)
    .bind(&secret)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(id)
}

#[server]
async fn delete_suburl(webspace_id: Uuid, suburl_id: Uuid) -> Result<(), ServerFnError> {
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

    // Delete the tag from changedetection.io if cached.
    let tag_and_cred = sqlx::query_as::<_, (Option<Uuid>, Option<Uuid>)>(
        "SELECT cs.tag_id, w.changedetection_credential_id \
         FROM changedetection_suburls cs \
         JOIN webspaces w ON w.id = cs.webspace_id \
         WHERE cs.id = $1 AND cs.webspace_id = $2",
    )
    .bind(suburl_id)
    .bind(webspace_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    if let Some((Some(tag_id), Some(cred_id))) = tag_and_cred {
        if let Ok((client, _)) =
            crate::credentials::changedetection_client(&pool, cred_id).await
        {
            let _ = client.delete_tag(&tag_id).await;
        }
    }

    // CASCADE deletes associated notifications.
    sqlx::query(
        "DELETE FROM changedetection_suburls WHERE id = $1 AND webspace_id = $2",
    )
    .bind(suburl_id)
    .bind(webspace_id)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server]
async fn get_suburl(webspace_id: Uuid, suburl_id: Uuid) -> Result<SubUrlDetail, ServerFnError> {
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

    let (id, path, settings_json) = sqlx::query_as::<_, (Uuid, String, serde_json::Value)>(
        "SELECT id, path, tag_settings FROM changedetection_suburls \
         WHERE id = $1 AND webspace_id = $2",
    )
    .bind(suburl_id)
    .bind(webspace_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let tag_settings: TagSettings =
        serde_json::from_value(settings_json).unwrap_or_default();

    Ok(SubUrlDetail {
        id,
        path,
        tag_settings,
    })
}

#[server]
async fn update_suburl_settings(
    webspace_id: Uuid,
    suburl_id: Uuid,
    settings: TagSettings,
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

    let settings_json = serde_json::to_value(&settings)
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Update the JSONB column.
    sqlx::query(
        "UPDATE changedetection_suburls SET tag_settings = $1, updated_at = now() \
         WHERE id = $2 AND webspace_id = $3",
    )
    .bind(&settings_json)
    .bind(suburl_id)
    .bind(webspace_id)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Push settings to changedetection.io immediately if the tag is cached.
    let tag_and_cred = sqlx::query_as::<_, (Option<Uuid>, Option<Uuid>)>(
        "SELECT cs.tag_id, w.changedetection_credential_id \
         FROM changedetection_suburls cs \
         JOIN webspaces w ON w.id = cs.webspace_id \
         WHERE cs.id = $1 AND cs.webspace_id = $2",
    )
    .bind(suburl_id)
    .bind(webspace_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    if let Some((Some(tag_id), Some(cred_id))) = tag_and_cred {
        if let Ok((client, _)) =
            crate::credentials::changedetection_client(&pool, cred_id).await
        {
            let update: changedetection_api::types::Tag =
                serde_json::from_value(settings_json).unwrap_or_default();
            if let Err(e) = client.update_tag(&tag_id, &update).await {
                return Err(ServerFnError::new(format!(
                    "Saved locally but failed to push to ChangeDetection.io: {e}"
                )));
            }
        }
    }

    Ok(())
}

// ── Notification Server Functions ────────────────────────────────────

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

    let rows = sqlx::query_as::<_, (Uuid, Option<String>, Option<String>, chrono::DateTime<chrono::Utc>, String)>(
        "SELECT n.id, n.title, n.body, n.created_at, cs.path \
         FROM changedetection_notifications n \
         JOIN changedetection_suburls cs ON cs.id = n.suburl_id \
         WHERE n.webspace_id = $1 \
         ORDER BY n.created_at DESC \
         LIMIT 100",
    )
    .bind(webspace_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|(id, title, body, created_at, path)| {
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
                path,
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

    let (title, body, created_at, ws_name, path) =
        sqlx::query_as::<_, (Option<String>, Option<String>, chrono::DateTime<chrono::Utc>, String, String)>(
            "SELECT n.title, n.body, n.created_at, w.name, cs.path \
             FROM changedetection_notifications n \
             JOIN webspaces w ON w.id = n.webspace_id \
             JOIN changedetection_suburls cs ON cs.id = n.suburl_id \
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
        path,
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

        SectionHeading { class: "mt-6", "Sub-URLs" }
        SubUrlsSection { webspace_id: wid, ws_id_str: id.clone() }

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

// ── Sub-URLs Section ────────────────────────────────────────────────

#[component]
fn SubUrlsSection(webspace_id: Uuid, ws_id_str: String) -> Element {
    let mut suburls =
        use_server_future(move || async move { list_suburls(webspace_id).await })?;
    let rows = match &*suburls.read() {
        Some(Ok(r)) => r.clone(),
        Some(Err(e)) => return rsx! { div { class: "text-danger", "Error: {e}" } },
        None => return rsx! { div { class: "text-fg-muted", "Loading..." } },
    };

    let mut new_path = use_signal(|| String::new());
    let mut creating = use_signal(|| false);
    let mut create_error = use_signal(|| None::<String>);
    let mut deleting_id = use_signal(|| None::<Uuid>);

    rsx! {
        Card {
            div { class: "p-6 space-y-4",
                if rows.is_empty() {
                    div { class: "text-sm text-fg-muted", "No sub-URLs configured. Add one to start monitoring." }
                } else {
                    table { class: "w-full",
                        thead {
                            tr {
                                Th { "Path" }
                                Th { "Created" }
                                Th { "" }
                            }
                        }
                        tbody {
                            for row in &rows {
                                tr {
                                    class: "hover:bg-bg-hover",
                                    Td {
                                        Link {
                                            to: crate::web::app::Route::WebspaceChangedetectionSuburl {
                                                id: ws_id_str.clone(),
                                                suburl_id: row.id.to_string(),
                                            },
                                            class: "text-brand underline",
                                            "{row.path}"
                                        }
                                    }
                                    TdMuted { "{row.created_at}" }
                                    Td {
                                        Button {
                                            variant: ButtonVariant::Danger,
                                            disabled: *deleting_id.read() == Some(row.id),
                                            onclick: {
                                                let sid = row.id;
                                                let wid = webspace_id;
                                                move |_| {
                                                    deleting_id.set(Some(sid));
                                                    spawn(async move {
                                                        match delete_suburl(wid, sid).await {
                                                            Ok(()) => suburls.restart(),
                                                            Err(e) => {
                                                                create_error.set(Some(format!("{e}")));
                                                            }
                                                        }
                                                        deleting_id.set(None);
                                                    });
                                                }
                                            },
                                            "Delete"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Add sub-URL form.
                div { class: "flex items-end gap-3 pt-2",
                    FormField { label: "Path",
                        input {
                            class: "input w-64",
                            r#type: "text",
                            placeholder: "/about",
                            value: "{new_path}",
                            oninput: move |evt| new_path.set(evt.value()),
                        }
                    }
                    Button {
                        variant: ButtonVariant::Primary,
                        disabled: *creating.read(),
                        onclick: {
                            let wid = webspace_id;
                            move |_| {
                                let p = new_path.read().clone();
                                if p.is_empty() {
                                    create_error.set(Some("Path cannot be empty".into()));
                                    return;
                                }
                                creating.set(true);
                                create_error.set(None);
                                spawn(async move {
                                    match create_suburl(wid, p).await {
                                        Ok(_) => {
                                            new_path.set(String::new());
                                            suburls.restart();
                                        }
                                        Err(e) => create_error.set(Some(format!("{e}"))),
                                    }
                                    creating.set(false);
                                });
                            }
                        },
                        if *creating.read() { "Adding..." } else { "Add Sub-URL" }
                    }
                }
                if let Some(ref msg) = *create_error.read() {
                    div { class: "text-sm text-danger", "{msg}" }
                }
            }
        }
    }
}

// ── Notifications ───────────────────────────────────────────────────

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
                            Th { "Path" }
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
                                TdMuted { "{row.path}" }
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
                    Badge { variant: BadgeVariant::Neutral, "{d.path}" }
                    Badge { variant: BadgeVariant::Neutral, "{d.created_at}" }
                }
                pre { class: "whitespace-pre-wrap text-sm text-fg font-mono bg-bg-inset p-4 rounded overflow-x-auto",
                    "{d.body}"
                }
            }
        }
    }
}

// ── Sub-URL Settings Page ────────────────────────────────────────────

#[component]
pub fn WebspaceChangedetectionSuburl(id: String, suburl_id: String) -> Element {
    let ws_id = match Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => return rsx! { div { class: "text-danger", "Invalid webspace ID" } },
    };
    let sid = match Uuid::parse_str(&suburl_id) {
        Ok(id) => id,
        Err(_) => return rsx! { div { class: "text-danger", "Invalid sub-URL ID" } },
    };

    let detail = use_server_future(move || async move { get_suburl(ws_id, sid).await })?;
    let d = match &*detail.read() {
        Some(Ok(d)) => d.clone(),
        Some(Err(e)) => return rsx! { div { class: "text-danger", "Error: {e}" } },
        None => return rsx! { div { class: "text-fg-muted", "Loading..." } },
    };

    let ws_name_res = use_server_future(move || async move { get_cd_config(ws_id).await })?;
    let ws_name = match &*ws_name_res.read() {
        Some(Ok(c)) => c.webspace_name.clone(),
        _ => "Webspace".to_string(),
    };

    rsx! {
        PageHeader {
            Link {
                to: crate::web::app::Route::WebspaceChangedetection { id: id.clone() },
                class: "text-brand underline",
                "{ws_name} / Change Detection"
            }
            " / {d.path}"
        }

        SubUrlSettingsForm { webspace_id: ws_id, suburl_id: sid, initial: d.tag_settings }
    }
}

/// Helper: split newline-separated text into a Vec, filtering empties.
fn lines_to_vec(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Helper: join a Vec into newline-separated text.
fn vec_to_lines(v: &[String]) -> String {
    v.join("\n")
}

#[component]
fn SubUrlSettingsForm(webspace_id: Uuid, suburl_id: Uuid, initial: TagSettings) -> Element {
    // Text-list fields as multiline text.
    let mut extract_text = use_signal(move || vec_to_lines(&initial.extract_text));
    let mut extract_lines = use_signal(move || vec_to_lines(&initial.extract_lines_containing));
    let mut text_not_present = use_signal(move || vec_to_lines(&initial.text_should_not_be_present));
    let mut include_filters = use_signal(move || vec_to_lines(&initial.include_filters));
    let mut subtractive = use_signal(move || vec_to_lines(&initial.subtractive_selectors));
    let mut ignore_text = use_signal(move || vec_to_lines(&initial.ignore_text));
    let mut trigger_text = use_signal(move || vec_to_lines(&initial.trigger_text));

    // Boolean toggles.
    let mut trim_ws = use_signal(move || initial.trim_text_whitespace.unwrap_or(false));
    let mut sort_alpha = use_signal(move || initial.sort_text_alphabetically.unwrap_or(false));
    let mut dedup = use_signal(move || initial.remove_duplicate_lines.unwrap_or(false));
    let mut unique = use_signal(move || initial.check_unique_lines.unwrap_or(false));
    let mut muted = use_signal(move || initial.notification_muted.unwrap_or(false));
    let mut screenshot = use_signal(move || initial.notification_screenshot.unwrap_or(false));

    // Match logic.
    let mut match_logic = use_signal(move || {
        initial
            .conditions_match_logic
            .clone()
            .unwrap_or_else(|| "ALL".to_string())
    });

    // Conditions as a dynamic list.
    let mut conditions = use_signal(move || initial.conditions.clone());

    // Notification fields.
    let mut notif_title = use_signal(move || initial.notification_title.clone().unwrap_or_default());
    let mut notif_body = use_signal(move || initial.notification_body.clone().unwrap_or_default());
    let mut notif_format = use_signal(move || initial.notification_format.clone().unwrap_or_default());

    let mut saving = use_signal(|| false);
    let mut result_msg = use_signal(|| None::<String>);

    let build_settings = move || -> TagSettings {
        TagSettings {
            conditions: conditions.read().clone(),
            conditions_match_logic: {
                let v = match_logic.read().clone();
                if v == "ALL" { None } else { Some(v) }
            },
            extract_text: lines_to_vec(&extract_text.read()),
            extract_lines_containing: lines_to_vec(&extract_lines.read()),
            text_should_not_be_present: lines_to_vec(&text_not_present.read()),
            include_filters: lines_to_vec(&include_filters.read()),
            subtractive_selectors: lines_to_vec(&subtractive.read()),
            ignore_text: lines_to_vec(&ignore_text.read()),
            trigger_text: lines_to_vec(&trigger_text.read()),
            trim_text_whitespace: if *trim_ws.read() { Some(true) } else { None },
            sort_text_alphabetically: if *sort_alpha.read() { Some(true) } else { None },
            remove_duplicate_lines: if *dedup.read() { Some(true) } else { None },
            check_unique_lines: if *unique.read() { Some(true) } else { None },
            notification_title: {
                let v = notif_title.read().clone();
                if v.is_empty() { None } else { Some(v) }
            },
            notification_body: {
                let v = notif_body.read().clone();
                if v.is_empty() { None } else { Some(v) }
            },
            notification_format: {
                let v = notif_format.read().clone();
                if v.is_empty() { None } else { Some(v) }
            },
            notification_muted: if *muted.read() { Some(true) } else { None },
            notification_screenshot: if *screenshot.read() { Some(true) } else { None },
        }
    };

    rsx! {
        Card {
            div { class: "p-6 space-y-6",

                // ── Conditions ───────────────────────────────────────
                SectionHeading { "Conditions" }
                div { class: "space-y-2",
                    div { class: "flex items-center gap-3 mb-2",
                        span { class: "text-sm text-fg-muted", "Match logic:" }
                        select {
                            class: "input w-32",
                            value: "{match_logic}",
                            oninput: move |evt| match_logic.set(evt.value()),
                            option { value: "ALL", "ALL" }
                            option { value: "ANY", "ANY" }
                        }
                    }

                    for (idx, _cond) in conditions.read().iter().enumerate() {
                        {
                            let conds = conditions.read().clone();
                            let cond = &conds[idx];
                            rsx! {
                                div { class: "flex items-center gap-2",
                                    input {
                                        class: "input w-40",
                                        r#type: "text",
                                        placeholder: "field",
                                        value: "{cond.field}",
                                        oninput: {
                                            move |evt: Event<FormData>| {
                                                let mut c = conditions.write();
                                                c[idx].field = evt.value();
                                            }
                                        },
                                    }
                                    input {
                                        class: "input w-40",
                                        r#type: "text",
                                        placeholder: "operator",
                                        value: "{cond.operator}",
                                        oninput: {
                                            move |evt: Event<FormData>| {
                                                let mut c = conditions.write();
                                                c[idx].operator = evt.value();
                                            }
                                        },
                                    }
                                    input {
                                        class: "input flex-1",
                                        r#type: "text",
                                        placeholder: "value",
                                        value: "{cond.value}",
                                        oninput: {
                                            move |evt: Event<FormData>| {
                                                let mut c = conditions.write();
                                                c[idx].value = evt.value();
                                            }
                                        },
                                    }
                                    Button {
                                        variant: ButtonVariant::Danger,
                                        onclick: {
                                            move |_| {
                                                let mut c = conditions.write();
                                                c.remove(idx);
                                            }
                                        },
                                        "X"
                                    }
                                }
                            }
                        }
                    }

                    Button {
                        variant: ButtonVariant::Secondary,
                        onclick: move |_| {
                            conditions.write().push(TagCondition::default());
                        },
                        "+ Add Condition"
                    }
                }

                // ── Text Filters ─────────────────────────────────────
                SectionHeading { "Text Filters" }
                div { class: "grid grid-cols-1 md:grid-cols-2 gap-4",
                    FormField { label: "Include Filters (CSS/XPath, one per line)",
                        textarea {
                            class: "input w-full h-24 font-mono text-sm",
                            value: "{include_filters}",
                            oninput: move |evt| include_filters.set(evt.value()),
                        }
                    }
                    FormField { label: "Subtractive Selectors (one per line)",
                        textarea {
                            class: "input w-full h-24 font-mono text-sm",
                            value: "{subtractive}",
                            oninput: move |evt| subtractive.set(evt.value()),
                        }
                    }
                    FormField { label: "Extract Text (regex, one per line)",
                        textarea {
                            class: "input w-full h-24 font-mono text-sm",
                            value: "{extract_text}",
                            oninput: move |evt| extract_text.set(evt.value()),
                        }
                    }
                    FormField { label: "Extract Lines Containing (one per line)",
                        textarea {
                            class: "input w-full h-24 font-mono text-sm",
                            value: "{extract_lines}",
                            oninput: move |evt| extract_lines.set(evt.value()),
                        }
                    }
                    FormField { label: "Text Should Not Be Present (one per line)",
                        textarea {
                            class: "input w-full h-24 font-mono text-sm",
                            value: "{text_not_present}",
                            oninput: move |evt| text_not_present.set(evt.value()),
                        }
                    }
                    FormField { label: "Ignore Text (one per line)",
                        textarea {
                            class: "input w-full h-24 font-mono text-sm",
                            value: "{ignore_text}",
                            oninput: move |evt| ignore_text.set(evt.value()),
                        }
                    }
                    FormField { label: "Trigger Text (one per line)",
                        textarea {
                            class: "input w-full h-24 font-mono text-sm",
                            value: "{trigger_text}",
                            oninput: move |evt| trigger_text.set(evt.value()),
                        }
                    }
                }

                // ── Text Processing ──────────────────────────────────
                SectionHeading { "Text Processing" }
                div { class: "flex flex-wrap gap-6",
                    label { class: "flex items-center gap-2 text-sm",
                        input {
                            r#type: "checkbox",
                            checked: *trim_ws.read(),
                            oninput: move |evt| trim_ws.set(evt.checked()),
                        }
                        "Trim whitespace"
                    }
                    label { class: "flex items-center gap-2 text-sm",
                        input {
                            r#type: "checkbox",
                            checked: *sort_alpha.read(),
                            oninput: move |evt| sort_alpha.set(evt.checked()),
                        }
                        "Sort alphabetically"
                    }
                    label { class: "flex items-center gap-2 text-sm",
                        input {
                            r#type: "checkbox",
                            checked: *dedup.read(),
                            oninput: move |evt| dedup.set(evt.checked()),
                        }
                        "Remove duplicate lines"
                    }
                    label { class: "flex items-center gap-2 text-sm",
                        input {
                            r#type: "checkbox",
                            checked: *unique.read(),
                            oninput: move |evt| unique.set(evt.checked()),
                        }
                        "Check unique lines"
                    }
                }

                // ── Notifications ────────────────────────────────────
                SectionHeading { "Notification Settings" }
                div { class: "grid grid-cols-1 md:grid-cols-2 gap-4",
                    FormField { label: "Notification Title",
                        input {
                            class: "input w-full",
                            r#type: "text",
                            value: "{notif_title}",
                            oninput: move |evt| notif_title.set(evt.value()),
                        }
                    }
                    FormField { label: "Notification Format",
                        select {
                            class: "input w-full",
                            value: "{notif_format}",
                            oninput: move |evt| notif_format.set(evt.value()),
                            option { value: "", "Default" }
                            option { value: "text", "Text" }
                            option { value: "html", "HTML" }
                            option { value: "markdown", "Markdown" }
                        }
                    }
                }
                FormField { label: "Notification Body",
                    textarea {
                        class: "input w-full h-24 font-mono text-sm",
                        value: "{notif_body}",
                        oninput: move |evt| notif_body.set(evt.value()),
                    }
                }
                div { class: "flex flex-wrap gap-6",
                    label { class: "flex items-center gap-2 text-sm",
                        input {
                            r#type: "checkbox",
                            checked: *muted.read(),
                            oninput: move |evt| muted.set(evt.checked()),
                        }
                        "Mute notifications"
                    }
                    label { class: "flex items-center gap-2 text-sm",
                        input {
                            r#type: "checkbox",
                            checked: *screenshot.read(),
                            oninput: move |evt| screenshot.set(evt.checked()),
                        }
                        "Include screenshot"
                    }
                }

                // ── Save Button ──────────────────────────────────────
                div { class: "flex items-center gap-3 pt-4",
                    Button {
                        variant: ButtonVariant::Primary,
                        disabled: *saving.read(),
                        onclick: move |_| {
                            let settings = build_settings();
                            saving.set(true);
                            result_msg.set(None);
                            spawn(async move {
                                match update_suburl_settings(webspace_id, suburl_id, settings).await {
                                    Ok(()) => result_msg.set(Some("Saved".into())),
                                    Err(e) => result_msg.set(Some(format!("Error: {e}"))),
                                }
                                saving.set(false);
                            });
                        },
                        if *saving.read() { "Saving..." } else { "Save Settings" }
                    }
                    if let Some(ref msg) = *result_msg.read() {
                        div { class: "text-sm text-fg-muted", "{msg}" }
                    }
                }
            }
        }
    }
}
