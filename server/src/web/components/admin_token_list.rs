use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api_mcp::endpoints::tokens::{
    AdminTokenCreateInput, AdminTokenRevokeInput, AdminTokensListInput, create_admin_token,
    list_admin_tokens, revoke_admin_token,
};
use crate::web::components::ui::{
    ErrorText, HelpText, TokenCreateForm, TokenCreateInput, TokenReveal, TokenRow, TokenTable,
};

#[component]
pub fn AdminTokenList() -> Element {
    let mut tokens =
        use_server_future(move || async move { list_admin_tokens(AdminTokensListInput {}).await })?;
    let mut new_token = use_signal(|| None::<String>);

    rsx! {
        if let Some(raw) = &*new_token.read() {
            TokenReveal { value: raw.clone(), label: t!("admin-token-new") }
        }

        TokenCreateForm {
            submit_label: t!("admin-token-create"),
            on_submit: move |input: TokenCreateInput| {
                spawn(async move {
                    match create_admin_token(AdminTokenCreateInput {
                        label: input.label,
                        expires_in_secs: input.expires_in_secs,
                    })
                    .await
                    {
                        Ok(raw) => {
                            new_token.set(Some(raw));
                            tokens.restart();
                        }
                        Err(e) => tracing::error!("failed to create admin token: {e}"),
                    }
                });
            },
        }

        {match &*tokens.read() {
            Some(Ok(list)) => {
                let rows = list.iter().map(|t| TokenRow {
                    id: t.id.clone(),
                    label: if t.label.is_empty() { t!("no-label") } else { t.label.clone() },
                    kind: None,
                    scope: None,
                    scope_href: None,
                    revoked: t.revoked,
                    expired: false,
                    created: t.created_at.format("%Y-%m-%d").to_string(),
                    expires: None,
                }).collect::<Vec<_>>();
                rsx! {
                    TokenTable {
                        rows,
                        on_revoke: move |id: String| {
                            spawn(async move {
                                let Ok(id) = id.parse::<uuid::Uuid>() else {
                                    return;
                                };
                                if revoke_admin_token(AdminTokenRevokeInput { id }).await.is_ok() {
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
