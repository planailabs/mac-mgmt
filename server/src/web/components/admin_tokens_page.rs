use dioxus::prelude::*;

use super::admin_token_list::AdminTokenList;
use super::federation_token_list::FederationTokenList;

#[component]
pub fn AdminTokens() -> Element {
    rsx! {
        h2 { class: "text-2xl font-bold mb-4 dark:text-white", "Admin Tokens" }
        p { class: "text-gray-600 dark:text-gray-300 mb-6 text-sm",
            "Admin tokens are not scoped to any cluster. They can list all clusters and create sync/setting tokens for any cluster."
        }
        AdminTokenList {}

        hr { class: "my-8 border-gray-200 dark:border-gray-700" }

        h2 { class: "text-2xl font-bold mb-4 dark:text-white", "Federation Tokens" }
        p { class: "text-gray-600 dark:text-gray-300 mb-6 text-sm",
            "Federation tokens allow remote management servers to access this instance's skill center catalog and resolve skills. Share these with management servers that pull from this skill center."
        }
        FederationTokenList {}
    }
}
