use dioxus::prelude::*;

use super::ui::PageHeader;

#[component]
pub fn Dashboard() -> Element {
    rsx! {
        PageHeader { "Dashboard" }

        p { class: "text-fg-muted mb-6", "Domain and webspace management overview" }

        div { class: "grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4",
            StatCard { label: "Domains", value: "0" }
            StatCard { label: "Webspaces", value: "0" }
            StatCard { label: "Credentials", value: "0" }
            StatCard { label: "Organizations", value: "0" }
        }
    }
}

#[component]
fn StatCard(label: &'static str, value: &'static str) -> Element {
    rsx! {
        div { class: "card p-6",
            div { class: "text-sm text-fg-muted", "{label}" }
            div { class: "text-3xl font-bold text-fg mt-1", "{value}" }
        }
    }
}
