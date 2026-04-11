use dioxus::prelude::*;

use crate::web::app::Route;

use super::navbar::Navbar;

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
    rsx! {
        div { class: "min-h-screen bg-gray-50 dark:bg-gray-900",
            Navbar {}
            main { class: "max-w-7xl mx-auto py-6 px-4 sm:px-6 lg:px-8",
                SuspenseBoundary {
                    fallback: |_| rsx! { LoadingSpinner {} },
                    Outlet::<Route> {}
                }
            }
        }
    }
}
