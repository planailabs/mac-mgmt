use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::anthropic::{GenerateContext, GeneratedNameDesc};
use crate::api_mcp::endpoints::skills::{BundleCreateInput, create_bundle};
use crate::web::app::Route;
use crate::web::components::generate_button::GenerateButton;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{Button, ButtonKind, ErrorText, FormField, PageHeader};

#[component]
pub fn BundleForm() -> Element {
    use_topbar(t!("nav-bundles"), None);
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
            match create_bundle(BundleCreateInput {
                slug: slug_val,
                name: name_val,
                description: desc_val,
            })
            .await
            {
                Ok(bundle) => {
                    nav.push(Route::BundleDetail {
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
        PageHeader { {t!("bundle-form-title")} }
        if let Some(err) = &*error.read() {
            ErrorText { class: "mb-4", "{err}" }
        }
        form { onsubmit: on_submit,
            FormField { label: t!("slug"),
                input {
                    class: "input font-mono",
                    r#type: "text",
                    required: true,
                    placeholder: t!("bundle-form-slug-placeholder"),
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
