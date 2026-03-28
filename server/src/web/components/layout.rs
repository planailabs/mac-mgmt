use dioxus::prelude::*;

use crate::web::app::Route;

#[component]
pub fn Layout() -> Element {
    rsx! {
        div { class: "min-h-screen bg-gray-50",
            nav { class: "bg-white shadow",
                div { class: "max-w-7xl mx-auto px-4 sm:px-6 lg:px-8",
                    div { class: "flex justify-between h-16 items-center",
                        Link { to: Route::CustomerList {},
                            h1 { class: "text-xl font-bold text-gray-900", "mac-mgmt" }
                        }
                        div { class: "flex space-x-4",
                            Link { to: Route::CustomerList {}, class: "text-gray-600 hover:text-gray-900", "Customers" }
                            Link { to: Route::SkillList {}, class: "text-gray-600 hover:text-gray-900", "Skills" }
                            Link { to: Route::BundleList {}, class: "text-gray-600 hover:text-gray-900", "Bundles" }
                            Link { to: Route::McpServerList {}, class: "text-gray-600 hover:text-gray-900", "MCP Servers" }
                            Link { to: Route::McpBundleList {}, class: "text-gray-600 hover:text-gray-900", "MCP Bundles" }
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
