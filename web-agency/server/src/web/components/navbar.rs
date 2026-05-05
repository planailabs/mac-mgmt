use dioxus::prelude::*;

use crate::web::app::Route;

#[derive(Clone, PartialEq)]
struct NavItem {
    label: &'static str,
    route: Route,
    icon: &'static str,
}

fn nav_items(is_admin: bool) -> Vec<NavItem> {
    let _ = is_admin;
    vec![
        NavItem {
            label: "Dashboard",
            route: Route::Dashboard {},
            icon: "M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6",
        },
        NavItem {
            label: "Credentials",
            route: Route::CredentialList {},
            // Key icon
            icon: "M15.75 5.25a3 3 0 013 3m3 0a6 6 0 01-7.029 5.912c-.563-.097-1.159.026-1.563.43L10.5 17.25H8.25v2.25H6v2.25H2.25v-2.818c0-.597.237-1.17.659-1.591l6.499-6.499c.404-.404.527-1 .43-1.563A6 6 0 1121.75 8.25z",
        },
    ]
}

#[component]
pub fn Sidebar(is_admin: bool) -> Element {
    let items = nav_items(is_admin);

    rsx! {
        aside { class: "hidden xl:flex flex-col w-56 shrink-0 border-r border-border bg-surface-1",
            // Logo area
            div { class: "h-14 flex items-center px-4 border-b border-border",
                span { class: "text-lg font-bold text-fg", "WA" }
            }

            nav { class: "flex-1 overflow-y-auto py-2 px-2",
                for item in &items {
                    Link {
                        to: item.route.clone(),
                        class: "flex items-center gap-3 px-3 py-2 rounded-md text-sm text-fg-muted hover:bg-surface-2 hover:text-fg transition-colors",
                        svg {
                            class: "h-5 w-5 shrink-0",
                            fill: "none",
                            view_box: "0 0 24 24",
                            stroke_width: "1.5",
                            stroke: "currentColor",
                            path {
                                stroke_linecap: "round",
                                stroke_linejoin: "round",
                                d: item.icon,
                            }
                        }
                        span { "{item.label}" }
                    }
                }
            }
        }
    }
}
