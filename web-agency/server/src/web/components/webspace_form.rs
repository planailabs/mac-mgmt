//! Create a webspace folder within a proxy host.
//!
//! Folders are path-mounted under the host's hostname. `hosting_type` is one of
//! the proxy-routed kinds (`local`/`relay`/`tunnel`); Cloudflare projects are
//! created at the host level, not as folders.

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::relay_picker::{RelayCredOption, RelayUrlPicker};
use super::ui::{Button, ButtonKind, ButtonVariant, FormField, PageHeader};

// create_folder now lives in the shared api_mcp layer.
use crate::api_mcp::endpoints::webspaces::{WebspaceCreateInput, create_folder};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredOption {
    id: Uuid,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BasicAuthOption {
    id: Uuid,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FolderFormData {
    host_name: String,
    host_kind: String,
    mac_mgmt_creds: Vec<CredOption>,
    basic_auth_lists: Vec<BasicAuthOption>,
}

#[server]
async fn load_folder_form_data(host_id: Uuid) -> Result<FolderFormData, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let (host_name, host_kind, org_id) = sqlx::query_as::<_, (String, String, Uuid)>(
        "SELECT name, kind, organization_id FROM webspace_hosts WHERE id = $1",
    )
    .bind(host_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("host not found"))?;

    use crate::web::user::WebUserExt;
    user.require_org_read(&org_id)?;

    let org_ids = user.org_ids();
    let mac_mgmt_creds = if user.is_admin {
        sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, name FROM credentials WHERE credential_type = 'mac-mgmt' ORDER BY name",
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    } else {
        sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, name FROM credentials WHERE credential_type = 'mac-mgmt' \
             AND (organization_id = ANY($1) OR organization_id IS NULL) ORDER BY name",
        )
        .bind(&org_ids)
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    };

    let basic_auth_rows = if user.is_admin {
        sqlx::query_as::<_, (Uuid, String)>("SELECT id, name FROM basic_auth_lists ORDER BY name")
            .fetch_all(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
    } else {
        sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, name FROM basic_auth_lists WHERE organization_id = ANY($1) ORDER BY name",
        )
        .bind(&org_ids)
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    };

    Ok(FolderFormData {
        host_name,
        host_kind,
        mac_mgmt_creds: mac_mgmt_creds
            .into_iter()
            .map(|(id, name)| CredOption { id, name })
            .collect(),
        basic_auth_lists: basic_auth_rows
            .into_iter()
            .map(|(id, name)| BasicAuthOption { id, name })
            .collect(),
    })
}

#[component]
pub fn WebspaceForm(host_id: String) -> Element {
    let host_uuid = Uuid::parse_str(&host_id).ok();
    let data = use_server_future(move || {
        let hid = host_uuid;
        async move {
            match hid {
                Some(id) => load_folder_form_data(id).await,
                None => Err(ServerFnError::new("invalid host id")),
            }
        }
    })?;
    let (host_name, host_kind, mac_mgmt_creds, basic_auth_lists) = match &*data.read() {
        Some(Ok(d)) => (
            d.host_name.clone(),
            d.host_kind.clone(),
            d.mac_mgmt_creds.clone(),
            d.basic_auth_lists.clone(),
        ),
        _ => (String::new(), String::new(), vec![], vec![]),
    };

    let host_list_route = crate::web::app::Route::WebspaceHostList {};
    let detail_route = crate::web::app::Route::WebspaceHostDetail {
        id: host_id.clone(),
    };

    if host_kind == "cloudflare" {
        return rsx! {
            PageHeader { "Add Folder" }
            div { class: "card p-6 mt-4 max-w-xl",
                div { class: "text-fg-muted", "Cloudflare hosts have a single project folder and cannot have additional path-mounted folders." }
                Link { to: detail_route.clone(), class: "btn btn-secondary mt-4", "Back to host" }
            }
        };
    }

    let mut name = use_signal(String::new);
    let mut path_prefix = use_signal(|| "/".to_string());
    let mut hosting_type = use_signal(|| "local".to_string());
    let mut runtime = use_signal(|| "static".to_string());
    let mut relay_url = use_signal(String::new);
    let relay_cred_id = use_signal(|| {
        mac_mgmt_creds
            .first()
            .map(|c| c.id.to_string())
            .unwrap_or_default()
    });
    let mut auth_mode = use_signal(|| "none".to_string());
    let mut auth_basic_list_id = use_signal(|| {
        basic_auth_lists
            .first()
            .map(|b| b.id.to_string())
            .unwrap_or_default()
    });
    let mut error = use_signal(|| None::<String>);
    let mut saving = use_signal(|| false);
    let nav = use_navigator();

    let host_uuid_for_submit = host_uuid;
    let detail_for_submit = detail_route.clone();

    rsx! {
        PageHeader { "Add Folder to {host_name}" }

        form {
            class: "card p-6 mt-4 max-w-xl space-y-4",
            onsubmit: move |evt| {
                evt.prevent_default();
                let n = name.read().clone();
                let pp = path_prefix.read().clone();
                let ht = hosting_type.read().clone();
                let rt = runtime.read().clone();
                let r_url = relay_url.read().clone();
                let r_cred = relay_cred_id.read().clone();
                let am = auth_mode.read().clone();
                let ba_list = auth_basic_list_id.read().clone();
                let nav = nav.clone();
                let detail = detail_for_submit.clone();
                saving.set(true);
                error.set(None);
                spawn(async move {
                    if let Some(hid) = host_uuid_for_submit {
                        let rt_opt = if ht == "local" { Some(rt) } else { None };
                        let relay_url_opt = if (ht == "relay" || ht == "tunnel") && !r_url.is_empty() { Some(r_url) } else { None };
                        let relay_cred_opt = if ht == "relay" { uuid::Uuid::parse_str(&r_cred).ok() } else { None };
                        let ba_list_opt = if am == "basic" { uuid::Uuid::parse_str(&ba_list).ok() } else { None };
                        match create_folder(WebspaceCreateInput {
                            host_id: hid,
                            name: n,
                            path_prefix: pp,
                            hosting_type: ht,
                            runtime: rt_opt,
                            relay_url: relay_url_opt,
                            relay_credential_id: relay_cred_opt,
                            auth_mode: am,
                            auth_basic_list_id: ba_list_opt,
                        }).await {
                            Ok(_) => { nav.push(detail); }
                            Err(e) => error.set(Some(format!("{e}"))),
                        }
                    } else {
                        error.set(Some("invalid host".into()));
                    }
                    saving.set(false);
                });
            },

            FormField { label: "Name",
                input {
                    class: "input",
                    r#type: "text",
                    placeholder: "e.g. api",
                    required: true,
                    value: "{name}",
                    oninput: move |evt| name.set(evt.value()),
                }
            }

            FormField { label: "Mount Path",
                help: "Path prefix under the host's hostname, e.g. /api. \"/\" is the main folder.",
                input {
                    class: "input font-mono",
                    r#type: "text",
                    required: true,
                    placeholder: "/api",
                    value: "{path_prefix}",
                    oninput: move |evt| path_prefix.set(evt.value()),
                }
            }

            FormField { label: "Hosting Type",
                select {
                    class: "input",
                    value: "{hosting_type}",
                    oninput: move |evt| hosting_type.set(evt.value()),
                    option { value: "local", "Local (runtime)" }
                    option { value: "relay", "Relay Tunnel (mac-mgmt)" }
                    option { value: "tunnel", "Tunnel (plain reverse proxy)" }
                }
            }

            if *hosting_type.read() == "local" {
                FormField { label: "Runtime",
                    select {
                        class: "input",
                        value: "{runtime}",
                        oninput: move |evt| runtime.set(evt.value()),
                        option { value: "static", "Static Files" }
                        option { value: "nodejs", "Node.js (stub)" }
                        option { value: "docker", "Docker (stub)" }
                    }
                }
            }

            if *hosting_type.read() == "tunnel" {
                FormField { label: "Upstream URL",
                    help: "URL to reverse-proxy to (e.g. https://backend.example.com)",
                    input {
                        class: "input font-mono",
                        r#type: "url",
                        required: true,
                        placeholder: "https://backend.example.com",
                        value: "{relay_url}",
                        oninput: move |evt| relay_url.set(evt.value()),
                    }
                }
            }

            if *hosting_type.read() == "relay" {
                RelayUrlPicker {
                    creds: mac_mgmt_creds.iter().map(|c| RelayCredOption { id: c.id, name: c.name.clone() }).collect::<Vec<_>>(),
                    credential_id: relay_cred_id,
                    relay_url,
                }
            }

            FormField { label: "Auth Mode",
                select {
                    class: "input",
                    value: "{auth_mode}",
                    oninput: move |evt| auth_mode.set(evt.value()),
                    option { value: "none", "None" }
                    option { value: "oidc", "OIDC (org members)" }
                    option { value: "basic", "HTTP Basic" }
                }
            }

            if *auth_mode.read() == "basic" {
                FormField { label: "Basic Auth List",
                    select {
                        class: "input",
                        value: "{auth_basic_list_id}",
                        oninput: move |evt| auth_basic_list_id.set(evt.value()),
                        if basic_auth_lists.is_empty() {
                            option { value: "", "No basic auth lists — create one first" }
                        }
                        for b in &basic_auth_lists {
                            option { value: "{b.id}", "{b.name}" }
                        }
                    }
                }
            }

            if let Some(err) = &*error.read() {
                div { class: "text-red-400 text-sm", "{err}" }
            }

            div { class: "flex gap-3",
                Button {
                    variant: ButtonVariant::Primary,
                    kind: ButtonKind::Submit,
                    disabled: *saving.read(),
                    if *saving.read() { "Creating..." } else { "Create" }
                }
                Link { to: host_list_route, class: "btn btn-secondary", "Cancel" }
            }
        }
    }
}
