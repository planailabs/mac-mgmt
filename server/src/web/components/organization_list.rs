use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};

use crate::web::app::Route;
use crate::web::components::table_utils::Searchable;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{DataTable, ErrorText, PageHeader, Td, TdMuted, Th};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct OrgRow {
    id: String,
    name: String,
    member_count: i64,
    cluster_count: i64,
    created_at: DateTime<Utc>,
}

impl Searchable for OrgRow {
    fn matches_search(&self, query: &str) -> bool {
        self.name.to_lowercase().contains(query)
    }
}

#[server]
async fn list_organizations() -> Result<Vec<OrgRow>, ServerFnError> {
    use crate::web::user::{WebUserExt, current_user};
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: uuid::Uuid,
        name: String,
        member_count: i64,
        cluster_count: i64,
        created_at: DateTime<Utc>,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT o.id, o.name, \
         (SELECT COUNT(*) FROM organization_members om WHERE om.organization_id = o.id) AS member_count, \
         (SELECT COUNT(*) FROM organization_clusters oc WHERE oc.organization_id = o.id) AS cluster_count, \
         o.created_at \
         FROM organizations o ORDER BY o.name",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| OrgRow {
            id: r.id.to_string(),
            name: r.name,
            member_count: r.member_count,
            cluster_count: r.cluster_count,
            created_at: r.created_at,
        })
        .collect())
}

#[component]
pub fn OrganizationList() -> Element {
    use_topbar(t!("org-list-title"), None);
    let orgs = use_server_future(list_organizations)?;

    rsx! {
        div { class: "flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3 mb-4",
            PageHeader { class: "mb-0", {t!("org-list-title")} }
            Link { to: Route::OrganizationForm {}, class: "btn btn-md btn-primary",
                {t!("org-list-new")}
            }
        }
        {match &*orgs.read() {
            Some(Ok(list)) => rsx! { OrgTable { list: list.clone() } },
            Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
            None => rsx! { p { {t!("loading")} } },
        }}
    }
}

#[component]
fn OrgTable(list: Vec<OrgRow>) -> Element {
    let search = use_signal(String::new);
    let limit = use_signal(|| 20usize);

    let list_clone = list.clone();
    let filtered = use_memo(move || {
        let q = search.read().to_lowercase();
        if q.is_empty() {
            list_clone.clone()
        } else {
            list_clone
                .iter()
                .filter(|o| o.matches_search(&q))
                .cloned()
                .collect()
        }
    });

    let total = list.len();
    let filtered_count = filtered.read().len();
    let limit_val = *limit.read();
    let shown = filtered_count.min(limit_val);

    rsx! {
        DataTable {
            search, limit, total, filtered: filtered_count, shown,
            headers: rsx! {
                Th { {t!("name")} }
                Th { {t!("org-list-col-members")} }
                Th { {t!("org-list-col-clusters")} }
                Th { {t!("created")} }
            },
            body: rsx! {
                for org in filtered.read().iter().take(limit_val) {
                    OrgRowView { key: "{org.id}", org: org.clone() }
                }
            },
        }
    }
}

#[component]
fn OrgRowView(org: OrgRow) -> Element {
    let created = org.created_at.format("%Y-%m-%d %H:%M").to_string();
    rsx! {
        tr {
            Td { class: "text-sm",
                Link { to: Route::OrganizationDetail { id: org.id.clone() },
                    class: "link font-medium",
                    "{org.name}"
                }
            }
            Td { class: "text-sm", "{org.member_count}" }
            Td { class: "text-sm", "{org.cluster_count}" }
            TdMuted { class: "text-sm", {created} }
        }
    }
}
