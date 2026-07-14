use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api_mcp::endpoints::organizations::{OrgCreateInput, create_organization};
use crate::web::app::Route;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{Button, ButtonKind, ErrorText, FormField, PageHeader};

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
            match create_organization(OrgCreateInput { name: name_val }).await {
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
