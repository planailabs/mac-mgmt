use dioxus::prelude::*;

/// Standard page hero — display title + optional right-hand actions or
/// status. The kicker slot has been retired; the breadcrumb trail
/// rendered above every page in `Layout` now plays that role uniformly.
///
/// Pages that compose richer titles (mixed weights, accent fragments)
/// pass an `Element` so the structure stays in the page that owns it.
/// The headline style itself (`.h-page`) matches `<PageHeader>` so list
/// pages and dashboards read at the same scale.
///
/// ```ignore
/// PageHero {
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
    title: Element,
    #[props(default)] right: Option<Element>,
    #[props(default, into)] class: String,
) -> Element {
    // Stack title and right-side content vertically on phones (where the
    // headline already eats most of the viewport) and put them side-by-
    // side from `sm` up. `flex-wrap` on the right cluster keeps long
    // status text from forcing a horizontal scroll.
    let cls = format!(
        "flex flex-col sm:flex-row sm:items-end sm:justify-between gap-3 sm:gap-6 mb-4 {class}"
    );
    rsx! {
        div { class: "{cls}",
            div { class: "min-w-0",
                h1 { class: "h-page mb-0", {title} }
            }
            if let Some(r) = right {
                div { class: "flex items-center gap-3 sm:gap-4 flex-wrap shrink-0", {r} }
            }
        }
    }
}
