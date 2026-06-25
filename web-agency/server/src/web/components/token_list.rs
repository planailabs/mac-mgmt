use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{
    Button, ButtonSize, ButtonVariant, PageHeader, TokenCreateForm, TokenCreateInput, TokenReveal,
    TokenRow, TokenTable,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenInfo {
    id: Uuid,
    label: String,
    kind: String,
    revoked: bool,
    created_at: String,
    expires_at: Option<String>,
}

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
async fn list_tokens() -> Result<Vec<TokenInfo>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    use crate::web::user::WebUserExt;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    let rows = sqlx::query_as::<_, (Uuid, String, String, bool, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>)>(
        "SELECT id, label, kind, revoked, created_at, expires_at FROM tokens ORDER BY created_at DESC LIMIT 100",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(
            |(id, label, kind, revoked, created_at, expires_at)| TokenInfo {
                id,
                label,
                kind,
                revoked,
                created_at: created_at.format("%Y-%m-%d %H:%M").to_string(),
                expires_at: expires_at.map(|d| d.format("%Y-%m-%d %H:%M").to_string()),
            },
        )
        .collect())
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

#[server]
async fn create_token(
    org_id: Option<Uuid>,
    label: String,
    kind: String,
    webspace_id: Option<Uuid>,
    expires_in_secs: Option<i64>,
) -> Result<String, ServerFnError> {
    use crate::web::user::WebUserExt;
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    use rand::Rng;
    let token_bytes: [u8; 32] = rand::rng().random();
    let token = hex::encode(token_bytes);

    use sha2::{Digest, Sha256};
    let hash = hex::encode(Sha256::digest(token.as_bytes()));

    let scopes = if kind == "deploy" {
        webspace_id.map(|wid| serde_json::json!({ "webspace_id": wid.to_string() }))
    } else {
        None
    };
    let expires_at = expires_in_secs.map(|s| chrono::Utc::now() + chrono::Duration::seconds(s));

    sqlx::query(
        "INSERT INTO tokens (organization_id, token_hash, label, kind, scopes, expires_at) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(org_id).bind(&hash).bind(&label).bind(&kind).bind(&scopes).bind(expires_at)
    .execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(token)
}

#[server]
async fn revoke_token(id: String) -> Result<(), ServerFnError> {
    use crate::web::user::WebUserExt;
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("UPDATE tokens SET revoked = true WHERE id = $1")
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

/// Unified token view: create a token and see the existing tokens on one page.
#[component]
pub fn TokenList() -> Element {
    let mut tokens = use_server_future(list_tokens)?;
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
                saving.set(true);
                error.set(None);
                spawn(async move {
                    let oid = uuid::Uuid::parse_str(&oid_str).ok();
                    let wid = uuid::Uuid::parse_str(&wid_str).ok();
                    match create_token(oid, input.label, k.clone(), wid, input.expires_in_secs).await {
                        Ok(token) => {
                            created.set(Some(NewToken { token, kind: k }));
                            tokens.restart();
                        }
                        Err(e) => error.set(Some(format!("{e}"))),
                    }
                    saving.set(false);
                });
            },

            select { class: "input w-auto py-1 text-sm", value: "{kind}", oninput: move |evt| kind.set(evt.value()),
                option { value: "api", "API" }
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
            } else {
                select { class: "input w-auto py-1 text-sm", value: "{org_id}", oninput: move |evt| org_id.set(evt.value()),
                    option { value: "", "Global" }
                    for org in &org_list {
                        option { value: "{org.id}", "{org.name}" }
                    }
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
                scope: None,
                revoked: t.revoked,
                expired: false,
                created: t.created_at.clone(),
                expires: t.expires_at.clone(),
            }).collect::<Vec<_>>(),
            show_kind: true,
            show_expires: true,
            on_revoke: move |id: String| {
                spawn(async move {
                    if revoke_token(id).await.is_ok() {
                        tokens.restart();
                    }
                });
            },
        }
    }
}
