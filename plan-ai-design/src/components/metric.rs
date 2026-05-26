use dioxus::prelude::*;

use super::chart::{ChartColor, Sparkline};

/// Uppercase tertiary label that sits above page hero titles and card
/// titles. Visual is owned by `.kicker`.
#[component]
pub fn Kicker(#[props(default, into)] class: String, children: Element) -> Element {
    rsx! { div { class: "kicker {class}", {children} } }
}

/// Wraps text in monospace for "facts" — hostnames, versions, hashes,
/// durations, timestamps, byte counts (principle 4). Tabular-nums is
/// applied via the global `.font-mono` rule so figures align.
#[component]
pub fn Mono(#[props(default, into)] class: String, children: Element) -> Element {
    rsx! { span { class: "font-mono {class}", {children} } }
}

/// Small per-metric tile used inside Cluster Detail's "Live system"
/// 2×2 grid (CPU / Memory / Net RX / Net TX). Smaller than KpiCard
/// — designed for in-card use rather than top-of-page.
#[component]
pub fn StatBlock(
    label: String,
    value: String,
    #[props(default, into)] sub: Option<String>,
    #[props(default)] data: Vec<f64>,
    #[props(default)] color: ChartColor,
) -> Element {
    rsx! {
        div { class: "bg-surface-2 border border-line rounded-lg p-3.5",
            div { class: "text-fg-muted text-[11px] mb-1", "{label}" }
            div { class: "flex items-baseline gap-1.5 mb-2",
                div { class: "text-[22px] font-semibold text-fg-strong tracking-[-0.02em]",
                    style: "font-feature-settings: \"tnum\";",
                    "{value}"
                }
                if let Some(s) = sub {
                    div { class: "text-[10px] text-fg-faint font-mono", "{s}" }
                }
            }
            if !data.is_empty() {
                Sparkline { data, color, height: 26 }
            }
        }
    }
}
