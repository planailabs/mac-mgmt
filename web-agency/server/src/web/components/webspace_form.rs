use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Button, ButtonKind, ButtonVariant, FormField, PageHeader};

use crate::web::user::OrgOption;

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
struct WsFormData {
    orgs: Vec<OrgOption>,
    mac_mgmt_creds: Vec<CredOption>,
    changedetection_creds: Vec<CredOption>,
    basic_auth_lists: Vec<BasicAuthOption>,
    /// Org-level default changedetection credential (keyed by org_id).
    org_cd_defaults: Vec<(Uuid, Option<Uuid>)>,
}

#[server]
async fn load_ws_form_data() -> Result<WsFormData, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let orgs = crate::web::user::list_user_write_orgs(&user, &pool).await?;

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

    let basic_auth_rows =
        if user.is_admin {
            sqlx::query_as::<_, (Uuid, String)>(
                "SELECT id, name FROM basic_auth_lists ORDER BY name",
            )
            .fetch_all(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
        } else {
            sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, name FROM basic_auth_lists WHERE organization_id = ANY($1) ORDER BY name",
        ).bind(&org_ids).fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
        };

    let cd_creds = if user.is_admin {
        sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, name FROM credentials WHERE credential_type = 'changedetection' ORDER BY name",
        ).fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
    } else {
        sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, name FROM credentials WHERE credential_type = 'changedetection' \
             AND (organization_id = ANY($1) OR organization_id IS NULL) ORDER BY name",
        )
        .bind(&org_ids)
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    };

    // Load org defaults for changedetection
    let org_cd_defaults = sqlx::query_as::<_, (Uuid, Option<Uuid>)>(
        "SELECT id, default_changedetection_credential_id FROM organizations WHERE id = ANY($1)",
    )
    .bind(&orgs.iter().map(|o| o.id).collect::<Vec<_>>())
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(WsFormData {
        orgs,
        mac_mgmt_creds: mac_mgmt_creds
            .into_iter()
            .map(|(id, name)| CredOption { id, name })
            .collect(),
        changedetection_creds: cd_creds
            .into_iter()
            .map(|(id, name)| CredOption { id, name })
            .collect(),
        basic_auth_lists: basic_auth_rows
            .into_iter()
            .map(|(id, name)| BasicAuthOption { id, name })
            .collect(),
        org_cd_defaults,
    })
}

#[server]
async fn create_webspace(
    org_id: Uuid,
    name: String,
    hosting_type: String,
    runtime: Option<String>,
    relay_url: Option<String>,
    relay_credential_id: Option<Uuid>,
    auth_mode: String,
    auth_basic_list_id: Option<Uuid>,
    changedetection_credential_id: Option<Uuid>,
) -> Result<Uuid, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO webspaces (organization_id, name, hosting_type, runtime, relay_url, relay_credential_id, auth_mode, auth_basic_list_id, changedetection_credential_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING id",
    )
    .bind(org_id)
    .bind(&name)
    .bind(&hosting_type)
    .bind(&runtime)
    .bind(&relay_url)
    .bind(relay_credential_id)
    .bind(&auth_mode)
    .bind(auth_basic_list_id)
    .bind(changedetection_credential_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(format!("failed to create webspace: {e}")))?;

    crate::api::internal::notify_proxy_reload();
    Ok(id)
}

#[component]
pub fn WebspaceForm() -> Element {
    let data = use_server_future(load_ws_form_data)?;
    let (org_list, mac_mgmt_creds, cd_creds, basic_auth_lists, org_cd_defaults) =
        match &*data.read() {
            Some(Ok(d)) => (
                d.orgs.clone(),
                d.mac_mgmt_creds.clone(),
                d.changedetection_creds.clone(),
                d.basic_auth_lists.clone(),
                d.org_cd_defaults.clone(),
            ),
            _ => (vec![], vec![], vec![], vec![], vec![]),
        };

    let mut name = use_signal(String::new);
    let mut hosting_type = use_signal(|| "local".to_string());
    let mut runtime = use_signal(|| "static".to_string());
    let mut org_id = use_signal(|| {
        org_list
            .first()
            .map(|o| o.id.to_string())
            .unwrap_or_default()
    });
    let mut relay_url = use_signal(String::new);
    let mut relay_cred_id = use_signal(|| {
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
    // Pre-fill changedetection from org default
    let initial_cd_cred = {
        let first_org = org_list.first().map(|o| o.id);
        first_org
            .and_then(|oid| org_cd_defaults.iter().find(|(id, _)| *id == oid))
            .and_then(|(_, default_id)| default_id.map(|id| id.to_string()))
            .unwrap_or_default()
    };
    let mut cd_cred_id = use_signal(move || initial_cd_cred.clone());
    let mut error = use_signal(|| None::<String>);
    let mut saving = use_signal(|| false);
    let nav = use_navigator();

    rsx! {
        PageHeader { "Create Webspace" }

        form {
            class: "card p-6 mt-4 max-w-xl space-y-4",
            onsubmit: move |evt| {
                evt.prevent_default();
                let n = name.read().clone();
                let ht = hosting_type.read().clone();
                let rt = runtime.read().clone();
                let oid_str = org_id.read().clone();
                let r_url = relay_url.read().clone();
                let r_cred = relay_cred_id.read().clone();
                let am = auth_mode.read().clone();
                let ba_list = auth_basic_list_id.read().clone();
                let cd_cred = cd_cred_id.read().clone();
                let nav = nav.clone();
                saving.set(true);
                error.set(None);
                spawn(async move {
                    let oid = uuid::Uuid::parse_str(&oid_str).ok();
                    if let Some(oid) = oid {
                        let rt_opt = if ht == "local" { Some(rt) } else { None };
                        let relay_url_opt = if (ht == "relay" || ht == "tunnel") && !r_url.is_empty() { Some(r_url) } else { None };
                        let relay_cred_opt = if ht == "relay" { uuid::Uuid::parse_str(&r_cred).ok() } else { None };
                        let am_val = if ht == "cloudflare_pages" { "none".to_string() } else { am };
                        let ba_list_opt = if am_val == "basic" { uuid::Uuid::parse_str(&ba_list).ok() } else { None };
                        let cd_cred_opt = uuid::Uuid::parse_str(&cd_cred).ok();
                        match create_webspace(oid, n, ht, rt_opt, relay_url_opt, relay_cred_opt, am_val, ba_list_opt, cd_cred_opt).await {
                            Ok(_) => { nav.push(crate::web::app::Route::WebspaceList {}); }
                            Err(e) => error.set(Some(format!("{e}"))),
                        }
                    } else {
                        error.set(Some("Select an organization".into()));
                    }
                    saving.set(false);
                });
            },

            FormField { label: "Organization",
                select {
                    class: "input",
                    value: "{org_id}",
                    oninput: move |evt| org_id.set(evt.value()),
                    for org in &org_list {
                        option { value: "{org.id}", "{org.name}" }
                    }
                }
            }

            FormField { label: "Name",
                input {
                    class: "input",
                    r#type: "text",
                    placeholder: "e.g. my-website",
                    required: true,
                    value: "{name}",
                    oninput: move |evt| name.set(evt.value()),
                }
            }

            FormField { label: "Hosting Type",
                select {
                    class: "input",
                    value: "{hosting_type}",
                    oninput: move |evt| hosting_type.set(evt.value()),
                    option { value: "local", "Local (Pingora + ACME)" }
                    option { value: "cloudflare_pages", "Cloudflare Pages" }
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
                    help: "Credential used to mint proxy tokens for the relay.",
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

            // ChangeDetection.io credential (optional, all hosting types)
            if !cd_creds.is_empty() {
                FormField { label: "ChangeDetection.io (optional)",
                    help: "Credential for automatic change monitoring of bound domains.",
                    select {
                        class: "input",
                        value: "{cd_cred_id}",
                        oninput: move |evt| cd_cred_id.set(evt.value()),
                        option { value: "", "None" }
                        for c in &cd_creds {
                            option { value: "{c.id}", "{c.name}" }
                        }
                    }
                }
            }

            // Auth mode (not for Pages)
            if *hosting_type.read() != "cloudflare_pages" {
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

                if *auth_mode.read() == "oidc" {
                    div { class: "text-sm text-fg-muted",
                        "Members of the webspace's organization will have access after logging in."
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
                Link {
                    to: crate::web::app::Route::WebspaceList {},
                    class: "btn btn-secondary",
                    "Cancel"
                }
            }
        }
    }
}
