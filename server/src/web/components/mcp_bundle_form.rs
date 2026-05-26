use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::anthropic::{GenerateContext, GeneratedNameDesc};
use crate::models::McpServerBundle;
use crate::web::app::Route;
use crate::web::components::generate_button::GenerateButton;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{Button, ButtonKind, ErrorText, FormField, PageHeader};
#[cfg(feature = "server")]
use crate::web::user::{WebUserExt, current_user};

#[server]
async fn create_mcp_bundle(
    slug: String,
    name: String,
    description: String,
) -> Result<McpServerBundle, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
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
    crate::api::push::notify_federation_global();
    Ok(bundle)
}

#[component]
pub fn McpBundleForm() -> Element {
    use_topbar(t!("nav-mcp-bundles"), None);
    let navigator = navigator();
    let mut slug = use_signal(String::new);
    let mut name = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);

    let on_submit = move |evt: FormEvent| {
        evt.prevent_default();
        let nav = navigator;
        let slug_val = slug.read().clone();
        let name_val = name.read().clone();
        let desc_val = description.read().clone();
        spawn(async move {
            match create_mcp_bundle(slug_val, name_val, desc_val).await {
                Ok(bundle) => {
                    nav.push(Route::McpBundleDetail {
                        id: bundle.id.to_string(),
                    });
                }
                Err(e) => {
                    error.set(Some(e.to_string()));
                }
            }
        });
    };

    rsx! {
        PageHeader { {t!("mcp-bundle-form-title")} }
        if let Some(err) = &*error.read() {
            ErrorText { class: "mb-4", "{err}" }
        }
        form { onsubmit: on_submit,
            FormField { label: t!("slug"),
                input {
                    class: "input font-mono",
                    r#type: "text",
                    required: true,
                    placeholder: t!("mcp-bundle-slug-placeholder"),
                    value: "{slug}",
                    oninput: move |evt| slug.set(evt.value()),
                }
            }
            FormField { label: t!("name"),
                input {
                    class: "input",
                    r#type: "text",
                    required: true,
                    value: "{name}",
                    oninput: move |evt| name.set(evt.value()),
                }
            }
            FormField { label: t!("description"),
                textarea {
                    class: "input",
                    rows: "3",
                    value: "{description}",
                    oninput: move |evt| description.set(evt.value()),
                }
            }
            div { class: "flex gap-3 items-center",
                Button { kind: ButtonKind::Submit, {t!("create")} }
                GenerateButton {
                    context: GenerateContext::McpBundle { slug: slug.read().clone(), items: vec![] },
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
