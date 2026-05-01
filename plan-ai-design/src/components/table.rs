use dioxus::prelude::*;

#[component]
pub fn Table(#[props(default, into)] class: String, children: Element) -> Element {
    rsx! {
        div { class: "overflow-x-auto",
            table { class: "table {class}", {children} }
        }
    }
}

#[component]
pub fn Thead(children: Element) -> Element {
    rsx! { thead { class: "thead", {children} } }
}

#[component]
pub fn Tbody(children: Element) -> Element {
    rsx! { tbody { class: "tbody", {children} } }
}

#[component]
pub fn Th(#[props(default, into)] class: String, children: Element) -> Element {
    rsx! { th { class: "th {class}", {children} } }
}

#[component]
pub fn Td(#[props(default, into)] class: String, children: Element) -> Element {
    rsx! { td { class: "td {class}", {children} } }
}

#[component]
pub fn TdMono(children: Element) -> Element {
    rsx! { td { class: "td-mono", {children} } }
}
