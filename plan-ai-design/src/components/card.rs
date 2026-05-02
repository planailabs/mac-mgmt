use dioxus::prelude::*;

/// Standard surface for tables and grouped content.
#[component]
pub fn Card(
    #[props(default, into)] class: String,
    children: Element,
) -> Element {
    rsx! {
        div { class: "card {class}",
            {children}
        }
    }
}
