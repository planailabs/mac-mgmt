use dioxus::prelude::*;

use super::ui::{Badge, BadgeVariant, Button, ButtonVariant, Card, PageHeader, Td, TdMuted, Th};

// CertRow + the list/issue endpoints now live in the shared api_mcp layer.
use crate::api_mcp::endpoints::certificates::{
    CertIssueInput, CertListInput, list_certificates, reissue_cert,
};

#[component]
pub fn CertificateList() -> Element {
    let mut certs = use_server_future(move || list_certificates(CertListInput::default()))?;
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
                                                            match reissue_cert(CertIssueInput { domain: d.clone() }).await {
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
