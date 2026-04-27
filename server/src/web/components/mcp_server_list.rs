use dioxus::prelude::*;
use dioxus_i18n::t;
use dioxus_tabular::*;

use crate::anthropic::{EntityKind, GenerateAllItem, GenerateContext};
use crate::web::app::Route;
use crate::web::components::generate_all_button::GenerateAllButton;
use crate::web::components::hidden_badge::HiddenColumn;
use crate::web::components::table_utils::*;

#[server]
async fn list_mcp_servers() -> Result<Vec<CatalogEntry>, ServerFnError> {
    use crate::web::user::current_user;
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    let local = sqlx::query_as::<_, crate::models::McpServer>(
        "SELECT * FROM mcp_servers ORDER BY slug",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut entries: Vec<CatalogEntry> = local
        .into_iter()
        .map(|s| CatalogEntry {
            id: s.id,
            slug: s.slug,
            name: s.name,
            description: s.description,
            created_at: Some(s.created_at),
            hide_from_public_catalog: s.hide_from_public_catalog,
            skill_center_name: None,
            route_kind: "mcp_server".to_string(),
        })
        .collect();

    if let Some(cache) = crate::skill_center_cache::SkillCenterCache::global() {
        let all = cache.get_all().await;
        let sc_names: std::collections::HashMap<uuid::Uuid, String> =
            sqlx::query_as::<_, (uuid::Uuid, String)>(
                "SELECT id, name FROM skill_centers WHERE enabled = true",
            )
            .fetch_all(&pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .collect();

        for (sc_id, cached) in &all {
            let sc_name = sc_names.get(sc_id).cloned().unwrap_or_default();
            if sc_name.is_empty() {
                continue;
            }
            for srv in &cached.catalog.mcp_servers {
                entries.push(CatalogEntry {
                    id: srv.id,
                    slug: srv.slug.clone(),
                    name: srv.name.clone(),
                    description: srv.description.clone(),
                    created_at: None,
                    hide_from_public_catalog: srv.hidden,
                    skill_center_name: Some(sc_name.clone()),
                    route_kind: "mcp_server".to_string(),
                });
            }
        }
    }

    entries.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(entries)
}

#[component]
pub fn McpServerList() -> Element {
    let mut servers = use_server_future(list_mcp_servers)?;

    rsx! {
        div { class: "flex items-center justify-between mb-4",
            h2 { class: "text-2xl font-bold", {t!("mcp-server-list-title")} }
            div { class: "flex items-center gap-2",
                {match &*servers.read() {
                    Some(Ok(list)) => {
                        let gen_items: Vec<GenerateAllItem> = list.iter()
                            .filter(|e| e.skill_center_name.is_none())
                            .map(|s| GenerateAllItem {
                                id: s.id.to_string(),
                                name: s.name.clone(),
                                description: s.description.clone(),
                                context: GenerateContext::McpServer {
                                    slug: s.slug.clone(),
                                    config_json: String::new(),
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
                    {t!("mcp-server-list-new")}
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
                            }
                        }
                    }
                }
            },
            Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400", {t!("error-message", message: e.to_string())} } },
            None => rsx! { p { {t!("loading")} } },
        }}
    }
}
