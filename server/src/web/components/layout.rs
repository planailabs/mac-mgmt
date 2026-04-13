use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::web::app::Route;

use super::navbar::{Navbar, Sidebar};

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
                    class: "animate-spin h-8 w-8 text-blue-600 dark:text-blue-400",
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
                span { class: "text-sm text-gray-500 dark:text-gray-400", "Loading…" }
            }
        }
    }
}

#[component]
pub fn Layout() -> Element {
    let user_info = use_server_future(get_current_user_info)?;
    let (is_admin, real_is_admin, impersonating_email, display_name) = match &*user_info.read() {
        Some(Ok(info)) => (info.is_admin, info.real_is_admin, info.impersonating_email.clone(), info.display_name.clone()),
        _ => (false, false, None, String::new()),
    };

    rsx! {
        div { class: "h-screen w-full flex flex-col bg-gray-50 dark:bg-gray-900 overflow-hidden",
            // Top Nav
            Navbar { is_admin, real_is_admin, display_name: display_name.clone() }

            // Impersonation banner
            if let Some(email) = &impersonating_email {
                div { class: "shrink-0 bg-yellow-500 text-yellow-900 text-center text-sm py-1.5 px-4 flex items-center justify-center gap-3 relative z-10",
                    span { "Impersonating " strong { "{email}" } }
                    button {
                        class: "bg-yellow-700 text-yellow-100 px-2 py-0.5 rounded text-xs hover:bg-yellow-800",
                        onclick: move |_| {
                            document::eval(
                                "document.cookie = 'impersonate_user_id=; Path=/; Max-Age=0'; window.location.reload();"
                            );
                        },
                        "Stop"
                    }
                }
            }

            // Body flex container
            div { class: "flex flex-1 overflow-hidden relative",
                // Sidebar (Desktop)
                Sidebar { is_admin }
                
                // Main content
                main { class: "flex-1 overflow-y-auto p-4 sm:p-6 lg:p-8 bg-gray-50 dark:bg-gray-900",
                    SuspenseBoundary {
                        fallback: |_| rsx! { LoadingSpinner {} },
                        Outlet::<Route> {}
                    }
                }
            }
        }
    }
}
