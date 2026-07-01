use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ui::{
    Badge, BadgeVariant, Button, ButtonVariant, Card, FormField, PageHeader, Td, TdMuted, Th,
};

use crate::web::user::OrgOption;

// discover/import now live in the shared api_mcp layer.
use crate::api_mcp::endpoints::webspaces::{
    DiscoveredProject, ImportResult, WebspaceDiscoverPagesInput, WebspaceImportPagesInput,
    discover_pages_projects, import_pages_projects,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredOption {
    id: Uuid,
    name: String,
}

// ── Server functions ────────────────────────────────────────────────

#[server]
async fn list_orgs() -> Result<Vec<OrgOption>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;
    crate::web::user::list_user_orgs(&user, &pool).await
}

#[server]
async fn list_cf_creds() -> Result<Vec<CredOption>, ServerFnError> {
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
        .map(|(id, name)| CredOption { id, name })
        .collect())
}

// ── Component ───────────────────────────────────────────────────────

#[component]
pub fn WebspaceImport() -> Element {
    let orgs = use_server_future(list_orgs)?;
    let creds = use_server_future(list_cf_creds)?;
    let org_list = match &*orgs.read() {
        Some(Ok(o)) => o.clone(),
        _ => vec![],
    };
    let cred_list = match &*creds.read() {
        Some(Ok(c)) => c.clone(),
        _ => vec![],
    };

    let mut org_id = use_signal(|| {
        org_list
            .first()
            .map(|o| o.id.to_string())
            .unwrap_or_default()
    });
    let mut cred_id = use_signal(|| {
        cred_list
            .first()
            .map(|c| c.id.to_string())
            .unwrap_or_default()
    });
    let mut discovered: Signal<Vec<DiscoveredProject>> = use_signal(Vec::new);
    let mut selected: Signal<Vec<String>> = use_signal(Vec::new);
    let mut discovering = use_signal(|| false);
    let mut importing = use_signal(|| false);
    let mut result = use_signal(|| None::<ImportResult>);
    let mut error = use_signal(|| None::<String>);

    rsx! {
        PageHeader { "Import Pages Projects" }

        Card {
            div { class: "p-6 space-y-4",
                div { class: "flex items-end gap-4 flex-wrap",
                    FormField { label: "Organization",
                        select { class: "input w-56", value: "{org_id}", oninput: move |evt| org_id.set(evt.value()),
                            for o in &org_list { option { value: "{o.id}", "{o.name}" } }
                        }
                    }
                    FormField { label: "Cloudflare Credential",
                        select { class: "input w-56", value: "{cred_id}", oninput: move |evt| cred_id.set(evt.value()),
                            for c in &cred_list { option { value: "{c.id}", "{c.name}" } }
                        }
                    }
                    Button {
                        variant: ButtonVariant::Primary,
                        disabled: *discovering.read() || cred_list.is_empty() || org_list.is_empty(),
                        onclick: move |_| {
                            let oid_str = org_id.read().clone();
                            let cid_str = cred_id.read().clone();
                            discovering.set(true);
                            error.set(None);
                            result.set(None);
                            discovered.set(vec![]);
                            selected.set(vec![]);
                            spawn(async move {
                                if let (Ok(oid), Ok(cid)) = (Uuid::parse_str(&oid_str), Uuid::parse_str(&cid_str)) {
                                    match discover_pages_projects(WebspaceDiscoverPagesInput { credential_id: cid, org_id: oid }).await {
                                        Ok(projects) => {
                                            let new_names: Vec<String> = projects.iter().filter(|p| !p.already_imported).map(|p| p.name.clone()).collect();
                                            selected.set(new_names);
                                            discovered.set(projects);
                                        }
                                        Err(e) => error.set(Some(format!("{e}"))),
                                    }
                                }
                                discovering.set(false);
                            });
                        },
                        if *discovering.read() { "Discovering..." } else { "Discover Projects" }
                    }
                }
                if let Some(err) = &*error.read() {
                    div { class: "text-danger text-sm", "{err}" }
                }
            }
        }

        if !discovered.read().is_empty() {
            Card { class: "mt-4",
                div { class: "overflow-x-auto",
                    table { class: "table w-full",
                        thead { tr {
                            th { class: "th w-8",
                                input { r#type: "checkbox",
                                    checked: !selected.read().is_empty(),
                                    oninput: {
                                        let projects = discovered.read().clone();
                                        move |evt| {
                                            if evt.checked() {
                                                selected.set(projects.iter().filter(|p| !p.already_imported).map(|p| p.name.clone()).collect());
                                            } else {
                                                selected.set(vec![]);
                                            }
                                        }
                                    }
                                }
                            }
                            Th { "Project" }
                            Th { "Subdomain" }
                            Th { "Status" }
                        }}
                        tbody {
                            for project in &*discovered.read() {
                                {
                                    let name = project.name.clone();
                                    let is_selected = selected.read().contains(&name);
                                    let already = project.already_imported;
                                    rsx! {
                                        tr {
                                            td { class: "td w-8",
                                                input { r#type: "checkbox", checked: is_selected, disabled: already,
                                                    oninput: {
                                                        let n = name.clone();
                                                        move |evt| {
                                                            let mut sel = selected.write();
                                                            if evt.checked() { if !sel.contains(&n) { sel.push(n.clone()); } }
                                                            else { sel.retain(|s| s != &n); }
                                                        }
                                                    }
                                                }
                                            }
                                            Td { "{name}" }
                                            TdMuted { {project.subdomain.as_deref().unwrap_or("-")} }
                                            Td {
                                                if already { Badge { variant: BadgeVariant::Neutral, "Already imported" } }
                                                else { Badge { variant: BadgeVariant::Success, "New" } }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                div { class: "p-4 border-t border-line-soft flex items-center gap-3",
                    Button {
                        variant: ButtonVariant::Primary,
                        disabled: selected.read().is_empty() || *importing.read(),
                        onclick: move |_| {
                            let oid_str = org_id.read().clone();
                            let cid_str = cred_id.read().clone();
                            let names = selected.read().clone();
                            importing.set(true);
                            error.set(None);
                            spawn(async move {
                                if let (Ok(oid), Ok(cid)) = (Uuid::parse_str(&oid_str), Uuid::parse_str(&cid_str)) {
                                    match import_pages_projects(WebspaceImportPagesInput { credential_id: cid, org_id: oid, project_names: names }).await {
                                        Ok(r) => result.set(Some(r)),
                                        Err(e) => error.set(Some(format!("{e}"))),
                                    }
                                }
                                importing.set(false);
                            });
                        },
                        if *importing.read() { "Importing..." } else { {format!("Import {} project(s)", selected.read().len())} }
                    }
                }
            }
        }

        if let Some(ref r) = *result.read() {
            Card { class: "mt-4",
                div { class: "p-4",
                    div { class: "text-sm",
                        Badge { variant: BadgeVariant::Success, "Imported: {r.imported}" }
                        span { class: "ml-2", }
                        Badge { variant: BadgeVariant::Neutral, "Skipped: {r.skipped}" }
                    }
                    if !r.errors.is_empty() {
                        div { class: "mt-2",
                            for err in &r.errors {
                                div { class: "text-danger text-sm", "{err}" }
                            }
                        }
                    }
                    div { class: "mt-3",
                        Link { to: crate::web::app::Route::WebspaceList {}, class: "btn btn-secondary", "View Webspaces" }
                    }
                }
            }
        }
    }
}
