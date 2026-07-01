use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{
    Badge, BadgeVariant, Button, ButtonVariant, Card, FormField, PageHeader, SectionHeading, Td, Th,
};

// Domain detail types + endpoints live in the shared api_mcp layer; the UI
// calls the macro-generated `#[server]` wrappers.
use crate::api_mcp::endpoints::domains::{
    DnsRecordCreateInput, DnsRecordDeleteInput, DomainDeleteInput, DomainDeployCloudflareInput,
    DomainGetInput, DomainMoveInput, DomainSetNameserversInput, DomainSyncRecordsInput,
    DomainUpdateInput, SubdomainCreateInput, SubdomainData, SubdomainDeleteInput, add_dns_record,
    create_subdomain, delete_dns_record, delete_domain, delete_subdomain, deploy_to_cloudflare,
    get_domain, move_domain, set_nameservers_at_registrar, sync_records_from_cloudflare,
    update_domain,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CfCredOption {
    id: Uuid,
    name: String,
}

// ── Server functions (form-option loaders stay local) ─────────────────

#[server]
async fn list_cf_credentials() -> Result<Vec<CfCredOption>, ServerFnError> {
    let _user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    let rows = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, name FROM credentials WHERE credential_type = 'cloudflare' ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|(id, name)| CfCredOption { id, name })
        .collect())
}

// ── Components ────────────────────────────────────────────────────────

#[component]
pub fn DomainDetail(id: String) -> Element {
    let domain_id = Uuid::parse_str(&id).ok();
    let refresh = use_context_provider(|| Signal::new(0u32));
    let domain = use_server_future(move || {
        let did = domain_id;
        let _ = *refresh.read(); // reactive dependency — bumping refresh re-runs this future
        async move {
            match did {
                Some(id) => get_domain(DomainGetInput { id }).await,
                None => Err(ServerFnError::new("invalid ID")),
            }
        }
    })?;

    let data = match &*domain.read() {
        Some(Ok(d)) => d.clone(),
        Some(Err(e)) => return rsx! { div { class: "text-danger", "Error: {e}" } },
        None => return rsx! { div { class: "text-fg-muted", "Loading..." } },
    };

    let has_cf = data.cloudflare_zone_id.is_some();

    rsx! {
        PageHeader { "{data.name}" }

        Card {
            div { class: "p-6 grid grid-cols-2 md:grid-cols-4 gap-4",
                div { div { class: "text-sm text-fg-muted", "Registrar" } div { class: "font-medium", {data.registrar_type.as_deref().unwrap_or("External")} } }
                div { div { class: "text-sm text-fg-muted", "SSL" } Badge { "{data.ssl_mode}" } }
                div { div { class: "text-sm text-fg-muted", "DNSSEC" } if data.dnssec_enabled { Badge { variant: BadgeVariant::Success, "On" } } else { span { class: "text-fg-muted", "Off" } } }
                div { div { class: "text-sm text-fg-muted", "Expires" } div { {data.expires_at.as_deref().unwrap_or("-")} } }
                div { div { class: "text-sm text-fg-muted", "Organization" } div { "{data.organization_name}" } }
            }
        }

        SectionHeading { class: "mt-6", "Cloudflare" }
        if has_cf {
            CloudflareDeployed {
                domain_id: data.id, zone_id: data.cloudflare_zone_id.clone().unwrap_or_default(),
                zone_status: data.cloudflare_zone_status.clone(), nameservers: data.cloudflare_nameservers.clone(),
                ssl_mode: data.ssl_mode.clone(), dnssec_enabled: data.dnssec_enabled, can_set_nameservers: data.can_set_nameservers,
                ai_bots_protection: data.ai_bots_protection.clone(),
            }
        } else {
            CloudflareDeployForm { domain_id: data.id, domain_name: data.name.clone(), can_set_nameservers: data.can_set_nameservers }
        }

        // DNSSEC DS record info
        if let Some(ref ds) = data.dnssec_ds {
            SectionHeading { class: "mt-6", "DNSSEC DS Record" }
            Card {
                div { class: "p-6 space-y-3",
                    div { class: "text-sm text-fg-muted", "Configure this DS record at your registrar to enable DNSSEC validation:" }
                    div { class: "font-mono text-sm bg-surface-2 px-4 py-2 rounded break-all", "{ds}" }
                    div { class: "grid grid-cols-2 md:grid-cols-4 gap-4 mt-3",
                        if let Some(ref kt) = data.dnssec_key_tag {
                            div { div { class: "text-sm text-fg-muted", "Key Tag" } div { class: "font-mono text-sm", "{kt}" } }
                        }
                        if let Some(ref alg) = data.dnssec_algorithm {
                            div { div { class: "text-sm text-fg-muted", "Algorithm" } div { class: "font-mono text-sm", "{alg}" } }
                        }
                        if let Some(ref dt) = data.dnssec_digest_type {
                            div { div { class: "text-sm text-fg-muted", "Digest Type" } div { class: "font-mono text-sm", "{dt}" } }
                        }
                        if let Some(ref dig) = data.dnssec_digest {
                            div { div { class: "text-sm text-fg-muted", "Digest" } div { class: "font-mono text-sm break-all", "{dig}" } }
                        }
                    }
                }
            }
        }

        // Subdomains & DNS records
        SectionHeading { class: "mt-6", "Subdomains & DNS Records" }
        if has_cf {
            SyncFromCloudflareButton { domain_id: data.id }
        }
        SubdomainsSection { domain_id: data.id, subdomains: data.subdomains.clone() }

        SectionHeading { class: "mt-6", "Danger Zone" }
        MoveDomainSection { domain_id: data.id, current_org_id: data.organization_id }
        Card {
            div { class: "p-6 flex items-center justify-between",
                div {
                    div { class: "font-medium text-danger", "Delete this domain" }
                    div { class: "text-sm text-fg-muted", "Removes domain and all records from the database." }
                }
                DeleteDomainButton { domain_id: data.id }
            }
        }
    }
}

// ── Subdomains section ────────────────────────────────────────────────

#[component]
fn SubdomainsSection(domain_id: Uuid, subdomains: Vec<SubdomainData>) -> Element {
    let mut refresh: Signal<u32> = use_context();
    let mut new_sub_name = use_signal(String::new);
    let mut adding_sub = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    rsx! {
        for sub in &subdomains {
            SubdomainCard { domain_id, subdomain: sub.clone() }
        }

        // Add subdomain form
        Card {
            div { class: "p-4",
                div { class: "flex items-end gap-3",
                    FormField { label: "New Subdomain",
                        input { class: "input w-48", r#type: "text", placeholder: "www, @, api, ...",
                            value: "{new_sub_name}", oninput: move |evt| new_sub_name.set(evt.value()) }
                    }
                    Button {
                        variant: ButtonVariant::Primary,
                        disabled: new_sub_name.read().is_empty() || *adding_sub.read(),
                        onclick: {
                            let did = domain_id;
                            move |_| {
                                let n = new_sub_name.read().clone();
                                adding_sub.set(true);
                                error.set(None);
                                spawn(async move {
                                    match create_subdomain(SubdomainCreateInput { domain_id: did, name: n }).await {
                                        Ok(_) => {
                                            new_sub_name.set(String::new());
                                            refresh += 1;
                                        }
                                        Err(e) => error.set(Some(format!("{e}"))),
                                    }
                                    adding_sub.set(false);
                                });
                            }
                        },
                        if *adding_sub.read() { "Creating..." } else { "Add Subdomain" }
                    }
                }
                if let Some(err) = &*error.read() {
                    div { class: "mt-2 text-danger text-sm", "{err}" }
                }
            }
        }
    }
}

#[component]
fn SubdomainCard(domain_id: Uuid, subdomain: SubdomainData) -> Element {
    let mut refresh: Signal<u32> = use_context();
    let mut new_type = use_signal(|| "A".to_string());
    let mut new_value = use_signal(String::new);
    let mut new_proxied = use_signal(|| true);
    let mut adding = use_signal(|| false);
    let mut deleting_rec: Signal<Option<Uuid>> = use_signal(|| None);
    let mut deleting_sub = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    let sub_id = subdomain.id;
    let did = domain_id;

    rsx! {
        Card { class: "mb-3",
            div { class: "px-4 py-3 border-b border-line-soft flex items-center justify-between",
                div { class: "flex items-center gap-2",
                    span { class: "font-mono font-semibold", "{subdomain.name}" }
                    Badge { variant: BadgeVariant::Neutral, "{subdomain.records.len()} record(s)" }
                }
                Button {
                    variant: ButtonVariant::Danger,
                    disabled: *deleting_sub.read(),
                    onclick: move |_| {
                        deleting_sub.set(true);
                        spawn(async move {
                            let _ = delete_subdomain(SubdomainDeleteInput { id: sub_id, domain_id: did }).await;
                            refresh += 1;
                        });
                    },
                    if *deleting_sub.read() { "..." } else { "Delete Subdomain" }
                }
            }

            if !subdomain.records.is_empty() {
                div { class: "overflow-x-auto",
                    table { class: "table w-full",
                        thead { tr { Th { "Type" } Th { "Value" } Th { "Proxied" } Th { "CF" } Th { "" } } }
                        tbody {
                            for rec in &subdomain.records {
                                {
                                    let rid = rec.id;
                                    let is_del = *deleting_rec.read() == Some(rid);
                                    rsx! {
                                        tr {
                                            Td { Badge { "{rec.record_type}" } }
                                            Td { class: "font-mono text-sm", "{rec.record_value}" }
                                            Td { if rec.proxied { Badge { variant: BadgeVariant::Success, "Yes" } } else { span { class: "text-fg-muted", "No" } } }
                                            Td { if rec.cloudflare_record_id.is_some() { Badge { variant: BadgeVariant::Info, "Synced" } } else { span { class: "text-fg-muted", "-" } } }
                                            Td {
                                                Button { variant: ButtonVariant::Danger, disabled: is_del,
                                                    onclick: move |_| {
                                                        deleting_rec.set(Some(rid));
                                                        spawn(async move {
                                                            let _ = delete_dns_record(DnsRecordDeleteInput { id: rid, domain_id: did }).await;
                                                            deleting_rec.set(None);
                                                            refresh += 1;
                                                        });
                                                    },
                                                    if is_del { "..." } else { "Del" }
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

            // Add record to this subdomain
            div { class: "p-3 border-t border-line-soft",
                div { class: "flex items-end gap-2 flex-wrap",
                    FormField { label: "Type",
                        select { class: "input w-24", value: "{new_type}", oninput: move |evt| new_type.set(evt.value()),
                            option { value: "A", "A" } option { value: "AAAA", "AAAA" } option { value: "CNAME", "CNAME" }
                            option { value: "MX", "MX" } option { value: "TXT", "TXT" } option { value: "NS", "NS" }
                        }
                    }
                    FormField { label: "Value",
                        input { class: "input w-48 font-mono text-sm", r#type: "text", placeholder: "1.2.3.4",
                            value: "{new_value}", oninput: move |evt| new_value.set(evt.value()) }
                    }
                    FormField { label: "Proxied",
                        input { class: "mt-2", r#type: "checkbox", checked: *new_proxied.read(),
                            oninput: move |evt| new_proxied.set(evt.checked()) }
                    }
                    Button { variant: ButtonVariant::Primary,
                        disabled: new_value.read().is_empty() || *adding.read(),
                        onclick: move |_| {
                            let t = new_type.read().clone();
                            let v = new_value.read().clone();
                            let p = *new_proxied.read();
                            adding.set(true); error.set(None);
                            spawn(async move {
                                match add_dns_record(DnsRecordCreateInput {
                                    domain_id: did,
                                    subdomain_id: sub_id,
                                    record_type: t,
                                    record_value: v,
                                    proxied: p,
                                }).await {
                                    Ok(()) => { new_value.set(String::new()); refresh += 1; }
                                    Err(e) => error.set(Some(format!("{e}"))),
                                }
                                adding.set(false);
                            });
                        },
                        if *adding.read() { "..." } else { "Add" }
                    }
                }
                if let Some(err) = &*error.read() {
                    div { class: "mt-1 text-danger text-sm", "{err}" }
                }
            }
        }
    }
}

// ── CF deploy + NS components (same as before) ───────────────────────

#[component]
fn CloudflareDeployForm(
    domain_id: Uuid,
    domain_name: String,
    can_set_nameservers: bool,
) -> Element {
    let mut refresh: Signal<u32> = use_context();
    let creds = use_server_future(list_cf_credentials)?;
    let cred_list = match &*creds.read() {
        Some(Ok(c)) => c.clone(),
        _ => vec![],
    };
    let mut cred_id = use_signal(|| {
        cred_list
            .first()
            .map(|c| c.id.to_string())
            .unwrap_or_default()
    });
    let mut deploying = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    rsx! {
        Card { div { class: "p-6",
            p { class: "text-fg-muted mb-4", "Deploy " span { class: "font-semibold text-fg", "{domain_name}" } " to Cloudflare." }
            if cred_list.is_empty() {
                p { class: "text-fg-muted", "No CF credentials. " Link { to: crate::web::app::Route::CredentialForm {}, class: "text-brand underline", "Add one" } " first." }
            } else {
                div { class: "flex items-end gap-3",
                    FormField { label: "Credential", select { class: "input", value: "{cred_id}", oninput: move |evt| cred_id.set(evt.value()),
                        for c in &cred_list { option { value: "{c.id}", "{c.name}" } }
                    }}
                    Button { variant: ButtonVariant::Primary, disabled: *deploying.read(),
                        onclick: { let did = domain_id; let cid_str = cred_id.read().clone();
                            move |_| { let cid_str = cid_str.clone(); deploying.set(true); error.set(None);
                                spawn(async move { if let Ok(cid) = uuid::Uuid::parse_str(&cid_str) {
                                    match deploy_to_cloudflare(DomainDeployCloudflareInput { id: did, credential_id: cid }).await { Ok(_) => refresh += 1, Err(e) => error.set(Some(format!("{e}"))) }
                                } deploying.set(false); });
                            }
                        },
                        if *deploying.read() { "Deploying..." } else { "Deploy" }
                    }
                }
            }
            if let Some(err) = &*error.read() { div { class: "mt-3 text-danger text-sm", "{err}" } }
        }}
    }
}

#[component]
fn CloudflareDeployed(
    domain_id: Uuid,
    zone_id: String,
    zone_status: Option<String>,
    nameservers: Vec<String>,
    ssl_mode: String,
    dnssec_enabled: bool,
    can_set_nameservers: bool,
    ai_bots_protection: Option<String>,
) -> Element {
    let mut refresh: Signal<u32> = use_context();
    let mut ssl = use_signal(move || ssl_mode.clone());
    let dnssec = use_signal(move || dnssec_enabled);
    let mut ai_bots = use_signal(move || ai_bots_protection.unwrap_or_default());
    let mut saving_ssl = use_signal(|| false);
    let mut saving_dnssec = use_signal(|| false);
    let mut saving_ai_bots = use_signal(|| false);
    let mut message = use_signal(|| None::<String>);

    rsx! {
        Card { div { class: "p-6 space-y-4",
            div { class: "flex items-center gap-3 flex-wrap",
                span { class: "text-sm text-fg-muted", "Zone:" } span { class: "font-mono text-sm", "{zone_id}" }
                match zone_status.as_deref() {
                    Some("active") => rsx! { Badge { variant: BadgeVariant::Success, "Active" } },
                    Some("pending") => rsx! { Badge { variant: BadgeVariant::Warn, "Pending" } },
                    Some(s) => rsx! { Badge { "{s}" } }, None => rsx! {},
                }
            }
            if !nameservers.is_empty() {
                div { div { class: "text-sm text-fg-muted mb-1", "Nameservers:" }
                    div { class: "flex gap-2 flex-wrap", for ns in &nameservers { span { class: "font-mono text-sm bg-surface-2 px-3 py-1 rounded", "{ns}" } } }
                    if can_set_nameservers && zone_status.as_deref() == Some("pending") { SetNsButton { domain_id, nameservers: nameservers.clone() } }
                }
            }
            div { class: "flex items-end gap-3",
                FormField { label: "SSL", select { class: "input w-40", value: "{ssl}", oninput: move |evt| ssl.set(evt.value()),
                    option { value: "off", "Off" } option { value: "flexible", "Flexible" } option { value: "full", "Full" } option { value: "strict", "Strict" }
                }}
                Button { variant: ButtonVariant::Secondary, disabled: *saving_ssl.read(),
                    onclick: { let did = domain_id; move |_| { let m = ssl.read().clone(); saving_ssl.set(true); message.set(None);
                        spawn(async move { match update_domain(DomainUpdateInput { id: did, ssl_mode: Some(m), dnssec_enabled: None, ai_bots_protection: None }).await { Ok(()) => refresh += 1, Err(e) => message.set(Some(format!("{e}"))) } saving_ssl.set(false); }); }},
                    if *saving_ssl.read() { "..." } else { "Update SSL" }
                }
            }
            div { class: "flex items-center gap-3",
                span { class: "text-sm", "DNSSEC:" }
                Button { variant: if *dnssec.read() { ButtonVariant::Danger } else { ButtonVariant::Primary }, disabled: *saving_dnssec.read(),
                    onclick: { let did = domain_id; move |_| { let ns = !*dnssec.read(); saving_dnssec.set(true); message.set(None);
                        spawn(async move { match update_domain(DomainUpdateInput { id: did, ssl_mode: None, dnssec_enabled: Some(ns), ai_bots_protection: None }).await { Ok(()) => refresh += 1, Err(e) => message.set(Some(format!("{e}"))) } saving_dnssec.set(false); }); }},
                    if *saving_dnssec.read() { "..." } else if *dnssec.read() { "Disable" } else { "Enable" }
                }
                if *dnssec.read() { Badge { variant: BadgeVariant::Success, "On" } }
            }
            div { class: "flex items-end gap-3",
                FormField { label: "AI Bot Protection",
                    select { class: "input w-48", value: "{ai_bots}", oninput: move |evt| ai_bots.set(evt.value()),
                        option { value: "block", "Block" }
                        option { value: "disabled", "Disabled" }
                    }
                }
                Button { variant: ButtonVariant::Secondary, disabled: *saving_ai_bots.read(),
                    onclick: { let did = domain_id; move |_| { let v = ai_bots.read().clone(); saving_ai_bots.set(true); message.set(None);
                        spawn(async move { match update_domain(DomainUpdateInput { id: did, ssl_mode: None, dnssec_enabled: None, ai_bots_protection: Some(v) }).await { Ok(()) => refresh += 1, Err(e) => message.set(Some(format!("{e}"))) } saving_ai_bots.set(false); }); }},
                    if *saving_ai_bots.read() { "..." } else { "Update" }
                }
            }
            if let Some(msg) = &*message.read() { div { class: "text-sm text-fg-muted", "{msg}" } }
        }}
    }
}

#[component]
fn SetNsButton(domain_id: Uuid, nameservers: Vec<String>) -> Element {
    let mut refresh: Signal<u32> = use_context();
    let mut setting = use_signal(|| false);
    let mut msg = use_signal(|| None::<String>);
    rsx! {
        div { class: "mt-2 flex items-center gap-3",
            Button { variant: ButtonVariant::Secondary, disabled: *setting.read(),
                onclick: { let did = domain_id; let ns = nameservers.clone();
                    move |_| { let ns = ns.clone(); setting.set(true); msg.set(None);
                        spawn(async move { match set_nameservers_at_registrar(DomainSetNameserversInput { id: did, nameservers: ns }).await { Ok(_) => refresh += 1, Err(e) => msg.set(Some(format!("{e}"))) } setting.set(false); }); }},
                if *setting.read() { "Setting..." } else { "Set NS at Registrar" }
            }
            if let Some(m) = &*msg.read() { span { class: "text-sm text-fg-muted", "{m}" } }
        }
    }
}

#[component]
fn SyncFromCloudflareButton(domain_id: Uuid) -> Element {
    let mut refresh: Signal<u32> = use_context();
    let mut syncing = use_signal(|| false);
    let mut message = use_signal(|| None::<String>);

    rsx! {
        div { class: "mb-3 flex items-center gap-3",
            Button {
                variant: ButtonVariant::Secondary,
                disabled: *syncing.read(),
                onclick: {
                    let did = domain_id;
                    move |_| {
                        syncing.set(true);
                        message.set(None);
                        spawn(async move {
                            match sync_records_from_cloudflare(DomainSyncRecordsInput { id: did }).await {
                                Ok(_) => refresh += 1,
                                Err(e) => message.set(Some(format!("{e}"))),
                            }
                            syncing.set(false);
                        });
                    }
                },
                if *syncing.read() { "Syncing..." } else { "Sync from Cloudflare" }
            }
            if let Some(msg) = &*message.read() {
                span { class: "text-sm text-fg-muted", "{msg}" }
            }
        }
    }
}

#[server]
async fn list_move_target_orgs() -> Result<Vec<crate::web::user::OrgOption>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    crate::web::user::list_user_write_orgs(&user, &pool).await
}

#[component]
fn MoveDomainSection(domain_id: Uuid, current_org_id: Uuid) -> Element {
    let mut refresh: Signal<u32> = use_context();
    let orgs = use_server_future(list_move_target_orgs)?;
    let mut selected_org = use_signal(String::new);
    let mut moving = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    let org_list = match &*orgs.read() {
        Some(Ok(list)) => list.clone(),
        _ => vec![],
    };

    // Filter out the current org
    let targets: Vec<_> = org_list
        .into_iter()
        .filter(|o| o.id != current_org_id)
        .collect();
    if targets.is_empty() {
        return rsx! {};
    }

    rsx! {
        Card {
            div { class: "p-4 flex items-center justify-between gap-4",
                div {
                    div { class: "font-medium text-danger", "Move to another organization" }
                    div { class: "text-sm text-fg-muted", "Transfers this domain to a different organization." }
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
                        onclick: {
                            let did = domain_id;
                            move |_| {
                                let target = selected_org.read().clone();
                                if let Ok(tid) = Uuid::parse_str(&target) {
                                    moving.set(true);
                                    error.set(None);
                                    spawn(async move {
                                        match move_domain(DomainMoveInput { id: did, target_org_id: tid }).await {
                                            Ok(()) => {
                                                refresh += 1;
                                            }
                                            Err(e) => {
                                                error.set(Some(format!("{e}")));
                                                moving.set(false);
                                            }
                                        }
                                    });
                                }
                            }
                        },
                        if *moving.read() { "Moving..." } else { "Move Domain" }
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
fn DeleteDomainButton(domain_id: Uuid) -> Element {
    let mut deleting = use_signal(|| false);
    let mut confirm = use_signal(|| false);
    let nav = use_navigator();

    if !*confirm.read() {
        return rsx! { Button { variant: ButtonVariant::Danger, onclick: move |_| confirm.set(true), "Delete Domain" } };
    }
    rsx! {
        div { class: "flex items-center gap-2",
            Button { variant: ButtonVariant::Danger, disabled: *deleting.read(),
                onclick: { let nav = nav.clone(); let did = domain_id; move |_| { let nav = nav.clone(); deleting.set(true);
                    spawn(async move { let _ = delete_domain(DomainDeleteInput { id: did }).await; nav.push(crate::web::app::Route::DomainList {}); }); }},
                if *deleting.read() { "Deleting..." } else { "Confirm Delete" }
            }
            Button { variant: ButtonVariant::Secondary, onclick: move |_| confirm.set(false), "Cancel" }
        }
    }
}
