use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api_mcp::endpoints::tokens::{
    SettingTokenCreateInput, SettingTokenRevokeInput, SettingTokensListInput, TokenEntry,
    create_setting_token, list_setting_tokens, revoke_setting_token,
};
use crate::web::components::ui::{
    ErrorText, HelpText, TokenCreateForm, TokenCreateInput, TokenReveal, TokenRow, TokenTable,
};

#[component]
pub fn SettingTokenList(cluster_id: String, read_only: bool) -> Element {
    let cid = cluster_id.clone();
    let mut tokens = use_server_future(move || {
        let cid = cid.clone();
        async move {
            let cluster_id: uuid::Uuid = cid
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            list_setting_tokens(SettingTokensListInput { cluster_id }).await
        }
    })?;

    let mut new_token = use_signal(|| None::<String>);
    let cid_create = cluster_id.clone();

    rsx! {
        if !read_only {
            if let Some(raw) = &*new_token.read() {
                TokenReveal { value: raw.clone(), label: t!("setting-token-new") }
            }

            TokenCreateForm {
                submit_label: t!("setting-token-create"),
                on_submit: move |input: TokenCreateInput| {
                    let cid = cid_create.clone();
                    spawn(async move {
                        let cluster_id: uuid::Uuid = match cid.parse() {
                            Ok(id) => id,
                            Err(e) => {
                                tracing::error!("invalid cluster id: {e}");
                                return;
                            }
                        };
                        match create_setting_token(SettingTokenCreateInput {
                            cluster_id,
                            label: input.label,
                            expires_in_secs: input.expires_in_secs,
                        })
                        .await
                        {
                            Ok(raw) => {
                                new_token.set(Some(raw));
                                tokens.restart();
                            }
                            Err(e) => tracing::error!("failed to create setting token: {e}"),
                        }
                    });
                },
            }
        }

        {match &*tokens.read() {
            Some(Ok(list)) => {
                let rows = list.iter().map(token_to_expiring_row).collect::<Vec<_>>();
                if read_only {
                    rsx! { TokenTable { rows, show_expires: true } }
                } else {
                    rsx! {
                        TokenTable {
                            rows,
                            show_expires: true,
                            on_revoke: move |id: String| {
                                spawn(async move {
                                    let Ok(id) = id.parse::<uuid::Uuid>() else {
                                        return;
                                    };
                                    if revoke_setting_token(SettingTokenRevokeInput { id })
                                        .await
                                        .is_ok()
                                    {
                                        tokens.restart();
                                    }
                                });
                            },
                        }
                    }
                }
            }
            Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
            None => rsx! { HelpText { {t!("loading")} } },
        }}
    }
}

/// Map an expiring (setting/sync) token into a shared [`TokenRow`]. Computes
/// the expired flag; the table renders the Expired status and suppresses
/// revoke. Shared by the setting and sync token lists.
pub fn token_to_expiring_row(token: &TokenEntry) -> TokenRow {
    TokenRow {
        id: token.id.clone(),
        label: if token.label.is_empty() {
            t!("no-label")
        } else {
            token.label.clone()
        },
        kind: None,
        scope: None,
        scope_href: None,
        revoked: token.revoked,
        expired: token.expires_at.is_some_and(|e| e < chrono::Utc::now()),
        created: token.created_at.format("%Y-%m-%d %H:%M").to_string(),
        expires: token
            .expires_at
            .map(|e| e.format("%Y-%m-%d %H:%M").to_string()),
    }
}
