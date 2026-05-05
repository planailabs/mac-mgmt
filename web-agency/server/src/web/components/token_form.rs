use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Button, ButtonKind, ButtonVariant, Card, FormField, PageHeader};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrgOption { id: Uuid, name: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenCreateResult {
    token: String, // plaintext, shown once
}

#[server]
async fn list_orgs_for_token() -> Result<Vec<OrgOption>, ServerFnError> {
    use crate::web::user::WebUserExt;
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let rows = sqlx::query_as::<_, (Uuid, String)>("SELECT id, name FROM organizations ORDER BY name")
        .fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(rows.into_iter().map(|(id, name)| OrgOption { id, name }).collect())
}

#[server]
async fn create_token(org_id: Option<Uuid>, label: String) -> Result<TokenCreateResult, ServerFnError> {
    use crate::web::user::WebUserExt;
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    // Generate random token
    use rand::Rng;
    let token_bytes: [u8; 32] = rand::rng().random();
    let token = hex::encode(token_bytes);

    // Hash for storage
    use sha2::{Sha256, Digest};
    let hash = hex::encode(Sha256::digest(token.as_bytes()));

    sqlx::query(
        "INSERT INTO tokens (organization_id, token_hash, label) VALUES ($1, $2, $3)",
    )
    .bind(org_id).bind(&hash).bind(&label)
    .execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(TokenCreateResult { token })
}

#[component]
pub fn TokenForm() -> Element {
    let orgs = use_server_future(list_orgs_for_token)?;
    let org_list = match &*orgs.read() {
        Some(Ok(o)) => o.clone(),
        _ => vec![],
    };

    let mut label = use_signal(String::new);
    let mut org_id = use_signal(String::new);
    let mut saving = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut result = use_signal(|| None::<TokenCreateResult>);

    if let Some(res) = &*result.read() {
        return rsx! {
            PageHeader { "Token Created" }
            Card {
                div { class: "p-6",
                    div { class: "text-sm text-fg-muted mb-2", "Copy this token now — it won't be shown again." }
                    div { class: "font-mono text-sm bg-surface-2 p-3 rounded break-all select-all", "{res.token}" }
                    div { class: "mt-4",
                        Link { to: crate::web::app::Route::TokenList {}, class: "btn btn-secondary", "Back to Tokens" }
                    }
                }
            }
        };
    }

    rsx! {
        PageHeader { "Create API Token" }
        form {
            class: "card p-6 mt-4 max-w-md space-y-4",
            onsubmit: move |evt| {
                evt.prevent_default();
                let l = label.read().clone();
                let oid_str = org_id.read().clone();
                saving.set(true);
                error.set(None);
                spawn(async move {
                    let oid = uuid::Uuid::parse_str(&oid_str).ok();
                    match create_token(oid, l).await {
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
            FormField { label: "Organization (optional)",
                select { class: "input", value: "{org_id}", oninput: move |evt| org_id.set(evt.value()),
                    option { value: "", "Global" }
                    for org in &org_list {
                        option { value: "{org.id}", "{org.name}" }
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
