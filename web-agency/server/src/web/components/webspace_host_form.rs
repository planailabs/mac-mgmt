//! Create a webspace-host.
//!
//! A `proxy` host is created with no folders (add them afterwards, including
//! one at `/`). A `cloudflare` host provisions a Cloudflare Pages project as
//! its single main-folder.

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
struct HostFormData {
    orgs: Vec<OrgOption>,
    cloudflare_creds: Vec<CredOption>,
}

#[server]
async fn load_host_form_data() -> Result<HostFormData, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let orgs = crate::web::user::list_user_write_orgs(&user, &pool).await?;

    let cloudflare_creds = if user.is_admin {
        sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, name FROM credentials WHERE credential_type = 'cloudflare' ORDER BY name",
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    } else {
        let org_ids = user.org_ids();
        sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, name FROM credentials WHERE credential_type = 'cloudflare' \
             AND (organization_id = ANY($1) OR organization_id IS NULL) ORDER BY name",
        )
        .bind(&org_ids)
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    };

    Ok(HostFormData {
        orgs,
        cloudflare_creds: cloudflare_creds
            .into_iter()
            .map(|(id, name)| CredOption { id, name })
            .collect(),
    })
}

#[server]
async fn create_host(
    org_id: Uuid,
    name: String,
    kind: String,
    cloudflare_credential_id: Option<Uuid>,
) -> Result<Uuid, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    if kind != "proxy" && kind != "cloudflare" {
        return Err(ServerFnError::new("invalid kind"));
    }

    let host_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO webspace_hosts (organization_id, name, kind) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(org_id)
    .bind(&name)
    .bind(&kind)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(format!("failed to create host: {e}")))?;

    if kind == "cloudflare" {
        let cred_id = cloudflare_credential_id
            .ok_or_else(|| ServerFnError::new("Cloudflare credential required"))?;
        let (client, account_id) = crate::credentials::cf_client_with_account(&pool, cred_id)
            .await
            .map_err(|e| ServerFnError::new(format!("{e}")))?;
        let project = match client.get_pages_project(&account_id, &name).await {
            Ok(p) => p,
            Err(_) => client
                .create_pages_project(&account_id, &name, "main")
                .await
                .map_err(|e| ServerFnError::new(format!("failed to create Pages project: {e}")))?,
        };
        sqlx::query(
            "INSERT INTO webspaces (organization_id, webspace_host_id, name, path_prefix, hosting_type, cloudflare_pages_project, cloudflare_pages_project_id, cloudflare_credential_id) \
             VALUES ($1, $2, $3, '/', 'cloudflare_pages', $4, $5, $6)",
        )
        .bind(org_id).bind(host_id).bind(&name).bind(&name).bind(&project.id).bind(cred_id)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    }
    // Proxy hosts start with no folders — add them (including one at "/") afterwards.

    crate::api::internal::notify_proxy_reload();
    Ok(host_id)
}

#[component]
pub fn WebspaceHostForm() -> Element {
    let data = use_server_future(load_host_form_data)?;
    let (org_list, cf_creds) = match &*data.read() {
        Some(Ok(d)) => (d.orgs.clone(), d.cloudflare_creds.clone()),
        _ => (vec![], vec![]),
    };

    let mut name = use_signal(String::new);
    let mut kind = use_signal(|| "proxy".to_string());
    let mut org_id = use_signal(|| org_list.first().map(|o| o.id.to_string()).unwrap_or_default());
    let mut cf_cred_id =
        use_signal(|| cf_creds.first().map(|c| c.id.to_string()).unwrap_or_default());
    let mut error = use_signal(|| None::<String>);
    let mut saving = use_signal(|| false);
    let nav = use_navigator();

    rsx! {
        PageHeader { "Create Webspace Host" }

        form {
            class: "card p-6 mt-4 max-w-xl space-y-4",
            onsubmit: move |evt| {
                evt.prevent_default();
                let n = name.read().clone();
                let k = kind.read().clone();
                let oid_str = org_id.read().clone();
                let cf_cred = cf_cred_id.read().clone();
                let nav = nav.clone();
                saving.set(true);
                error.set(None);
                spawn(async move {
                    let oid = uuid::Uuid::parse_str(&oid_str).ok();
                    if let Some(oid) = oid {
                        let cf_cred_opt = if k == "cloudflare" { uuid::Uuid::parse_str(&cf_cred).ok() } else { None };
                        match create_host(oid, n, k, cf_cred_opt).await {
                            Ok(id) => { nav.push(crate::web::app::Route::WebspaceHostDetail { id: id.to_string() }); }
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
                    placeholder: "e.g. shop.example.com",
                    required: true,
                    value: "{name}",
                    oninput: move |evt| name.set(evt.value()),
                }
            }

            FormField { label: "Kind",
                select {
                    class: "input",
                    value: "{kind}",
                    oninput: move |evt| kind.set(evt.value()),
                    option { value: "proxy", "Proxy (path-mounted folders: local / relay / tunnel)" }
                    option { value: "cloudflare", "Cloudflare Pages (single project)" }
                }
            }

            if *kind.read() == "cloudflare" {
                FormField { label: "Cloudflare Credential",
                    select {
                        class: "input",
                        value: "{cf_cred_id}",
                        oninput: move |evt| cf_cred_id.set(evt.value()),
                        if cf_creds.is_empty() {
                            option { value: "", "No Cloudflare credentials — add one first" }
                        }
                        for c in &cf_creds {
                            option { value: "{c.id}", "{c.name}" }
                        }
                    }
                }
            } else {
                div { class: "text-sm text-fg-muted",
                    "No folders are created automatically. Add folders (including one at \"/\") and set runtimes afterwards."
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
                Link { to: crate::web::app::Route::WebspaceHostList {}, class: "btn btn-secondary", "Cancel" }
            }
        }
    }
}
