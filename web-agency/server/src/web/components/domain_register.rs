use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Badge, BadgeVariant, Button, ButtonVariant, Card, FormField, PageHeader, SectionHeading, Td, Th};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredOption { id: Uuid, name: String, credential_type: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AvailabilityResult {
    domain: String,
    available: bool,
    price: Option<String>,
    currency: Option<String>,
    provider: String,
}

#[server]
async fn list_registrar_creds() -> Result<Vec<CredOption>, ServerFnError> {
    let _user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let rows = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT id, name, credential_type FROM credentials \
         WHERE credential_type IN ('cloudflare', 'spaceship') ORDER BY name",
    )
    .fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(rows.into_iter().map(|(id, name, credential_type)| CredOption { id, name, credential_type }).collect())
}

#[server]
async fn check_domain_availability(credential_id: Uuid, domain: String) -> Result<AvailabilityResult, ServerFnError> {
    let _user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let cred = sqlx::query_as::<_, (String, Vec<u8>)>(
        "SELECT credential_type, encrypted_data FROM credentials WHERE id = $1",
    ).bind(credential_id).fetch_optional(&pool).await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("credential not found"))?;

    let (cred_type, encrypted) = cred;
    let decrypted = crate::crypto::decrypt(&encrypted)
        .map_err(|e| ServerFnError::new(format!("decryption: {e}")))?;
    let data: serde_json::Value = serde_json::from_slice(&decrypted)
        .map_err(|e| ServerFnError::new(format!("invalid credential: {e}")))?;

    match cred_type.as_str() {
        "cloudflare" => {
            let token = data["api_token"].as_str().ok_or_else(|| ServerFnError::new("missing api_token"))?;
            let account_id = data["account_id"].as_str().unwrap_or("");
            if account_id.is_empty() {
                return Err(ServerFnError::new("account_id required"));
            }
            let client = cloudflare_api::Client::new(token);
            let results = client.check_domains(account_id, &[domain.clone()]).await
                .map_err(|e| ServerFnError::new(format!("CF API: {e}")))?;
            let r = results.into_iter().next().ok_or_else(|| ServerFnError::new("no result"))?;
            Ok(AvailabilityResult {
                domain: r.name,
                available: r.registrable,
                price: r.pricing.as_ref().and_then(|p| p.registration_cost.clone()),
                currency: r.pricing.map(|p| p.currency),
                provider: "cloudflare".into(),
            })
        }
        "spaceship" => {
            let key = data["api_key"].as_str().ok_or_else(|| ServerFnError::new("missing api_key"))?;
            let secret = data["api_secret"].as_str().ok_or_else(|| ServerFnError::new("missing api_secret"))?;
            let client = spaceship_api::Client::new(key, secret);
            let r = client.check_availability(&domain).await
                .map_err(|e| ServerFnError::new(format!("Spaceship API: {e}")))?;
            let available = r.status.as_deref() == Some("available");
            let reg_price = r.premium_pricing.iter().find(|p| p.operation == "register");
            Ok(AvailabilityResult {
                domain: r.domain.unwrap_or(domain),
                available,
                price: reg_price.map(|p| format!("{:.2}", p.price)),
                currency: reg_price.map(|p| p.currency.clone()),
                provider: "spaceship".into(),
            })
        }
        _ => Err(ServerFnError::new("unsupported registrar")),
    }
}

#[component]
pub fn DomainRegister() -> Element {
    let creds = use_server_future(list_registrar_creds)?;
    let cred_list = match &*creds.read() {
        Some(Ok(c)) => c.clone(),
        _ => vec![],
    };

    let mut domain = use_signal(String::new);
    let mut cred_id = use_signal(|| cred_list.first().map(|c| c.id.to_string()).unwrap_or_default());
    let mut checking = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut result = use_signal(|| None::<AvailabilityResult>);

    rsx! {
        PageHeader { "Register Domain" }

        Card {
            div { class: "p-6 space-y-4",
                p { class: "text-fg-muted text-sm mb-2",
                    "Check domain availability and pricing via your registrar credentials."
                }

                div { class: "flex items-end gap-3",
                    FormField { label: "Domain",
                        input { class: "input w-64 font-mono", r#type: "text", placeholder: "example.com",
                            value: "{domain}", oninput: move |evt| { domain.set(evt.value()); result.set(None); } }
                    }
                    FormField { label: "Registrar Credential",
                        select { class: "input", value: "{cred_id}",
                            oninput: move |evt| { cred_id.set(evt.value()); result.set(None); },
                            for c in &cred_list {
                                option { value: "{c.id}", "{c.name} ({c.credential_type})" }
                            }
                        }
                    }
                    Button {
                        variant: ButtonVariant::Primary,
                        disabled: domain.read().is_empty() || cred_id.read().is_empty() || *checking.read(),
                        onclick: {
                            let d = domain.read().clone();
                            let cid_str = cred_id.read().clone();
                            move |_| {
                                let d = d.clone();
                                let cid_str = cid_str.clone();
                                checking.set(true);
                                error.set(None);
                                result.set(None);
                                spawn(async move {
                                    if let Ok(cid) = uuid::Uuid::parse_str(&cid_str) {
                                        match check_domain_availability(cid, d).await {
                                            Ok(r) => result.set(Some(r)),
                                            Err(e) => error.set(Some(format!("{e}"))),
                                        }
                                    }
                                    checking.set(false);
                                });
                            }
                        },
                        if *checking.read() { "Checking..." } else { "Check Availability" }
                    }
                }

                if let Some(err) = &*error.read() {
                    div { class: "text-danger text-sm", "{err}" }
                }
            }
        }

        if let Some(res) = &*result.read() {
            SectionHeading { class: "mt-6", "Result" }
            Card {
                div { class: "p-6",
                    table { class: "table w-full",
                        thead { tr { Th { "Domain" } Th { "Available" } Th { "Price" } Th { "Provider" } } }
                        tbody {
                            tr {
                                Td { class: "font-mono", "{res.domain}" }
                                Td {
                                    if res.available {
                                        Badge { variant: BadgeVariant::Success, "Available" }
                                    } else {
                                        Badge { variant: BadgeVariant::Danger, "Taken" }
                                    }
                                }
                                Td {
                                    if let (Some(p), Some(c)) = (&res.price, &res.currency) {
                                        span { class: "font-mono", "{p} {c}" }
                                    } else {
                                        span { class: "text-fg-muted", "-" }
                                    }
                                }
                                Td { Badge { variant: BadgeVariant::Info, "{res.provider}" } }
                            }
                        }
                    }
                    if !res.available {
                        p { class: "mt-3 text-sm text-fg-muted", "This domain is not available for registration." }
                    } else {
                        p { class: "mt-3 text-sm text-fg-muted",
                            "Domain registration via the API requires contacts to be configured. "
                            "Use the Spaceship or Cloudflare dashboard to complete the registration, "
                            "then import the domain."
                        }
                    }
                }
            }
        }
    }
}
