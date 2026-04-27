use dioxus::prelude::*;
use dioxus_tabular::*;

use crate::anthropic::{EntityKind, GenerateAllItem, GenerateContext};
use crate::models::McpServer;
use crate::web::app::Route;
use crate::web::components::generate_all_button::GenerateAllButton;
use crate::web::components::hidden_badge::HiddenColumn;
use crate::web::components::table_utils::*;
#[cfg(feature = "server")]
use crate::web::user::current_user;

/// An MCP server from a remote skill center.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RemoteMcpServer {
    pub slug: String,
    pub name: String,
    pub skill_center_name: String,
}

#[server]
async fn list_remote_mcp_servers() -> Result<Vec<RemoteMcpServer>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;

    let cache = crate::skill_center_cache::SkillCenterCache::global()
        .ok_or_else(|| ServerFnError::new("skill center cache not initialized"))?;
    let all = cache.get_all().await;

    let pool = crate::server_pool()?;
    let rows = sqlx::query_as::<_, (uuid::Uuid, String)>(
        "SELECT id, name FROM skill_centers WHERE enabled = true",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    let names: std::collections::HashMap<uuid::Uuid, String> =
        rows.into_iter().collect();

    let mut result = Vec::new();
    for (sc_id, cached) in &all {
        let sc_name = names.get(sc_id).cloned().unwrap_or_else(|| sc_id.to_string());
        for srv in &cached.catalog.mcp_servers {
            result.push(RemoteMcpServer {
                slug: srv.slug.clone(),
                name: srv.name.clone(),
                skill_center_name: sc_name.clone(),
            });
        }
    }
    result.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(result)
}

#[server]
async fn list_mcp_servers() -> Result<Vec<McpServer>, ServerFnError> {
    load_admin_list::<McpServer>("SELECT * FROM mcp_servers ORDER BY slug").await
}

#[component]
pub fn McpServerList() -> Element {
    let mut servers = use_server_future(list_mcp_servers)?;
    let remote_servers = use_server_future(list_remote_mcp_servers)?;

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
                                {match &*remote_servers.read() {
                                    Some(Ok(remote)) => rsx! {
                                        for item in remote.iter() {
                                            tr { class: "text-purple-700 dark:text-purple-400",
                                                td { class: "px-6 py-4 whitespace-nowrap text-sm font-mono", "{item.slug}" }
                                                td { class: "px-6 py-4 whitespace-nowrap text-sm", "{item.name}" }
                                                td { class: "px-6 py-4 whitespace-nowrap text-xs", "via {item.skill_center_name}" }
                                                td { class: "px-6 py-4 whitespace-nowrap text-sm", "" }
                                            }
                                        }
                                    },
                                    _ => rsx! {},
                                }}
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
