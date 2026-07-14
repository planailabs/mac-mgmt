use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api_mcp::endpoints::skill_centers::{SkillCenterCreateInput, create_skill_center};
use crate::web::app::Route;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{Button, ButtonKind, ErrorText, FormField, HelpText, PageHeader};

#[component]
pub fn SkillCenterForm() -> Element {
    use_topbar(t!("nav-skill-centers"), None);
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
            match create_skill_center(SkillCenterCreateInput {
                name: name_val,
                url: url_val,
                federation_token: token_val,
                priority: priority_val,
                enabled: enabled_val,
            })
            .await
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
            PageHeader { {t!("skill-center-form-title")} }
            form { onsubmit: submit,
                if let Some(err) = &*error.read() {
                    ErrorText { class: "mb-4", "{err}" }
                }
                FormField { label: t!("name"),
                    input {
                        class: "input",
                        r#type: "text",
                        value: "{name}",
                        oninput: move |e| name.set(e.value()),
                        placeholder: t!("skill-center-form-name-placeholder"),
                        autofocus: true,
                    }
                }
                FormField { label: t!("skill-center-form-url-label"),
                    input {
                        class: "input",
                        r#type: "text",
                        value: "{url}",
                        oninput: move |e| url.set(e.value()),
                        placeholder: t!("skill-center-form-url-placeholder"),
                    }
                }
                FormField { label: t!("skill-center-form-token-label"),
                    input {
                        class: "input",
                        r#type: "password",
                        value: "{federation_token}",
                        oninput: move |e| federation_token.set(e.value()),
                        placeholder: t!("skill-center-form-token-placeholder"),
                    }
                }
                FormField { label: t!("skill-center-form-priority-label"),
                    input {
                        class: "input",
                        r#type: "number",
                        value: "{priority}",
                        oninput: move |e| priority.set(e.value()),
                    }
                    HelpText { xs: true, class: "mt-1", {t!("skill-center-form-priority-help")} }
                }
                div { class: "flex items-center gap-2 mb-4",
                    input {
                        r#type: "checkbox",
                        checked: "{enabled}",
                        oninput: move |e| enabled.set(e.value() == "true"),
                        class: "rounded border-line",
                        id: "enabled-checkbox",
                    }
                    label { r#for: "enabled-checkbox", class: "text-sm text-fg",
                        {t!("skill-center-form-enabled")}
                    }
                }
                Button { kind: ButtonKind::Submit, {t!("create")} }
            }
        }
    }
}
