use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use super::ui::{Button, ButtonVariant, PageHeader};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DashboardStats {
    is_admin: bool,
    domains: i64,
    domains_expiring_soon: i64,
    webspaces: i64,
    credentials: i64,
    organizations: i64,
    /// Only set when billing is visible (admin, or user has orgs with show_billing).
    billing_total_cents: Option<i64>,
}

#[server]
async fn get_dashboard_stats() -> Result<DashboardStats, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

    if user.is_admin {
        let domains = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM domains")
            .fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
        let domains_expiring_soon = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM domains WHERE expires_at IS NOT NULL AND expires_at < now() + interval '30 days'",
        ).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
        let webspaces = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM webspaces")
            .fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
        let credentials = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM credentials")
            .fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
        let organizations = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM organizations")
            .fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
        let billing_total_cents = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT sum(amount_cents) FROM billing_entries",
        ).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?.unwrap_or(0);

        Ok(DashboardStats {
            is_admin: true, domains, domains_expiring_soon, webspaces, credentials, organizations,
            billing_total_cents: Some(billing_total_cents),
        })
    } else {
        let org_ids = user.org_ids();
        if org_ids.is_empty() {
            return Ok(DashboardStats::default());
        }

        let domains = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM domains WHERE organization_id = ANY($1)",
        ).bind(&org_ids).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
        let domains_expiring_soon = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM domains WHERE organization_id = ANY($1) AND expires_at IS NOT NULL AND expires_at < now() + interval '30 days'",
        ).bind(&org_ids).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
        let webspaces = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM webspaces WHERE organization_id = ANY($1)",
        ).bind(&org_ids).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;
        let credentials = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM credentials WHERE organization_id = ANY($1) OR organization_id IS NULL",
        ).bind(&org_ids).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

        // Billing: only show if user has at least one org with show_billing = true
        let billing_total_cents = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT sum(b.amount_cents) FROM billing_entries b \
             JOIN organizations o ON o.id = b.organization_id \
             WHERE b.organization_id = ANY($1) AND o.show_billing = true",
        ).bind(&org_ids).fetch_one(&pool).await.map_err(|e| ServerFnError::new(e.to_string()))?;

        Ok(DashboardStats {
            is_admin: false, domains, domains_expiring_soon, webspaces, credentials,
            organizations: 0,
            billing_total_cents,
        })
    }
}

#[server]
async fn trigger_sync() -> Result<String, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    if !user.is_admin {
        return Err(ServerFnError::new("admin required"));
    }
    let pool = crate::server_pool()?;
    let pool2 = pool.clone();
    tokio::spawn(async move {
        crate::api::sync::trigger_sync(&pool2).await;
    });
    Ok("Sync triggered".to_string())
}

#[component]
pub fn Dashboard() -> Element {
    let stats = use_server_future(get_dashboard_stats)?;
    let s = match &*stats.read() {
        Some(Ok(s)) => s.clone(),
        _ => DashboardStats::default(),
    };

    let mut sync_status = use_signal(|| None::<String>);

    rsx! {
        PageHeader { "Dashboard" }

        p { class: "text-fg-muted mb-6", "Domain and webspace management overview" }

        div { class: "grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4",
            StatCard { label: "Domains", value: s.domains.to_string() }
            StatCard { label: "Expiring (30d)", value: s.domains_expiring_soon.to_string(), warn: s.domains_expiring_soon > 0 }
            StatCard { label: "Webspaces", value: s.webspaces.to_string() }
            StatCard { label: "Credentials", value: s.credentials.to_string() }
            if s.is_admin {
                StatCard { label: "Organizations", value: s.organizations.to_string() }
            }
            if let Some(cents) = s.billing_total_cents {
                StatCard {
                    label: "Billing Total",
                    value: format!("{:.2} EUR", cents as f64 / 100.0),
                }
            }
        }

        if s.is_admin {
            div { class: "mt-8",
                h3 { class: "text-lg font-semibold text-fg mb-3", "Admin Actions" }
                div { class: "flex items-center gap-4",
                    Button {
                        variant: ButtonVariant::Secondary,
                        onclick: move |_| {
                            spawn(async move {
                                sync_status.set(Some("Syncing...".to_string()));
                                match trigger_sync().await {
                                    Ok(msg) => sync_status.set(Some(msg)),
                                    Err(e) => sync_status.set(Some(format!("Error: {e}"))),
                                }
                            });
                        },
                        "Trigger Sync"
                    }
                    if let Some(msg) = sync_status() {
                        span { class: "text-sm text-fg-muted", "{msg}" }
                    }
                }
            }
        }
    }
}

#[component]
fn StatCard(label: &'static str, value: String, #[props(default)] warn: bool) -> Element {
    rsx! {
        div { class: "card p-6",
            div { class: "text-sm text-fg-muted", "{label}" }
            div { class: if warn { "text-3xl font-bold text-danger mt-1" } else { "text-3xl font-bold text-fg mt-1" },
                "{value}"
            }
        }
    }
}
