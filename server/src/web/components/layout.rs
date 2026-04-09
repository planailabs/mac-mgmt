use dioxus::prelude::*;

use crate::web::app::Route;

#[component]
pub fn Layout() -> Element {
    // State for the mobile hamburger menu
    let mut is_open = use_signal(|| false);

    let nav_links = [
        (Route::CustomerList {}, "Customers"),
        (Route::SkillList {}, "Skills"),
        (Route::BundleList {}, "Bundles"),
        (Route::McpServerList {}, "MCP Servers"),
        (Route::McpBundleList {}, "MCP Bundles"),
        (Route::AdminTokens {}, "Admin Tokens"),
        (Route::FleetDashboard {}, "Fleet"),
        (Route::RolloutList {}, "Rollouts"),
        (Route::DaemonVersionList {}, "Daemon Versions"),
    ];

    rsx! {
        div { class: "min-h-screen bg-gray-50",
            nav { class: "bg-white shadow",
                div { class: "max-w-7xl mx-auto px-4 sm:px-6 lg:px-8",
                    div { class: "flex justify-between h-16 items-center",
                        Link { to: Route::CustomerList {},
                            h1 { class: "text-xl font-bold text-gray-900", "mac-mgmt" }
                        }
                        
                        // Desktop menu (visible on xl and larger)
                        div { class: "hidden xl:flex xl:space-x-1 xl:items-center",
                            for (route, label) in nav_links.clone() {
                                Link { 
                                    key: "{label}", 
                                    to: route, 
                                    class: "whitespace-nowrap px-3 py-2 rounded-md text-sm font-medium text-gray-600 hover:text-gray-900 hover:bg-gray-100 transition-colors", 
                                    active_class: "!bg-gray-100 !text-gray-900",
                                    "{label}" 
                                }
                            }
                        }

                        // Hamburger button (visible below xl)
                        div { class: "flex items-center xl:hidden",
                            button {
                                onclick: move |_| is_open.set(!is_open()),
                                class: "inline-flex items-center justify-center p-2 rounded-md text-gray-400 hover:text-gray-500 hover:bg-gray-100 focus:outline-none focus:ring-2 focus:ring-inset focus:ring-blue-500",
                                "aria-expanded": "{is_open}",
                                "aria-controls": "mobile-menu",
                                span { class: "sr-only", "Open main menu" }
                                svg {
                                    class: "h-6 w-6",
                                    fill: "none",
                                    stroke: "currentColor",
                                    view_box: "0 0 24 24",
                                    if *is_open.read() {
                                        path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M6 18L18 6M6 6l12 12" }
                                    } else {
                                        path { stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2", d: "M4 6h16M4 12h16M4 18h16" }
                                    }
                                }
                            }
                        }
                    }
                }

                // Mobile Dropdown Menu
                if *is_open.read() {
                    div { 
                        id: "mobile-menu",
                        class: "xl:hidden border-t border-gray-100 bg-white shadow-lg",
                        div { class: "px-2 pt-2 pb-3 space-y-1 sm:px-3",
                            for (route, label) in nav_links.clone() {
                                Link { 
                                    key: "{label}",
                                    to: route, 
                                    class: "block px-3 py-2 rounded-md text-base font-medium text-gray-600 hover:text-gray-900 hover:bg-gray-50 transition-colors",
                                    active_class: "!bg-gray-100 !text-gray-900",
                                    onclick: move |_| is_open.set(false), // Close menu when navigating
                                    "{label}" 
                                }
                            }
                        }
                    }
                }
            }
            main { class: "max-w-7xl mx-auto py-6 px-4 sm:px-6 lg:px-8",
                Outlet::<Route> {}
            }
        }
    }
}
