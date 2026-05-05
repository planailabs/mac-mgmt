use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{Badge, BadgeVariant, Button, ButtonVariant, Card, FormField, PageHeader, SectionHeading, Td, TdMuted, Th};

// ── Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WebspaceData {
    id: Uuid,
    name: String,
    hosting_type: String,
    cloudflare_pages_project: Option<String>,
    cloudflare_credential_id: Option<Uuid>,
    runtime: Option<String>,
    local_status: Option<String>,
    organization_name: String,
    bindings: Vec<DomainBinding>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct DomainBinding {
    binding_id: Uuid,
    domain_id: Uuid,
    domain_name: String,
    subdomain_name: Option<String>,
    /// Full hostname: subdomain.domain or just domain
    hostname: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredOption {
    id: Uuid,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DomainOption {
    id: Uuid,
    name: String,
    cloudflare_zone_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PagesDeployResult {
    project_name: String,
    subdomain: Option<String>,
}

// ── Server functions ──────────────────────────────────────────────────

#[server]
async fn get_webspace(webspace_id: Uuid) -> Result<WebspaceData, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let row = sqlx::query_as::<_, (Uuid, String, String, Option<String>, Option<Uuid>, Option<String>, Option<String>, Uuid)>(
        "SELECT w.id, w.name, w.hosting_type, w.cloudflare_pages_project, w.cloudflare_credential_id, \
         w.runtime, w.local_status, w.organization_id \
         FROM webspaces w WHERE w.id = $1",
    )
    .bind(webspace_id).fetch_optional(&pool).await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("webspace not found"))?;

    let (id, name, hosting_type, cf_project, cf_cred_id, runtime, local_status, org_id) = row;

    if !user.is_admin && !user.org_ids().contains(&org_id) {
        return Err(ServerFnError::new("access denied"));
    }

    let org_name = sqlx::query_scalar::<_, String>("SELECT name FROM organizations WHERE id = $1")
        .bind(org_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let bindings = sqlx::query_as::<_, (Uuid, Uuid, String, Option<String>)>(
        "SELECT wd.id, wd.domain_id, d.name, s.name \
         FROM webspace_domains wd \
         JOIN domains d ON d.id = wd.domain_id \
         LEFT JOIN subdomains s ON s.id = wd.subdomain_id \
         WHERE wd.webspace_id = $1 ORDER BY d.name",
    )
    .bind(webspace_id).fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?
    .into_iter()
    .map(|(binding_id, domain_id, domain_name, subdomain_name)| {
        let hostname = match &subdomain_name {
            Some(sub) if sub != "@" => format!("{sub}.{domain_name}"),
            _ => domain_name.clone(),
        };
        DomainBinding { binding_id, domain_id, domain_name, subdomain_name, hostname }
    })
    .collect();

    Ok(WebspaceData {
        id, name, hosting_type, cloudflare_pages_project: cf_project,
        cloudflare_credential_id: cf_cred_id, runtime, local_status,
        organization_name: org_name, bindings,
    })
}

#[server]
async fn list_cf_creds_for_pages() -> Result<Vec<CredOption>, ServerFnError> {
    let _user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let rows = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, name FROM credentials WHERE credential_type = 'cloudflare' ORDER BY name",
    )
    .fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(rows.into_iter().map(|(id, name)| CredOption { id, name }).collect())
}

#[server]
async fn list_domains_for_binding(webspace_id: Uuid) -> Result<Vec<DomainOption>, ServerFnError> {
    let _user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT organization_id FROM webspaces WHERE id = $1",
    )
    .bind(webspace_id).fetch_one(&pool).await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let rows = sqlx::query_as::<_, (Uuid, String, Option<String>)>(
        "SELECT id, name, cloudflare_zone_id FROM domains WHERE organization_id = $1 ORDER BY name",
    )
    .bind(org_id).fetch_all(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows.into_iter().map(|(id, name, cloudflare_zone_id)| DomainOption { id, name, cloudflare_zone_id }).collect())
}

/// Deploy a CF Pages project for this webspace.
#[server]
async fn deploy_pages_project(webspace_id: Uuid, credential_id: Uuid) -> Result<PagesDeployResult, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let row = sqlx::query_as::<_, (String, Uuid)>(
        "SELECT name, organization_id FROM webspaces WHERE id = $1",
    )
    .bind(webspace_id).fetch_optional(&pool).await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("webspace not found"))?;

    let (ws_name, org_id) = row;
    if !user.is_admin && !user.write_org_ids().contains(&org_id) {
        return Err(ServerFnError::new("write access required"));
    }

    let (client, account_id) = build_cf_pages_client(&pool, credential_id).await?;

    // Check if project already exists
    let project = match client.get_pages_project(&account_id, &ws_name).await {
        Ok(p) => {
            tracing::info!("Pages project {ws_name} already exists");
            p
        }
        Err(_) => {
            let p = client.create_pages_project(&account_id, &ws_name, "main").await
                .map_err(|e| ServerFnError::new(format!("failed to create Pages project: {e}")))?;
            tracing::info!("created Pages project {ws_name}");
            p
        }
    };

    sqlx::query(
        "UPDATE webspaces SET cloudflare_pages_project = $1, cloudflare_credential_id = $2, updated_at = now() WHERE id = $3",
    )
    .bind(&ws_name).bind(credential_id).bind(webspace_id)
    .execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(PagesDeployResult {
        project_name: ws_name,
        subdomain: project.subdomain,
    })
}

/// Bind a domain to this webspace. For CF Pages webspaces, adds the custom domain.
#[server]
async fn bind_domain(webspace_id: Uuid, domain_id: Uuid, hostname: String) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT organization_id FROM webspaces WHERE id = $1",
    )
    .bind(webspace_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    if !user.is_admin && !user.write_org_ids().contains(&org_id) {
        return Err(ServerFnError::new("write access required"));
    }

    // Insert binding
    sqlx::query(
        "INSERT INTO webspace_domains (webspace_id, domain_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(webspace_id).bind(domain_id)
    .execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    // For CF Pages webspaces, add as custom domain
    let ws = sqlx::query_as::<_, (Option<String>, Option<Uuid>)>(
        "SELECT cloudflare_pages_project, cloudflare_credential_id FROM webspaces WHERE id = $1",
    )
    .bind(webspace_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    if let (Some(project_name), Some(cred_id)) = ws {
        let (client, account_id) = build_cf_pages_client(&pool, cred_id).await?;
        if let Err(e) = client.add_pages_custom_domain(&account_id, &project_name, &hostname).await {
            tracing::warn!("failed to add custom domain {hostname} to Pages project {project_name}: {e}");
            // Don't fail the binding — CF custom domain can be added later
        } else {
            tracing::info!("added custom domain {hostname} to Pages project {project_name}");
        }
    }

    Ok(())
}

/// Remove a domain binding. For CF Pages, removes the custom domain.
#[server]
async fn unbind_domain(webspace_id: Uuid, binding_id: Uuid, hostname: String) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    let org_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT organization_id FROM webspaces WHERE id = $1",
    )
    .bind(webspace_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    if !user.is_admin && !user.write_org_ids().contains(&org_id) {
        return Err(ServerFnError::new("write access required"));
    }

    // Remove CF Pages custom domain
    let ws = sqlx::query_as::<_, (Option<String>, Option<Uuid>)>(
        "SELECT cloudflare_pages_project, cloudflare_credential_id FROM webspaces WHERE id = $1",
    )
    .bind(webspace_id).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    if let (Some(project_name), Some(cred_id)) = ws {
        let (client, account_id) = build_cf_pages_client(&pool, cred_id).await?;
        let _ = client.remove_pages_custom_domain(&account_id, &project_name, &hostname).await;
    }

    sqlx::query("DELETE FROM webspace_domains WHERE id = $1")
        .bind(binding_id).execute(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[cfg(feature = "server")]
async fn build_cf_pages_client(pool: &sqlx::PgPool, cred_id: Uuid) -> Result<(cloudflare_api::Client, String), ServerFnError> {
    let encrypted = sqlx::query_scalar::<_, Vec<u8>>(
        "SELECT encrypted_data FROM credentials WHERE id = $1 AND credential_type = 'cloudflare'",
    )
    .bind(cred_id).fetch_optional(pool).await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("Cloudflare credential not found"))?;

    let decrypted = crate::crypto::decrypt(&encrypted)
        .map_err(|e| ServerFnError::new(format!("decryption failed: {e}")))?;
    let data: serde_json::Value = serde_json::from_slice(&decrypted)
        .map_err(|e| ServerFnError::new(format!("invalid credential: {e}")))?;

    let token = data["api_token"].as_str()
        .ok_or_else(|| ServerFnError::new("missing api_token"))?;
    let account_id = data["account_id"].as_str().unwrap_or("").to_string();

    if account_id.is_empty() {
        return Err(ServerFnError::new("account_id required in credential for Pages"));
    }

    Ok((cloudflare_api::Client::new(token), account_id))
}

// ── Component ─────────────────────────────────────────────────────────

#[component]
pub fn WebspaceDetail(id: String) -> Element {
    let webspace_id = Uuid::parse_str(&id).ok();
    let webspace = use_server_future(move || {
        let wid = webspace_id;
        async move {
            match wid {
                Some(id) => get_webspace(id).await,
                None => Err(ServerFnError::new("invalid webspace ID")),
            }
        }
    })?;

    let data = match &*webspace.read() {
        Some(Ok(d)) => d.clone(),
        Some(Err(e)) => return rsx! { div { class: "text-danger", "Error: {e}" } },
        None => return rsx! { div { class: "text-fg-muted", "Loading..." } },
    };

    let is_pages = data.hosting_type == "cloudflare_pages";
    let has_project = data.cloudflare_pages_project.is_some();

    rsx! {
        PageHeader { "{data.name}" }

        // Info card
        Card {
            div { class: "p-6 grid grid-cols-2 md:grid-cols-4 gap-4",
                div {
                    div { class: "text-sm text-fg-muted", "Hosting Type" }
                    match data.hosting_type.as_str() {
                        "cloudflare_pages" => rsx! { Badge { variant: BadgeVariant::Info, "Cloudflare Pages" } },
                        "local" => rsx! { Badge { "Local" } },
                        _ => rsx! { span { "{data.hosting_type}" } },
                    }
                }
                if is_pages {
                    div {
                        div { class: "text-sm text-fg-muted", "Pages Project" }
                        if let Some(ref proj) = data.cloudflare_pages_project {
                            span { class: "font-mono text-sm", "{proj}" }
                        } else {
                            span { class: "text-fg-muted", "Not deployed" }
                        }
                    }
                } else {
                    div {
                        div { class: "text-sm text-fg-muted", "Runtime" }
                        div { {data.runtime.as_deref().unwrap_or("-")} }
                    }
                    div {
                        div { class: "text-sm text-fg-muted", "Status" }
                        match data.local_status.as_deref() {
                            Some("running") => rsx! { Badge { variant: BadgeVariant::Success, "Running" } },
                            Some("error") => rsx! { Badge { variant: BadgeVariant::Danger, "Error" } },
                            Some(s) => rsx! { Badge { "{s}" } },
                            None => rsx! { span { class: "text-fg-muted", "-" } },
                        }
                    }
                }
                div {
                    div { class: "text-sm text-fg-muted", "Organization" }
                    div { "{data.organization_name}" }
                }
            }
        }

        // CF Pages deployment
        if is_pages && !has_project {
            SectionHeading { class: "mt-6", "Deploy Pages Project" }
            PagesDeploySection { webspace_id: data.id }
        }

        // Domain bindings
        SectionHeading { class: "mt-6", "Domain Bindings" }
        DomainBindingsSection {
            webspace_id: data.id,
            bindings: data.bindings.clone(),
            is_pages,
        }
    }
}

/// Deploy a CF Pages project for this webspace.
#[component]
fn PagesDeploySection(webspace_id: Uuid) -> Element {
    let creds = use_server_future(list_cf_creds_for_pages)?;
    let cred_list = match &*creds.read() {
        Some(Ok(c)) => c.clone(),
        _ => vec![],
    };

    let mut cred_id = use_signal(|| cred_list.first().map(|c| c.id.to_string()).unwrap_or_default());
    let mut deploying = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut result = use_signal(|| None::<PagesDeployResult>);

    if let Some(res) = &*result.read() {
        return rsx! {
            Card {
                div { class: "p-6",
                    div { class: "flex items-center gap-2 mb-2",
                        Badge { variant: BadgeVariant::Success, "Deployed" }
                        span { class: "font-mono text-sm", "{res.project_name}" }
                    }
                    if let Some(ref sub) = res.subdomain {
                        div { class: "text-sm text-fg-muted",
                            "Preview URL: "
                            span { class: "font-mono", "https://{sub}" }
                        }
                    }
                    div { class: "mt-2 text-sm text-fg-muted", "Reload the page to manage domain bindings." }
                }
            }
        };
    }

    rsx! {
        Card {
            div { class: "p-6",
                if cred_list.is_empty() {
                    p { class: "text-fg-muted",
                        "No Cloudflare credentials. "
                        Link { to: crate::web::app::Route::CredentialForm {}, class: "text-brand underline", "Add one" }
                        " first."
                    }
                } else {
                    div { class: "flex items-end gap-3",
                        FormField { label: "Cloudflare Credential",
                            select {
                                class: "input",
                                value: "{cred_id}",
                                oninput: move |evt| cred_id.set(evt.value()),
                                for c in &cred_list {
                                    option { value: "{c.id}", "{c.name}" }
                                }
                            }
                        }
                        Button {
                            variant: ButtonVariant::Primary,
                            disabled: *deploying.read(),
                            onclick: {
                                let wid = webspace_id;
                                let cid_str = cred_id.read().clone();
                                move |_| {
                                    let cid_str = cid_str.clone();
                                    deploying.set(true);
                                    error.set(None);
                                    spawn(async move {
                                        if let Ok(cid) = uuid::Uuid::parse_str(&cid_str) {
                                            match deploy_pages_project(wid, cid).await {
                                                Ok(r) => result.set(Some(r)),
                                                Err(e) => error.set(Some(format!("{e}"))),
                                            }
                                        }
                                        deploying.set(false);
                                    });
                                }
                            },
                            if *deploying.read() { "Creating project..." } else { "Create Pages Project" }
                        }
                    }
                }
                if let Some(err) = &*error.read() {
                    div { class: "mt-3 text-danger text-sm", "{err}" }
                }
            }
        }
    }
}

/// Domain bindings list + add form.
#[component]
fn DomainBindingsSection(webspace_id: Uuid, bindings: Vec<DomainBinding>, is_pages: bool) -> Element {
    let domains = use_server_future(move || {
        let wid = webspace_id;
        async move { list_domains_for_binding(wid).await }
    })?;
    let domain_list = match &*domains.read() {
        Some(Ok(d)) => d.clone(),
        _ => vec![],
    };

    let mut selected_domain = use_signal(String::new);
    let mut adding = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut removing: Signal<Option<Uuid>> = use_signal(|| None);

    rsx! {
        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead {
                        tr {
                            Th { "Hostname" }
                            Th { "Domain" }
                            if is_pages {
                                Th { "CF Pages" }
                            }
                            Th { "" }
                        }
                    }
                    tbody {
                        if bindings.is_empty() {
                            tr {
                                td {
                                    class: "td text-fg-muted text-center",
                                    colspan: if is_pages { "4" } else { "3" },
                                    "No domains bound"
                                }
                            }
                        }
                        for b in &bindings {
                            {
                                let bid = b.binding_id;
                                let hostname = b.hostname.clone();
                                let is_removing = *removing.read() == Some(bid);
                                rsx! {
                                    tr {
                                        Td { class: "font-mono", "{b.hostname}" }
                                        TdMuted { "{b.domain_name}" }
                                        if is_pages {
                                            Td { Badge { variant: BadgeVariant::Info, "Custom Domain" } }
                                        }
                                        Td {
                                            Button {
                                                variant: ButtonVariant::Danger,
                                                disabled: is_removing,
                                                onclick: {
                                                    let wid = webspace_id;
                                                    let hostname = hostname.clone();
                                                    move |_| {
                                                        let hostname = hostname.clone();
                                                        removing.set(Some(bid));
                                                        spawn(async move {
                                                            let _ = unbind_domain(wid, bid, hostname).await;
                                                            removing.set(None);
                                                            // Trigger re-fetch by navigating to same page
                                                            navigator().replace(crate::web::app::Route::WebspaceDetail { id: wid.to_string() });
                                                        });
                                                    }
                                                },
                                                if is_removing { "Removing..." } else { "Remove" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Add binding form
            div { class: "p-4 border-t border-line-soft",
                div { class: "flex items-end gap-3",
                    FormField { label: "Add Domain",
                        select {
                            class: "input",
                            value: "{selected_domain}",
                            oninput: move |evt| selected_domain.set(evt.value()),
                            option { value: "", "Select a domain..." }
                            for d in &domain_list {
                                option { value: "{d.id}|{d.name}", "{d.name}" }
                            }
                        }
                    }
                    Button {
                        variant: ButtonVariant::Primary,
                        disabled: selected_domain.read().is_empty() || *adding.read(),
                        onclick: {
                            let wid = webspace_id;
                            let sel = selected_domain.read().clone();
                            move |_| {
                                let sel = sel.clone();
                                adding.set(true);
                                error.set(None);
                                spawn(async move {
                                    if let Some((id_str, hostname)) = sel.split_once('|') {
                                        if let Ok(did) = uuid::Uuid::parse_str(id_str) {
                                            match bind_domain(wid, did, hostname.to_string()).await {
                                                Ok(()) => {
                                                    navigator().replace(crate::web::app::Route::WebspaceDetail { id: wid.to_string() });
                                                }
                                                Err(e) => error.set(Some(format!("{e}"))),
                                            }
                                        }
                                    }
                                    adding.set(false);
                                });
                            }
                        },
                        if *adding.read() { "Adding..." } else { "Bind Domain" }
                    }
                }
                if let Some(err) = &*error.read() {
                    div { class: "mt-2 text-danger text-sm", "{err}" }
                }
            }
        }
    }
}
