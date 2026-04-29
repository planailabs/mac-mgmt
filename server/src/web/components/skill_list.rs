use dioxus::prelude::*;
use dioxus_i18n::t;
use dioxus_tabular::*;

use crate::anthropic::{EntityKind, GenerateAllItem, GenerateContext};
use crate::web::components::generate_all_button::GenerateAllButton;
use crate::web::components::hidden_badge::HiddenColumn;
use crate::web::components::table_utils::*;
use crate::web::components::ui::{
    Button, ButtonVariant, DataTable, ErrorText, HelpText, PageHeader, SuccessText,
};
#[cfg(feature = "server")]
use crate::web::user::current_user;

#[server]
async fn list_skills() -> Result<Vec<CatalogEntry>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    let local = sqlx::query_as::<_, crate::models::Skill>(
        "SELECT * FROM skills ORDER BY slug",
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
            route_kind: "skill".to_string(),
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
            for ch in &cached.catalog.skill_channels {
                entries.push(CatalogEntry {
                    id: ch.id,
                    slug: ch.skill_slug.clone(),
                    name: ch.skill_name.clone(),
                    description: ch.skill_description.clone(),
                    created_at: None,
                    hide_from_public_catalog: ch.hidden,
                    skill_center_name: Some(sc_name.clone()),
                    route_kind: "skill".to_string(),
                });
            }
        }
    }

    entries.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(entries)
}

/// Mirror of `crate::xzar::SyncResult` that is visible in WASM builds
/// (the xzar module is behind the `server` feature gate).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct SyncResult {
    created_skills: u32,
    created_channels: u32,
    removed_channels: u32,
    removed_skills: u32,
}

/// Fetch all pins from xzar, parse `skill/{slug}/{channel}` pins, and upsert
/// skills + channels into the database.
#[server]
async fn sync_from_xzar() -> Result<SyncResult, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let r = crate::xzar::sync_skills_db(&pool)
        .await
        .map_err(|e| ServerFnError::new(e))?;
    Ok(SyncResult {
        created_skills: r.created_skills,
        created_channels: r.created_channels,
        removed_channels: r.removed_channels,
        removed_skills: r.removed_skills,
    })
}

#[component]
pub fn SkillList() -> Element {
    let mut skills = use_server_future(list_skills)?;
    let mut syncing = use_signal(|| false);
    let mut sync_msg = use_signal(|| None::<String>);
    let mut sync_err = use_signal(|| None::<String>);

    rsx! {
        div { class: "flex items-center justify-between mb-4",
            PageHeader { class: "mb-0", {t!("skill-list-title")} }
            div { class: "flex items-center gap-2",
                {match &*skills.read() {
                    Some(Ok(list)) => {
                        let gen_items: Vec<GenerateAllItem> = list.iter()
                            .filter(|e| e.skill_center_name.is_none())
                            .map(|s| GenerateAllItem {
                                id: s.id.to_string(),
                                name: s.name.clone(),
                                description: s.description.clone(),
                                context: GenerateContext::Skill { skill_id: s.id.to_string() },
                                entity_kind: EntityKind::Skill,
                            }).collect();
                        rsx! {
                            GenerateAllButton {
                                items: gen_items,
                                on_complete: move |_| { skills.restart(); },
                            }
                        }
                    },
                    _ => rsx! {},
                }}
                Button {
                    variant: ButtonVariant::Primary,
                    disabled: *syncing.read(),
                    onclick: move |_| {
                        syncing.set(true);
                        sync_msg.set(None);
                        sync_err.set(None);
                        spawn(async move {
                            match sync_from_xzar().await {
                                Ok(result) => {
                                    sync_msg.set(Some(format!(
                                        "Synced: +{} skills, +{} channels, -{} channels, -{} skills",
                                        result.created_skills, result.created_channels,
                                        result.removed_channels, result.removed_skills,
                                    )));
                                    skills.restart();
                                }
                                Err(e) => {
                                    sync_err.set(Some(e.to_string()));
                                }
                            }
                            syncing.set(false);
                        });
                    },
                    if *syncing.read() { {t!("skill-list-syncing")} } else { {t!("skill-list-sync")} }
                }
            }
        }
        if let Some(msg) = &*sync_msg.read() {
            SuccessText { class: "mb-4", "{msg}" }
        }
        if let Some(err) = &*sync_err.read() {
            ErrorText { class: "mb-4", "{err}" }
        }
        {match &*skills.read() {
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
