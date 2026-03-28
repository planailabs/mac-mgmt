use dioxus::prelude::*;

use crate::models::McpServerBundle;
use crate::web::app::Route;

#[server]
async fn create_mcp_bundle(slug: String, name: String, description: String) -> Result<McpServerBundle, ServerFnError> {
    let pool = crate::server_pool()?;
    let bundle = sqlx::query_as::<_, McpServerBundle>(
        "INSERT INTO mcp_server_bundles (slug, name, description) VALUES ($1, $2, $3) RETURNING *",
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
pub fn McpBundleForm() -> Element {
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
            match create_mcp_bundle(slug_val, name_val, desc_val).await {
                Ok(bundle) => {
                    nav.push(Route::McpBundleDetail { id: bundle.id.to_string() });
                }
                Err(e) => {
                    error.set(Some(e.to_string()));
                }
            }
        });
    };

    rsx! {
        h2 { class: "text-2xl font-bold mb-4", "New MCP Bundle" }
        if let Some(err) = &*error.read() {
            p { class: "text-red-600 mb-4", "{err}" }
        }
        form { onsubmit: on_submit,
            div { class: "mb-4",
                label { class: "block text-sm font-medium text-gray-700 mb-1", "Slug" }
                input {
                    class: "w-full border border-gray-300 rounded px-3 py-2 font-mono",
                    r#type: "text",
                    required: true,
                    placeholder: "my-mcp-bundle",
                    value: "{slug}",
                    oninput: move |evt| slug.set(evt.value()),
                }
            }
            div { class: "mb-4",
                label { class: "block text-sm font-medium text-gray-700 mb-1", "Name" }
                input {
                    class: "w-full border border-gray-300 rounded px-3 py-2",
                    r#type: "text",
                    required: true,
                    value: "{name}",
                    oninput: move |evt| name.set(evt.value()),
                }
            }
            div { class: "mb-4",
                label { class: "block text-sm font-medium text-gray-700 mb-1", "Description" }
                textarea {
                    class: "w-full border border-gray-300 rounded px-3 py-2",
                    rows: "3",
                    value: "{description}",
                    oninput: move |evt| description.set(evt.value()),
                }
            }
            button {
                class: "bg-blue-600 text-white px-4 py-2 rounded hover:bg-blue-700",
                r#type: "submit",
                "Create"
            }
        }
    }
}
