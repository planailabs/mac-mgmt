//! Per-webspace ChangeDetection.io monitoring endpoints: credential binding,
//! monitored sub-URLs (with tag settings), and the notification inbox.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use dioxus::prelude::*;
use plan_ai_api_mcp_macros::api_mcp_dioxus_server;

#[cfg(feature = "server")]
use crate::server_pool;
#[cfg(feature = "server")]
use crate::web::user::{current_user, principal_from, to_serverfn};
#[cfg(feature = "server")]
use plan_ai_api_mcp::{ApiError, Principal};

// ── DTOs ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CdConfig {
    pub webspace_name: String,
    pub credential_id: Option<Uuid>,
    pub credential_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CdGetInput {
    /// Webspace host id.
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CdSetInput {
    /// Webspace host id.
    pub id: Uuid,
    /// ChangeDetection.io credential to bind, or None to unbind.
    pub credential_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SubUrlRow {
    pub id: Uuid,
    pub path: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SubUrlListInput {
    /// Webspace host id.
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SubUrlCreateInput {
    /// Webspace host id.
    pub id: Uuid,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SubUrlGetInput {
    /// Webspace host id.
    pub id: Uuid,
    pub suburl_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SubUrlUpdateInput {
    /// Webspace host id.
    pub id: Uuid,
    pub suburl_id: Uuid,
    pub settings: TagSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SubUrlDeleteInput {
    /// Webspace host id.
    pub id: Uuid,
    pub suburl_id: Uuid,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SubUrlDetail {
    pub id: Uuid,
    pub path: String,
    pub tag_settings: TagSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NotificationRow {
    pub id: Uuid,
    pub title: String,
    pub body_preview: String,
    pub path: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NotificationListInput {
    /// Webspace host id.
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NotificationGetInput {
    /// Webspace host id.
    pub id: Uuid,
    pub notification_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NotificationDetail {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub created_at: String,
    pub webspace_name: String,
    pub path: String,
}

/// Tag settings stored in changedetection_suburls.tag_settings JSONB.
/// Maps 1:1 to ChangeDetection.io tag update fields.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TagCondition {
    #[serde(default)]
    pub field: String,
    #[serde(default)]
    pub operator: String,
    #[serde(default)]
    pub value: String,
}

// ── Config endpoints ──────────────────────────────────────────────────

/// Get a webspace's change-detection config (requires org read).
#[api_mcp_dioxus_server(server = "get_cd_config")]
pub async fn changedetection_get(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: CdGetInput,
) -> Result<CdConfig, ApiError> {
    let org_id = super::owning_org(pool, "webspace_hosts", input.id, "webspace host").await?;
    principal.require_read(&org_id)?;

    let (name, cred_id) = sqlx::query_as::<_, (String, Option<Uuid>)>(
        "SELECT name, changedetection_credential_id FROM webspace_hosts WHERE id = $1",
    )
    .bind(input.id)
    .fetch_one(pool)
    .await
    .map_err(super::internal)?;

    let cred_name = if let Some(cid) = cred_id {
        sqlx::query_scalar::<_, String>("SELECT name FROM credentials WHERE id = $1")
            .bind(cid)
            .fetch_optional(pool)
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

/// Bind or unbind a ChangeDetection.io credential on a webspace (requires org
/// write). Switching away from a credential undeploys its tags, clears cached
/// tag ids, and deletes the old notifications.
#[api_mcp_dioxus_server(server = "set_webspace_changedetection")]
pub async fn changedetection_set(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: CdSetInput,
) -> Result<(), ApiError> {
    let (org_id, old_cred_id, ws_name) = sqlx::query_as::<_, (Uuid, Option<Uuid>, String)>(
        "SELECT organization_id, changedetection_credential_id, name \
             FROM webspace_hosts WHERE id = $1",
    )
    .bind(input.id)
    .fetch_optional(pool)
    .await
    .map_err(super::internal)?
    .ok_or_else(|| ApiError::not_found("webspace host not found"))?;

    principal.require_write(&org_id)?;

    // Undeploy old credential: clean up tags, watches, and notifications so
    // nothing from the old credential instance is left behind.
    if let Some(old_cred) = old_cred_id {
        if input.credential_id != Some(old_cred) {
            if let Ok((client, group_name)) =
                crate::credentials::changedetection_client(pool, old_cred).await
            {
                // Delete all sub-URL tags from the old CD instance.
                let suburl_tags = sqlx::query_scalar::<_, Uuid>(
                    "SELECT tag_id FROM changedetection_suburls \
                     WHERE webspace_host_id = $1 AND tag_id IS NOT NULL",
                )
                .bind(input.id)
                .fetch_all(pool)
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
                "UPDATE changedetection_suburls SET tag_id = NULL WHERE webspace_host_id = $1",
            )
            .bind(input.id)
            .execute(pool)
            .await
            .map_err(super::internal)?;

            sqlx::query("DELETE FROM changedetection_notifications WHERE webspace_host_id = $1")
                .bind(input.id)
                .execute(pool)
                .await
                .map_err(super::internal)?;
        }
    }

    sqlx::query(
        "UPDATE webspace_hosts SET changedetection_credential_id = $1, updated_at = now() WHERE id = $2",
    )
    .bind(input.credential_id)
    .bind(input.id)
    .execute(pool)
    .await
    .map_err(super::internal)?;

    Ok(())
}

// ── Sub-URL CRUD ─────────────────────────────────────────────────────

/// List a webspace's monitored sub-URLs (requires org read).
#[api_mcp_dioxus_server(server = "list_suburls")]
pub async fn changedetection_suburl_list(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: SubUrlListInput,
) -> Result<Vec<SubUrlRow>, ApiError> {
    let org_id = super::owning_org(pool, "webspace_hosts", input.id, "webspace host").await?;
    principal.require_read(&org_id)?;

    let rows = sqlx::query_as::<_, (Uuid, String, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, path, created_at FROM changedetection_suburls \
         WHERE webspace_host_id = $1 ORDER BY path",
    )
    .bind(input.id)
    .fetch_all(pool)
    .await
    .map_err(super::internal)?;

    Ok(rows
        .into_iter()
        .map(|(id, path, created_at)| SubUrlRow {
            id,
            path,
            created_at: created_at.format("%Y-%m-%d %H:%M").to_string(),
        })
        .collect())
}

/// Add a monitored sub-URL to a webspace (requires org write). The path is
/// normalized; returns the new sub-URL's id.
#[api_mcp_dioxus_server(server = "create_suburl")]
pub async fn changedetection_suburl_create(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: SubUrlCreateInput,
) -> Result<Uuid, ApiError> {
    let org_id = super::owning_org(pool, "webspace_hosts", input.id, "webspace host").await?;
    principal.require_write(&org_id)?;

    // Normalize path: must start with /, no trailing / (unless root), no double slashes.
    let path = input.path.trim().to_string();
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

    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO changedetection_suburls (webspace_host_id, path, secret) \
         VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(input.id)
    .bind(&path)
    .bind(&secret)
    .fetch_one(pool)
    .await
    .map_err(super::internal)
}

/// Get a sub-URL's path and tag settings (requires org read).
#[api_mcp_dioxus_server(server = "get_suburl")]
pub async fn changedetection_suburl_get(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: SubUrlGetInput,
) -> Result<SubUrlDetail, ApiError> {
    let org_id = super::owning_org(pool, "webspace_hosts", input.id, "webspace host").await?;
    principal.require_read(&org_id)?;

    let (id, path, settings_json) = sqlx::query_as::<_, (Uuid, String, serde_json::Value)>(
        "SELECT id, path, tag_settings FROM changedetection_suburls \
         WHERE id = $1 AND webspace_host_id = $2",
    )
    .bind(input.suburl_id)
    .bind(input.id)
    .fetch_optional(pool)
    .await
    .map_err(super::internal)?
    .ok_or_else(|| ApiError::not_found("sub-URL not found"))?;

    let tag_settings: TagSettings = serde_json::from_value(settings_json).unwrap_or_default();

    Ok(SubUrlDetail {
        id,
        path,
        tag_settings,
    })
}

/// Update a sub-URL's tag settings (requires org write). Saves locally and
/// pushes to ChangeDetection.io when a tag is already deployed.
#[api_mcp_dioxus_server(server = "update_suburl_settings")]
pub async fn changedetection_suburl_update(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: SubUrlUpdateInput,
) -> Result<(), ApiError> {
    let org_id = super::owning_org(pool, "webspace_hosts", input.id, "webspace host").await?;
    principal.require_write(&org_id)?;

    let settings_json = serde_json::to_value(&input.settings).map_err(super::internal)?;

    // Update the JSONB column.
    sqlx::query(
        "UPDATE changedetection_suburls SET tag_settings = $1, updated_at = now() \
         WHERE id = $2 AND webspace_host_id = $3",
    )
    .bind(&settings_json)
    .bind(input.suburl_id)
    .bind(input.id)
    .execute(pool)
    .await
    .map_err(super::internal)?;

    // Push settings to changedetection.io immediately if the tag is cached.
    let tag_and_cred = sqlx::query_as::<_, (Option<Uuid>, Option<Uuid>)>(
        "SELECT cs.tag_id, h.changedetection_credential_id \
         FROM changedetection_suburls cs \
         JOIN webspace_hosts h ON h.id = cs.webspace_host_id \
         WHERE cs.id = $1 AND cs.webspace_host_id = $2",
    )
    .bind(input.suburl_id)
    .bind(input.id)
    .fetch_optional(pool)
    .await
    .map_err(super::internal)?;

    if let Some((Some(tag_id), Some(cred_id))) = tag_and_cred {
        if let Ok((client, _)) = crate::credentials::changedetection_client(pool, cred_id).await {
            let update: changedetection_api::types::Tag =
                serde_json::from_value(settings_json).unwrap_or_default();
            if let Err(e) = client.update_tag(&tag_id, &update).await {
                return Err(ApiError::internal(format!(
                    "Saved locally but failed to push to ChangeDetection.io: {e}"
                )));
            }
        }
    }

    Ok(())
}

/// Delete a monitored sub-URL (requires org write). Removes its deployed tag
/// from ChangeDetection.io and cascades its notifications.
#[api_mcp_dioxus_server(server = "delete_suburl")]
pub async fn changedetection_suburl_delete(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: SubUrlDeleteInput,
) -> Result<(), ApiError> {
    let org_id = super::owning_org(pool, "webspace_hosts", input.id, "webspace host").await?;
    principal.require_write(&org_id)?;

    // Delete the tag from changedetection.io if cached.
    let tag_and_cred = sqlx::query_as::<_, (Option<Uuid>, Option<Uuid>)>(
        "SELECT cs.tag_id, h.changedetection_credential_id \
         FROM changedetection_suburls cs \
         JOIN webspace_hosts h ON h.id = cs.webspace_host_id \
         WHERE cs.id = $1 AND cs.webspace_host_id = $2",
    )
    .bind(input.suburl_id)
    .bind(input.id)
    .fetch_optional(pool)
    .await
    .map_err(super::internal)?;

    if let Some((Some(tag_id), Some(cred_id))) = tag_and_cred {
        if let Ok((client, _)) = crate::credentials::changedetection_client(pool, cred_id).await {
            let _ = client.delete_tag(&tag_id).await;
        }
    }

    // CASCADE deletes associated notifications.
    sqlx::query("DELETE FROM changedetection_suburls WHERE id = $1 AND webspace_host_id = $2")
        .bind(input.suburl_id)
        .bind(input.id)
        .execute(pool)
        .await
        .map_err(super::internal)?;

    Ok(())
}

// ── Notifications ────────────────────────────────────────────────────

/// List a webspace's latest change notifications (requires org read; newest
/// first, capped at 100).
#[api_mcp_dioxus_server(server = "list_notifications")]
pub async fn changedetection_notifications(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: NotificationListInput,
) -> Result<Vec<NotificationRow>, ApiError> {
    let org_id = super::owning_org(pool, "webspace_hosts", input.id, "webspace host").await?;
    principal.require_read(&org_id)?;

    let rows = sqlx::query_as::<
        _,
        (
            Uuid,
            Option<String>,
            Option<String>,
            chrono::DateTime<chrono::Utc>,
            String,
        ),
    >(
        "SELECT n.id, n.title, n.body, n.created_at, cs.path \
         FROM changedetection_notifications n \
         JOIN changedetection_suburls cs ON cs.id = n.suburl_id \
         WHERE n.webspace_host_id = $1 \
         ORDER BY n.created_at DESC \
         LIMIT 100",
    )
    .bind(input.id)
    .fetch_all(pool)
    .await
    .map_err(super::internal)?;

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

/// Get a single change notification with full body (requires org read).
#[api_mcp_dioxus_server(server = "get_notification")]
pub async fn changedetection_notification_get(
    pool: &sqlx::PgPool,
    principal: &Principal,
    input: NotificationGetInput,
) -> Result<NotificationDetail, ApiError> {
    let org_id = super::owning_org(pool, "webspace_hosts", input.id, "webspace host").await?;
    principal.require_read(&org_id)?;

    let (title, body, created_at, ws_name, path) = sqlx::query_as::<
        _,
        (
            Option<String>,
            Option<String>,
            chrono::DateTime<chrono::Utc>,
            String,
            String,
        ),
    >(
        "SELECT n.title, n.body, n.created_at, h.name, cs.path \
             FROM changedetection_notifications n \
             JOIN webspace_hosts h ON h.id = n.webspace_host_id \
             JOIN changedetection_suburls cs ON cs.id = n.suburl_id \
             WHERE n.id = $1 AND n.webspace_host_id = $2",
    )
    .bind(input.notification_id)
    .bind(input.id)
    .fetch_optional(pool)
    .await
    .map_err(super::internal)?
    .ok_or_else(|| ApiError::not_found("notification not found"))?;

    Ok(NotificationDetail {
        id: input.notification_id,
        title: title.unwrap_or_default(),
        body: body.unwrap_or_default(),
        created_at: created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        webspace_name: ws_name,
        path,
    })
}
