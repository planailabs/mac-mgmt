use dioxus::prelude::*;
use dioxus_tabular::*;

use crate::anthropic::{EntityKind, GenerateAllItem, GenerateContext};
use crate::models::McpServer;
use crate::web::app::Route;
use crate::web::components::generate_all_button::GenerateAllButton;
use crate::web::components::table_utils::*;

#[server]
async fn list_mcp_servers() -> Result<Vec<McpServer>, ServerFnError> {
    let pool = crate::server_pool()?;
    let servers = sqlx::query_as::<_, McpServer>("SELECT * FROM mcp_servers ORDER BY slug")
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(servers)
}

#[component]
pub fn McpServerList() -> Element {
    let mut servers = use_server_future(list_mcp_servers)?;

    rsx! {
        div { class: "flex items-center justify-between mb-4",
            h2 { class: "text-2xl font-bold", "MCP Servers" }
            div { class: "flex items-center gap-2",
                {match &*servers.read() {
                    Some(Ok(list)) => {
                        let gen_items: Vec<GenerateAllItem> = list.iter().map(|s| GenerateAllItem {
                            id: s.id.to_string(),
                            name: s.name.clone(),
                            description: s.description.clone(),
                            context: GenerateContext::McpServer {
                                slug: s.slug.clone(),
                                config_json: serde_json::to_string(&s.config_json).unwrap_or_default(),
                            },
                            entity_kind: EntityKind::McpServer,
                        }).collect();
                        rsx! {
                            GenerateAllButton {
                                items: gen_items,
                                on_complete: move |_| { servers.restart(); },
                            }
                        }
                    },
                    _ => rsx! {},
                }}
                Link {
                    to: Route::McpServerForm {},
                    class: "bg-blue-600 text-white px-4 py-2 rounded text-sm hover:bg-blue-700",
                    "New MCP Server"
                }
            }
        }
        {match &*servers.read() {
            Some(Ok(list)) => {
                let search = use_signal(String::new);
                let limit = use_signal(|| 20usize);

                let list_clone = list.clone();
                let filtered = use_memo(move || {
                    let q = search.read().to_lowercase();
                    if q.is_empty() {
                        list_clone.clone()
                    } else {
                        list_clone.iter().filter(|s| s.matches_search(&q)).cloned().collect()
                    }
                });

                let total = list.len();
                let data = use_tabular(
                    (LinkColumn { header: "Slug" }, TextColumn { header: "Name" }, CreatedAtColumn),
                    filtered.into(),
                );
                let all_rows: Vec<_> = data.rows().collect();
                let filtered_count = all_rows.len();
                let limit_val = *limit.read();
                let shown = filtered_count.min(limit_val);

                rsx! {
                    TableToolbar { search, limit, total, filtered: filtered_count, shown }
                    table { class: "min-w-full divide-y divide-gray-200",
                        thead { class: "bg-gray-50",
                            tr { TableHeaders { data } }
                        }
                        tbody { class: "bg-white divide-y divide-gray-200",
                            for row in all_rows.into_iter().take(limit_val) {
                                tr { key: "{row.key()}", TableCells { row } }
                            }
                        }
                    }
                }
            },
            Some(Err(e)) => rsx! { p { class: "text-red-600", "Error: {e}" } },
            None => rsx! { p { "Loading..." } },
        }}
    }
}
