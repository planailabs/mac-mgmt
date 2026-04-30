use dioxus::prelude::*;

use super::metric::Kicker;

/// Standard page hero — kicker label + display title + optional right-
/// hand actions or status. Used at the top of Fleet, Cluster Detail,
/// Healer, Rollouts. Replaces the simpler `.h-page` h2 for the new
/// design; lower-priority pages keep `<PageHeader>` until the long-
/// tail sweep migrates them.
///
/// Composition pattern (title + accented fragment + dim connective):
/// ```ignore
/// PageHero {
///     kicker: "FLEET DASHBOARD",
///     title: rsx! {
///         "47 instances "
///         span { class: "text-fg-muted", "healthy across" }
///         " 14 clusters"
///     },
///     right: rsx! { ... live indicator ... },
/// }
/// ```
#[component]
pub fn PageHero(
    #[props(default, into)] kicker: Option<String>,
    title: Element,
    #[props(default)] right: Option<Element>,
    #[props(default, into)] class: String,
) -> Element {
    let cls = format!("flex items-end justify-between gap-6 mb-1 {class}");
    rsx! {
        div { class: "{cls}",
            div {
                if let Some(k) = kicker {
                    Kicker { class: "mb-2", "{k}" }
                }
                h1 { class: "h-display", {title} }
            }
            if let Some(r) = right {
                div { class: "flex items-center gap-4 shrink-0", {r} }
            }
        }
    }
}
