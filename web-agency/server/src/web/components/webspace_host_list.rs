//! Webspace-host list page.
//!
//! Hosts own the hostname, domain bindings, the CNAME flow, and
//! ChangeDetection (their `webspace` children are path-mounted folders). The
//! mass operations that used to live on the flat webspace list — bulk
//! ChangeDetection assign/remove, and the "Missing CNAME" filter — therefore
//! live here, at the host level.

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{
    Badge, BadgeVariant, Button, ButtonVariant, Card, FormField, PageHeader, Td, TdMuted, Th,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HostRow {
    id: Uuid,
    name: String,
    kind: String,
    organization_name: String,
    folder_count: i64,
    hostname: Option<String>,
    has_changedetection: bool,
    has_missing_cname: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CdCredOption {
    id: Uuid,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BulkOpResult {
    succeeded: usize,
    failed: usize,
}

#[server]
async fn list_hosts() -> Result<Vec<HostRow>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_ids = user.org_ids();
    if org_ids.is_empty() && !user.is_admin {
        return Ok(vec![]);
    }

    // hostname: first bound (domain, subdomain) for the host, rendered as a FQDN.
    let hostname_subquery = "(SELECT CASE WHEN s.name IS NOT NULL AND s.name != '@' \
            THEN s.name || '.' || d.name ELSE d.name END \
         FROM webspace_host_domains whd \
         JOIN domains d ON d.id = whd.domain_id \
         LEFT JOIN subdomains s ON s.id = whd.subdomain_id \
         WHERE whd.webspace_host_id = h.id ORDER BY d.name LIMIT 1)";

    // has_missing_cname: the host has at least one domain binding where no CNAME
    // dns_record exists for that subdomain on that domain.
    let missing_cname_subquery = "\
        EXISTS( \
            SELECT 1 FROM webspace_host_domains whd2 \
            JOIN domains d2 ON d2.id = whd2.domain_id \
            LEFT JOIN subdomains s2 ON s2.id = whd2.subdomain_id \
            WHERE whd2.webspace_host_id = h.id \
              AND NOT EXISTS( \
                  SELECT 1 FROM dns_records dr \
                  WHERE dr.domain_id = d2.id \
                    AND dr.record_type = 'CNAME' \
                    AND dr.name = COALESCE(s2.name, '@') \
              ) \
        )";

    type Row = (
        Uuid,
        String,
        String,
        String,
        i64,
        Option<String>,
        bool,
        bool,
    );
    let query = format!(
        "SELECT h.id, h.name, h.kind, o.name, \
         (SELECT count(*) FROM webspaces w WHERE w.webspace_host_id = h.id), \
         {hostname_subquery}, \
         h.changedetection_credential_id IS NOT NULL, \
         {missing_cname_subquery} \
         FROM webspace_hosts h JOIN organizations o ON o.id = h.organization_id"
    );

    let rows = if user.is_admin {
        sqlx::query_as::<_, Row>(&format!("{query} ORDER BY h.name"))
            .fetch_all(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
    } else {
        sqlx::query_as::<_, Row>(&format!(
            "{query} WHERE h.organization_id = ANY($1) ORDER BY h.name"
        ))
        .bind(&org_ids)
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    };

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                name,
                kind,
                organization_name,
                folder_count,
                hostname,
                has_changedetection,
                has_missing_cname,
            )| {
                HostRow {
                    id,
                    name,
                    kind,
                    organization_name,
                    folder_count,
                    hostname,
                    has_changedetection,
                    has_missing_cname,
                }
            },
        )
        .collect())
}

#[server]
async fn list_cd_creds_for_bulk() -> Result<Vec<CdCredOption>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let rows = if user.is_admin {
        sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, name FROM credentials WHERE credential_type = 'changedetection' ORDER BY name",
        ).fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
    } else {
        let org_ids = user.org_ids();
        sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, name FROM credentials WHERE credential_type = 'changedetection' \
             AND (organization_id = ANY($1) OR organization_id IS NULL) ORDER BY name",
        )
        .bind(&org_ids)
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    };
    Ok(rows
        .into_iter()
        .map(|(id, name)| CdCredOption { id, name })
        .collect())
}

#[server]
async fn bulk_assign_changedetection(
    host_ids: Vec<Uuid>,
    credential_id: Uuid,
) -> Result<BulkOpResult, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    if !user.is_admin {
        return Err(ServerFnError::new("admin required"));
    }
    let pool = crate::server_pool()?;

    let result = sqlx::query(
        "UPDATE webspace_hosts SET changedetection_credential_id = $1, updated_at = now() \
         WHERE id = ANY($2)",
    )
    .bind(credential_id)
    .bind(&host_ids)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(BulkOpResult {
        succeeded: result.rows_affected() as usize,
        failed: 0,
    })
}

#[server]
async fn bulk_remove_changedetection(host_ids: Vec<Uuid>) -> Result<BulkOpResult, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    if !user.is_admin {
        return Err(ServerFnError::new("admin required"));
    }
    let pool = crate::server_pool()?;

    let result = sqlx::query(
        "UPDATE webspace_hosts SET changedetection_credential_id = NULL, updated_at = now() \
         WHERE id = ANY($1) AND changedetection_credential_id IS NOT NULL",
    )
    .bind(&host_ids)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(BulkOpResult {
        succeeded: result.rows_affected() as usize,
        failed: 0,
    })
}

#[component]
pub fn WebspaceHostList() -> Element {
    let hosts = use_server_future(list_hosts)?;
    let cd_creds = use_server_future(list_cd_creds_for_bulk)?;

    let all_rows: Vec<HostRow> = match &*hosts.read() {
        Some(Ok(r)) => r.clone(),
        _ => vec![],
    };
    let cred_list: Vec<CdCredOption> = match &*cd_creds.read() {
        Some(Ok(c)) => c.clone(),
        _ => vec![],
    };

    let mut selected: Signal<Vec<Uuid>> = use_signal(Vec::new);
    let mut bulk_result = use_signal(|| None::<BulkOpResult>);
    let mut running = use_signal(|| false);
    let mut cd_cred_id = use_signal(|| {
        cred_list
            .first()
            .map(|c| c.id.to_string())
            .unwrap_or_default()
    });
    let mut filter = use_signal(|| "all".to_string());

    let filtered_rows: Vec<&HostRow> = match filter.read().as_str() {
        "cloudflare" => all_rows.iter().filter(|r| r.kind == "cloudflare").collect(),
        "proxy" => all_rows.iter().filter(|r| r.kind != "cloudflare").collect(),
        "missing_cname" => all_rows.iter().filter(|r| r.has_missing_cname).collect(),
        "no_cd" => all_rows.iter().filter(|r| !r.has_changedetection).collect(),
        "has_cd" => all_rows.iter().filter(|r| r.has_changedetection).collect(),
        _ => all_rows.iter().collect(),
    };

    let sel = selected.read();
    let sel_count = sel.len();
    let filtered_count = filtered_rows.len();

    let no_cd_selected: Vec<Uuid> = filtered_rows
        .iter()
        .filter(|r| sel.contains(&r.id) && !r.has_changedetection)
        .map(|r| r.id)
        .collect();
    let no_cd_count = no_cd_selected.len();

    let has_cd_selected: Vec<Uuid> = filtered_rows
        .iter()
        .filter(|r| sel.contains(&r.id) && r.has_changedetection)
        .map(|r| r.id)
        .collect();
    let has_cd_count = has_cd_selected.len();

    let filtered_ids: Vec<Uuid> = filtered_rows.iter().map(|r| r.id).collect();
    drop(sel);

    rsx! {
        PageHeader { "Webspace Hosts" }

        div { class: "mt-4 flex justify-end mb-4",
            Link {
                to: crate::web::app::Route::WebspaceHostForm {},
                class: "btn btn-primary",
                "Create Host"
            }
        }

        // Filter bar
        div { class: "mb-4 flex items-center gap-2 flex-wrap",
            span { class: "text-sm text-fg-muted", "Filter:" }
            {
                let filters: Vec<(&str, &str, usize)> = vec![
                    ("all", "All", all_rows.len()),
                    ("cloudflare", "Cloudflare", all_rows.iter().filter(|r| r.kind == "cloudflare").count()),
                    ("proxy", "Proxy", all_rows.iter().filter(|r| r.kind != "cloudflare").count()),
                    ("missing_cname", "Missing CNAME", all_rows.iter().filter(|r| r.has_missing_cname).count()),
                    ("no_cd", "No Change Detection", all_rows.iter().filter(|r| !r.has_changedetection).count()),
                    ("has_cd", "Has Change Detection", all_rows.iter().filter(|r| r.has_changedetection).count()),
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

        // Bulk actions bar
        if sel_count > 0 {
            Card { class: "mb-4",
                div { class: "p-4 flex items-end gap-3 flex-wrap",
                    span { class: "text-sm font-medium", "{sel_count} selected" }

                    if no_cd_count > 0 && !cred_list.is_empty() {
                        FormField { label: "CD Credential",
                            select { class: "input w-48", value: "{cd_cred_id}", oninput: move |evt| cd_cred_id.set(evt.value()),
                                for c in &cred_list { option { value: "{c.id}", "{c.name}" } }
                            }
                        }
                        Button {
                            variant: ButtonVariant::Primary,
                            disabled: *running.read(),
                            onclick: {
                                let ids = no_cd_selected.clone();
                                move |_| {
                                    let ids = ids.clone();
                                    let cid_str = cd_cred_id.read().clone();
                                    running.set(true); bulk_result.set(None);
                                    spawn(async move {
                                        if let Ok(cid) = Uuid::parse_str(&cid_str) {
                                            match bulk_assign_changedetection(ids, cid).await {
                                                Ok(r) => bulk_result.set(Some(r)),
                                                Err(_) => bulk_result.set(Some(BulkOpResult { succeeded: 0, failed: 1 })),
                                            }
                                        }
                                        running.set(false);
                                    });
                                }
                            },
                            if *running.read() { "Assigning..." } else { "Assign CD ({no_cd_count})" }
                        }
                    }

                    if has_cd_count > 0 {
                        Button {
                            variant: ButtonVariant::Secondary,
                            disabled: *running.read(),
                            onclick: {
                                let ids = has_cd_selected.clone();
                                move |_| {
                                    let ids = ids.clone();
                                    running.set(true); bulk_result.set(None);
                                    spawn(async move {
                                        match bulk_remove_changedetection(ids).await {
                                            Ok(r) => bulk_result.set(Some(r)),
                                            Err(_) => bulk_result.set(Some(BulkOpResult { succeeded: 0, failed: 1 })),
                                        }
                                        running.set(false);
                                    });
                                }
                            },
                            if *running.read() { "Removing..." } else { "Remove CD ({has_cd_count})" }
                        }
                    }
                }
            }
        }

        // Bulk operation results
        if let Some(ref result) = *bulk_result.read() {
            Card { class: "mb-4",
                div { class: "p-4",
                    if result.succeeded > 0 {
                        div { class: "text-sm mb-2",
                            Badge { variant: BadgeVariant::Success, "{result.succeeded} updated" }
                        }
                    }
                    if result.failed > 0 {
                        div { class: "text-sm",
                            Badge { variant: BadgeVariant::Warn, "{result.failed} failed" }
                        }
                    }
                    div { class: "mt-2",
                        Button {
                            variant: ButtonVariant::Secondary,
                            onclick: move |_| {
                                bulk_result.set(None);
                                selected.set(vec![]);
                                navigator().replace(crate::web::app::Route::WebspaceHostList {});
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
                            Th { "Name" }
                            Th { "Kind" }
                            Th { "Hostname" }
                            Th { "Folders" }
                            Th { "CD" }
                            Th { "Org" }
                        }
                    }
                    tbody {
                        if filtered_rows.is_empty() {
                            tr { td { class: "td text-fg-muted text-center", colspan: "7", "No hosts yet" } }
                        }
                        for row in &filtered_rows {
                            {
                                let rid = row.id;
                                let is_selected = selected.read().contains(&rid);
                                rsx! {
                                    tr { class: "cursor-pointer hover:bg-surface-2",
                                        onclick: {
                                            let id = rid.to_string();
                                            move |_| { navigator().push(crate::web::app::Route::WebspaceHostDetail { id: id.clone() }); }
                                        },
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
                                        Td { "{row.name}" }
                                        Td {
                                            if row.kind == "cloudflare" {
                                                Badge { variant: BadgeVariant::Info, "Cloudflare" }
                                            } else {
                                                Badge { variant: BadgeVariant::Accent, "Proxy" }
                                            }
                                        }
                                        Td {
                                            if let Some(ref h) = row.hostname {
                                                span { class: "font-mono text-sm", "{h}" }
                                            } else {
                                                span { class: "text-fg-muted", "-" }
                                            }
                                        }
                                        Td { "{row.folder_count}" }
                                        Td {
                                            if row.has_changedetection {
                                                Badge { variant: BadgeVariant::Success, "Yes" }
                                            } else {
                                                span { class: "text-fg-muted", "-" }
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
