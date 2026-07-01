use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{
    Badge, BadgeVariant, Button, ButtonVariant, Card, FormField, PageHeader, Td, TdMuted, Th,
};

// DomainRow, the list endpoint and the bulk operations now live in the shared
// api_mcp layer; the UI calls the macro-generated `#[server]` wrappers.
use crate::api_mcp::endpoints::domains::{
    BulkFailure, BulkOpResult, DomainBulkCreatePagesInput, DomainBulkDeployCloudflareInput,
    DomainBulkSetAiBotsInput, DomainBulkSetNameserversInput, DomainBulkSetSslModeInput,
    DomainListInput, DomainRow, bulk_create_pages_project, bulk_deploy_to_cloudflare,
    bulk_set_ai_bots_protection, bulk_set_nameservers, bulk_set_ssl_mode, list_domains,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CfCredOption {
    id: Uuid,
    name: String,
}

#[server]
async fn list_cf_credentials_for_bulk() -> Result<Vec<CfCredOption>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let rows =
        if user.is_admin {
            sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, name FROM credentials WHERE credential_type = 'cloudflare' ORDER BY name",
        ).fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
        } else {
            let org_ids = user.org_ids();
            sqlx::query_as::<_, (Uuid, String)>(
                "SELECT id, name FROM credentials WHERE credential_type = 'cloudflare' \
             AND (organization_id = ANY($1) OR organization_id IS NULL) ORDER BY name",
            )
            .bind(&org_ids)
            .fetch_all(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
        };
    Ok(rows
        .into_iter()
        .map(|(id, name)| CfCredOption { id, name })
        .collect())
}

#[component]
pub fn DomainList() -> Element {
    let domains = use_server_future(move || list_domains(DomainListInput::default()))?;
    let creds = use_server_future(list_cf_credentials_for_bulk)?;
    let all_rows: Vec<DomainRow> = match &*domains.read() {
        Some(Ok(r)) => r.clone(),
        _ => vec![],
    };
    let cred_list: Vec<CfCredOption> = match &*creds.read() {
        Some(Ok(c)) => c.clone(),
        _ => vec![],
    };

    let mut selected: Signal<Vec<Uuid>> = use_signal(Vec::new);
    let mut bulk_result = use_signal(|| None::<BulkOpResult>);
    let mut running = use_signal(|| false);
    let mut cred_id = use_signal(|| {
        cred_list
            .first()
            .map(|c| c.id.to_string())
            .unwrap_or_default()
    });
    let mut filter = use_signal(|| "all".to_string());
    let mut bulk_ssl_mode = use_signal(|| "full".to_string());
    let mut bulk_ai_bots = use_signal(|| "block".to_string());

    // Apply filter
    let filtered_rows: Vec<&DomainRow> = match filter.read().as_str() {
        "no_cf" => all_rows
            .iter()
            .filter(|r| r.cloudflare_zone_id.is_none())
            .collect(),
        "needs_ns" => all_rows.iter().filter(|r| r.ns_ok == Some(false)).collect(),
        "no_webspace" => all_rows
            .iter()
            .filter(|r| !r.has_webspace && r.cloudflare_zone_id.is_some())
            .collect(),
        "ai_crawl_off" => all_rows
            .iter()
            .filter(|r| r.ai_bots_protection.as_deref() != Some("block"))
            .collect(),
        "ai_crawl_on" => all_rows
            .iter()
            .filter(|r| r.ai_bots_protection.as_deref() == Some("block"))
            .collect(),
        "ssl_not_full" => all_rows
            .iter()
            .filter(|r| {
                r.cloudflare_zone_id.is_some() && r.ssl_mode != "full" && r.ssl_mode != "strict"
            })
            .collect(),
        "expires_soon" => all_rows.iter().filter(|r| r.expires_soon).collect(),
        _ => all_rows.iter().collect(),
    };

    // Count how many selected domains are eligible for each operation
    let sel = selected.read();
    let no_cf_count = filtered_rows
        .iter()
        .filter(|r| sel.contains(&r.id) && r.cloudflare_zone_id.is_none())
        .count();
    let ns_eligible_count = filtered_rows
        .iter()
        .filter(|r| sel.contains(&r.id) && r.ns_ok == Some(false))
        .count();
    let sel_count = sel.len();
    let filtered_count = filtered_rows.len();
    // Collect IDs for bulk ops before dropping sel
    let deploy_ids: Vec<Uuid> = filtered_rows
        .iter()
        .filter(|r| sel.contains(&r.id) && r.cloudflare_zone_id.is_none())
        .map(|r| r.id)
        .collect();
    let ns_ids: Vec<Uuid> = filtered_rows
        .iter()
        .filter(|r| sel.contains(&r.id) && r.ns_ok == Some(false))
        .map(|r| r.id)
        .collect();
    let filtered_ids: Vec<Uuid> = filtered_rows.iter().map(|r| r.id).collect();
    let cf_selected_count = filtered_rows
        .iter()
        .filter(|r| sel.contains(&r.id) && r.cloudflare_zone_id.is_some())
        .count();
    let cf_selected_ids: Vec<Uuid> = filtered_rows
        .iter()
        .filter(|r| sel.contains(&r.id) && r.cloudflare_zone_id.is_some())
        .map(|r| r.id)
        .collect();
    let no_ws_count = filtered_rows
        .iter()
        .filter(|r| sel.contains(&r.id) && !r.has_webspace && r.cloudflare_zone_id.is_some())
        .count();
    let no_ws_ids: Vec<Uuid> = filtered_rows
        .iter()
        .filter(|r| sel.contains(&r.id) && !r.has_webspace && r.cloudflare_zone_id.is_some())
        .map(|r| r.id)
        .collect();
    drop(sel);

    rsx! {
        PageHeader { "Domains" }

        div { class: "mt-4 flex justify-end gap-3 mb-4",
            Link {
                to: crate::web::app::Route::DomainRegister {},
                class: "btn btn-secondary",
                "Register Domain"
            }
            Link {
                to: crate::web::app::Route::DomainImport {},
                class: "btn btn-secondary",
                "Import from Credential"
            }
            Link {
                to: crate::web::app::Route::DomainAdd {},
                class: "btn btn-primary",
                "Add Domain"
            }
        }

        // Filter bar
        div { class: "mb-4 flex items-center gap-2 flex-wrap",
            span { class: "text-sm text-fg-muted", "Filter:" }
            {
                let filters: Vec<(&str, &str, usize)> = vec![
                    ("all", "All", all_rows.len()),
                    ("no_cf", "No CF Zone", all_rows.iter().filter(|r| r.cloudflare_zone_id.is_none()).count()),
                    ("needs_ns", "Needs NS", all_rows.iter().filter(|r| r.ns_ok == Some(false)).count()),
                    ("no_webspace", "No Webspace", all_rows.iter().filter(|r| !r.has_webspace && r.cloudflare_zone_id.is_some()).count()),
                    ("ai_crawl_off", "AI Crawl Off", all_rows.iter().filter(|r| r.ai_bots_protection.as_deref() != Some("block")).count()),
                    ("ai_crawl_on", "AI Crawl On", all_rows.iter().filter(|r| r.ai_bots_protection.as_deref() == Some("block")).count()),
                    ("ssl_not_full", "SSL Not Full", all_rows.iter().filter(|r| r.cloudflare_zone_id.is_some() && r.ssl_mode != "full" && r.ssl_mode != "strict").count()),
                    ("expires_soon", "Expires Soon", all_rows.iter().filter(|r| r.expires_soon).count()),
                ];
                rsx! {
                    for (key, label, count) in filters {
                        Button {
                            variant: if *filter.read() == key { ButtonVariant::Primary } else { ButtonVariant::Secondary },
                            onclick: {
                                let k = key.to_string();
                                move |_| { filter.set(k.clone()); selected.set(vec![]); }
                            },
                            "{label} ({count})"
                        }
                    }
                }
            }
        }

        // Bulk actions bar (visible when domains are selected)
        if sel_count > 0 {
            Card { class: "mb-4",
                div { class: "p-4 flex items-end gap-3 flex-wrap",
                    span { class: "text-sm font-medium", "{sel_count} selected" }

                    if no_cf_count > 0 && !cred_list.is_empty() {
                        FormField { label: "CF Credential",
                            select { class: "input w-48", value: "{cred_id}", oninput: move |evt| cred_id.set(evt.value()),
                                for c in &cred_list { option { value: "{c.id}", "{c.name}" } }
                            }
                        }
                        Button {
                            variant: ButtonVariant::Primary,
                            disabled: *running.read(),
                            onclick: {
                                let ids = deploy_ids.clone();
                                move |_| {
                                    let ids = ids.clone();
                                    let cid_str = cred_id.read().clone();
                                    running.set(true);
                                    bulk_result.set(None);
                                    spawn(async move {
                                        if let Ok(cid) = Uuid::parse_str(&cid_str) {
                                            match bulk_deploy_to_cloudflare(DomainBulkDeployCloudflareInput { domain_ids: ids, credential_id: cid }).await {
                                                Ok(r) => bulk_result.set(Some(r)),
                                                Err(e) => bulk_result.set(Some(BulkOpResult { succeeded: vec![], failed: vec![BulkFailure { name: "error".into(), error: format!("{e}") }] })),
                                            }
                                        }
                                        running.set(false);
                                    });
                                }
                            },
                            if *running.read() { "Deploying..." } else { "Deploy to CF ({no_cf_count})" }
                        }
                    }

                    if ns_eligible_count > 0 {
                        Button {
                            variant: ButtonVariant::Secondary,
                            disabled: *running.read(),
                            onclick: {
                                let ids = ns_ids.clone();
                                move |_| {
                                    let ids = ids.clone();
                                    running.set(true);
                                    bulk_result.set(None);
                                    spawn(async move {
                                        match bulk_set_nameservers(DomainBulkSetNameserversInput { domain_ids: ids }).await {
                                            Ok(r) => bulk_result.set(Some(r)),
                                            Err(e) => bulk_result.set(Some(BulkOpResult { succeeded: vec![], failed: vec![BulkFailure { name: "error".into(), error: format!("{e}") }] })),
                                        }
                                        running.set(false);
                                    });
                                }
                            },
                            if *running.read() { "Setting NS..." } else { "Set NS at Registrar ({ns_eligible_count})" }
                        }
                    }

                    // Bulk SSL mode
                    if cf_selected_count > 0 {
                        FormField { label: "SSL",
                            select { class: "input w-32", value: "{bulk_ssl_mode}", oninput: move |evt| bulk_ssl_mode.set(evt.value()),
                                option { value: "off", "Off" } option { value: "flexible", "Flexible" } option { value: "full", "Full" } option { value: "strict", "Strict" }
                            }
                        }
                        Button {
                            variant: ButtonVariant::Secondary,
                            disabled: *running.read(),
                            onclick: {
                                let ids = cf_selected_ids.clone();
                                move |_| {
                                    let ids = ids.clone();
                                    let mode = bulk_ssl_mode.read().clone();
                                    running.set(true); bulk_result.set(None);
                                    spawn(async move {
                                        match bulk_set_ssl_mode(DomainBulkSetSslModeInput { domain_ids: ids, ssl_mode: mode }).await {
                                            Ok(r) => bulk_result.set(Some(r)),
                                            Err(e) => bulk_result.set(Some(BulkOpResult { succeeded: vec![], failed: vec![BulkFailure { name: "error".into(), error: format!("{e}") }] })),
                                        }
                                        running.set(false);
                                    });
                                }
                            },
                            "Set SSL ({cf_selected_count})"
                        }
                    }

                    // Bulk AI bot protection
                    if cf_selected_count > 0 {
                        FormField { label: "AI Bots",
                            select { class: "input w-32", value: "{bulk_ai_bots}", oninput: move |evt| bulk_ai_bots.set(evt.value()),
                                option { value: "block", "Block" } option { value: "disabled", "Disabled" }
                            }
                        }
                        Button {
                            variant: ButtonVariant::Secondary,
                            disabled: *running.read(),
                            onclick: {
                                let ids = cf_selected_ids.clone();
                                move |_| {
                                    let ids = ids.clone();
                                    let val = bulk_ai_bots.read().clone();
                                    running.set(true); bulk_result.set(None);
                                    spawn(async move {
                                        match bulk_set_ai_bots_protection(DomainBulkSetAiBotsInput { domain_ids: ids, value: val }).await {
                                            Ok(r) => bulk_result.set(Some(r)),
                                            Err(e) => bulk_result.set(Some(BulkOpResult { succeeded: vec![], failed: vec![BulkFailure { name: "error".into(), error: format!("{e}") }] })),
                                        }
                                        running.set(false);
                                    });
                                }
                            },
                            "Set AI Bots ({cf_selected_count})"
                        }
                    }

                    // Bulk create Pages projects (for domains without webspaces)
                    if no_ws_count > 0 && !cred_list.is_empty() {
                        Button {
                            variant: ButtonVariant::Primary,
                            disabled: *running.read(),
                            onclick: {
                                let ids = no_ws_ids.clone();
                                move |_| {
                                    let ids = ids.clone();
                                    let cid_str = cred_id.read().clone();
                                    running.set(true); bulk_result.set(None);
                                    spawn(async move {
                                        if let Ok(cid) = Uuid::parse_str(&cid_str) {
                                            match bulk_create_pages_project(DomainBulkCreatePagesInput { domain_ids: ids, credential_id: cid }).await {
                                                Ok(r) => bulk_result.set(Some(r)),
                                                Err(e) => bulk_result.set(Some(BulkOpResult { succeeded: vec![], failed: vec![BulkFailure { name: "error".into(), error: format!("{e}") }] })),
                                            }
                                        }
                                        running.set(false);
                                    });
                                }
                            },
                            "Create Pages ({no_ws_count})"
                        }
                    }
                }
            }
        }

        // Bulk operation results
        if let Some(ref result) = *bulk_result.read() {
            Card { class: "mb-4",
                div { class: "p-4",
                    if !result.succeeded.is_empty() {
                        div { class: "text-sm mb-2",
                            Badge { variant: BadgeVariant::Success, "{result.succeeded.len()} succeeded" }
                        }
                    }
                    if !result.failed.is_empty() {
                        div { class: "text-sm",
                            Badge { variant: BadgeVariant::Warn, "{result.failed.len()} failed" }
                            for f in &result.failed {
                                div { class: "text-sm text-fg-muted mt-1", "{f.name}: {f.error}" }
                            }
                        }
                    }
                    div { class: "mt-2",
                        Button {
                            variant: ButtonVariant::Secondary,
                            onclick: move |_| {
                                bulk_result.set(None);
                                selected.set(vec![]);
                                navigator().replace(crate::web::app::Route::DomainList {});
                            },
                            "Dismiss & Reload"
                        }
                    }
                }
            }
        }

        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead {
                        tr {
                            th { class: "th w-8",
                                input {
                                    r#type: "checkbox",
                                    checked: sel_count > 0 && sel_count == filtered_count,
                                    oninput: {
                                        let all = filtered_ids.clone();
                                        move |evt| {
                                            if evt.checked() {
                                                selected.set(all.clone());
                                            } else {
                                                selected.set(vec![]);
                                            }
                                        }
                                    }
                                }
                            }
                            Th { "Domain" }
                            Th { "Registrar" }
                            Th { "SSL" }
                            Th { "DNSSEC" }
                            Th { "CF Zone" }
                            Th { "Expires" }
                            Th { "Org" }
                        }
                    }
                    tbody {
                        if filtered_rows.is_empty() {
                            tr {
                                td { class: "td text-fg-muted text-center", colspan: "8", "No domains yet" }
                            }
                        }
                        for row in &filtered_rows {
                            {
                                let rid = row.id;
                                let is_selected = selected.read().contains(&rid);
                                rsx! {
                                    tr { class: "cursor-pointer hover:bg-surface-2",
                                        td { class: "td w-8",
                                            input {
                                                r#type: "checkbox",
                                                checked: is_selected,
                                                onclick: move |evt| { evt.stop_propagation(); },
                                                oninput: move |evt| {
                                                    let mut sel = selected.write();
                                                    if evt.checked() {
                                                        if !sel.contains(&rid) { sel.push(rid); }
                                                    } else {
                                                        sel.retain(|&id| id != rid);
                                                    }
                                                }
                                            }
                                        }
                                        td { class: "td cursor-pointer",
                                            onclick: {
                                                let id = rid.to_string();
                                                move |_| {
                                                    navigator().push(crate::web::app::Route::DomainDetail { id: id.clone() });
                                                }
                                            },
                                            "{row.name}"
                                        }
                                        Td {
                                            match row.registrar_type.as_deref() {
                                                Some("cloudflare") => rsx! { Badge { variant: BadgeVariant::Info, "CF" } },
                                                Some("spaceship") => rsx! { Badge { variant: BadgeVariant::Info, "SS" } },
                                                Some("external") => rsx! { Badge { "Ext" } },
                                                _ => rsx! { span { class: "text-fg-muted", "-" } },
                                            }
                                        }
                                        Td {
                                            Badge { "{row.ssl_mode}" }
                                        }
                                        Td {
                                            if row.dnssec_enabled {
                                                Badge { variant: BadgeVariant::Success, "On" }
                                            } else {
                                                span { class: "text-fg-muted", "Off" }
                                            }
                                        }
                                        Td {
                                            if row.cloudflare_zone_id.is_some() {
                                                Badge { variant: BadgeVariant::Success, "Yes" }
                                            } else {
                                                span { class: "text-fg-muted", "-" }
                                            }
                                        }
                                        Td {
                                            if row.expires_soon {
                                                span { class: "text-danger", {row.expires_at.as_deref().unwrap_or("-")} }
                                            } else {
                                                span { class: "text-fg-muted", {row.expires_at.as_deref().unwrap_or("-")} }
                                            }
                                        }
                                        TdMuted { "{row.organization_name}" }
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
