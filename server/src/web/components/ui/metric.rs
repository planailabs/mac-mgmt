//! `KpiCard` — Route-coupled drill-down KPI tile.
//!
//! The visual sibling primitives (`Kicker`, `Mono`, `StatBlock`) live
//! in the `plan-ai-design` crate. `KpiCard` stays here because its
//! `to: Option<Route>` prop is tied to the server's specific route
//! enum — exposing it from the design crate would require a generic
//! parameter on the component. Pragmatic split: visuals in the crate,
//! routing wrappers in the app.

use dioxus::prelude::*;

use plan_ai_design::{ChartColor, Sparkline};

use crate::web::app::Route;

/// 4-up KPI strip card — number + delta + sparkline. Used in Fleet
/// Overview, Command Center and other dashboards. Pass `delta_kind` to
/// color the delta (`Ok` = positive, `Muted` = neutral/negative).
///
/// When `to` is set, the whole card becomes a router link with a brand-
/// orange hover border — gives the dashboard one-click drill-down to
/// the matching detail page.
#[component]
pub fn KpiCard(
    label: String,
    value: String,
    #[props(default, into)] delta: Option<String>,
    #[props(default = ChartColor::Ok)] delta_kind: ChartColor,
    #[props(default)] data: Vec<f64>,
    #[props(default)] color: ChartColor,
    #[props(default, into)] class: String,
    /// Optional drill-down target. When set the card renders as a
    /// `Link` and lights up its border in brand orange on hover so
    /// users can tell at a glance which cards are interactive.
    #[props(default)] to: Option<Route>,
) -> Element {
    let delta_cls = delta_kind.text_class();
    let interactive = if to.is_some() {
        " hover:border-brand transition-colors cursor-pointer block"
    } else {
        ""
    };
    let cls = format!("card p-[18px] relative overflow-hidden {class}{interactive}");
    let inner = rsx! {
        div { class: "text-fg-muted text-[11.5px] font-medium mb-3", "{label}" }
        div { class: "flex items-baseline gap-2.5",
            div { class: "text-[34px] font-semibold leading-none tracking-[-0.03em] text-fg-strong",
                style: "font-feature-settings: \"tnum\";",
                "{value}"
            }
            if let Some(d) = delta {
                div { class: "text-xs font-mono {delta_cls}", "{d}" }
            }
        }
        if !data.is_empty() {
            div { class: "mt-2.5",
                Sparkline { data, color, height: 36 }
            }
        }
    };
    match to {
        Some(target) => rsx! {
            Link { to: target, class: "{cls}", {inner} }
        },
        None => rsx! {
            div { class: "{cls}", {inner} }
        },
    }
}
