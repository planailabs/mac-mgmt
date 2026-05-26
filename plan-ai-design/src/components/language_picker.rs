//! Language picker — dropdown to switch between locales.
//!
//! Persists to `localStorage['lang']`. Consuming apps should restore
//! the selection on startup via `use_effect` + `i18n.set_language(...)`.

use dioxus::prelude::*;
use dioxus_i18n::{prelude::*, t, unic_langid::langid};

pub const LOCALES: &[(&str, &str)] = &[("en-US", "English"), ("de-DE", "Deutsch")];

#[component]
pub fn LanguagePicker() -> Element {
    let mut i18n = i18n();
    let current = i18n.language();
    let current_tag = current.to_string();

    let on_change = move |evt: Event<FormData>| {
        let val = evt.value();
        if val == "de-DE" {
            let _ = i18n.set_language(langid!("de-DE"));
        } else {
            let _ = i18n.set_language(langid!("en-US"));
        }
        document::eval(&format!(
            "try {{ localStorage.setItem('lang', '{}'); }} catch(e) {{}}",
            val
        ));
    };

    rsx! {
        div { class: "relative",
            label { class: "sr-only", r#for: "lang-picker", {t!("language-picker-label")} }
            select {
                id: "lang-picker",
                class: "appearance-none bg-transparent text-fg-muted hover:text-fg-strong text-sm rounded-md px-2 py-1.5 pr-6 cursor-pointer focus:outline-none focus:ring-2 focus:ring-info transition-colors",
                value: "{current_tag}",
                onchange: on_change,
                for &(tag, label) in LOCALES.iter() {
                    option {
                        key: "{tag}",
                        value: "{tag}",
                        selected: tag == current_tag,
                        "{label}"
                    }
                }
            }
            svg {
                class: "pointer-events-none absolute right-1 top-1/2 -translate-y-1/2 h-3 w-3 text-fg-faint",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                view_box: "0 0 24 24",
                path { stroke_linecap: "round", stroke_linejoin: "round", d: "M19 9l-7 7-7-7" }
            }
        }
    }
}
