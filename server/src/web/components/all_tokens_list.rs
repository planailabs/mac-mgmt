use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api_mcp::endpoints::tokens::{
    AllTokensListInput, AnyTokenRevokeInput, list_all_tokens, revoke_any_token,
};
use crate::web::components::ui::{ErrorText, HelpText, TokenRow, TokenTable};

/// Read-only master list of every token across the system, with scope. No
/// creation menu — this is purely for visibility/audit (admin only).
#[component]
pub fn AllTokensList() -> Element {
    let mut tokens =
        use_server_future(move || async move { list_all_tokens(AllTokensListInput {}).await })?;

    rsx! {
        {match &*tokens.read() {
            Some(Ok(list)) => {
                let rows = list.iter().map(|t| TokenRow {
                    id: t.id.clone(),
                    label: if t.label.is_empty() { t!("no-label") } else { t.label.clone() },
                    kind: Some(t.kind.clone()),
                    scope: t.scope.clone(),
                    scope_href: t.scope_href.clone(),
                    revoked: t.revoked,
                    expired: t.expired,
                    created: t.created_at.format("%Y-%m-%d %H:%M").to_string(),
                    expires: t.expires_at.map(|e| e.format("%Y-%m-%d %H:%M").to_string()),
                }).collect::<Vec<_>>();
                rsx! {
                    TokenTable {
                        rows,
                        show_kind: true,
                        show_scope: true,
                        show_expires: true,
                        on_revoke: move |id: String| {
                            spawn(async move {
                                let Ok(id) = id.parse::<uuid::Uuid>() else {
                                    return;
                                };
                                if revoke_any_token(AnyTokenRevokeInput { id }).await.is_ok() {
                                    tokens.restart();
                                }
                            });
                        },
                    }
                }
            }
            Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
            None => rsx! { HelpText { {t!("loading")} } },
        }}
    }
}
