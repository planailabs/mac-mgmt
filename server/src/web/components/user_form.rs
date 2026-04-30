use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::web::app::Route;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{Button, ButtonKind, ErrorText, FormField, PageHeader};
#[cfg(feature = "server")]
use crate::web::user::current_user;

#[server]
async fn create_user(email: String, name: String, is_admin: bool) -> Result<String, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    let id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, name, is_admin) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(&email)
    .bind(&name)
    .bind(is_admin)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(id.to_string())
}

#[component]
pub fn UserForm() -> Element {
    use_topbar(t!("nav-users"), None);
    let navigator = navigator();
    let mut email = use_signal(String::new);
    let mut name = use_signal(String::new);
    let mut is_admin = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    let on_submit = move |evt: FormEvent| {
        evt.prevent_default();
        let nav = navigator;
        let email_val = email.read().clone();
        let name_val = name.read().clone();
        let admin_val = *is_admin.read();
        spawn(async move {
            match create_user(email_val, name_val, admin_val).await {
                Ok(id) => {
                    nav.push(Route::UserDetail { id });
                }
                Err(e) => {
                    error.set(Some(e.to_string()));
                }
            }
        });
    };

    rsx! {
        PageHeader { {t!("user-form-title")} }
        if let Some(err) = &*error.read() {
            ErrorText { class: "mb-4", "{err}" }
        }
        form { onsubmit: on_submit, class: "max-w-md",
            FormField { label: t!("email"),
                input {
                    class: "input",
                    r#type: "email",
                    required: true,
                    value: "{email}",
                    oninput: move |evt| email.set(evt.value()),
                    placeholder: t!("user-form-email-placeholder"),
                    autofocus: true,
                }
            }
            FormField { label: t!("name"),
                input {
                    class: "input",
                    r#type: "text",
                    value: "{name}",
                    oninput: move |evt| name.set(evt.value()),
                    placeholder: t!("user-form-name-placeholder"),
                }
            }
            div { class: "flex items-center gap-2 mb-4",
                input {
                    r#type: "checkbox",
                    checked: *is_admin.read(),
                    class: "h-4 w-4 rounded border-line text-brand focus:ring-brand",
                    onchange: move |e: Event<FormData>| is_admin.set(e.checked()),
                }
                label { class: "text-sm font-medium text-fg-strong", {t!("admin")} }
            }
            Button { kind: ButtonKind::Submit, {t!("create")} }
        }
    }
}
