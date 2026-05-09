use dioxus::prelude::*;
use dioxus_i18n::t;
use dioxus_tabular::*;

use crate::anthropic::{EntityKind, GenerateAllItem, GenerateContext};
use crate::web::app::Route;
use crate::web::components::generate_all_button::GenerateAllButton;
use crate::web::components::hidden_badge::HiddenColumn;
use crate::web::components::table_utils::*;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{DataTable, ErrorText, HelpText, PageHeader};

#[server]
async fn list_bundles() -> Result<Vec<CatalogEntry>, ServerFnError> {
    use crate::web::user::{current_user, WebUserExt};
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    let local = sqlx::query_as::<_, crate::models::Bundle>(
        "SELECT * FROM bundles ORDER BY slug",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut entries: Vec<CatalogEntry> = local
        .into_iter()
        .map(|b| CatalogEntry {
            id: b.id,
            slug: b.slug,
            name: b.name,
            description: b.description,
            created_at: Some(b.created_at),
            hide_from_public_catalog: b.hide_from_public_catalog,
            skill_center_name: None,
            route_kind: "bundle".to_string(),
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
            for bundle in &cached.catalog.bundles {
                entries.push(CatalogEntry {
                    id: bundle.id,
                    slug: bundle.slug.clone(),
                    name: bundle.name.clone(),
                    description: bundle.description.clone(),
                    created_at: None,
                    hide_from_public_catalog: bundle.hidden,
                    skill_center_name: Some(sc_name.clone()),
                    route_kind: "bundle".to_string(),
                });
            }
        }
    }

    entries.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(entries)
}

#[component]
pub fn BundleList() -> Element {
    use_topbar(t!("bundle-list-title"), None);
    let mut bundles = use_server_future(list_bundles)?;

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
            Some(Ok(list)) => rsx! { CatalogTable { list: list.clone() } },
            Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
            None => rsx! { HelpText { {t!("loading")} } },
        }}
    }
}

#[component]
fn CatalogTable(list: Vec<CatalogEntry>) -> Element {
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
        DataTable {
            search, limit, total, filtered: filtered_count, shown,
            headers: rsx! { TableHeaders { data } },
            body: rsx! {
                for row in all_rows.into_iter().take(limit_val) {
                    tr { key: "{row.key()}", TableCells { row } }
                }
            },
        }
    }
}
