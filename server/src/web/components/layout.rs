use dioxus::prelude::*;

use crate::web::app::Route;

use super::navbar::Navbar;

#[server]
async fn get_current_user_info() -> Result<(bool,), ServerFnError> {
    use crate::web::user::current_user;
    match current_user().await {
        Ok(user) => Ok((user.is_admin,)),
        // If no user in extensions (e.g. OIDC disabled), default to admin
        Err(_) => Ok((true,)),
    }
}

/// Loading spinner shown during page transitions via SuspenseBoundary.
#[component]
fn LoadingSpinner() -> Element {
    rsx! {
        div { class: "flex items-center justify-center py-20",
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
    let is_admin = match &*user_info.read() {
        Some(Ok((admin,))) => *admin,
        _ => false,
    };

    rsx! {
        div { class: "min-h-screen bg-gray-50 dark:bg-gray-900",
            Navbar { is_admin }
            main { class: "max-w-7xl mx-auto py-6 px-4 sm:px-6 lg:px-8",
                SuspenseBoundary {
                    fallback: |_| rsx! { LoadingSpinner {} },
                    Outlet::<Route> {}
                }
            }
        }
    }
}
