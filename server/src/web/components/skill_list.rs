use dioxus::prelude::*;
use dioxus_tabular::*;

use crate::anthropic::{EntityKind, GenerateAllItem, GenerateContext};
use crate::web::components::generate_all_button::GenerateAllButton;
use crate::web::components::hidden_badge::HiddenColumn;
use crate::web::components::table_utils::*;
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

    // Add remote items from cache
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

/// Sync result returned to the UI.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncResult {
    pub created_skills: u32,
    pub created_channels: u32,
    pub removed_channels: u32,
    pub removed_skills: u32,
}

/// Fetch all pins from xzar, parse `skill/{slug}/{channel}` pins, and upsert
/// skills + channels into the database.
#[server]
async fn sync_from_xzar() -> Result<SyncResult, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let cfg = crate::config::config();

    let xzar = cfg
        .xzar
        .as_ref()
        .ok_or_else(|| ServerFnError::new("xzar not configured".to_string()))?;
    let pins = crate::xzar::fetch_pins(&xzar.url, &xzar.token)
        .await
        .map_err(|e| ServerFnError::new(format!("xzar error: {e}")))?;

    let mut created_skills: u32 = 0;
    let mut created_channels: u32 = 0;

    for pin in &pins {
        if pin.abandoned || pin.roots.is_empty() {
            continue;
        }

        // Parse pins matching "skill/{slug}/{channel}/{arch}"
        let parts: Vec<&str> = pin.name.splitn(4, '/').collect();
        if parts.len() != 4 || parts[0] != "skill" {
            continue;
        }
        let slug = parts[1];
        let channel = parts[2];
        // parts[3] is architecture — we don't store it, just deduplicate (slug, channel)

        // Upsert skill
        let inserted = sqlx::query_scalar::<_, bool>(
            "INSERT INTO skills (slug, name) VALUES ($1, $1) \
             ON CONFLICT (slug) DO NOTHING \
             RETURNING true",
        )
        .bind(slug)
        .fetch_optional(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

        if inserted.is_some() {
            created_skills += 1;
        }

        // Get the skill id
        let skill_id = sqlx::query_scalar::<_, uuid::Uuid>("SELECT id FROM skills WHERE slug = $1")
            .bind(slug)
            .fetch_one(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        // Upsert channel
        let ch_inserted = sqlx::query_scalar::<_, bool>(
            "INSERT INTO skill_channels (skill_id, channel) VALUES ($1, $2) \
             ON CONFLICT (skill_id, channel) DO NOTHING \
             RETURNING true",
        )
        .bind(skill_id)
        .bind(channel)
        .fetch_optional(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

        if ch_inserted.is_some() {
            created_channels += 1;
        }
    }

    // Collect the set of valid (slug, channel) pairs from xzar
    let mut valid_pairs: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();
    for pin in &pins {
        if pin.abandoned || pin.roots.is_empty() {
            continue;
        }
        let parts: Vec<&str> = pin.name.splitn(4, '/').collect();
        if parts.len() == 4 && parts[0] == "skill" {
            valid_pairs.insert((parts[1].to_string(), parts[2].to_string()));
        }
    }

    // Resolve which clusters are affected BEFORE deleting — the cascade
    // will wipe cluster_skills/bundle_items rows and we'd otherwise lose
    // the ability to push them a sync.
    let slugs_vec: Vec<String> = valid_pairs.iter().map(|(s, _)| s.clone()).collect();
    let channels_vec: Vec<String> = valid_pairs.iter().map(|(_, c)| c.clone()).collect();
    let to_remove_channel_ids: Vec<uuid::Uuid> = sqlx::query_scalar(
        "SELECT sc.id FROM skill_channels sc \
         JOIN skills s ON sc.skill_id = s.id \
         WHERE NOT EXISTS ( \
             SELECT 1 FROM unnest($1::text[], $2::text[]) AS v(slug, channel) \
             WHERE v.slug = s.slug AND v.channel = sc.channel \
         )",
    )
    .bind(&slugs_vec)
    .bind(&channels_vec)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    if !to_remove_channel_ids.is_empty() {
        crate::api::push::notify_federation_global();
        crate::api::push::notify_skill_channels_global(&to_remove_channel_ids).await;
    }

    // Remove channels that no longer exist in xzar
    let removed_channels = sqlx::query_scalar::<_, i64>(
        "WITH deleted AS ( \
             DELETE FROM skill_channels WHERE id = ANY($1) RETURNING id \
         ) SELECT count(*) FROM deleted",
    )
    .bind(&to_remove_channel_ids)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Remove skills that have no channels left
    let removed_skills = sqlx::query_scalar::<_, i64>(
        "WITH deleted AS ( \
             DELETE FROM skills s \
             WHERE NOT EXISTS ( \
                 SELECT 1 FROM skill_channels sc WHERE sc.skill_id = s.id \
             ) \
             RETURNING s.id \
         ) SELECT count(*) FROM deleted",
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(SyncResult {
        created_skills,
        created_channels,
        removed_channels: removed_channels as u32,
        removed_skills: removed_skills as u32,
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
            h2 { class: "text-2xl font-bold", "Skills" }
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
                button {
                    class: "bg-blue-600 text-white px-4 py-2 rounded text-sm hover:bg-blue-700 disabled:opacity-50",
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
                    if *syncing.read() { "Syncing..." } else { "Sync from xzar" }
                }
            }
        }
        if let Some(msg) = &*sync_msg.read() {
            p { class: "text-green-600 dark:text-green-400 text-sm mb-4", "{msg}" }
        }
        if let Some(err) = &*sync_err.read() {
            p { class: "text-red-600 dark:text-red-400 text-sm mb-4", "{err}" }
        }
        {match &*skills.read() {
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
            Some(Err(e)) => rsx! { p { class: "text-red-600 dark:text-red-400", "Error: {e}" } },
            None => rsx! { p { "Loading..." } },
        }}
    }
}
