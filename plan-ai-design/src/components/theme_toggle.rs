//! Dark mode toggle — cycles system → light → dark → system.
//!
//! Persists to `localStorage['theme']` and toggles the `.dark` class
//! on `<html>`. CSS variables in `input.css` do the rest.

use dioxus::prelude::*;
use dioxus_i18n::t;

#[derive(Clone, Copy, PartialEq)]
pub enum ThemeMode {
    System,
    Light,
    Dark,
}

impl ThemeMode {
    fn next(self) -> Self {
        match self {
            Self::System => Self::Light,
            Self::Light => Self::Dark,
            Self::Dark => Self::System,
        }
    }
}

/// Theme init script — sets `.dark` class on `<html>` before CSS loads.
/// Place in a `script { dangerous_inner_html: THEME_INIT_SCRIPT }` BEFORE
/// the CSS stylesheet link to prevent theme flash.
pub const THEME_INIT_SCRIPT: &str = r#"
(function(){
    try {
        var d = document.documentElement;
        var t = localStorage.getItem('theme');
        var dark = t === 'dark' || (!t && window.matchMedia('(prefers-color-scheme: dark)').matches);
        d.classList.toggle('dark', dark);
        d.style.colorScheme = dark ? 'dark' : 'light';
    } catch(e){}
})();
"#;

/// Pre-hydration WASM loading banner with self-contained styling.
/// Wrap in `div { id: "wasm-loading", style: WASM_LOADING_STYLE, dangerous_inner_html: WASM_LOADING_INNER }`
/// and remove with `use_effect(|| { document::eval("document.getElementById('wasm-loading')?.remove();"); });`
pub const WASM_LOADING_INNER: &str = r#"<style>@media(prefers-color-scheme:dark){#wasm-loading{background:#1a1f2e!important;color:#7b9fe0!important;border-bottom-color:#2a3040!important}}#wasm-loading svg{animation:wasm-spin 1s linear infinite;width:16px;height:16px}@keyframes wasm-spin{from{transform:rotate(0deg)}to{transform:rotate(360deg)}}</style><svg viewBox="0 0 24 24" fill="none"><circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" opacity="0.25"/><path d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" fill="currentColor" opacity="0.75"/></svg>Loading&hellip;"#;

pub const WASM_LOADING_STYLE: &str = "position:fixed;top:0;left:0;right:0;display:flex;align-items:center;justify-content:center;gap:8px;padding:10px;background:#f0f4ff;color:#3b5998;font-family:system-ui,-apple-system,sans-serif;font-size:13px;z-index:9999;border-bottom:1px solid #d0d8e8";

#[component]
pub fn ThemeToggle() -> Element {
    let mut theme = use_signal(|| ThemeMode::System);

    use_effect(move || {
        spawn(async move {
            let result = document::eval(
                r#"
                try {
                    var t = localStorage.getItem('theme');
                    if (t === 'dark') return 'dark';
                    if (t === 'light') return 'light';
                    return 'system';
                } catch(e) { return 'system'; }
                "#,
            )
            .await;
            if let Ok(val) = result {
                if let Some(s) = val.as_str() {
                    theme.set(match s {
                        "dark" => ThemeMode::Dark,
                        "light" => ThemeMode::Light,
                        _ => ThemeMode::System,
                    });
                }
            }
        });
    });

    let toggle = move |_| {
        let next = theme().next();
        theme.set(next);
        let store = match next {
            ThemeMode::System => "localStorage.removeItem('theme');",
            ThemeMode::Light => "localStorage.setItem('theme', 'light');",
            ThemeMode::Dark => "localStorage.setItem('theme', 'dark');",
        };
        document::eval(&format!(
            r#"
            {store}
            var d = document.documentElement;
            var t = localStorage.getItem('theme');
            var dark = t === 'dark' || (!t && window.matchMedia('(prefers-color-scheme: dark)').matches);
            d.classList.toggle('dark', dark);
            d.style.colorScheme = dark ? 'dark' : 'light';
            "#
        ));
    };

    let aria = match theme() {
        ThemeMode::System => t!("theme-system"),
        ThemeMode::Light => t!("theme-light"),
        ThemeMode::Dark => t!("theme-dark"),
    };

    rsx! {
        button {
            class: "nav-icon-btn",
            onclick: toggle,
            "aria-label": aria.clone(),
            title: aria,
            ThemeIcon { mode: theme() }
        }
    }
}

#[component]
fn ThemeIcon(mode: ThemeMode) -> Element {
    match mode {
        ThemeMode::System => rsx! {
            svg {
                class: "h-5 w-5",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "1.5",
                view_box: "0 0 24 24",
                rect { x: "2", y: "3", width: "20", height: "14", rx: "2", ry: "2" }
                line { x1: "8", y1: "21", x2: "16", y2: "21" }
                line { x1: "12", y1: "17", x2: "12", y2: "21" }
            }
        },
        ThemeMode::Light => rsx! {
            svg {
                class: "h-5 w-5",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                view_box: "0 0 24 24",
                circle { cx: "12", cy: "12", r: "5" }
                line { x1: "12", y1: "1", x2: "12", y2: "3" }
                line { x1: "12", y1: "21", x2: "12", y2: "23" }
                line { x1: "4.22", y1: "4.22", x2: "5.64", y2: "5.64" }
                line { x1: "18.36", y1: "18.36", x2: "19.78", y2: "19.78" }
                line { x1: "1", y1: "12", x2: "3", y2: "12" }
                line { x1: "21", y1: "12", x2: "23", y2: "12" }
                line { x1: "4.22", y1: "19.78", x2: "5.64", y2: "18.36" }
                line { x1: "18.36", y1: "5.64", x2: "19.78", y2: "4.22" }
            }
        },
        ThemeMode::Dark => rsx! {
            svg {
                class: "h-5 w-5",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                view_box: "0 0 24 24",
                path { d: "M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" }
            }
        },
    }
}
