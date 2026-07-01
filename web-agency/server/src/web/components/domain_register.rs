use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// The availability check lives in the shared api_mcp layer; the UI calls the
// macro-generated `#[server]` wrapper.
use crate::api_mcp::endpoints::domains::{
    AvailabilityResult, DomainCheckAvailabilityInput, check_domain_availability,
};

use super::ui::{
    Badge, BadgeVariant, Button, ButtonVariant, Card, FormField, PageHeader, SectionHeading, Td, Th,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredOption {
    id: Uuid,
    name: String,
    credential_type: String,
}

#[server]
async fn list_registrar_creds() -> Result<Vec<CredOption>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let rows = if user.is_admin {
        sqlx::query_as::<_, (Uuid, String, String)>(
            "SELECT id, name, credential_type FROM credentials \
             WHERE credential_type IN ('cloudflare', 'spaceship') ORDER BY name",
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    } else {
        let org_ids = user.org_ids();
        sqlx::query_as::<_, (Uuid, String, String)>(
            "SELECT id, name, credential_type FROM credentials \
             WHERE credential_type IN ('cloudflare', 'spaceship') \
             AND (organization_id = ANY($1) OR organization_id IS NULL) ORDER BY name",
        )
        .bind(&org_ids)
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    };
    Ok(rows
        .into_iter()
        .map(|(id, name, credential_type)| CredOption {
            id,
            name,
            credential_type,
        })
        .collect())
}

#[component]
pub fn DomainRegister() -> Element {
    let creds = use_server_future(list_registrar_creds)?;
    let cred_list = match &*creds.read() {
        Some(Ok(c)) => c.clone(),
        _ => vec![],
    };

    let mut domain = use_signal(String::new);
    let mut cred_id = use_signal(|| {
        cred_list
            .first()
            .map(|c| c.id.to_string())
            .unwrap_or_default()
    });
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
                                        match check_domain_availability(DomainCheckAvailabilityInput {
                                            credential_id: cid,
                                            domain: d,
                                        })
                                        .await
                                        {
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
