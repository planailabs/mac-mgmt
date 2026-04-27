use dioxus::prelude::*;
use dioxus_i18n::t;

use super::admin_token_list::AdminTokenList;
use super::federation_token_list::FederationTokenList;

#[component]
pub fn AdminTokens() -> Element {
    rsx! {
        h2 { class: "text-2xl font-bold mb-4 dark:text-white", {t!("admin-tokens-title")} }
        p { class: "text-gray-600 dark:text-gray-300 mb-6 text-sm",
            {t!("admin-tokens-description")}
        }
        AdminTokenList {}

        hr { class: "my-8 border-gray-200 dark:border-gray-700" }

        h2 { class: "text-2xl font-bold mb-4 dark:text-white", {t!("federation-tokens-title")} }
        p { class: "text-gray-600 dark:text-gray-300 mb-6 text-sm",
            {t!("federation-tokens-description")}
        }
        FederationTokenList {}
    }
}
