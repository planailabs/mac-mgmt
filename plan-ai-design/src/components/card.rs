use dioxus::prelude::*;

#[component]
pub fn Card(#[props(default, into)] class: String, children: Element) -> Element {
    rsx! {
        div { class: "card card-pad {class}", {children} }
    }
}
