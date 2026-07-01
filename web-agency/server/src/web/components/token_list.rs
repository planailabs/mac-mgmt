use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{
    Button, ButtonSize, ButtonVariant, PageHeader, TokenCreateForm, TokenCreateInput, TokenReveal,
    TokenRow, TokenTable,
};

// Token CRUD lives in the shared api_mcp layer; only the form-option loader
// stays component-local.
use crate::api_mcp::endpoints::tokens::{
    TokenCreateInput as ApiTokenCreateInput, TokenListInput, TokenRevokeInput, create_token,
    list_tokens, revoke_token,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrgOption {
    id: Uuid,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WsOption {
    id: Uuid,
    name: String,
}

/// The freshly-minted secret plus the kind it was created with (drives the
/// usage hint shown to the user).
#[derive(Debug, Clone)]
struct NewToken {
    token: String,
    kind: String,
}

#[server]
async fn token_form_options() -> Result<(Vec<OrgOption>, Vec<WsOption>), ServerFnError> {
    use crate::web::user::WebUserExt;
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    let orgs =
        sqlx::query_as::<_, (Uuid, String)>("SELECT id, name FROM organizations ORDER BY name")
            .fetch_all(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    let webspaces = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, name FROM webspaces WHERE hosting_type = 'cloudflare_pages' AND cloudflare_pages_project IS NOT NULL ORDER BY name",
    ).fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok((
        orgs.into_iter()
            .map(|(id, name)| OrgOption { id, name })
            .collect(),
        webspaces
            .into_iter()
            .map(|(id, name)| WsOption { id, name })
            .collect(),
    ))
}

/// Unified token view: create a token and see the existing tokens on one page.
#[component]
pub fn TokenList() -> Element {
    let mut tokens = use_server_future(|| list_tokens(TokenListInput::default()))?;
    let opts = use_server_future(token_form_options)?;

    let rows = match &*tokens.read() {
        Some(Ok(r)) => r.clone(),
        _ => vec![],
    };
    let (org_list, ws_list) = match &*opts.read() {
        Some(Ok((o, w))) => (o.clone(), w.clone()),
        _ => (vec![], vec![]),
    };

    let mut kind = use_signal(|| "api".to_string());
    let mut org_id = use_signal(String::new);
    let mut ws_id = use_signal(String::new);
    let mut role = use_signal(|| "write".to_string());
    let mut saving = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut created = use_signal(|| None::<NewToken>);

    let kind_val = kind.read().clone();
    let is_deploy = kind_val == "deploy";

    rsx! {
        PageHeader { "API Tokens" }

        // Newly created secret — shown once, dismissable.
        if let Some(nt) = &*created.read() {
            TokenReveal { value: nt.token.clone(), label: "Token created".to_string(),
                if nt.kind == "deploy" {
                    div { class: "mt-2 text-sm text-fg-muted", "Use this token to deploy:" }
                    div { class: "font-mono text-xs bg-surface-2 p-2 rounded mt-1 select-all break-all",
                        "curl -X POST -H 'Authorization: Bearer {nt.token}' \\\n  --data-binary @site.tar.gz \\\n  https://your-server/api/v1/deploy/WEBSPACE_ID" }
                }
                if nt.kind == "metrics" {
                    div { class: "mt-2 text-sm text-fg-muted", "Use this token to scrape Prometheus metrics:" }
                    div { class: "font-mono text-xs bg-surface-2 p-2 rounded mt-1 select-all break-all",
                        "curl -H 'Authorization: Bearer {nt.token}' \\\n  https://your-server/api/metrics" }
                }
                if nt.kind == "admin" || nt.kind == "api" {
                    div { class: "mt-2 text-sm text-fg-muted", "Use with the HTTP API (/api/v1, docs at /api/v1/docs) or MCP (/mcp):" }
                    div { class: "font-mono text-xs bg-surface-2 p-2 rounded mt-1 select-all break-all",
                        "curl -H 'Authorization: Bearer {nt.token}' https://your-server/api/v1/domains\n# MCP endpoint: https://your-server/mcp" }
                }
                div { class: "mt-3",
                    Button { variant: ButtonVariant::Secondary, size: ButtonSize::Sm, onclick: move |_| created.set(None), "Dismiss" }
                }
            }
        }

        // Create form (compact, shared). Kind + scope are inline children;
        // expiry + label are owned by the shared form.
        TokenCreateForm {
            submit_label: "Create token".to_string(),
            submitting: *saving.read(),
            on_submit: move |input: TokenCreateInput| {
                let k = kind.read().clone();
                let oid_str = org_id.read().clone();
                let wid_str = ws_id.read().clone();
                let role_val = role.read().clone();
                saving.set(true);
                error.set(None);
                spawn(async move {
                    let oid = uuid::Uuid::parse_str(&oid_str).ok();
                    let wid = uuid::Uuid::parse_str(&wid_str).ok();
                    let req = ApiTokenCreateInput {
                        kind: k.clone(),
                        label: input.label,
                        organization_id: oid,
                        webspace_id: wid,
                        role: Some(role_val),
                        expires_in_secs: input.expires_in_secs,
                    };
                    match create_token(req).await {
                        Ok(r) => {
                            created.set(Some(NewToken { token: r.token, kind: k }));
                            tokens.restart();
                        }
                        Err(e) => error.set(Some(format!("{e}"))),
                    }
                    saving.set(false);
                });
            },

            select { class: "input w-auto py-1 text-sm", value: "{kind}", oninput: move |evt| kind.set(evt.value()),
                option { value: "api", "API (HTTP + MCP)" }
                option { value: "admin", "Admin (global, MCP)" }
                option { value: "deploy", "Deploy" }
                option { value: "metrics", "Metrics" }
            }
            if is_deploy {
                select { class: "input w-auto py-1 text-sm", value: "{ws_id}", oninput: move |evt| ws_id.set(evt.value()),
                    option { value: "", "All webspaces" }
                    for ws in &ws_list {
                        option { value: "{ws.id}", "{ws.name}" }
                    }
                }
            } else if kind_val == "metrics" {
                select { class: "input w-auto py-1 text-sm", value: "{org_id}", oninput: move |evt| org_id.set(evt.value()),
                    option { value: "", "All orgs (admin)" }
                    for org in &org_list {
                        option { value: "{org.id}", "{org.name}" }
                    }
                }
            } else if kind_val == "admin" {
                span { class: "text-sm text-fg-muted self-center px-2", "Global — full read/write on all orgs" }
            } else {
                // api: org scope + role
                select { class: "input w-auto py-1 text-sm", value: "{org_id}", oninput: move |evt| org_id.set(evt.value()),
                    option { value: "", "Global (read-only)" }
                    for org in &org_list {
                        option { value: "{org.id}", "{org.name}" }
                    }
                }
                select { class: "input w-auto py-1 text-sm", value: "{role}", oninput: move |evt| role.set(evt.value()),
                    option { value: "write", "Read/Write" }
                    option { value: "read", "Read only" }
                }
            }
        }

        if let Some(err) = &*error.read() {
            div { class: "text-danger text-sm mb-4", "{err}" }
        }

        // Existing tokens.
        TokenTable {
            rows: rows.iter().map(|t| TokenRow {
                id: t.id.to_string(),
                label: t.label.clone(),
                kind: Some(t.kind.clone()),
                scope: t.scope.clone(),
                scope_href: t.scope_href.clone(),
                revoked: t.revoked,
                expired: t.expired,
                created: t.created_at.clone(),
                expires: t.expires_at.clone(),
            }).collect::<Vec<_>>(),
            show_kind: true,
            show_scope: true,
            show_expires: true,
            on_revoke: move |id: String| {
                spawn(async move {
                    if revoke_token(TokenRevokeInput { id }).await.is_ok() {
                        tokens.restart();
                    }
                });
            },
        }
    }
}
