//! Webspace-host detail page.
//!
//! A webspace-host owns the hostname: domain bindings, the CNAME flow, and
//! ChangeDetection all live here. Its `webspace` children are path-mounted
//! folders. `kind` is either `proxy` (folders route through web-agency-proxy;
//! CNAME → agency_domain) or `cloudflare` (a single Pages-project folder at `/`;
//! CNAME → {project}.pages.dev).

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{
    Badge, BadgeVariant, Button, ButtonVariant, Card, FormField, PageHeader, SectionHeading, Td,
    TdMuted, Th,
};

// The host CRUD + domain-binding + ChangeDetection endpoints (and their DTOs)
// live in the shared api_mcp layer.
use crate::api_mcp::endpoints::webspace_hosts::{
    DomainBinding, HostBindDomainInput, HostDeleteInput, HostFixCnameInput, HostGetInput,
    HostMoveInput, HostRecheckCustomDomainInput, HostSetChangedetectionInput,
    HostUnbindDomainInput, HostUpdateInput, bind_domain, delete_host, fix_cname, get_host,
    move_host, recheck_custom_domain, set_host_changedetection, unbind_domain,
    update_host_settings,
};

// ── Types ─────────────────────────────────────────────────────────────

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
    subdomains: Vec<SubdomainOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SubdomainOption {
    id: Uuid,
    name: String,
}

// ── Server functions ──────────────────────────────────────────────────

#[server]
async fn list_domains_for_binding(host_id: Uuid) -> Result<Vec<DomainOption>, ServerFnError> {
    let _user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let (org_id, _kind) = crate::api_mcp::endpoints::webspace_hosts::host_org(&pool, host_id)
        .await
        .map_err(crate::web::user::to_serverfn)?;

    let rows = sqlx::query_as::<_, (Uuid, String, Option<String>)>(
        "SELECT id, name, cloudflare_zone_id FROM domains WHERE organization_id = $1 ORDER BY name",
    )
    .bind(org_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut domains = Vec::new();
    for (id, name, cloudflare_zone_id) in rows {
        let subs = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, name FROM subdomains WHERE domain_id = $1 ORDER BY name",
        )
        .bind(id)
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
        domains.push(DomainOption {
            id,
            name,
            cloudflare_zone_id,
            subdomains: subs
                .into_iter()
                .map(|(sid, sname)| SubdomainOption {
                    id: sid,
                    name: sname,
                })
                .collect(),
        });
    }
    Ok(domains)
}

#[server]
async fn list_cd_creds() -> Result<Vec<CredOption>, ServerFnError> {
    let _user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let rows = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, name FROM credentials WHERE credential_type = 'changedetection' ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|(id, name)| CredOption { id, name })
        .collect())
}

#[server]
async fn list_host_move_target_orgs() -> Result<Vec<crate::web::user::OrgOption>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    crate::web::user::list_user_write_orgs(&user, &pool).await
}

// ── Component ─────────────────────────────────────────────────────────

#[component]
pub fn WebspaceHostDetail(id: String) -> Element {
    let host_id = Uuid::parse_str(&id).ok();
    let refresh = use_context_provider(|| Signal::new(0u32));
    let host = use_server_future(move || {
        let hid = host_id;
        let _ = *refresh.read();
        async move {
            match hid {
                Some(id) => get_host(HostGetInput { id }).await,
                None => Err(ServerFnError::new("invalid host ID")),
            }
        }
    })?;

    let data = match &*host.read() {
        Some(Ok(d)) => d.clone(),
        Some(Err(e)) => return rsx! { div { class: "text-danger", "Error: {e}" } },
        None => return rsx! { div { class: "text-fg-muted", "Loading..." } },
    };

    let is_cloudflare = data.kind == "cloudflare";

    rsx! {
        PageHeader { "{data.name}" }

        Card {
            div { class: "p-6 grid grid-cols-2 md:grid-cols-4 gap-4",
                div {
                    div { class: "text-sm text-fg-muted", "Kind" }
                    if is_cloudflare {
                        Badge { variant: BadgeVariant::Info, "Cloudflare" }
                    } else {
                        Badge { variant: BadgeVariant::Accent, "Proxy" }
                    }
                }
                div {
                    div { class: "text-sm text-fg-muted", "Organization" }
                    div { "{data.organization_name}" }
                }
                div {
                    div { class: "text-sm text-fg-muted", "Folders" }
                    div { "{data.folders.len()}" }
                }
            }
        }

        // Folders
        div { class: "mt-6 flex items-center justify-between",
            SectionHeading { "Folders" }
            if !is_cloudflare {
                Link {
                    to: crate::web::app::Route::WebspaceForm { host_id: data.id.to_string() },
                    class: "btn btn-primary",
                    "Add Folder"
                }
            }
        }
        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead {
                        tr {
                            Th { "Path" }
                            Th { "Name" }
                            Th { "Type" }
                            Th { "Runtime / Status" }
                        }
                    }
                    tbody {
                        if data.folders.is_empty() {
                            tr { td { class: "td text-fg-muted text-center", colspan: "4", "No folders" } }
                        }
                        for f in &data.folders {
                            {
                                let fid = f.id.to_string();
                                rsx! {
                                    tr { class: "cursor-pointer hover:bg-surface-2",
                                        onclick: move |_| { navigator().push(crate::web::app::Route::WebspaceDetail { id: fid.clone() }); },
                                        Td { class: "font-mono", "{f.path_prefix}" }
                                        Td { "{f.name}" }
                                        Td {
                                            match f.hosting_type.as_str() {
                                                "cloudflare_pages" => rsx! { Badge { variant: BadgeVariant::Info, "CF Pages" } },
                                                "local" => rsx! { Badge { "Local" } },
                                                "relay" => rsx! { Badge { variant: BadgeVariant::Accent, "Relay" } },
                                                "tunnel" => rsx! { Badge { variant: BadgeVariant::Accent, "Tunnel" } },
                                                _ => rsx! { span { "-" } },
                                            }
                                        }
                                        Td {
                                            if let Some(ref rt) = f.runtime {
                                                Badge { "{rt}" }
                                                if let Some(ref st) = f.local_status {
                                                    span { class: "text-sm text-fg-muted ml-2", "{st}" }
                                                }
                                            } else {
                                                span { class: "text-fg-muted", "-" }
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

        // Change Detection
        SectionHeading { class: "mt-6", "Change Detection" }
        ChangeDetectionSection {
            host_id: data.id,
            current_credential_id: data.changedetection_credential_id,
            current_credential_name: data.changedetection_credential_name.clone(),
        }
        Card { class: "mt-2",
            div { class: "p-4",
                Link {
                    to: crate::web::app::Route::WebspaceChangedetection { id: data.id.to_string() },
                    class: "text-brand underline text-sm",
                    "Settings & Notifications"
                }
            }
        }

        // Domain bindings
        SectionHeading { class: "mt-6", "Domain Bindings" }
        DomainBindingsSection {
            host_id: data.id,
            kind: data.kind.clone(),
            bindings: data.bindings.clone(),
        }

        // Settings
        SectionHeading { class: "mt-6", "Settings" }
        HostSettingsSection { host_id: data.id, current_name: data.name.clone() }
        MoveHostSection { host_id: data.id, current_org_id: data.organization_id }
        DeleteHostSection { host_id: data.id }
    }
}

#[component]
fn HostSettingsSection(host_id: Uuid, current_name: String) -> Element {
    let mut refresh: Signal<u32> = use_context();
    let mut name = use_signal(move || current_name.clone());
    let mut saving = use_signal(|| false);
    let mut message = use_signal(|| None::<String>);

    rsx! {
        Card {
            div { class: "p-4 flex items-end gap-3",
                FormField { label: "Name",
                    input {
                        class: "input w-64",
                        value: "{name}",
                        oninput: move |evt| name.set(evt.value()),
                    }
                }
                Button {
                    variant: ButtonVariant::Primary,
                    disabled: name.read().is_empty() || *saving.read(),
                    onclick: move |_| {
                        let n = name.read().clone();
                        saving.set(true);
                        message.set(None);
                        spawn(async move {
                            match update_host_settings(HostUpdateInput { id: host_id, name: n }).await {
                                Ok(()) => { message.set(Some("Saved".into())); refresh += 1; }
                                Err(e) => message.set(Some(format!("{e}"))),
                            }
                            saving.set(false);
                        });
                    },
                    if *saving.read() { "Saving..." } else { "Save" }
                }
                if let Some(ref msg) = *message.read() {
                    span { class: "text-sm text-fg-muted", "{msg}" }
                }
            }
        }
    }
}

#[component]
fn MoveHostSection(host_id: Uuid, current_org_id: Uuid) -> Element {
    let mut refresh: Signal<u32> = use_context();
    let orgs = use_server_future(list_host_move_target_orgs)?;
    let mut selected_org = use_signal(String::new);
    let mut moving = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    let targets: Vec<_> = match &*orgs.read() {
        Some(Ok(list)) => list
            .iter()
            .filter(|o| o.id != current_org_id)
            .cloned()
            .collect(),
        _ => vec![],
    };
    if targets.is_empty() {
        return rsx! {};
    }

    rsx! {
        Card { class: "mt-2",
            div { class: "p-4 flex items-center justify-between gap-4",
                div {
                    div { class: "font-medium", "Move to another organization" }
                    div { class: "text-sm text-fg-muted", "Transfers this host and its folders to a different organization." }
                }
                div { class: "flex items-center gap-2",
                    select {
                        class: "input w-48",
                        value: "{selected_org}",
                        onchange: move |evt| selected_org.set(evt.value()),
                        option { value: "", "Select org..." }
                        for org in &targets {
                            option { value: "{org.id}", "{org.name}" }
                        }
                    }
                    Button {
                        variant: ButtonVariant::Danger,
                        disabled: selected_org.read().is_empty() || *moving.read(),
                        onclick: move |_| {
                            let target = selected_org.read().clone();
                            if let Ok(tid) = Uuid::parse_str(&target) {
                                moving.set(true);
                                error.set(None);
                                spawn(async move {
                                    match move_host(HostMoveInput { id: host_id, target_org_id: tid }).await {
                                        Ok(()) => { refresh += 1; }
                                        Err(e) => { error.set(Some(format!("{e}"))); moving.set(false); }
                                    }
                                });
                            }
                        },
                        if *moving.read() { "Moving..." } else { "Move Host" }
                    }
                }
            }
            if let Some(err) = &*error.read() {
                div { class: "px-4 pb-4 text-danger text-sm", "{err}" }
            }
        }
    }
}

#[component]
fn DeleteHostSection(host_id: Uuid) -> Element {
    let mut deleting = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    rsx! {
        Card { class: "mt-2",
            div { class: "p-4 flex items-center justify-between",
                div {
                    div { class: "text-sm font-medium text-danger", "Delete this host" }
                    div { class: "text-sm text-fg-muted", "All folders, domain bindings, and deployments will be removed." }
                }
                Button {
                    variant: ButtonVariant::Danger,
                    disabled: *deleting.read(),
                    onclick: move |_| {
                        deleting.set(true);
                        error.set(None);
                        spawn(async move {
                            match delete_host(HostDeleteInput { id: host_id }).await {
                                Ok(()) => { navigator().push(crate::web::app::Route::WebspaceHostList {}); }
                                Err(e) => { error.set(Some(format!("{e}"))); deleting.set(false); }
                            }
                        });
                    },
                    if *deleting.read() { "Deleting..." } else { "Delete Host" }
                }
            }
            if let Some(err) = &*error.read() {
                div { class: "px-4 pb-4 text-danger text-sm", "{err}" }
            }
        }
    }
}

#[component]
fn ChangeDetectionSection(
    host_id: Uuid,
    current_credential_id: Option<Uuid>,
    current_credential_name: Option<String>,
) -> Element {
    let mut refresh: Signal<u32> = use_context();
    let cd_creds = use_server_future(list_cd_creds)?;
    let cred_list: Vec<CredOption> = match &*cd_creds.read() {
        Some(Ok(c)) => c.clone(),
        _ => vec![],
    };

    let mut selected_cred = use_signal(move || {
        current_credential_id
            .map(|id| id.to_string())
            .unwrap_or_default()
    });
    let mut saving = use_signal(|| false);
    let mut result_msg = use_signal(|| None::<String>);

    rsx! {
        Card {
            div { class: "p-6 space-y-4",
                div { class: "flex items-center gap-3",
                    span { class: "text-sm text-fg-muted", "Current:" }
                    if let Some(ref name) = current_credential_name {
                        Badge { variant: BadgeVariant::Info, "{name}" }
                    } else {
                        span { class: "text-fg-muted text-sm", "Not configured" }
                    }
                }
                if cred_list.is_empty() {
                    div { class: "text-sm text-fg-muted", "No ChangeDetection.io credentials available. Create one first." }
                } else {
                    div { class: "flex items-end gap-3",
                        FormField { label: "Credential",
                            select {
                                class: "input w-64",
                                value: "{selected_cred}",
                                oninput: move |evt| selected_cred.set(evt.value()),
                                option { value: "", "None" }
                                for c in &cred_list {
                                    option { value: "{c.id}", "{c.name}" }
                                }
                            }
                        }
                        Button {
                            variant: ButtonVariant::Primary,
                            disabled: *saving.read(),
                            onclick: move |_| {
                                let cred_str = selected_cred.read().clone();
                                saving.set(true);
                                result_msg.set(None);
                                spawn(async move {
                                    let cid = uuid::Uuid::parse_str(&cred_str).ok();
                                    match set_host_changedetection(HostSetChangedetectionInput {
                                        id: host_id,
                                        credential_id: cid,
                                    })
                                    .await
                                    {
                                        Ok(()) => { result_msg.set(Some("Saved".into())); *refresh.write() += 1; }
                                        Err(e) => result_msg.set(Some(format!("Error: {e}"))),
                                    }
                                    saving.set(false);
                                });
                            },
                            if *saving.read() { "Saving..." } else { "Save" }
                        }
                    }
                }
                if let Some(ref msg) = *result_msg.read() {
                    div { class: "text-sm text-fg-muted", "{msg}" }
                }
            }
        }
    }
}

#[component]
fn DomainBindingsSection(host_id: Uuid, kind: String, bindings: Vec<DomainBinding>) -> Element {
    let mut refresh: Signal<u32> = use_context();
    let is_cloudflare = kind == "cloudflare";
    let domains =
        use_server_future(move || async move { list_domains_for_binding(host_id).await })?;
    let domain_list = match &*domains.read() {
        Some(Ok(d)) => d.clone(),
        _ => vec![],
    };

    let mut selected_domain = use_signal(String::new);
    let mut adding = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut removing: Signal<Option<Uuid>> = use_signal(|| None);
    let mut fixing: Signal<Option<Uuid>> = use_signal(|| None);
    let mut rechecking: Signal<Option<Uuid>> = use_signal(|| None);

    rsx! {
        Card {
            div { class: "overflow-x-auto",
                table { class: "table w-full",
                    thead {
                        tr {
                            Th { "Hostname" }
                            Th { "Domain" }
                            Th { "CNAME" }
                            if is_cloudflare {
                                Th { "Verification" }
                            }
                            Th { "" }
                        }
                    }
                    tbody {
                        if bindings.is_empty() {
                            tr { td { class: "td text-fg-muted text-center", colspan: if is_cloudflare { "5" } else { "4" }, "No domains bound" } }
                        }
                        for b in &bindings {
                            {
                                let bid = b.binding_id;
                                let did = b.domain_id;
                                let sub_id = b.subdomain_id;
                                let hostname = b.hostname.clone();
                                let cname_ok = b.cname_ok;
                                let cf_status = b.cf_domain_status.clone();
                                let is_removing = *removing.read() == Some(bid);
                                let is_fixing = *fixing.read() == Some(bid);
                                let is_rechecking = *rechecking.read() == Some(bid);
                                rsx! {
                                    tr {
                                        Td { class: "font-mono", "{b.hostname}" }
                                        TdMuted { "{b.domain_name}" }
                                        Td {
                                            if cname_ok {
                                                Badge { variant: BadgeVariant::Success, "OK" }
                                            } else {
                                                div { class: "flex items-center gap-2",
                                                    Badge { variant: BadgeVariant::Danger, "Missing" }
                                                    Button {
                                                        variant: ButtonVariant::Secondary,
                                                        disabled: is_fixing,
                                                        onclick: {
                                                            let hostname = hostname.clone();
                                                            move |_| {
                                                                let hostname = hostname.clone();
                                                                fixing.set(Some(bid));
                                                                spawn(async move {
                                                                    let _ = fix_cname(HostFixCnameInput { id: host_id, domain_id: did, subdomain_id: sub_id, hostname }).await;
                                                                    fixing.set(None);
                                                                    refresh += 1;
                                                                });
                                                            }
                                                        },
                                                        if is_fixing { "Fixing..." } else { "Fix" }
                                                    }
                                                }
                                            }
                                        }
                                        if is_cloudflare {
                                            Td {
                                                match cf_status.as_deref() {
                                                    Some("active") => rsx! { Badge { variant: BadgeVariant::Success, "Active" } },
                                                    Some("pending") | Some("verifying") => rsx! {
                                                        div { class: "flex items-center gap-2",
                                                            Badge { variant: BadgeVariant::Warn, {cf_status.as_deref().unwrap_or("pending")} }
                                                            Button {
                                                                variant: ButtonVariant::Secondary,
                                                                disabled: is_rechecking,
                                                                onclick: {
                                                                    let hostname = hostname.clone();
                                                                    move |_| {
                                                                        let hostname = hostname.clone();
                                                                        rechecking.set(Some(bid));
                                                                        spawn(async move {
                                                                            let _ = recheck_custom_domain(HostRecheckCustomDomainInput { id: host_id, hostname }).await;
                                                                            rechecking.set(None);
                                                                            refresh += 1;
                                                                        });
                                                                    }
                                                                },
                                                                if is_rechecking { "..." } else { "Recheck" }
                                                            }
                                                        }
                                                    },
                                                    Some(s) => rsx! { Badge { "{s}" } },
                                                    None => rsx! {
                                                        div { class: "flex items-center gap-2",
                                                            Badge { variant: BadgeVariant::Danger, "Not on CF" }
                                                            Button {
                                                                variant: ButtonVariant::Secondary,
                                                                disabled: is_rechecking,
                                                                onclick: {
                                                                    let hostname = hostname.clone();
                                                                    move |_| {
                                                                        let hostname = hostname.clone();
                                                                        rechecking.set(Some(bid));
                                                                        spawn(async move {
                                                                            let _ = recheck_custom_domain(HostRecheckCustomDomainInput { id: host_id, hostname }).await;
                                                                            rechecking.set(None);
                                                                            refresh += 1;
                                                                        });
                                                                    }
                                                                },
                                                                if is_rechecking { "..." } else { "Register" }
                                                            }
                                                        }
                                                    },
                                                }
                                            }
                                        }
                                        Td {
                                            Button {
                                                variant: ButtonVariant::Danger,
                                                disabled: is_removing,
                                                onclick: {
                                                    let hostname = hostname.clone();
                                                    move |_| {
                                                        let hostname = hostname.clone();
                                                        removing.set(Some(bid));
                                                        spawn(async move {
                                                            let _ = unbind_domain(HostUnbindDomainInput { id: host_id, binding_id: bid, hostname }).await;
                                                            removing.set(None);
                                                            refresh += 1;
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

            div { class: "p-4 border-t border-line-soft",
                div { class: "flex items-end gap-3",
                    FormField { label: "Bind Domain / Subdomain",
                        select {
                            class: "input",
                            value: "{selected_domain}",
                            oninput: move |evt| selected_domain.set(evt.value()),
                            option { value: "", "Select..." }
                            for d in &domain_list {
                                option { value: "{d.id}||{d.name}", "{d.name} (root)" }
                                for s in &d.subdomains {
                                    {
                                        let hostname = if s.name == "@" { d.name.clone() } else { format!("{}.{}", s.name, d.name) };
                                        rsx! { option { value: "{d.id}|{s.id}|{hostname}", "  {hostname}" } }
                                    }
                                }
                            }
                        }
                    }
                    Button {
                        variant: ButtonVariant::Primary,
                        disabled: selected_domain.read().is_empty() || *adding.read(),
                        onclick: {
                            let sel = selected_domain.read().clone();
                            move |_| {
                                let sel = sel.clone();
                                adding.set(true);
                                error.set(None);
                                spawn(async move {
                                    let parts: Vec<&str> = sel.split('|').collect();
                                    if parts.len() == 3 {
                                        let domain_id = Uuid::parse_str(parts[0]).ok();
                                        let subdomain_id = Uuid::parse_str(parts[1]).ok();
                                        let hostname = parts[2].to_string();
                                        if let Some(domain_id) = domain_id {
                                            match bind_domain(HostBindDomainInput { id: host_id, domain_id, subdomain_id, hostname }).await {
                                                Ok(()) => { selected_domain.set(String::new()); refresh += 1; }
                                                Err(e) => error.set(Some(format!("{e}"))),
                                            }
                                        }
                                    }
                                    adding.set(false);
                                });
                            }
                        },
                        if *adding.read() { "Binding..." } else { "Bind" }
                    }
                }
                if let Some(ref e) = *error.read() {
                    div { class: "text-danger text-sm mt-2", "{e}" }
                }
            }
        }
    }
}
