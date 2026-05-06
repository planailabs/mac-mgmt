use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Button, ButtonKind, ButtonVariant, FormField, PageHeader};

use crate::web::user::OrgOption;

#[server]
async fn list_user_orgs_for_ws() -> Result<Vec<OrgOption>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    crate::web::user::list_user_write_orgs(&user, &pool).await
}

#[server]
async fn create_webspace(
    org_id: Uuid,
    name: String,
    hosting_type: String,
    runtime: Option<String>,
) -> Result<Uuid, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    use crate::web::user::WebUserExt;
    user.require_org_write(&org_id)?;

    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO webspaces (organization_id, name, hosting_type, runtime) \
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(org_id)
    .bind(&name)
    .bind(&hosting_type)
    .bind(&runtime)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(format!("failed to create webspace: {e}")))?;

    Ok(id)
}

#[component]
pub fn WebspaceForm() -> Element {
    let orgs = use_server_future(list_user_orgs_for_ws)?;
    let org_list = match &*orgs.read() {
        Some(Ok(o)) => o.clone(),
        _ => vec![],
    };

    let mut name = use_signal(String::new);
    let mut hosting_type = use_signal(|| "local".to_string());
    let mut runtime = use_signal(|| "static".to_string());
    let mut org_id = use_signal(|| org_list.first().map(|o| o.id.to_string()).unwrap_or_default());
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
                let nav = nav.clone();
                saving.set(true);
                error.set(None);
                spawn(async move {
                    let oid = uuid::Uuid::parse_str(&oid_str).ok();
                    if let Some(oid) = oid {
                        let rt_opt = if ht == "local" { Some(rt) } else { None };
                        match create_webspace(oid, n, ht, rt_opt).await {
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
                    option { value: "local", "Local (nginx + ACME)" }
                    option { value: "cloudflare_pages", "Cloudflare Pages" }
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
