use dioxus::prelude::*;

use super::pill::{Dot, PillVariant};

#[derive(Clone, PartialEq)]
pub struct ActivityItem {
    pub kind: PillVariant,
    pub text: String,
    pub time: String,
}

/// Live-activity feed used by Command Center and similar pages —
/// vertical list of `{kind, text, time}` rows with a leading colored
/// dot and trailing mono timestamp. Hairline-divided.
#[component]
pub fn ActivityFeed(items: Vec<ActivityItem>) -> Element {
    let last = items.len().saturating_sub(1);
    rsx! {
        div { class: "card overflow-hidden",
            for (i, item) in items.iter().cloned().enumerate() {
                {
                    let border = if i < last { "border-b border-line" } else { "" };
                    rsx! {
                        div {
                            key: "{i}",
                            class: "flex items-center gap-3 px-[22px] py-2.5 {border}",
                            Dot { variant: item.kind }
                            span { class: "flex-1 text-[12.5px] text-fg", "{item.text}" }
                            span { class: "font-mono text-[11px] text-fg-faint", "{item.time}" }
                        }
                    }
                }
            }
        }
    }
}
