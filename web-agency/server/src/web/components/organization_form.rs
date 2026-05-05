use dioxus::prelude::*;

use super::ui::{Button, ButtonKind, ButtonVariant, FormField, PageHeader};

#[server]
async fn create_organization(name: String) -> Result<uuid::Uuid, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    let id = sqlx::query_scalar::<_, uuid::Uuid>(
        "INSERT INTO organizations (name) VALUES ($1) RETURNING id",
    )
    .bind(&name)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(format!("failed to create organization: {e}")))?;

    Ok(id)
}

#[component]
pub fn OrganizationForm() -> Element {
    let mut name = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut saving = use_signal(|| false);
    let nav = use_navigator();

    rsx! {
        PageHeader { "Create Organization" }

        form {
            class: "card p-6 mt-4 max-w-md space-y-4",
            onsubmit: move |evt| {
                evt.prevent_default();
                let n = name.read().clone();
                let nav = nav.clone();
                saving.set(true);
                error.set(None);
                spawn(async move {
                    match create_organization(n).await {
                        Ok(_) => { nav.push(crate::web::app::Route::OrganizationList {}); }
                        Err(e) => error.set(Some(format!("{e}"))),
                    }
                    saving.set(false);
                });
            },

            FormField { label: "Name",
                input { class: "input", r#type: "text", required: true, placeholder: "e.g. Acme Corp",
                    value: "{name}", oninput: move |evt| name.set(evt.value()) }
            }

            if let Some(err) = &*error.read() {
                div { class: "text-red-400 text-sm", "{err}" }
            }

            div { class: "flex gap-3",
                Button { variant: ButtonVariant::Primary, kind: ButtonKind::Submit, disabled: *saving.read(),
                    if *saving.read() { "Creating..." } else { "Create" } }
                Link { to: crate::web::app::Route::OrganizationList {}, class: "btn btn-secondary", "Cancel" }
            }
        }
    }
}
