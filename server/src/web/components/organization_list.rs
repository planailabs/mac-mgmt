use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api_mcp::endpoints::organizations::{OrgListInput, OrgRow, list_organizations};
use crate::web::app::Route;
use crate::web::components::table_utils::Searchable;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{DataTable, ErrorText, PageHeader, Td, TdMuted, Th, page_window};

impl Searchable for OrgRow {
    fn matches_search(&self, query: &str) -> bool {
        self.name.to_lowercase().contains(query)
    }
}

#[component]
pub fn OrganizationList() -> Element {
    use_topbar(t!("org-list-title"), None);
    let orgs = use_server_future(|| list_organizations(OrgListInput {}))?;

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
    let page = use_signal(|| 0usize);

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
    let (start, shown) = page_window(*page.read(), limit_val, filtered_count);

    rsx! {
        DataTable {
            search, limit, page, total, filtered: filtered_count, shown,
            headers: rsx! {
                Th { {t!("name")} }
                Th { {t!("org-list-col-members")} }
                Th { {t!("org-list-col-clusters")} }
                Th { {t!("created")} }
            },
            body: rsx! {
                for org in filtered.read().iter().skip(start).take(limit_val) {
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
