use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::web::components::topbar::use_topbar;

use super::admin_token_list::AdminTokenList;
use super::all_tokens_list::AllTokensList;
use super::federation_token_list::FederationTokenList;

#[component]
pub fn AdminTokens() -> Element {
    use_topbar(t!("admin-tokens-title"), None);
    rsx! {
        h2 { class: "h-page text-fg-strong", {t!("admin-tokens-title")} }
        p { class: "text-fg mb-6 text-sm",
            {t!("admin-tokens-description")}
        }
        AdminTokenList {}

        hr { class: "my-8 border-line-soft" }

        h2 { class: "h-page text-fg-strong", {t!("federation-tokens-title")} }
        p { class: "text-fg mb-6 text-sm",
            {t!("federation-tokens-description")}
        }
        FederationTokenList {}

        hr { class: "my-8 border-line-soft" }

        h2 { class: "h-page text-fg-strong", {t!("all-tokens-title")} }
        p { class: "text-fg mb-6 text-sm",
            {t!("all-tokens-description")}
        }
        AllTokensList {}
    }
}
