use dioxus::prelude::*;
use uuid::Uuid;

use super::ui::{Button, ButtonKind, ButtonVariant, FormField, PageHeader};
use crate::web::user::OrgOption;

// create_basic_auth_list now lives in the shared api_mcp layer.
use crate::api_mcp::endpoints::basic_auth::{BasicAuthCreateInput, create_basic_auth_list};

#[server]
async fn load_orgs() -> Result<Vec<OrgOption>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    crate::web::user::list_user_write_orgs(&user, &pool).await
}

#[component]
pub fn BasicAuthForm() -> Element {
    let orgs = use_server_future(load_orgs)?;
    let org_list = match &*orgs.read() {
        Some(Ok(o)) => o.clone(),
        _ => vec![],
    };

    let mut name = use_signal(String::new);
    let mut org_id = use_signal(|| {
        org_list
            .first()
            .map(|o| o.id.to_string())
            .unwrap_or_default()
    });
    let mut error = use_signal(|| None::<String>);
    let mut saving = use_signal(|| false);
    let nav = use_navigator();

    rsx! {
        PageHeader { "Create Basic Auth List" }

        form {
            class: "card p-6 mt-4 max-w-xl space-y-4",
            onsubmit: move |evt| {
                evt.prevent_default();
                let n = name.read().clone();
                let oid_str = org_id.read().clone();
                let nav = nav.clone();
                saving.set(true);
                error.set(None);
                spawn(async move {
                    if let Some(oid) = Uuid::parse_str(&oid_str).ok() {
                        match create_basic_auth_list(BasicAuthCreateInput { organization_id: oid, name: n }).await {
                            Ok(id) => { nav.push(crate::web::app::Route::BasicAuthDetail { id: id.to_string() }); }
                            Err(e) => error.set(Some(format!("{e}"))),
                        }
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
                    placeholder: "e.g. team-access",
                    required: true,
                    value: "{name}",
                    oninput: move |evt| name.set(evt.value()),
                }
            }

            if let Some(err) = &*error.read() {
                div { class: "text-danger text-sm", "{err}" }
            }

            div { class: "flex gap-3",
                Button {
                    variant: ButtonVariant::Primary,
                    kind: ButtonKind::Submit,
                    disabled: *saving.read(),
                    if *saving.read() { "Creating..." } else { "Create" }
                }
                Link {
                    to: crate::web::app::Route::BasicAuthList {},
                    class: "btn btn-secondary",
                    "Cancel"
                }
            }
        }
    }
}
