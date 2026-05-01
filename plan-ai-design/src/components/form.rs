use dioxus::prelude::*;

#[component]
pub fn FormField(
    label: String,
    #[props(default, into)] help: String,
    #[props(default, into)] error: String,
    children: Element,
) -> Element {
    rsx! {
        div { class: "mb-4",
            label { class: "label", "{label}" }
            {children}
            if !help.is_empty() {
                p { class: "help", "{help}" }
            }
            if !error.is_empty() {
                p { class: "err", "{error}" }
            }
        }
    }
}
