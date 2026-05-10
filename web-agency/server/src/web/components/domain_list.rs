use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Badge, BadgeVariant, Button, ButtonVariant, Card, FormField, PageHeader, Td, TdMuted, Th};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DomainRow {
    id: Uuid,
    name: String,
    registrar_type: Option<String>,
    ssl_mode: String,
    dnssec_enabled: bool,
    cloudflare_zone_id: Option<String>,
    expires_at: Option<String>,
    organization_name: String,
    /// Whether the domain can have nameservers set at registrar (spaceship with credential).
    can_set_nameservers: bool,
    /// Cached NS match status: None = unknown, Some(true) = NS match, Some(false) = mismatch.
    ns_ok: Option<bool>,
    has_webspace: bool,
    ai_bots_protection: Option<String>,
    expires_soon: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CfCredOption { id: Uuid, name: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BulkOpResult {
    succeeded: Vec<String>,
    failed: Vec<(String, String)>,
}

#[server]
async fn list_domains() -> Result<Vec<DomainRow>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_ids = user.org_ids();
    if org_ids.is_empty() && !user.is_admin {
        return Ok(vec![]);
    }

    type Row = (Uuid, String, Option<String>, String, bool, Option<String>, Option<chrono::DateTime<chrono::Utc>>, String, Option<String>, Option<Uuid>, Option<bool>, bool, Option<String>, bool);
    let query = "SELECT d.id, d.name, d.registrar_type, d.ssl_mode, d.dnssec_enabled, d.cloudflare_zone_id, d.expires_at, o.name, \
         d.registrar_type, d.registrar_credential_id, d.ns_ok, \
         EXISTS(SELECT 1 FROM webspace_domains wd WHERE wd.domain_id = d.id) AS has_webspace, \
         d.ai_bots_protection, \
         (d.expires_at IS NOT NULL AND d.expires_at < now() + interval '30 days') AS expires_soon \
         FROM domains d JOIN organizations o ON o.id = d.organization_id";

    let rows = if user.is_admin {
        sqlx::query_as::<_, Row>(&format!("{query} ORDER BY d.name"))
            .fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
    } else {
        sqlx::query_as::<_, Row>(&format!("{query} WHERE d.organization_id = ANY($1) ORDER BY d.name"))
            .bind(&org_ids).fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
    };

    Ok(rows
        .into_iter()
        .map(|(id, name, registrar_type, ssl_mode, dnssec_enabled, cloudflare_zone_id, expires_at, organization_name, reg_type, reg_cred_id, ns_ok, has_webspace, ai_bots_protection, expires_soon)| DomainRow {
            id,
            name,
            registrar_type,
            ssl_mode,
            dnssec_enabled,
            cloudflare_zone_id,
            expires_at: expires_at.map(|d| d.format("%Y-%m-%d").to_string()),
            organization_name,
            can_set_nameservers: reg_type.as_deref() == Some("spaceship") && reg_cred_id.is_some(),
            ns_ok,
            has_webspace,
            ai_bots_protection,
            expires_soon,
        })
        .collect())
}

#[server]
async fn list_cf_credentials_for_bulk() -> Result<Vec<CfCredOption>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let rows = if user.is_admin {
        sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, name FROM credentials WHERE credential_type = 'cloudflare' ORDER BY name",
        ).fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
    } else {
        let org_ids = user.org_ids();
        sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, name FROM credentials WHERE credential_type = 'cloudflare' \
             AND (organization_id = ANY($1) OR organization_id IS NULL) ORDER BY name",
        ).bind(&org_ids).fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
    };
    Ok(rows.into_iter().map(|(id, name)| CfCredOption { id, name }).collect())
}

/// Deploy multiple domains to Cloudflare under one credential.
#[server]
async fn bulk_deploy_to_cloudflare(domain_ids: Vec<Uuid>, credential_id: Uuid) -> Result<BulkOpResult, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    if !user.is_admin {
        return Err(ServerFnError::new("admin required"));
    }
    let pool = crate::server_pool()?;

    let (client, account_id) = crate::credentials::cf_client_with_account(&pool, credential_id).await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;

    let mut succeeded = Vec::new();
    let mut failed = Vec::new();

    for domain_id in &domain_ids {
        let row = sqlx::query_as::<_, (String,)>(
            "SELECT name FROM domains WHERE id = $1",
        ).bind(domain_id).fetch_optional(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
        let Some((domain_name,)) = row else { continue };

        match deploy_single(&client, &account_id, &pool, *domain_id, &domain_name, credential_id).await {
            Ok(()) => succeeded.push(domain_name),
            Err(e) => failed.push((domain_name, format!("{e}"))),
        }
    }

    Ok(BulkOpResult { succeeded, failed })
}

#[cfg(feature = "server")]
async fn deploy_single(
    client: &cloudflare_api::compat::SimpleClient,
    account_id: &str,
    pool: &sqlx::PgPool,
    domain_id: Uuid,
    domain_name: &str,
    credential_id: Uuid,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let existing = client.list_zones(Some(domain_name)).await?;
    let zone = if let Some(z) = existing.into_iter().find(|z| z.name == domain_name) {
        z
    } else {
        client.create_zone(domain_name, account_id).await?
    };

    sqlx::query("UPDATE domains SET cloudflare_zone_id = $1, cloudflare_credential_id = $2, updated_at = now() WHERE id = $3")
        .bind(&zone.id).bind(credential_id).bind(domain_id)
        .execute(pool).await?;

    Ok(())
}

/// Set Cloudflare nameservers at registrar for domains that have a CF zone
/// but haven't had their nameservers updated yet.
#[server]
async fn bulk_set_nameservers(domain_ids: Vec<Uuid>) -> Result<BulkOpResult, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    if !user.is_admin {
        return Err(ServerFnError::new("admin required"));
    }
    let pool = crate::server_pool()?;

    let mut succeeded = Vec::new();
    let mut failed = Vec::new();

    for domain_id in &domain_ids {
        let row = sqlx::query_as::<_, (String, Option<String>, Option<Uuid>)>(
            "SELECT name, cloudflare_zone_id, cloudflare_credential_id FROM domains WHERE id = $1",
        ).bind(domain_id).fetch_optional(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

        let Some((domain_name, Some(zone_id), Some(cred_id))) = row else {
            continue; // skip domains without CF zone
        };

        match set_ns_single(&pool, *domain_id, &domain_name, &zone_id, cred_id).await {
            Ok(()) => succeeded.push(domain_name),
            Err(e) => failed.push((domain_name, format!("{e}"))),
        }
    }

    Ok(BulkOpResult { succeeded, failed })
}

#[cfg(feature = "server")]
async fn set_ns_single(
    pool: &sqlx::PgPool,
    domain_id: Uuid,
    domain_name: &str,
    zone_id: &str,
    cred_id: Uuid,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cf_client = crate::credentials::cf_client(pool, cred_id).await?;
    let zone = cf_client.get_zone(zone_id).await?;
    let nameservers = zone.name_servers.unwrap_or_default();
    if nameservers.is_empty() {
        return Err("zone has no nameservers".into());
    }

    let reg = sqlx::query_as::<_, (Option<String>, Option<Uuid>)>(
        "SELECT registrar_type, registrar_credential_id FROM domains WHERE id = $1",
    ).bind(domain_id).fetch_one(pool).await?;

    match reg {
        (Some(ref rt), Some(reg_cred_id)) if rt == "spaceship" => {
            let ss_client = crate::credentials::spaceship_client(pool, reg_cred_id).await?;
            ss_client.set_nameservers(domain_name, &spaceship_api::compat::NameserverConfig {
                provider: "custom".into(), hosts: Some(nameservers),
            }).await?;
            Ok(())
        }
        _ => Err("no supported registrar".into()),
    }
}

/// Bulk set SSL mode for domains with CF zones.
#[server]
async fn bulk_set_ssl_mode(domain_ids: Vec<Uuid>, ssl_mode: String) -> Result<BulkOpResult, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    if !user.is_admin { return Err(ServerFnError::new("admin required")); }
    let pool = crate::server_pool()?;

    let mut succeeded = Vec::new();
    let mut failed = Vec::new();

    for domain_id in &domain_ids {
        let row = sqlx::query_as::<_, (String, Option<String>, Option<Uuid>)>(
            "SELECT name, cloudflare_zone_id, cloudflare_credential_id FROM domains WHERE id = $1",
        ).bind(domain_id).fetch_optional(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

        let Some((domain_name, Some(zone_id), Some(cred_id))) = row else { continue };

        match async {
            let client = crate::credentials::cf_client(&pool, cred_id).await?;
            client.set_ssl_mode(&zone_id, &ssl_mode).await?;
            sqlx::query("UPDATE domains SET ssl_mode = $1, updated_at = now() WHERE id = $2")
                .bind(&ssl_mode).bind(domain_id).execute(&pool).await?;
            Ok::<_, anyhow::Error>(())
        }.await {
            Ok(()) => succeeded.push(domain_name),
            Err(e) => failed.push((domain_name, format!("{e}"))),
        }
    }
    Ok(BulkOpResult { succeeded, failed })
}

/// Bulk set AI bot protection for domains with CF zones.
#[server]
async fn bulk_set_ai_bots_protection(domain_ids: Vec<Uuid>, value: String) -> Result<BulkOpResult, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    if !user.is_admin { return Err(ServerFnError::new("admin required")); }
    let pool = crate::server_pool()?;

    let mut succeeded = Vec::new();
    let mut failed = Vec::new();

    for domain_id in &domain_ids {
        let row = sqlx::query_as::<_, (String, Option<String>, Option<Uuid>)>(
            "SELECT name, cloudflare_zone_id, cloudflare_credential_id FROM domains WHERE id = $1",
        ).bind(domain_id).fetch_optional(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

        let Some((domain_name, Some(zone_id), Some(cred_id))) = row else { continue };

        match async {
            let client = crate::credentials::cf_client(&pool, cred_id).await?;
            client.set_bot_management(&zone_id, &serde_json::json!({"ai_bots_protection": value})).await?;
            sqlx::query("UPDATE domains SET ai_bots_protection = $1, updated_at = now() WHERE id = $2")
                .bind(&value).bind(domain_id).execute(&pool).await?;
            Ok::<_, anyhow::Error>(())
        }.await {
            Ok(()) => succeeded.push(domain_name),
            Err(e) => failed.push((domain_name, format!("{e}"))),
        }
    }
    Ok(BulkOpResult { succeeded, failed })
}

/// Bulk create CF Pages direct-upload projects for domains without webspaces.
#[server]
async fn bulk_create_pages_project(domain_ids: Vec<Uuid>, credential_id: Uuid) -> Result<BulkOpResult, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    if !user.is_admin { return Err(ServerFnError::new("admin required")); }
    let pool = crate::server_pool()?;

    let (client, account_id) = crate::credentials::cf_client_with_account(&pool, credential_id).await
        .map_err(|e| ServerFnError::new(format!("{e}")))?;

    let mut succeeded = Vec::new();
    let mut failed = Vec::new();

    for domain_id in &domain_ids {
        let row = sqlx::query_as::<_, (String, Uuid)>(
            "SELECT name, organization_id FROM domains WHERE id = $1",
        ).bind(domain_id).fetch_optional(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

        let Some((domain_name, org_id)) = row else { continue };

        // Project name: replace dots with hyphens (CF Pages doesn't allow dots)
        let project_name = domain_name.replace('.', "-");

        match async {
            let project = client.create_pages_project(&account_id, &project_name, "main").await?;
            let project_id = project.id.clone();

            // Create webspace
            let ws_id = sqlx::query_scalar::<_, Uuid>(
                "INSERT INTO webspaces (organization_id, name, hosting_type, cloudflare_pages_project, cloudflare_pages_project_id, cloudflare_credential_id) \
                 VALUES ($1, $2, 'cloudflare_pages', $3, $4, $5) RETURNING id",
            )
            .bind(org_id).bind(&project_name).bind(&project_name).bind(&project_id).bind(credential_id)
            .fetch_one(&pool).await?;

            // Link domain to webspace
            sqlx::query(
                "INSERT INTO webspace_domains (webspace_id, domain_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            ).bind(ws_id).bind(domain_id).execute(&pool).await?;

            Ok::<_, anyhow::Error>(())
        }.await {
            Ok(()) => succeeded.push(domain_name),
            Err(e) => failed.push((domain_name, format!("{e}"))),
        }
    }
    if !succeeded.is_empty() {
        crate::api::internal::notify_proxy_reload();
    }
    Ok(BulkOpResult { succeeded, failed })
}

#[component]
pub fn DomainList() -> Element {
    let domains = use_server_future(list_domains)?;
    let creds = use_server_future(list_cf_credentials_for_bulk)?;
    let all_rows: Vec<DomainRow> = match &*domains.read() {
        Some(Ok(r)) => r.clone(),
        _ => vec![],
    };
    let cred_list: Vec<CfCredOption> = match &*creds.read() { Some(Ok(c)) => c.clone(), _ => vec![] };

    let mut selected: Signal<Vec<Uuid>> = use_signal(Vec::new);
    let mut bulk_result = use_signal(|| None::<BulkOpResult>);
    let mut running = use_signal(|| false);
    let mut cred_id = use_signal(|| cred_list.first().map(|c| c.id.to_string()).unwrap_or_default());
    let mut filter = use_signal(|| "all".to_string());
    let mut bulk_ssl_mode = use_signal(|| "full".to_string());
    let mut bulk_ai_bots = use_signal(|| "block".to_string());

    // Apply filter
    let filtered_rows: Vec<&DomainRow> = match filter.read().as_str() {
        "no_cf" => all_rows.iter().filter(|r| r.cloudflare_zone_id.is_none()).collect(),
        "needs_ns" => all_rows.iter().filter(|r| r.ns_ok == Some(false)).collect(),
        "no_webspace" => all_rows.iter().filter(|r| !r.has_webspace && r.cloudflare_zone_id.is_some()).collect(),
        "ai_crawl_off" => all_rows.iter().filter(|r| r.ai_bots_protection.as_deref() != Some("block")).collect(),
        "ai_crawl_on" => all_rows.iter().filter(|r| r.ai_bots_protection.as_deref() == Some("block")).collect(),
        "ssl_not_full" => all_rows.iter().filter(|r| r.cloudflare_zone_id.is_some() && r.ssl_mode != "full" && r.ssl_mode != "strict").collect(),
        "expires_soon" => all_rows.iter().filter(|r| r.expires_soon).collect(),
        _ => all_rows.iter().collect(),
    };

    // Count how many selected domains are eligible for each operation
    let sel = selected.read();
    let no_cf_count = filtered_rows.iter().filter(|r| sel.contains(&r.id) && r.cloudflare_zone_id.is_none()).count();
    let ns_eligible_count = filtered_rows.iter().filter(|r| sel.contains(&r.id) && r.ns_ok == Some(false)).count();
    let sel_count = sel.len();
    let filtered_count = filtered_rows.len();
    // Collect IDs for bulk ops before dropping sel
    let deploy_ids: Vec<Uuid> = filtered_rows.iter()
        .filter(|r| sel.contains(&r.id) && r.cloudflare_zone_id.is_none())
        .map(|r| r.id).collect();
    let ns_ids: Vec<Uuid> = filtered_rows.iter()
        .filter(|r| sel.contains(&r.id) && r.ns_ok == Some(false))
        .map(|r| r.id).collect();
    let filtered_ids: Vec<Uuid> = filtered_rows.iter().map(|r| r.id).collect();
    let cf_selected_count = filtered_rows.iter().filter(|r| sel.contains(&r.id) && r.cloudflare_zone_id.is_some()).count();
    let cf_selected_ids: Vec<Uuid> = filtered_rows.iter()
        .filter(|r| sel.contains(&r.id) && r.cloudflare_zone_id.is_some())
        .map(|r| r.id).collect();
    let no_ws_count = filtered_rows.iter().filter(|r| sel.contains(&r.id) && !r.has_webspace && r.cloudflare_zone_id.is_some()).count();
    let no_ws_ids: Vec<Uuid> = filtered_rows.iter()
        .filter(|r| sel.contains(&r.id) && !r.has_webspace && r.cloudflare_zone_id.is_some())
        .map(|r| r.id).collect();
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
                                            match bulk_deploy_to_cloudflare(ids, cid).await {
                                                Ok(r) => bulk_result.set(Some(r)),
                                                Err(e) => bulk_result.set(Some(BulkOpResult { succeeded: vec![], failed: vec![("error".into(), format!("{e}"))] })),
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
                                        match bulk_set_nameservers(ids).await {
                                            Ok(r) => bulk_result.set(Some(r)),
                                            Err(e) => bulk_result.set(Some(BulkOpResult { succeeded: vec![], failed: vec![("error".into(), format!("{e}"))] })),
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
                                        match bulk_set_ssl_mode(ids, mode).await {
                                            Ok(r) => bulk_result.set(Some(r)),
                                            Err(e) => bulk_result.set(Some(BulkOpResult { succeeded: vec![], failed: vec![("error".into(), format!("{e}"))] })),
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
                                        match bulk_set_ai_bots_protection(ids, val).await {
                                            Ok(r) => bulk_result.set(Some(r)),
                                            Err(e) => bulk_result.set(Some(BulkOpResult { succeeded: vec![], failed: vec![("error".into(), format!("{e}"))] })),
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
                                            match bulk_create_pages_project(ids, cid).await {
                                                Ok(r) => bulk_result.set(Some(r)),
                                                Err(e) => bulk_result.set(Some(BulkOpResult { succeeded: vec![], failed: vec![("error".into(), format!("{e}"))] })),
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
                            for (name, err) in &result.failed {
                                div { class: "text-sm text-fg-muted mt-1", "{name}: {err}" }
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
