use dioxus::prelude::*;

use super::admin_token_list::AdminTokenList;

#[component]
pub fn AdminTokens() -> Element {
    rsx! {
        h2 { class: "text-2xl font-bold mb-4", "Admin Tokens" }
        p { class: "text-gray-600 dark:text-gray-300 mb-6 text-sm",
            "Admin tokens are not scoped to any cluster. They can list all clusters and create sync/setting tokens for any cluster."
        }
        AdminTokenList {}
    }
}
