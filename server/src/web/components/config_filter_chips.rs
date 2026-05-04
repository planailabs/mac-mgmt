//! "All / Enabled / Modified / Errors" pill row above the editor.
//!
//! Reads counts from the `EditorStats` context that `StructuredEditor`
//! publishes on every render and writes the active filter back into
//! its parent's `Signal<SectionFilter>` so the editor can prune which
//! section cards render.
use dioxus::prelude::*;
use dioxus_i18n::t;

use super::config_editor::{EditorStats, SectionFilter};

#[component]
pub fn FilterChips(filter: Signal<SectionFilter>) -> Element {
    let cur = *filter.read();
    let stats = use_context::<EditorStats>();
    let total = *stats.total_sections.read();
    let enabled = *stats.enabled_sections.read();
    let modified = *stats.modified_sections.read();
    let errors = *stats.error_sections.read();
    let chip = |opt: SectionFilter, label_key: &'static str, count: usize| {
        let active = cur == opt;
        let cls = if active {
            "config-chip config-chip-active"
        } else {
            "config-chip"
        };
        // The count badge inside each chip mirrors the design's
        // "All 17 / Enabled 12 / Modified 3 / Errors 0" treatment.
        // Active chips render the count on a translucent dark pill so
        // it stays legible on the brand-orange background.
        let count_cls = if active {
            "ml-1.5 px-1.5 py-px text-[10px] font-mono rounded-full bg-black/25"
        } else {
            "ml-1.5 px-1.5 py-px text-[10px] font-mono rounded-full bg-surface-3 text-fg-muted"
        };
        rsx! {
            button {
                class: "{cls}",
                onclick: move |_| filter.set(opt),
                {t!(label_key)}
                span { class: "{count_cls}", "{count}" }
            }
        }
    };
    rsx! {
        div { class: "config-chips",
            {chip(SectionFilter::All, "config-filter-all", total)}
            {chip(SectionFilter::Enabled, "config-filter-enabled", enabled)}
            {chip(SectionFilter::Modified, "config-filter-modified", modified)}
            {chip(SectionFilter::Errors, "config-filter-errors", errors)}
        }
    }
}
