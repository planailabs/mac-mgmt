use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Button, ButtonKind, ButtonVariant, Card, FormField, PageHeader};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenCreateResult {
    token: String,
}

#[server]
async fn list_orgs_and_webspaces_for_token()
-> Result<(Vec<OrgOption>, Vec<WsOption>), ServerFnError> {
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
) -> Result<TokenCreateResult, ServerFnError> {
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

    Ok(TokenCreateResult { token })
}

#[component]
pub fn TokenForm() -> Element {
    let data = use_server_future(list_orgs_and_webspaces_for_token)?;
    let (org_list, ws_list) = match &*data.read() {
        Some(Ok((o, w))) => (o.clone(), w.clone()),
        _ => (vec![], vec![]),
    };

    let mut label = use_signal(String::new);
    let mut kind = use_signal(|| "api".to_string());
    let mut org_id = use_signal(String::new);
    let mut ws_id = use_signal(String::new);
    let mut saving = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut result = use_signal(|| None::<TokenCreateResult>);

    let kind_val = kind.read().clone();
    let is_deploy = kind_val == "deploy";

    if let Some(res) = &*result.read() {
        return rsx! {
            PageHeader { "Token Created" }
            Card { div { class: "p-6",
                div { class: "text-sm text-fg-muted mb-2", "Copy this token now — it won't be shown again." }
                div { class: "font-mono text-sm bg-surface-2 p-3 rounded break-all select-all", "{res.token}" }
                if is_deploy {
                    div { class: "mt-4 text-sm text-fg-muted",
                        "Use this token to deploy:"
                    }
                    div { class: "font-mono text-sm bg-surface-2 p-3 rounded mt-2 select-all",
                        "curl -X POST -H 'Authorization: Bearer {res.token}' \\\n  --data-binary @site.tar.gz \\\n  https://your-server/api/v1/deploy/WEBSPACE_ID"
                    }
                }
                if kind_val == "metrics" {
                    div { class: "mt-4 text-sm text-fg-muted",
                        "Use this token to scrape Prometheus metrics:"
                    }
                    div { class: "font-mono text-sm bg-surface-2 p-3 rounded mt-2 select-all",
                        "curl -H 'Authorization: Bearer {res.token}' \\\n  https://your-server/api/metrics"
                    }
                }
                div { class: "mt-4",
                    Link { to: crate::web::app::Route::TokenList {}, class: "btn btn-secondary", "Back to Tokens" }
                }
            }}
        };
    }

    rsx! {
        PageHeader { "Create API Token" }
        form {
            class: "card p-6 mt-4 max-w-lg space-y-4",
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
                    match create_token(oid, l, k, wid).await {
                        Ok(r) => result.set(Some(r)),
                        Err(e) => error.set(Some(format!("{e}"))),
                    }
                    saving.set(false);
                });
            },

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

            div { class: "flex gap-3",
                Button { variant: ButtonVariant::Primary, kind: ButtonKind::Submit, disabled: *saving.read(),
                    if *saving.read() { "Creating..." } else { "Create Token" } }
                Link { to: crate::web::app::Route::TokenList {}, class: "btn btn-secondary", "Cancel" }
            }
        }
    }
}
