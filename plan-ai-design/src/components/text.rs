use dioxus::prelude::*;

/// Top-of-page heading. The `.h-page` class includes `mb-4`.
#[component]
pub fn PageHeader(
    #[props(default, into)] class: String,
    children: Element,
) -> Element {
    rsx! { h2 { class: "h-page {class}", {children} } }
}

/// Heading for a section within a page. `.h-section` includes `mb-3`.
#[component]
pub fn SectionHeading(
    #[props(default, into)] class: String,
    children: Element,
) -> Element {
    rsx! { h3 { class: "h-section {class}", {children} } }
}

/// Muted helper text. Layout (margins) is the caller's responsibility.
#[component]
pub fn HelpText(
    #[props(default)] xs: bool,
    #[props(default, into)] class: String,
    children: Element,
) -> Element {
    let base = if xs { "help-xs" } else { "help" };
    rsx! { p { class: "{base} {class}", {children} } }
}

/// Inline error message in danger color.
#[component]
pub fn ErrorText(
    #[props(default, into)] class: String,
    children: Element,
) -> Element {
    rsx! { p { class: "err {class}", {children} } }
}

/// Inline success message in success color.
#[component]
pub fn SuccessText(
    #[props(default, into)] class: String,
    children: Element,
) -> Element {
    rsx! { p { class: "ok {class}", {children} } }
}
