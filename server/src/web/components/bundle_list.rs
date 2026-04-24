use dioxus::prelude::*;
use dioxus_tabular::*;

use crate::anthropic::{EntityKind, GenerateAllItem, GenerateContext};
use crate::models::Bundle;
use crate::web::app::Route;
use crate::web::components::generate_all_button::GenerateAllButton;
use crate::web::components::hidden_badge::HiddenColumn;
use crate::web::components::table_utils::*;
#[server]
async fn list_bundles() -> Result<Vec<Bundle>, ServerFnError> {
    load_admin_list::<Bundle>("SELECT * FROM bundles ORDER BY slug").await
}

#[component]
pub fn BundleList() -> Element {
    let mut bundles = use_server_future(list_bundles)?;

    rsx! {
        div { class: "flex items-center justify-between mb-4",
            h2 { class: "text-2xl font-bold", "Bundles" }
            div { class: "flex items-center gap-2",
                {match &*bundles.read() {
                    Some(Ok(list)) => {
                        let gen_items: Vec<GenerateAllItem> = list.iter().map(|b| GenerateAllItem {
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
                Link {
                    to: Route::BundleForm {},
                    class: "bg-blue-600 text-white px-4 py-2 rounded text-sm hover:bg-blue-700",
                    "New Bundle"
                }
            }
        }
        {match &*bundles.read() {
            Some(Ok(list)) => {
                let search = use_signal(String::new);
                let limit = use_signal(|| 20usize);

                let list_clone = list.clone();
                let filtered = use_memo(move || {
                    let q = search.read().to_lowercase();
                    if q.is_empty() {
                        list_clone.clone()
                    } else {
                        list_clone.iter().filter(|b| b.matches_search(&q)).cloned().collect()
                    }
                });

                let total = list.len();
                let data = use_tabular(
                    (LinkColumn { header: "Slug" }, TextColumn { header: "Name" }, HiddenColumn, CreatedAtColumn),
                    filtered.into(),
                );
                let all_rows: Vec<_> = data.rows().collect();
                let filtered_count = all_rows.len();
                let limit_val = *limit.read();
                let shown = filtered_count.min(limit_val);

                rsx! {
                    TableToolbar { search, limit, total, filtered: filtered_count, shown }
                    div { class: "bg-white dark:bg-gray-800 rounded shadow dark:shadow-gray-900/30 overflow-hidden",
                        table { class: "min-w-full divide-y divide-gray-200 dark:divide-gray-700",
                            thead { class: "bg-gray-50 dark:bg-gray-700",
                                tr { TableHeaders { data } }
                            }
                            tbody { class: "bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700",
                                for row in all_rows.into_iter().take(limit_val) {
                                    tr { key: "{row.key()}", TableCells { row } }
                                }
                            }
                        }
                    }
                }
            },
            Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400", "Error: {e}" } },
            None => rsx! { p { "Loading..." } },
        }}
    }
}
