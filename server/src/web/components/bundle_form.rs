use dioxus::prelude::*;

use crate::anthropic::{GenerateContext, GeneratedNameDesc};
use crate::models::Bundle;
use crate::web::app::Route;
use crate::web::components::generate_button::GenerateButton;

#[server]
async fn create_bundle(slug: String, name: String, description: String) -> Result<Bundle, ServerFnError> {
    let pool = crate::server_pool()?;
    let bundle = sqlx::query_as::<_, Bundle>(
        "INSERT INTO bundles (slug, name, description) VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(&slug)
    .bind(&name)
    .bind(&description)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(bundle)
}

#[component]
pub fn BundleForm() -> Element {
    let navigator = navigator();
    let mut slug = use_signal(String::new);
    let mut name = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);

    let on_submit = move |evt: FormEvent| {
        evt.prevent_default();
        let nav = navigator.clone();
        let slug_val = slug.read().clone();
        let name_val = name.read().clone();
        let desc_val = description.read().clone();
        spawn(async move {
            match create_bundle(slug_val, name_val, desc_val).await {
                Ok(bundle) => {
                    nav.push(Route::BundleDetail { id: bundle.id.to_string() });
                }
                Err(e) => {
                    error.set(Some(e.to_string()));
                }
            }
        });
    };

    rsx! {
        h2 { class: "text-2xl font-bold mb-4", "New Bundle" }
        if let Some(err) = &*error.read() {
            p { class: "text-red-600 dark:text-red-400 mb-4", "{err}" }
        }
        form { onsubmit: on_submit,
            div { class: "mb-4",
                label { class: "block text-sm font-medium text-gray-700 dark:text-gray-200 mb-1", "Slug" }
                input {
                    class: "w-full border border-gray-300 dark:border-gray-600 rounded px-3 py-2 font-mono dark:bg-gray-700 dark:text-white",
                    r#type: "text",
                    required: true,
                    placeholder: "my-bundle",
                    value: "{slug}",
                    oninput: move |evt| slug.set(evt.value()),
                }
            }
            div { class: "mb-4",
                label { class: "block text-sm font-medium text-gray-700 dark:text-gray-200 mb-1", "Name" }
                input {
                    class: "w-full border border-gray-300 dark:border-gray-600 rounded px-3 py-2 dark:bg-gray-700 dark:text-white",
                    r#type: "text",
                    required: true,
                    value: "{name}",
                    oninput: move |evt| name.set(evt.value()),
                }
            }
            div { class: "mb-4",
                label { class: "block text-sm font-medium text-gray-700 dark:text-gray-200 mb-1", "Description" }
                textarea {
                    class: "w-full border border-gray-300 dark:border-gray-600 rounded px-3 py-2 dark:bg-gray-700 dark:text-white",
                    rows: "3",
                    value: "{description}",
                    oninput: move |evt| description.set(evt.value()),
                }
            }
            div { class: "flex gap-3 items-center",
                button {
                    class: "bg-blue-600 text-white px-4 py-2 rounded hover:bg-blue-700",
                    r#type: "submit",
                    "Create"
                }
                GenerateButton {
                    context: GenerateContext::Bundle { slug: slug.read().clone(), items: vec![] },
                    current_name: name.read().clone(),
                    current_desc: description.read().clone(),
                    on_generated: move |result: GeneratedNameDesc| {
                        name.set(result.name);
                        description.set(result.description);
                    },
                }
            }
        }
    }
}
