use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{
    Badge, BadgeVariant, Button, ButtonKind, ButtonSize, ButtonVariant, Card, FormField,
    PageHeader, Td, TdMuted, Th, TokenReveal,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenRow {
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
async fn list_tokens() -> Result<Vec<TokenRow>, ServerFnError> {
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
            |(id, label, kind, revoked, created_at, expires_at)| TokenRow {
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

    sqlx::query(
        "INSERT INTO tokens (organization_id, token_hash, label, kind, scopes) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(org_id).bind(&hash).bind(&label).bind(&kind).bind(&scopes)
    .execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(token)
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

    let mut label = use_signal(String::new);
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

        // Create form.
        form {
            class: "card p-6 mt-4 mb-6 max-w-lg space-y-4",
            onsubmit: move |evt| {
                evt.prevent_default();
                let l = label.read().clone();
                let k = kind.read().clone();
                let oid_str = org_id.read().clone();
                let wid_str = ws_id.read().clone();
                saving.set(true);
                error.set(None);
                spawn(async move {
                    let oid = uuid::Uuid::parse_str(&oid_str).ok();
                    let wid = uuid::Uuid::parse_str(&wid_str).ok();
                    match create_token(oid, l, k.clone(), wid).await {
                        Ok(token) => {
                            created.set(Some(NewToken { token, kind: k }));
                            label.set(String::new());
                            tokens.restart();
                        }
                        Err(e) => error.set(Some(format!("{e}"))),
                    }
                    saving.set(false);
                });
            },

            div { class: "font-semibold", "Create token" }

            FormField { label: "Label",
                input { class: "input", r#type: "text", required: true, placeholder: "e.g. CI deploy token",
                    value: "{label}", oninput: move |evt| label.set(evt.value()) }
            }

            FormField { label: "Token Kind",
                select { class: "input", value: "{kind}", oninput: move |evt| kind.set(evt.value()),
                    option { value: "api", "API (general)" }
                    option { value: "deploy", "Deploy (upload tarballs)" }
                    option { value: "metrics", "Metrics (Prometheus scrape)" }
                }
            }

            if is_deploy {
                FormField { label: "Webspace Scope",
                    help: "Restrict this token to a specific webspace, or leave as 'All' for any.",
                    select { class: "input", value: "{ws_id}", oninput: move |evt| ws_id.set(evt.value()),
                        option { value: "", "All webspaces" }
                        for ws in &ws_list {
                            option { value: "{ws.id}", "{ws.name}" }
                        }
                    }
                }
            } else if kind_val == "metrics" {
                FormField { label: "Organization Scope",
                    help: "Scope to an org to see only its metrics, or leave as 'All' for admin access (all orgs + global counters).",
                    select { class: "input", value: "{org_id}", oninput: move |evt| org_id.set(evt.value()),
                        option { value: "", "All (admin)" }
                        for org in &org_list {
                            option { value: "{org.id}", "{org.name}" }
                        }
                    }
                }
            } else {
                FormField { label: "Organization (optional)",
                    select { class: "input", value: "{org_id}", oninput: move |evt| org_id.set(evt.value()),
                        option { value: "", "Global" }
                        for org in &org_list {
                            option { value: "{org.id}", "{org.name}" }
                        }
                    }
                }
            }

            if let Some(err) = &*error.read() {
                div { class: "text-danger text-sm", "{err}" }
            }

            div {
                Button { variant: ButtonVariant::Primary, kind: ButtonKind::Submit, disabled: *saving.read(),
                    if *saving.read() { "Creating..." } else { "Create Token" } }
            }
        }

        // Existing tokens.
        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead { tr { Th { "Label" } Th { "Kind" } Th { "Status" } Th { "Created" } Th { "Expires" } } }
                    tbody {
                        if rows.is_empty() {
                            tr { td { class: "td text-fg-muted text-center", colspan: "5", "No tokens" } }
                        }
                        for row in &rows {
                            tr {
                                Td { "{row.label}" }
                                Td { Badge { variant: BadgeVariant::Info, "{row.kind}" } }
                                Td {
                                    if row.revoked {
                                        Badge { variant: BadgeVariant::Danger, "Revoked" }
                                    } else {
                                        Badge { variant: BadgeVariant::Success, "Active" }
                                    }
                                }
                                TdMuted { "{row.created_at}" }
                                TdMuted { {row.expires_at.as_deref().unwrap_or("Never")} }
                            }
                        }
                    }
                }
            }
        }
    }
}
