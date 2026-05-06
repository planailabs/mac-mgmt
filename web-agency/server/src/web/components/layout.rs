use dioxus::prelude::*;
use plan_ai_design::{LanguagePicker, ThemeToggle};
use serde::{Deserialize, Serialize};

use crate::web::app::Route;
use super::navbar::Sidebar;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct UserInfo {
    is_admin: bool,
    /// If impersonating, this is the target user's email.
    impersonating_email: Option<String>,
    /// True if the real user (before impersonation) is admin.
    real_is_admin: bool,
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
                span { class: "text-sm text-fg-muted", "Loading..." }
            }
        }
    }
}

#[component]
pub fn Layout() -> Element {
    let user_info = use_server_future(get_current_user_info)?;
    let (is_admin, display_name, impersonating_email) = match &*user_info.read() {
        Some(Ok(info)) => (
            info.real_is_admin,
            info.display_name.clone(),
            info.impersonating_email.clone(),
        ),
        _ => (false, String::new(), None),
    };

    rsx! {
        div { class: "h-screen h-dvh w-full flex overflow-hidden",
            Sidebar { is_admin }

            div { class: "flex-1 flex flex-col min-w-0 overflow-hidden",
                // Topbar
                header { class: "shrink-0 h-14 border-b border-border flex items-center px-6 gap-4",
                    h1 { class: "text-lg font-semibold text-fg", "Web Agency" }
                    div { class: "flex-1" }
                    div { class: "flex items-center gap-1",
                        LanguagePicker {}
                        ThemeToggle {}
                        if !display_name.is_empty() {
                            span { class: "text-sm text-fg-muted ml-2", "{display_name}" }
                        }
                        a {
                            class: "nav-icon-btn hover:!text-danger",
                            href: "/auth/logout",
                            title: "Logout",
                            svg {
                                class: "h-5 w-5",
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

                if let Some(email) = &impersonating_email {
                    div { class: "shrink-0 banner banner-warn flex items-center justify-center gap-3",
                        span { "Impersonating {email}" }
                        button {
                            class: "btn btn-xs btn-warn",
                            onclick: move |_| {
                                document::eval(
                                    "document.cookie = 'impersonate_user_id=; Path=/; Max-Age=0'; window.location.reload();"
                                );
                            },
                            "Stop"
                        }
                    }
                }

                main { class: "flex-1 min-w-0 overflow-y-auto overflow-x-hidden overscroll-contain p-4 sm:p-6 lg:p-8",
                    SuspenseBoundary {
                        fallback: |_| rsx! { LoadingSpinner {} },
                        Outlet::<Route> {}
                    }
                }
            }
        }
    }
}
