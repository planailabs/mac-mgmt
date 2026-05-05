use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Badge, BadgeVariant, Card, PageHeader, Td, TdMuted, Th};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BillingRow {
    id: Uuid,
    entry_type: String,
    description: String,
    amount_cents: i32,
    currency: String,
    provider: Option<String>,
    period_start: Option<String>,
    period_end: Option<String>,
    created_at: String,
}

#[server]
async fn list_billing() -> Result<Vec<BillingRow>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_ids = user.org_ids();
    if org_ids.is_empty() && !user.is_admin {
        return Ok(vec![]);
    }

    let rows = if user.is_admin {
        sqlx::query_as::<_, (Uuid, String, String, i32, String, Option<String>, Option<chrono::NaiveDate>, Option<chrono::NaiveDate>, chrono::DateTime<chrono::Utc>)>(
            "SELECT id, entry_type, description, amount_cents, currency, provider, period_start, period_end, created_at \
             FROM billing_entries ORDER BY created_at DESC LIMIT 100",
        )
        .fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
    } else {
        sqlx::query_as::<_, (Uuid, String, String, i32, String, Option<String>, Option<chrono::NaiveDate>, Option<chrono::NaiveDate>, chrono::DateTime<chrono::Utc>)>(
            "SELECT id, entry_type, description, amount_cents, currency, provider, period_start, period_end, created_at \
             FROM billing_entries WHERE organization_id = ANY($1) ORDER BY created_at DESC LIMIT 100",
        )
        .bind(&org_ids)
        .fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
    };

    Ok(rows
        .into_iter()
        .map(|(id, entry_type, description, amount_cents, currency, provider, period_start, period_end, created_at)| BillingRow {
            id,
            entry_type,
            description,
            amount_cents,
            currency,
            provider,
            period_start: period_start.map(|d| d.to_string()),
            period_end: period_end.map(|d| d.to_string()),
            created_at: created_at.format("%Y-%m-%d %H:%M").to_string(),
        })
        .collect())
}

fn format_amount(cents: i32, currency: &str) -> String {
    let euros = cents as f64 / 100.0;
    format!("{euros:.2} {currency}")
}

#[component]
pub fn BillingLog() -> Element {
    let billing = use_server_future(list_billing)?;
    let rows = billing.read();
    let rows = match &*rows {
        Some(Ok(r)) => r.as_slice(),
        _ => &[],
    };

    rsx! {
        PageHeader { "Billing" }

        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead {
                        tr {
                            Th { "Date" }
                            Th { "Type" }
                            Th { "Description" }
                            Th { "Amount" }
                            Th { "Provider" }
                            Th { "Period" }
                        }
                    }
                    tbody {
                        if rows.is_empty() {
                            tr {
                                td { class: "td text-fg-muted text-center", colspan: "6", "No billing entries" }
                            }
                        }
                        for row in rows {
                            tr {
                                TdMuted { "{row.created_at}" }
                                Td {
                                    match row.entry_type.as_str() {
                                        "domain_registration" => rsx! { Badge { variant: BadgeVariant::Info, "Registration" } },
                                        "domain_renewal" => rsx! { Badge { variant: BadgeVariant::Accent, "Renewal" } },
                                        "webspace_hosting" => rsx! { Badge { "Hosting" } },
                                        _ => rsx! { Badge { "{row.entry_type}" } },
                                    }
                                }
                                Td { "{row.description}" }
                                Td { class: "font-mono", {format_amount(row.amount_cents, &row.currency)} }
                                TdMuted { {row.provider.as_deref().unwrap_or("-")} }
                                TdMuted {
                                    {match (&row.period_start, &row.period_end) {
                                        (Some(s), Some(e)) => format!("{s} - {e}"),
                                        _ => "-".to_string(),
                                    }}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
