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

use super::navbar::{MobileDrawer, Sidebar};
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

    rsx! {
        div { class: "h-screen w-full flex overflow-hidden",
            // ── Desktop sidebar (220px column, hidden under xl)
            Sidebar { is_admin }

            // ── Main column: topbar + optional banner + page content
            div { class: "flex-1 flex flex-col min-w-0 overflow-hidden",
                Topbar {
                    display_name: display_name.clone(),
                    is_drawer_open: drawer_open,
                }

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

                main { class: "flex-1 overflow-y-auto p-4 sm:p-6 lg:p-8",
                    Breadcrumbs {}
                    SuspenseBoundary {
                        fallback: |_| rsx! { LoadingSpinner {} },
                        Outlet::<Route> {}
                    }
                }
            }

            // ── Mobile drawer (overlay, hidden on xl+)
            MobileDrawer {
                is_admin,
                display_name,
                is_open: drawer_open,
            }
        }
    }
}
