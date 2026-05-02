use dioxus::prelude::*;

/// Wraps a label + slot for an arbitrary input element. The slot lets
/// callers use any input type (text, number, checkbox, custom widgets)
/// without the form scaffolding caring.
///
/// ```ignore
/// FormField { label: t!("name"),
///     input { class: "input", r#type: "text", value: "{name}",
///             oninput: move |e| name.set(e.value()) }
/// }
/// ```
#[component]
pub fn FormField(
    #[props(into)] label: String,
    #[props(default, into)] help: Option<String>,
    #[props(default, into)] class: String,
    children: Element,
) -> Element {
    rsx! {
        div { class: "mb-4 {class}",
            label { class: "label", "{label}" }
            {children}
            if let Some(text) = help {
                p { class: "help-xs mt-1", "{text}" }
            }
        }
    }
}
