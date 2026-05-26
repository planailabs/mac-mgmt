use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::web::app::Route;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{Button, ButtonKind, ErrorText, FormField, PageHeader};

#[server]
async fn create_organization(name: String) -> Result<String, ServerFnError> {
    use crate::web::user::{WebUserExt, current_user};
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(ServerFnError::new("Name is required"));
    }

    let id: uuid::Uuid =
        sqlx::query_scalar("INSERT INTO organizations (name) VALUES ($1) RETURNING id")
            .bind(&name)
            .fetch_one(&pool)
            .await
            .map_err(|e| {
                // Postgres unique_violation code is 23505; surface a friendly
                // message instead of the raw constraint error.
                if let sqlx::Error::Database(db_err) = &e {
                    if db_err.code().as_deref() == Some("23505") {
                        return ServerFnError::new(format!(
                            "An organization named '{name}' already exists"
                        ));
                    }
                }
                ServerFnError::new(e.to_string())
            })?;

    Ok(id.to_string())
}

#[component]
pub fn OrganizationForm() -> Element {
    use_topbar(t!("nav-organizations"), None);
    let mut name = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let nav = navigator();

    let submit = move |e: Event<FormData>| {
        e.prevent_default();
        let name_val = name.read().clone();
        if name_val.trim().is_empty() {
            error.set(Some(t!("org-form-name-required")));
            return;
        }
        spawn(async move {
            match create_organization(name_val).await {
                Ok(id) => {
                    nav.push(Route::OrganizationDetail { id });
                }
                Err(e) => {
                    error.set(Some(e.to_string()));
                }
            }
        });
    };

    rsx! {
        PageHeader { {t!("org-form-title")} }
        form { onsubmit: submit, class: "max-w-md",
            if let Some(err) = &*error.read() {
                ErrorText { class: "mb-4", "{err}" }
            }
            FormField { label: t!("name"),
                input {
                    class: "input",
                    r#type: "text",
                    value: "{name}",
                    oninput: move |e| name.set(e.value()),
                    placeholder: t!("org-form-name-placeholder"),
                    autofocus: true,
                }
            }
            Button { kind: ButtonKind::Submit, {t!("create")} }
        }
    }
}
