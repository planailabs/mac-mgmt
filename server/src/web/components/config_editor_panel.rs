//! Server-side wrapper around the shared `mac_mgmt_config_ui::ConfigEditor`.
//!
//! The editor UI itself lives in the `mac-mgmt-config-ui` crate (shared with the
//! sovereign-AI USB overview). This wrapper supplies the server's data layer:
//! it loads the cluster's config + JSON Schema via `#[server]` functions and
//! persists edits back to postgres, passing them to the shared component as
//! props. (These three server fns previously lived in the now-deleted
//! `config_editor.rs`.)

use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::models::ClusterConfig;
#[cfg(feature = "server")]
use crate::web::user::{WebUserExt, current_user};

#[server]
async fn get_current_config(cluster_id: String) -> Result<Option<ClusterConfig>, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_id
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
    let config = sqlx::query_as::<_, ClusterConfig>(
        "SELECT * FROM cluster_configs WHERE cluster_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(uuid)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(config.map(|mut c| {
        mac_mgmt_common::config_migrate::migrate(&mut c.config_json);
        c
    }))
}

#[server]
async fn save_config(cluster_id: String, config_json: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    let mut json: serde_json::Value = serde_json::from_str(&config_json)
        .map_err(|e| ServerFnError::new(format!("invalid JSON: {e}")))?;
    mac_mgmt_common::config_migrate::migrate(&mut json);
    // Validate
    let _: mac_mgmt_common::ClusterConfig = serde_json::from_value(json.clone())
        .map_err(|e| ServerFnError::new(format!("invalid config: {e}")))?;

    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if let Some(ids) = user
        .writable_cluster_ids(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    {
        if !ids.contains(&uuid) {
            return Err(ServerFnError::new("access denied"));
        }
    }
    sqlx::query("INSERT INTO cluster_configs (cluster_id, config_json) VALUES ($1, $2)")
        .bind(uuid)
        .bind(&json)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    crate::api::push::notify_global(uuid, crate::api::push::PushMessage::SyncConfig).await;
    Ok(())
}

#[server]
async fn get_config_schema() -> Result<serde_json::Value, ServerFnError> {
    let _user = current_user().await?;
    let schema = schemars::schema_for!(mac_mgmt_common::ClusterConfig);
    let value = serde_json::to_value(&schema).map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(value)
}

/// Server-backed config editor: loads config + schema, renders the shared
/// editor, and persists saves to postgres.
#[component]
pub fn ConfigEditorPanel(cluster_id: String, read_only: bool) -> Element {
    let cid = cluster_id.clone();
    let mut config = use_server_future(move || {
        let cid = cid.clone();
        async move { get_current_config(cid).await }
    })?;
    let schema = use_server_future(|| async { get_config_schema().await })?;

    let mut saving = use_signal(|| false);
    let mut save_error = use_signal(|| None::<String>);

    // Schema: required to render the form.
    let schema_val = match &*schema.read() {
        Some(Ok(v)) => v.clone(),
        Some(Err(e)) => {
            return rsx! { p { class: "text-danger text-sm", "{e}" } };
        }
        None => return rsx! { p { class: "text-sm", {t!("config-editor-loading-schema")} } },
    };

    // Current config (empty object when none saved yet).
    let initial_val = match &*config.read() {
        Some(Ok(Some(c))) => c.config_json.clone(),
        Some(Ok(None)) => serde_json::json!({}),
        Some(Err(e)) => {
            return rsx! { p { class: "text-danger text-sm", "{e}" } };
        }
        None => return rsx! { p { class: "text-sm", {t!("config-editor-loading-schema")} } },
    };

    let cid_save = cluster_id.clone();
    rsx! {
        mac_mgmt_config_ui::ConfigEditor {
            cluster_id,
            read_only,
            schema: schema_val,
            initial: initial_val,
            saving: saving(),
            save_error: save_error(),
            on_save: move |json: String| {
                let cid = cid_save.clone();
                saving.set(true);
                save_error.set(None);
                spawn(async move {
                    match save_config(cid, json).await {
                        Ok(()) => {
                            config.restart();
                        }
                        Err(e) => {
                            save_error.set(Some(e.to_string()));
                        }
                    }
                    saving.set(false);
                });
            },
        }
    }
}
