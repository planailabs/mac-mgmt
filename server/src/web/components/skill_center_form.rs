use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::web::app::Route;

#[server]
async fn create_skill_center(
    name: String,
    url: String,
    federation_token: String,
    priority: i32,
    enabled: bool,
) -> Result<String, ServerFnError> {
    use crate::web::user::current_user;
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(ServerFnError::new("name-required"));
    }
    let url = url.trim().to_string();
    if url.is_empty() {
        return Err(ServerFnError::new("url-required"));
    }
    let federation_token = federation_token.trim().to_string();
    if federation_token.is_empty() {
        return Err(ServerFnError::new("token-required"));
    }

    let id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO skill_centers (name, url, federation_token, priority, enabled) \
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(&name)
    .bind(&url)
    .bind(&federation_token)
    .bind(priority)
    .bind(enabled)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(db_err) = &e {
            if db_err.code().as_deref() == Some("23505") {
                return ServerFnError::new(format!(
                    "A skill center with URL '{url}' already exists"
                ));
            }
        }
        ServerFnError::new(e.to_string())
    })?;

    Ok(id.to_string())
}

#[component]
pub fn SkillCenterForm() -> Element {
    let mut name = use_signal(String::new);
    let mut url = use_signal(String::new);
    let mut federation_token = use_signal(String::new);
    let mut priority = use_signal(|| "0".to_string());
    let mut enabled = use_signal(|| true);
    let mut error = use_signal(|| None::<String>);
    let nav = navigator();

    let submit = move |e: Event<FormData>| {
        e.prevent_default();
        let name_val = name.read().clone();
        let url_val = url.read().clone();
        let token_val = federation_token.read().clone();
        let priority_val: i32 = priority.read().parse().unwrap_or(0);
        let enabled_val = *enabled.read();

        if name_val.trim().is_empty() {
            error.set(Some(t!("skill-center-form-name-required")));
            return;
        }
        if url_val.trim().is_empty() {
            error.set(Some(t!("skill-center-form-url-required")));
            return;
        }
        if token_val.trim().is_empty() {
            error.set(Some(t!("skill-center-form-token-required")));
            return;
        }

        spawn(async move {
            match create_skill_center(name_val, url_val, token_val, priority_val, enabled_val).await
            {
                Ok(id) => {
                    nav.push(Route::SkillCenterDetail { id });
                }
                Err(e) => {
                    error.set(Some(e.to_string()));
                }
            }
        });
    };

    rsx! {
        div { class: "px-6 py-8 max-w-lg mx-auto",
            h2 { class: "text-2xl font-bold mb-4 dark:text-white", {t!("skill-center-form-title")} }
            form { onsubmit: submit,
                class: "space-y-4",
                if let Some(err) = &*error.read() {
                    p { class: "text-red-600 text-sm", "{err}" }
                }
                div {
                    label { class: "block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1", {t!("name")} }
                    input {
                        r#type: "text",
                        value: "{name}",
                        oninput: move |e| name.set(e.value()),
                        class: "w-full border border-gray-300 dark:border-gray-600 dark:bg-gray-700 dark:text-white rounded px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500",
                        placeholder: t!("skill-center-form-name-placeholder"),
                        autofocus: true,
                    }
                }
                div {
                    label { class: "block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1", {t!("skill-center-form-url-label")} }
                    input {
                        r#type: "text",
                        value: "{url}",
                        oninput: move |e| url.set(e.value()),
                        class: "w-full border border-gray-300 dark:border-gray-600 dark:bg-gray-700 dark:text-white rounded px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500",
                        placeholder: t!("skill-center-form-url-placeholder"),
                    }
                }
                div {
                    label { class: "block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1", {t!("skill-center-form-token-label")} }
                    input {
                        r#type: "password",
                        value: "{federation_token}",
                        oninput: move |e| federation_token.set(e.value()),
                        class: "w-full border border-gray-300 dark:border-gray-600 dark:bg-gray-700 dark:text-white rounded px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500",
                        placeholder: t!("skill-center-form-token-placeholder"),
                    }
                }
                div {
                    label { class: "block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1", {t!("skill-center-form-priority-label")} }
                    input {
                        r#type: "number",
                        value: "{priority}",
                        oninput: move |e| priority.set(e.value()),
                        class: "w-full border border-gray-300 dark:border-gray-600 dark:bg-gray-700 dark:text-white rounded px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500",
                    }
                    p { class: "text-xs text-gray-500 dark:text-gray-400 mt-1", {t!("skill-center-form-priority-help")} }
                }
                div { class: "flex items-center gap-2",
                    input {
                        r#type: "checkbox",
                        checked: "{enabled}",
                        oninput: move |e| enabled.set(e.value() == "true"),
                        class: "rounded border-gray-300 dark:border-gray-600",
                        id: "enabled-checkbox",
                    }
                    label { r#for: "enabled-checkbox", class: "text-sm text-gray-700 dark:text-gray-300", {t!("skill-center-form-enabled")} }
                }
                button {
                    r#type: "submit",
                    class: "bg-blue-600 text-white px-4 py-2 rounded text-sm hover:bg-blue-700",
                    {t!("create")}
                }
            }
        }
    }
}
