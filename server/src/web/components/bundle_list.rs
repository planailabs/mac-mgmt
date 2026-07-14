use dioxus::prelude::*;
use dioxus_i18n::t;
use dioxus_tabular::*;

use crate::anthropic::{EntityKind, GenerateAllItem, GenerateContext};
use crate::api_mcp::endpoints::skills::{BundleListInput, CatalogEntryDto, list_bundles};
use crate::web::app::Route;
use crate::web::components::generate_all_button::GenerateAllButton;
use crate::web::components::hidden_badge::HiddenColumn;
use crate::web::components::table_utils::*;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{DataTable, ErrorText, HelpText, PageHeader, page_window};

/// Map the API DTO to the table row type shared by the catalog pages.
fn to_catalog_entry(e: &CatalogEntryDto) -> CatalogEntry {
    CatalogEntry {
        id: e.id,
        slug: e.slug.clone(),
        name: e.name.clone(),
        description: e.description.clone(),
        created_at: e.created_at,
        hide_from_public_catalog: e.hide_from_public_catalog,
        skill_center_name: e.skill_center_name.clone(),
        route_kind: e.route_kind.clone(),
    }
}

#[component]
pub fn BundleList() -> Element {
    use_topbar(t!("bundle-list-title"), None);
    let mut bundles = use_server_future(|| async move { list_bundles(BundleListInput {}).await })?;

    rsx! {
        div { class: "flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3 mb-4",
            PageHeader { class: "mb-0", {t!("bundle-list-title")} }
            div { class: "flex items-center gap-2",
                {match &*bundles.read() {
                    Some(Ok(list)) => {
                        let gen_items: Vec<GenerateAllItem> = list.iter()
                            .filter(|e| e.skill_center_name.is_none())
                            .map(|b| GenerateAllItem {
                                id: b.id.to_string(),
                                name: b.name.clone(),
                                description: b.description.clone(),
                                context: GenerateContext::Bundle { slug: b.slug.clone(), items: vec![] },
                                entity_kind: EntityKind::Bundle,
                            }).collect();
                        rsx! {
                            GenerateAllButton {
                                items: gen_items,
                                on_complete: move |_| { bundles.restart(); },
                            }
                        }
                    },
                    _ => rsx! {},
                }}
                Link { to: Route::BundleForm {}, class: "btn btn-md btn-primary",
                    {t!("bundle-list-new")}
                }
            }
        }
        {match &*bundles.read() {
            Some(Ok(list)) => rsx! { CatalogTable { list: list.iter().map(to_catalog_entry).collect::<Vec<_>>() } },
            Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
            None => rsx! { HelpText { {t!("loading")} } },
        }}
    }
}

#[component]
fn CatalogTable(list: Vec<CatalogEntry>) -> Element {
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
                .filter(|b| b.matches_search(&q))
                .cloned()
                .collect()
        }
    });

    let total = list.len();
    let data = use_tabular(
        (
            LinkColumn { header: "Slug" },
            TextColumn { header: "Name" },
            HiddenColumn,
            CreatedAtColumn,
        ),
        filtered.into(),
    );
    let all_rows: Vec<_> = data.rows().collect();
    let filtered_count = all_rows.len();
    let limit_val = *limit.read();
    let (start, shown) = page_window(*page.read(), limit_val, filtered_count);

    rsx! {
        DataTable {
            search, limit, page, total, filtered: filtered_count, shown,
            headers: rsx! { TableHeaders { data } },
            body: rsx! {
                for row in all_rows.into_iter().skip(start).take(limit_val) {
                    tr { key: "{row.key()}", TableCells { row } }
                }
            },
        }
    }
}
