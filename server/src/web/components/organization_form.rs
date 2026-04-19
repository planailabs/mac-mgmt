use dioxus::prelude::*;

use crate::web::app::Route;

#[server]
async fn create_organization(name: String) -> Result<String, ServerFnError> {
    use crate::web::user::current_user;
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
    let mut name = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let nav = navigator();

    let submit = move |e: Event<FormData>| {
        e.prevent_default();
        let name_val = name.read().clone();
        if name_val.trim().is_empty() {
            error.set(Some("Name is required".to_string()));
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
        h2 { class: "text-2xl font-bold mb-4", "New Organization" }
        form { onsubmit: submit,
            class: "max-w-md space-y-4",
            if let Some(err) = &*error.read() {
                p { class: "text-red-600 text-sm", "{err}" }
            }
            div {
                label { class: "block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1", "Name" }
                input {
                    r#type: "text",
                    value: "{name}",
                    oninput: move |e| name.set(e.value()),
                    class: "w-full border border-gray-300 dark:border-gray-600 dark:bg-gray-700 dark:text-white rounded px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500",
                    placeholder: "Organization name",
                    autofocus: true,
                }
            }
            button {
                r#type: "submit",
                class: "bg-blue-600 text-white px-4 py-2 rounded text-sm hover:bg-blue-700",
                "Create"
            }
        }
    }
}
