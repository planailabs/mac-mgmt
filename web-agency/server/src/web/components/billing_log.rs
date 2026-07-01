use dioxus::prelude::*;

use super::ui::{Badge, BadgeVariant, Card, PageHeader, Td, TdMuted, Th};

// BillingRow + the list endpoint now live in the shared api_mcp layer.
use crate::api_mcp::endpoints::billing::{BillingListInput, list_billing};

fn format_amount(cents: i32, currency: &str) -> String {
    let euros = cents as f64 / 100.0;
    format!("{euros:.2} {currency}")
}

#[component]
pub fn BillingLog() -> Element {
    let billing = use_server_future(move || list_billing(BillingListInput::default()))?;
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
