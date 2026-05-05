use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::web::app::Route;
use super::navbar::Sidebar;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct UserInfo {
    is_admin: bool,
    display_name: String,
}

#[server]
async fn get_current_user_info() -> Result<UserInfo, ServerFnError> {
    use crate::web::user::current_user;
    match current_user().await {
        Ok(user) => Ok(UserInfo {
            is_admin: user.is_admin,
            display_name: user.name,
        }),
        Err(_) => Ok(UserInfo {
            is_admin: true,
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
    let (is_admin, display_name) = match &*user_info.read() {
        Some(Ok(info)) => (info.is_admin, info.display_name.clone()),
        _ => (false, String::new()),
    };

    rsx! {
        div { class: "h-screen h-dvh w-full flex overflow-hidden",
            Sidebar { is_admin }

            div { class: "flex-1 flex flex-col min-w-0 overflow-hidden",
                // Topbar
                header { class: "shrink-0 h-14 border-b border-border flex items-center px-6 gap-4",
                    h1 { class: "text-lg font-semibold text-fg", "Web Agency" }
                    div { class: "flex-1" }
                    if !display_name.is_empty() {
                        span { class: "text-sm text-fg-muted", "{display_name}" }
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
