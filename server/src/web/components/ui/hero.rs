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
    // Stack title and right-side content vertically on phones (where the
    // 36px display title already eats most of the viewport) and put them
    // side-by-side from `sm` up. `flex-wrap` on the right cluster keeps
    // long status text from forcing a horizontal scroll.
    let cls = format!("flex flex-col sm:flex-row sm:items-end sm:justify-between gap-3 sm:gap-6 mb-1 {class}");
    rsx! {
        div { class: "{cls}",
            div { class: "min-w-0",
                if let Some(k) = kicker {
                    Kicker { class: "mb-2", "{k}" }
                }
                h1 { class: "h-display", {title} }
            }
            if let Some(r) = right {
                div { class: "flex items-center gap-3 sm:gap-4 flex-wrap shrink-0", {r} }
            }
        }
    }
}
