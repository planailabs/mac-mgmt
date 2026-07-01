use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Button, ButtonKind, ButtonVariant, FormField, PageHeader};

// The add-domain endpoint lives in the shared api_mcp layer; the UI calls the
// macro-generated `add_domain` #[server] wrapper.
use crate::api_mcp::endpoints::domains::{DomainCreateInput, add_domain};
use crate::web::user::OrgOption;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredentialOption {
    id: Uuid,
    name: String,
    credential_type: String,
}

#[server]
async fn list_orgs_and_cf_creds() -> Result<(Vec<OrgOption>, Vec<CredentialOption>), ServerFnError>
{
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let orgs = crate::web::user::list_user_orgs(&user, &pool).await?;

    let creds = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT id, name, credential_type FROM credentials \
         WHERE credential_type = 'cloudflare' ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok((
        orgs,
        creds
            .into_iter()
            .map(|(id, name, credential_type)| CredentialOption {
                id,
                name,
                credential_type,
            })
            .collect(),
    ))
}

#[component]
pub fn DomainAdd() -> Element {
    let data = use_server_future(list_orgs_and_cf_creds)?;
    let (org_list, cred_list) = match &*data.read() {
        Some(Ok((o, c))) => (o.clone(), c.clone()),
        _ => (vec![], vec![]),
    };

    let mut domain_name = use_signal(String::new);
    let mut org_id = use_signal(|| {
        org_list
            .first()
            .map(|o| o.id.to_string())
            .unwrap_or_default()
    });
    let mut cf_cred_id = use_signal(String::new);
    let mut registrar_type = use_signal(|| "external".to_string());
    let mut error = use_signal(|| None::<String>);
    let mut saving = use_signal(|| false);
    let nav = use_navigator();

    rsx! {
        PageHeader { "Add Domain" }

        form {
            class: "card p-6 mt-4 max-w-xl space-y-4",
            onsubmit: move |evt| {
                evt.prevent_default();
                let dn = domain_name.read().clone();
                let oid_str = org_id.read().clone();
                let cred_str = cf_cred_id.read().clone();
                let reg = registrar_type.read().clone();
                let nav = nav.clone();
                saving.set(true);
                error.set(None);
                spawn(async move {
                    let oid = uuid::Uuid::parse_str(&oid_str).ok();
                    let cred = uuid::Uuid::parse_str(&cred_str).ok();
                    if let Some(oid) = oid {
                        match add_domain(DomainCreateInput {
                            org_id: oid,
                            domain_name: dn,
                            cf_credential_id: cred,
                            registrar_type: reg,
                        })
                        .await
                        {
                            Ok(_) => { nav.push(crate::web::app::Route::DomainList {}); }
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

            FormField { label: "Domain Name",
                input {
                    class: "input",
                    r#type: "text",
                    placeholder: "example.com",
                    required: true,
                    value: "{domain_name}",
                    oninput: move |evt| domain_name.set(evt.value()),
                }
            }

            FormField { label: "Cloudflare Credential (optional)",
                help: "Select to manage DNS via Cloudflare. Existing zones are detected automatically.",
                select {
                    class: "input",
                    value: "{cf_cred_id}",
                    oninput: move |evt| cf_cred_id.set(evt.value()),
                    option { value: "", "None" }
                    for cred in &cred_list {
                        option { value: "{cred.id}", "{cred.name}" }
                    }
                }
            }

            FormField { label: "Registrar",
                select {
                    class: "input",
                    value: "{registrar_type}",
                    oninput: move |evt| registrar_type.set(evt.value()),
                    option { value: "external", "External" }
                    option { value: "cloudflare", "Cloudflare" }
                    option { value: "spaceship", "Spaceship" }
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
                    if *saving.read() { "Adding..." } else { "Add Domain" }
                }
                Link {
                    to: crate::web::app::Route::DomainList {},
                    class: "btn btn-secondary",
                    "Cancel"
                }
            }
        }
    }
}
