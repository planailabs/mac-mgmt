use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Badge, BadgeVariant, Button, ButtonVariant, Card, PageHeader, Td, TdMuted, Th};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CertRow {
    id: Uuid,
    domain: String,
    issuer: String,
    acme_status: String,
    not_before: String,
    not_after: String,
    expires_soon: bool,
    last_error: Option<String>,
}

#[server]
async fn list_certificates() -> Result<Vec<CertRow>, ServerFnError> {
    use crate::web::user::WebUserExt;
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    let rows = sqlx::query_as::<_, (Uuid, String, String, String, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>, bool, Option<String>)>(
        "SELECT id, domain, issuer, acme_status, not_before, not_after, \
         not_after < now() + interval '30 days', last_error \
         FROM certificates ORDER BY domain",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows.into_iter().map(|(id, domain, issuer, acme_status, not_before, not_after, expires_soon, last_error)| CertRow {
        id, domain, issuer, acme_status,
        not_before: not_before.format("%Y-%m-%d").to_string(),
        not_after: not_after.format("%Y-%m-%d").to_string(),
        expires_soon,
        last_error,
    }).collect())
}

#[server]
async fn reissue_cert(domain: String) -> Result<String, ServerFnError> {
    use crate::web::user::WebUserExt;
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    match crate::api::acme::issue_cert(&pool, &domain).await {
        Ok(()) => Ok("issued".into()),
        Err(e) => Ok(format!("error: {e}")),
    }
}

#[component]
pub fn CertificateList() -> Element {
    let mut certs = use_server_future(list_certificates)?;
    let rows = match &*certs.read() {
        Some(Ok(r)) => r.clone(),
        _ => vec![],
    };
    let mut reissue_result = use_signal(|| None::<String>);
    let mut reissuing = use_signal(|| None::<String>);

    rsx! {
        PageHeader { "Certificates" }

        if let Some(msg) = &*reissue_result.read() {
            div { class: "mb-4 text-sm text-fg-muted", "{msg}" }
        }

        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead {
                        tr {
                            Th { "Domain" }
                            Th { "Issuer" }
                            Th { "Status" }
                            Th { "Valid From" }
                            Th { "Expires" }
                            Th { "Error" }
                            Th { "" }
                        }
                    }
                    tbody {
                        if rows.is_empty() {
                            tr { td { class: "td text-fg-muted text-center", colspan: "7", "No certificates" } }
                        }
                        for row in &rows {
                            {
                                let domain = row.domain.clone();
                                let is_reissuing = *reissuing.read() == Some(domain.clone());
                                rsx! {
                                    tr {
                                        Td { class: "font-mono", "{row.domain}" }
                                        TdMuted { "{row.issuer}" }
                                        Td {
                                            match row.acme_status.as_str() {
                                                "valid" => rsx! { Badge { variant: BadgeVariant::Success, "Valid" } },
                                                "pending" => rsx! { Badge { variant: BadgeVariant::Warn, "Pending" } },
                                                "failed" => rsx! { Badge { variant: BadgeVariant::Danger, "Failed" } },
                                                "none" => rsx! { Badge { "None" } },
                                                s => rsx! { Badge { "{s}" } },
                                            }
                                        }
                                        TdMuted { "{row.not_before}" }
                                        Td {
                                            if row.expires_soon {
                                                Badge { variant: BadgeVariant::Warn, "{row.not_after}" }
                                            } else {
                                                span { class: "text-fg-muted", "{row.not_after}" }
                                            }
                                        }
                                        Td {
                                            if let Some(err) = &row.last_error {
                                                span { class: "text-sm text-danger truncate max-w-xs", title: "{err}", "{err}" }
                                            } else {
                                                span { class: "text-fg-muted", "-" }
                                            }
                                        }
                                        Td {
                                            Button {
                                                variant: ButtonVariant::Secondary,
                                                disabled: is_reissuing,
                                                onclick: {
                                                    let d = domain.clone();
                                                    move |_| {
                                                        let d = d.clone();
                                                        reissuing.set(Some(d.clone()));
                                                        reissue_result.set(None);
                                                        spawn(async move {
                                                            match reissue_cert(d.clone()).await {
                                                                Ok(msg) => reissue_result.set(Some(format!("{d}: {msg}"))),
                                                                Err(e) => reissue_result.set(Some(format!("{d}: {e}"))),
                                                            }
                                                            reissuing.set(None);
                                                            certs.restart();
                                                        });
                                                    }
                                                },
                                                if is_reissuing { "Issuing..." } else { "Reissue" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
