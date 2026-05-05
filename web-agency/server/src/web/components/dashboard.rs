use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use super::ui::PageHeader;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DashboardStats {
    domains: i64,
    domains_expiring_soon: i64,
    webspaces: i64,
    credentials: i64,
    organizations: i64,
    billing_total_cents: i64,
}

#[server]
async fn get_dashboard_stats() -> Result<DashboardStats, ServerFnError> {
    let _user = crate::web::user::current_user().await?;
    let pool = crate::server_pool()?;

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

    Ok(DashboardStats { domains, domains_expiring_soon, webspaces, credentials, organizations, billing_total_cents })
}

#[component]
pub fn Dashboard() -> Element {
    let stats = use_server_future(get_dashboard_stats)?;
    let s = match &*stats.read() {
        Some(Ok(s)) => s.clone(),
        _ => DashboardStats::default(),
    };

    rsx! {
        PageHeader { "Dashboard" }

        p { class: "text-fg-muted mb-6", "Domain and webspace management overview" }

        div { class: "grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4",
            StatCard { label: "Domains", value: s.domains.to_string() }
            StatCard { label: "Expiring (30d)", value: s.domains_expiring_soon.to_string(), warn: s.domains_expiring_soon > 0 }
            StatCard { label: "Webspaces", value: s.webspaces.to_string() }
            StatCard { label: "Credentials", value: s.credentials.to_string() }
            StatCard { label: "Organizations", value: s.organizations.to_string() }
            StatCard {
                label: "Billing Total",
                value: format!("{:.2} EUR", s.billing_total_cents as f64 / 100.0),
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
