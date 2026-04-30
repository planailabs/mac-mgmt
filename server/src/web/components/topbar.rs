//! Slim top chrome bar.
//!
//! The new design language splits navigation into two pieces:
//!
//!   * **Sidebar** (`navbar.rs::Sidebar`) — desktop-only 220px column
//!     that owns the logo + nav groups.
//!   * **Topbar** (this file) — always-visible 56px bar that owns the
//!     page title (read from a context signal), the global controls
//!     (language picker, theme toggle, user/logout), and the mobile
//!     hamburger that opens the drawer.
//!
//! Pages that want to populate the topbar title call `use_topbar`
//! near the top of their component. Pages that don't, leave the
//! topbar title empty — preferable to faking a value.

use dioxus::prelude::*;
use dioxus_i18n::{prelude::*, t, unic_langid::langid};

use crate::web::app::Route;

/// Page-level metadata read by the `Topbar` component. Pages set this
/// via `use_topbar`. Keep it small — anything heavier (per-page
/// action buttons, search) belongs in the page hero.
#[derive(Clone, PartialEq, Default, Debug)]
pub struct TopbarMeta {
    pub title: String,
    pub subtitle: Option<String>,
}

/// Subscribe a page's title/subtitle to the topbar. Safe to call
/// multiple times during a render — the underlying signal compares
/// with PartialEq before rebroadcasting.
///
/// Usage:
/// ```ignore
/// #[component]
/// pub fn FleetDashboard() -> Element {
///     use_topbar("Fleet".into(), Some(format!("{} instances", n)));
///     // ...
/// }
/// ```
pub fn use_topbar(title: String, subtitle: Option<String>) {
    let mut meta = use_context::<Signal<TopbarMeta>>();
    use_effect(move || {
        let next = TopbarMeta {
            title: title.clone(),
            subtitle: subtitle.clone(),
        };
        if *meta.read() != next {
            meta.set(next);
        }
    });
}

/// Available locales with their native display names. Single source
/// of truth shared with the mobile drawer.
const LOCALES: &[(&str, &str)] = &[("en-US", "English"), ("de-DE", "Deutsch")];

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

#[component]
pub fn Topbar(
    /// Effective user's display name. Empty when unauthenticated.
    display_name: String,
    /// Bound to the mobile drawer. Topbar's hamburger toggles this.
    is_drawer_open: Signal<bool>,
) -> Element {
    let meta = use_context::<Signal<TopbarMeta>>();
    let title = meta.read().title.clone();
    let subtitle = meta.read().subtitle.clone();

    rsx! {
        header { class: "topbar",
            // ── Left: page title (hidden on very small screens to give
            // room for the hamburger and user pill).
            div { class: "min-w-0 flex items-center gap-3",
                // Mobile hamburger lives on the left for thumb reach.
                MobileMenuButton { is_open: is_drawer_open }
                if !title.is_empty() {
                    span { class: "text-fg-strong font-semibold text-sm truncate",
                        "{title}"
                    }
                }
                if let Some(s) = subtitle {
                    span { class: "hidden sm:inline text-fg-muted text-xs truncate",
                        "{s}"
                    }
                }
            }

            // ── Right: global controls. Below xl the language picker
            // and user pill move into the mobile drawer (the hamburger
            // already opens it), so we keep only the theme toggle on
            // narrow screens to avoid the cluster overflowing the topbar.
            div { class: "flex items-center gap-1",
                div { class: "hidden xl:flex items-center gap-1",
                    LanguagePicker {}
                }
                ThemeToggle {}
                if !display_name.is_empty() {
                    div { class: "hidden xl:block",
                        UserPill { display_name }
                    }
                }
            }
        }
    }
}

#[component]
fn MobileMenuButton(is_open: Signal<bool>) -> Element {
    let open = *is_open.read();
    rsx! {
        button {
            class: "xl:hidden nav-icon-btn",
            "aria-expanded": "{open}",
            "aria-controls": "mobile-drawer",
            "aria-label": t!("nav-open-main-menu"),
            onclick: move |_| is_open.set(!open),
            svg {
                class: "h-5 w-5",
                fill: "none",
                stroke: "currentColor",
                view_box: "0 0 24 24",
                if open {
                    path {
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        stroke_width: "2",
                        d: "M6 18L18 6M6 6l12 12",
                    }
                } else {
                    path {
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        stroke_width: "2",
                        d: "M4 6h16M4 12h16M4 18h16",
                    }
                }
            }
        }
    }
}

#[component]
fn LanguagePicker() -> Element {
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

#[component]
fn ThemeToggle() -> Element {
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
        // Toggling the `.dark` class on <html> flips every CSS variable
        // in input.css — we never touch inline colors here.
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

/// Compact user pill — initials in a brand-soft circle + name + logout
/// hint. The full pill is hidden below `sm` to keep the topbar usable
/// on phones; only the avatar circle remains there as a profile link.
#[component]
fn UserPill(display_name: String) -> Element {
    let initials = display_name
        .split_whitespace()
        .filter_map(|word| word.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase();

    rsx! {
        div { class: "flex items-center gap-2 ml-1",
            Link {
                to: Route::Profile {},
                class: "flex items-center gap-2 px-2 py-1 rounded-md hover:bg-surface-3 text-fg transition-colors",
                title: display_name.clone(),
                span {
                    class: "w-6 h-6 rounded-full bg-brand-soft text-brand text-[10.5px] font-semibold flex items-center justify-center shrink-0",
                    "{initials}"
                }
                span { class: "hidden sm:inline text-sm truncate max-w-[160px]",
                    "{display_name}"
                }
            }
            a {
                href: "/auth/logout",
                class: "nav-icon-btn hover:!text-danger",
                title: t!("nav-sign-out"),
                svg {
                    class: "h-4 w-4",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "1.5",
                    view_box: "0 0 24 24",
                    path {
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        d: "M15.75 9V5.25A2.25 2.25 0 0 0 13.5 3h-6a2.25 2.25 0 0 0-2.25 2.25v13.5A2.25 2.25 0 0 0 7.5 21h6a2.25 2.25 0 0 0 2.25-2.25V15m3-3h-9m9 0-3-3m3 3-3 3",
                    }
                }
            }
        }
    }
}
