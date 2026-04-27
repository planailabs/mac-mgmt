use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::web::app::Route;
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
    let navigator = navigator();
    let mut email = use_signal(String::new);
    let mut name = use_signal(String::new);
    let mut is_admin = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    let on_submit = move |evt: FormEvent| {
        evt.prevent_default();
        let nav = navigator.clone();
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
        h2 { class: "text-2xl font-bold mb-4", {t!("user-form-title")} }
        if let Some(err) = &*error.read() {
            p { class: "text-red-600 dark:text-red-400 mb-4", "{err}" }
        }
        form { onsubmit: on_submit,
            class: "max-w-md space-y-4",
            div {
                label { class: "block text-sm font-medium text-gray-700 dark:text-gray-200 mb-1", {t!("email")} }
                input {
                    class: "w-full border border-gray-300 dark:border-gray-600 rounded px-3 py-2 dark:bg-gray-700 dark:text-white",
                    r#type: "email",
                    required: true,
                    value: "{email}",
                    oninput: move |evt| email.set(evt.value()),
                    placeholder: t!("user-form-email-placeholder"),
                    autofocus: true,
                }
            }
            div {
                label { class: "block text-sm font-medium text-gray-700 dark:text-gray-200 mb-1", {t!("name")} }
                input {
                    class: "w-full border border-gray-300 dark:border-gray-600 rounded px-3 py-2 dark:bg-gray-700 dark:text-white",
                    r#type: "text",
                    value: "{name}",
                    oninput: move |evt| name.set(evt.value()),
                    placeholder: t!("user-form-name-placeholder"),
                }
            }
            div { class: "flex items-center gap-2",
                input {
                    r#type: "checkbox",
                    checked: *is_admin.read(),
                    class: "h-4 w-4 rounded border-gray-300 text-blue-600 focus:ring-blue-500",
                    onchange: move |e: Event<FormData>| is_admin.set(e.checked()),
                }
                label { class: "text-sm font-medium text-gray-700 dark:text-gray-200", {t!("admin")} }
            }
            button {
                class: "bg-blue-600 text-white px-4 py-2 rounded hover:bg-blue-700",
                r#type: "submit",
                {t!("create")}
            }
        }
    }
}
