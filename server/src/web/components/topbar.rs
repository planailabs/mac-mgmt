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
use dioxus_i18n::t;
use plan_ai_design::{LanguagePicker, ThemeToggle};

use crate::web::app::Route;
use crate::web::components::navbar::Logo;

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

#[component]
pub fn Topbar(
    /// Effective user's display name. Empty when unauthenticated.
    display_name: String,
    /// Bound to the mobile drawer. Topbar's hamburger toggles this.
    is_drawer_open: Signal<bool>,
) -> Element {
    rsx! {
        header { class: "topbar",
            // ── Left segment: brand logo. On desktop it sits over the
            // sidebar column (220px wide, no border-b — seamless to
            // the sidebar below). On mobile it just sits at the left
            // and the controls share the same hairline at the bottom.
            div { class: "topbar-logo-pad",
                Logo {}
            }

            // ── Right segment: global controls. Carries the hairline
            // border-b that separates topbar from main content. Mobile
            // order: [theme][hamburger]. Desktop: [lang][theme][user].
            div { class: "topbar-controls",
                div { class: "hidden xl:flex items-center gap-1",
                    LanguagePicker {}
                }
                ThemeToggle {}
                if !display_name.is_empty() {
                    div { class: "hidden xl:block",
                        UserPill { display_name }
                    }
                }
                MobileMenuButton { is_open: is_drawer_open }
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
