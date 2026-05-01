use dioxus::prelude::*;

#[component]
pub fn PageHeader(children: Element) -> Element {
    rsx! { h1 { class: "h-page", {children} } }
}

#[component]
pub fn SectionHeading(children: Element) -> Element {
    rsx! { h2 { class: "h-section", {children} } }
}

#[component]
pub fn HelpText(children: Element) -> Element {
    rsx! { p { class: "help", {children} } }
}

#[component]
pub fn ErrorText(children: Element) -> Element {
    rsx! { p { class: "err", {children} } }
}
