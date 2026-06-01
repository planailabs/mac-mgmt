//! Create a webspace folder within a proxy host.
//!
//! Folders are path-mounted under the host's hostname. `hosting_type` is one of
//! the proxy-routed kinds (`local`/`relay`/`tunnel`); Cloudflare projects are
//! created at the host level, not as folders.

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Button, ButtonKind, ButtonVariant, FormField, PageHeader};

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

    let (host_name, host_kind, org_id) =
        sqlx::query_as::<_, (String, String, Uuid)>(
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

#[server]
async fn create_folder(
    host_id: Uuid,
    name: String,
    path_prefix: String,
    hosting_type: String,
    runtime: Option<String>,
    relay_url: Option<String>,
    relay_credential_id: Option<Uuid>,
    auth_mode: String,
    auth_basic_list_id: Option<Uuid>,
) -> Result<Uuid, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let (org_id, kind) = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT organization_id, kind FROM webspace_hosts WHERE id = $1",
    )
    .bind(host_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("host not found"))?;

    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    if kind != "proxy" {
        return Err(ServerFnError::new("folders can only be added to proxy hosts"));
    }
    if !matches!(hosting_type.as_str(), "local" | "relay" | "tunnel") {
        return Err(ServerFnError::new("invalid folder hosting type"));
    }
    // Normalize the mount path: leading slash, no trailing slash, no doubles.
    let mut path = path_prefix.trim().to_string();
    if !path.starts_with('/') {
        return Err(ServerFnError::new("path must start with /"));
    }
    while path.contains("//") {
        path = path.replace("//", "/");
    }
    while path.len() > 1 && path.ends_with('/') {
        path.pop();
    }
    if path == "/" {
        return Err(ServerFnError::new("\"/\" is reserved for the main folder"));
    }
    let path = path.as_str();

    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO webspaces (organization_id, webspace_host_id, name, path_prefix, hosting_type, runtime, relay_url, relay_credential_id, auth_mode, auth_basic_list_id, local_status) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'stopped') RETURNING id",
    )
    .bind(org_id)
    .bind(host_id)
    .bind(&name)
    .bind(path)
    .bind(&hosting_type)
    .bind(&runtime)
    .bind(&relay_url)
    .bind(relay_credential_id)
    .bind(&auth_mode)
    .bind(auth_basic_list_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(format!("failed to create folder: {e}")))?;

    crate::api::internal::notify_proxy_reload();
    Ok(id)
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
    let detail_route = crate::web::app::Route::WebspaceHostDetail { id: host_id.clone() };

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
    let mut relay_cred_id =
        use_signal(|| mac_mgmt_creds.first().map(|c| c.id.to_string()).unwrap_or_default());
    let mut auth_mode = use_signal(|| "none".to_string());
    let mut auth_basic_list_id =
        use_signal(|| basic_auth_lists.first().map(|b| b.id.to_string()).unwrap_or_default());
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
                        match create_folder(hid, n, pp, ht, rt_opt, relay_url_opt, relay_cred_opt, am, ba_list_opt).await {
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
                FormField { label: "Relay URL",
                    help: "Full URL from the relay (e.g. https://abc123-ollama.relay.plan.ai)",
                    input {
                        class: "input font-mono",
                        r#type: "url",
                        required: true,
                        placeholder: "https://instance-tunnel.relay.example.com",
                        value: "{relay_url}",
                        oninput: move |evt| relay_url.set(evt.value()),
                    }
                }
                FormField { label: "mac-mgmt Credential",
                    select {
                        class: "input",
                        value: "{relay_cred_id}",
                        oninput: move |evt| relay_cred_id.set(evt.value()),
                        if mac_mgmt_creds.is_empty() {
                            option { value: "", "No mac-mgmt credentials — add one first" }
                        }
                        for c in &mac_mgmt_creds {
                            option { value: "{c.id}", "{c.name}" }
                        }
                    }
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
