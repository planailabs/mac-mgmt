use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{
    Badge, BadgeVariant, Button, ButtonVariant, Card, FormField, PageHeader, Td, TdMuted, Th,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WebspaceRow {
    id: Uuid,
    name: String,
    hosting_type: String,
    runtime: Option<String>,
    local_status: Option<String>,
    cloudflare_pages_project: Option<String>,
    relay_url: Option<String>,
    auth_mode: String,
    domain_count: i64,
    organization_name: String,
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
async fn list_webspaces() -> Result<Vec<WebspaceRow>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_ids = user.org_ids();
    if org_ids.is_empty() && !user.is_admin {
        return Ok(vec![]);
    }

    // has_missing_cname: the webspace has at least one domain binding where
    // no CNAME dns_record exists for that subdomain on that domain.
    let missing_cname_subquery = "\
        EXISTS( \
            SELECT 1 FROM webspace_domains wd2 \
            JOIN domains d2 ON d2.id = wd2.domain_id \
            LEFT JOIN subdomains s2 ON s2.id = wd2.subdomain_id \
            WHERE wd2.webspace_id = w.id \
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
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        i64,
        String,
        bool,
        bool,
    );
    let rows = if user.is_admin {
        sqlx::query_as::<_, Row>(
            &format!(
                "SELECT w.id, w.name, w.hosting_type, w.runtime, w.local_status, w.cloudflare_pages_project, w.relay_url, w.auth_mode, \
                 (SELECT count(*) FROM webspace_domains wd WHERE wd.webspace_id = w.id), o.name, \
                 w.changedetection_credential_id IS NOT NULL, \
                 {missing_cname_subquery} \
                 FROM webspaces w JOIN organizations o ON o.id = w.organization_id \
                 ORDER BY w.name"
            ),
        )
        .fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
    } else {
        sqlx::query_as::<_, Row>(
            &format!(
                "SELECT w.id, w.name, w.hosting_type, w.runtime, w.local_status, w.cloudflare_pages_project, w.relay_url, w.auth_mode, \
                 (SELECT count(*) FROM webspace_domains wd WHERE wd.webspace_id = w.id), o.name, \
                 w.changedetection_credential_id IS NOT NULL, \
                 {missing_cname_subquery} \
                 FROM webspaces w JOIN organizations o ON o.id = w.organization_id \
                 WHERE w.organization_id = ANY($1) \
                 ORDER BY w.name"
            ),
        )
        .bind(&org_ids)
        .fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
    };

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                name,
                hosting_type,
                runtime,
                local_status,
                cloudflare_pages_project,
                relay_url,
                auth_mode,
                domain_count,
                organization_name,
                has_changedetection,
                has_missing_cname,
            )| {
                WebspaceRow {
                    id,
                    name,
                    hosting_type,
                    runtime,
                    local_status,
                    cloudflare_pages_project,
                    relay_url,
                    auth_mode,
                    domain_count,
                    organization_name,
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
    webspace_ids: Vec<Uuid>,
    credential_id: Uuid,
) -> Result<BulkOpResult, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    if !user.is_admin {
        return Err(ServerFnError::new("admin required"));
    }
    let pool = crate::server_pool()?;

    let result = sqlx::query(
        "UPDATE webspaces SET changedetection_credential_id = $1, updated_at = now() WHERE id = ANY($2)",
    )
    .bind(credential_id)
    .bind(&webspace_ids)
    .execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(BulkOpResult {
        succeeded: result.rows_affected() as usize,
        failed: 0,
    })
}

#[server]
async fn bulk_remove_changedetection(
    webspace_ids: Vec<Uuid>,
) -> Result<BulkOpResult, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    if !user.is_admin {
        return Err(ServerFnError::new("admin required"));
    }
    let pool = crate::server_pool()?;

    let result = sqlx::query(
        "UPDATE webspaces SET changedetection_credential_id = NULL, updated_at = now() \
         WHERE id = ANY($1) AND changedetection_credential_id IS NOT NULL",
    )
    .bind(&webspace_ids)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(BulkOpResult {
        succeeded: result.rows_affected() as usize,
        failed: 0,
    })
}

#[component]
pub fn WebspaceList() -> Element {
    let webspaces = use_server_future(list_webspaces)?;
    let cd_creds = use_server_future(list_cd_creds_for_bulk)?;

    let all_rows: Vec<WebspaceRow> = match &*webspaces.read() {
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

    // Apply filter
    let filtered_rows: Vec<&WebspaceRow> = match filter.read().as_str() {
        "pages" => all_rows
            .iter()
            .filter(|r| r.hosting_type == "cloudflare_pages")
            .collect(),
        "local" => all_rows
            .iter()
            .filter(|r| r.hosting_type == "local")
            .collect(),
        "relay" => all_rows
            .iter()
            .filter(|r| r.hosting_type == "relay")
            .collect(),
        "tunnel" => all_rows
            .iter()
            .filter(|r| r.hosting_type == "tunnel")
            .collect(),
        "missing_cname" => all_rows.iter().filter(|r| r.has_missing_cname).collect(),
        "no_cd" => all_rows.iter().filter(|r| !r.has_changedetection).collect(),
        "has_cd" => all_rows.iter().filter(|r| r.has_changedetection).collect(),
        _ => all_rows.iter().collect(),
    };

    let sel = selected.read();
    let sel_count = sel.len();
    let filtered_count = filtered_rows.len();

    // Count selected webspaces without CD for assign
    let no_cd_selected: Vec<Uuid> = filtered_rows
        .iter()
        .filter(|r| sel.contains(&r.id) && !r.has_changedetection)
        .map(|r| r.id)
        .collect();
    let no_cd_count = no_cd_selected.len();

    // Count selected webspaces with CD for remove
    let has_cd_selected: Vec<Uuid> = filtered_rows
        .iter()
        .filter(|r| sel.contains(&r.id) && r.has_changedetection)
        .map(|r| r.id)
        .collect();
    let has_cd_count = has_cd_selected.len();

    let filtered_ids: Vec<Uuid> = filtered_rows.iter().map(|r| r.id).collect();
    drop(sel);

    rsx! {
        PageHeader { "Webspaces" }

        div { class: "mt-4 flex justify-end gap-3 mb-4",
            Link {
                to: crate::web::app::Route::WebspaceImport {},
                class: "btn btn-secondary",
                "Import from Credential"
            }
            Link {
                to: crate::web::app::Route::WebspaceForm {},
                class: "btn btn-primary",
                "Create Webspace"
            }
        }

        // Filter bar
        div { class: "mb-4 flex items-center gap-2 flex-wrap",
            span { class: "text-sm text-fg-muted", "Filter:" }
            {
                let filters: Vec<(&str, &str, usize)> = vec![
                    ("all", "All", all_rows.len()),
                    ("pages", "Pages", all_rows.iter().filter(|r| r.hosting_type == "cloudflare_pages").count()),
                    ("local", "Local", all_rows.iter().filter(|r| r.hosting_type == "local").count()),
                    ("relay", "Relay", all_rows.iter().filter(|r| r.hosting_type == "relay").count()),
                    ("tunnel", "Tunnel", all_rows.iter().filter(|r| r.hosting_type == "tunnel").count()),
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

                    // Assign changedetection
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

                    // Remove changedetection
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
                                navigator().replace(crate::web::app::Route::WebspaceList {});
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
                            Th { "Type" }
                            Th { "Target" }
                            Th { "Status" }
                            Th { "Auth" }
                            Th { "CD" }
                            Th { "Domains" }
                            Th { "Org" }
                        }
                    }
                    tbody {
                        if filtered_rows.is_empty() {
                            tr {
                                td { class: "td text-fg-muted text-center", colspan: "9", "No webspaces yet" }
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
                                                    navigator().push(crate::web::app::Route::WebspaceDetail { id: id.clone() });
                                                }
                                            },
                                            "{row.name}"
                                        }
                                        Td {
                                            match row.hosting_type.as_str() {
                                                "cloudflare_pages" => rsx! { Badge { variant: BadgeVariant::Info, "CF Pages" } },
                                                "local" => rsx! { Badge { "Local" } },
                                                "relay" => rsx! { Badge { variant: BadgeVariant::Accent, "Relay" } },
                                                "tunnel" => rsx! { Badge { variant: BadgeVariant::Accent, "Tunnel" } },
                                                _ => rsx! { span { "-" } },
                                            }
                                        }
                                        Td {
                                            if let Some(ref proj) = row.cloudflare_pages_project {
                                                span { class: "font-mono text-sm", "{proj}" }
                                            } else if let Some(ref url) = row.relay_url {
                                                span { class: "font-mono text-sm truncate max-w-xs", title: "{url}", "{url}" }
                                            } else if let Some(ref rt) = row.runtime {
                                                Badge { "{rt}" }
                                            } else {
                                                span { class: "text-fg-muted", "-" }
                                            }
                                        }
                                        Td {
                                            match (row.hosting_type.as_str(), row.local_status.as_deref()) {
                                                ("local", Some("running")) => rsx! { Badge { variant: BadgeVariant::Success, "Running" } },
                                                ("local", Some("error")) => rsx! { Badge { variant: BadgeVariant::Danger, "Error" } },
                                                ("local", Some("starting")) => rsx! { Badge { variant: BadgeVariant::Warn, "Starting" } },
                                                ("local", Some("stopped")) => rsx! { span { class: "text-fg-muted", "Stopped" } },
                                                _ => rsx! { span { class: "text-fg-muted", "-" } },
                                            }
                                        }
                                        Td {
                                            match row.auth_mode.as_str() {
                                                "oidc" => rsx! { Badge { variant: BadgeVariant::Info, "OIDC" } },
                                                "basic" => rsx! { Badge { variant: BadgeVariant::Accent, "Basic" } },
                                                _ => rsx! { span { class: "text-fg-muted", "-" } },
                                            }
                                        }
                                        Td {
                                            if row.has_changedetection {
                                                Badge { variant: BadgeVariant::Success, "Yes" }
                                            } else {
                                                span { class: "text-fg-muted", "-" }
                                            }
                                        }
                                        Td { "{row.domain_count}" }
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
