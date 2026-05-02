//! Application shell.
//!
//! Layout is the row of [Sidebar | (Topbar / impersonation banner /
//! main content)]. Sidebar is desktop-only (xl+); under that breakpoint
//! the content fills the screen and the topbar's hamburger reveals
//! the mobile drawer.
//!
//! The shell also provides the `TopbarMeta` context so any page can
//! call `use_topbar(...)` to populate the topbar title without a
//! second prop-drilling chain.

use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};

use crate::web::app::Route;

use super::navbar::{ExpandedGroups, MobileDrawer, Sidebar, SidebarCollapsed};
use super::topbar::{Topbar, TopbarMeta};
use super::ui::Breadcrumbs;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct UserInfo {
    is_admin: bool,
    /// If impersonating, this is the target user's email.
    impersonating_email: Option<String>,
    /// True if the real user (before impersonation) is admin.
    real_is_admin: bool,
    /// Display name of the effective (possibly impersonated) user.
    display_name: String,
}

#[server]
async fn get_current_user_info() -> Result<UserInfo, ServerFnError> {
    use crate::web::user::current_user;
    match current_user().await {
        Ok(user) => Ok(UserInfo {
            is_admin: user.is_admin,
            impersonating_email: if user.impersonating_from.is_some() {
                Some(user.email.clone())
            } else {
                None
            },
            real_is_admin: user.impersonating_from.is_some() || user.is_admin,
            display_name: user.name,
        }),
        Err(_) => Ok(UserInfo {
            is_admin: true,
            impersonating_email: None,
            real_is_admin: true,
            display_name: String::new(),
        }),
    }
}

/// Loading spinner shown during page transitions via SuspenseBoundary.
#[component]
fn LoadingSpinner() -> Element {
    rsx! {
        div { class: "flex items-center justify-center py-20 w-full h-full",
            div { class: "flex flex-col items-center gap-3",
                svg {
                    class: "animate-spin h-8 w-8 text-brand",
                    fill: "none",
                    view_box: "0 0 24 24",
                    circle {
                        class: "opacity-25",
                        cx: "12",
                        cy: "12",
                        r: "10",
                        stroke: "currentColor",
                        stroke_width: "4",
                    }
                    path {
                        class: "opacity-75",
                        fill: "currentColor",
                        d: "M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z",
                    }
                }
                span { class: "text-sm text-fg-muted", {t!("loading")} }
            }
        }
    }
}

#[component]
pub fn Layout() -> Element {
    let user_info = use_server_future(get_current_user_info)?;
    let (is_admin, impersonating_email, display_name) = match &*user_info.read() {
        Some(Ok(info)) => (
            info.is_admin,
            info.impersonating_email.clone(),
            info.display_name.clone(),
        ),
        _ => (false, None, String::new()),
    };

    // Provide the topbar context so any descendant page can populate
    // its own title via `use_topbar`. Default is empty — pages that
    // forget to populate get an empty topbar rather than a wrong one.
    use_context_provider::<Signal<TopbarMeta>>(|| Signal::new(TopbarMeta::default()));

    // Mobile drawer open/closed — shared between the topbar's
    // hamburger and the drawer itself.
    let drawer_open = use_signal(|| false);

    // Sidebar collapsed state (desktop only). Shared via context so the
    // topbar's Logo button can toggle it without prop-drilling. Restored
    // from localStorage on first paint, persisted whenever it flips.
    let mut sidebar_collapsed = use_signal(|| false);
    use_context_provider(|| SidebarCollapsed(sidebar_collapsed));
    use_effect(move || {
        spawn(async move {
            let r = document::eval(
                "try { return localStorage.getItem('nav.sidebar.collapsed') || '0'; } catch(e) { return '0'; }",
            )
            .await;
            if let Ok(val) = r {
                if val.as_str() == Some("1") {
                    sidebar_collapsed.set(true);
                }
            }
        });
    });
    use_effect(move || {
        let v = if *sidebar_collapsed.read() { "1" } else { "0" };
        document::eval(&format!(
            "try {{ localStorage.setItem('nav.sidebar.collapsed', '{v}'); }} catch(e) {{}}",
        ));
    });

    // Per-group expansion (desktop sidebar). Default: only Overview
    // expanded — keeps the sidebar tidy on first visit. Persisted as a
    // comma-joined list of nav-* keys to avoid pulling serde_json into
    // the WASM bundle.
    let mut expanded_groups = use_signal(|| vec!["nav-overview".to_string()]);
    use_context_provider(|| ExpandedGroups(expanded_groups));
    use_effect(move || {
        spawn(async move {
            let r = document::eval(
                "try { var v = localStorage.getItem('nav.groups.expanded'); return v == null ? null : v; } catch(e) { return null; }",
            )
            .await;
            if let Ok(val) = r {
                if let Some(s) = val.as_str() {
                    let parsed: Vec<String> = s
                        .split(',')
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                        .collect();
                    expanded_groups.set(parsed);
                }
            }
        });
    });
    use_effect(move || {
        let v = expanded_groups.read().join(",");
        // Group keys are static `nav-*` strings — no quote-escaping needed.
        document::eval(&format!(
            "try {{ localStorage.setItem('nav.groups.expanded', '{v}'); }} catch(e) {{}}",
        ));
    });

    rsx! {
        // Topbar at the top stretches the full viewport — its left
        // segment owns the brand mark over the sidebar column, the
        // right segment carries the controls + the hairline border-b
        // that separates chrome from content. Sidebar + main share
        // the row beneath.
        div { class: "h-screen w-full flex flex-col overflow-hidden",
            Topbar {
                display_name: display_name.clone(),
                is_drawer_open: drawer_open,
            }

            div { class: "flex flex-1 overflow-hidden",
                Sidebar { is_admin }

                div { class: "flex-1 flex flex-col min-w-0 overflow-hidden",
                    if let Some(email) = &impersonating_email {
                        div { class: "shrink-0 banner banner-warn flex items-center justify-center gap-3",
                            span { {t!("impersonating", email: email.clone())} }
                            button {
                                class: "btn btn-xs btn-warn",
                                onclick: move |_| {
                                    document::eval(
                                        "document.cookie = 'impersonate_user_id=; Path=/; Max-Age=0'; window.location.reload();"
                                    );
                                },
                                {t!("impersonate-stop")}
                            }
                        }
                    }

                    main { class: "flex-1 min-w-0 overflow-y-auto overflow-x-hidden p-4 sm:p-6 lg:p-8",
                        Breadcrumbs {}
                        SuspenseBoundary {
                            fallback: |_| rsx! { LoadingSpinner {} },
                            Outlet::<Route> {}
                        }
                    }
                }
            }

            // Mobile drawer (overlay, hidden on xl+)
            MobileDrawer {
                is_admin,
                display_name,
                is_open: drawer_open,
            }
        }
    }
}
