//! Server-side wrapper around the shared `mac_mgmt_config_ui::ConfigEditor`.
//!
//! The editor UI itself lives in the `mac-mgmt-config-ui` crate (shared with the
//! sovereign-AI USB overview). This wrapper supplies the server's data layer:
//! it loads the cluster's config + JSON Schema via the api-mcp cluster
//! endpoints and persists edits back to postgres, passing them to the shared
//! component as props.

use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api_mcp::endpoints::clusters::{
    ConfigGetInput, ConfigSaveInput, ConfigSchemaInput, get_config_schema, get_current_config,
    save_config,
};

/// Server-backed config editor: loads config + schema, renders the shared
/// editor, and persists saves to postgres.
#[component]
pub fn ConfigEditorPanel(cluster_id: String, read_only: bool) -> Element {
    let cid = cluster_id.clone();
    let mut config = use_server_future(move || {
        let cid = cid.clone();
        async move {
            let id: uuid::Uuid = cid
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            get_current_config(ConfigGetInput { id }).await
        }
    })?;
    let schema = use_server_future(|| async { get_config_schema(ConfigSchemaInput {}).await })?;

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
                    let result = match cid.parse::<uuid::Uuid>() {
                        Ok(id) => save_config(ConfigSaveInput { id, config_json: json })
                            .await
                            .map_err(|e| e.to_string()),
                        Err(e) => Err(e.to_string()),
                    };
                    match result {
                        Ok(()) => {
                            config.restart();
                        }
                        Err(e) => {
                            save_error.set(Some(e));
                        }
                    }
                    saving.set(false);
                });
            },
        }
    }
}
