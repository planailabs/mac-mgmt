use dioxus::prelude::*;

use crate::models::Skill;
use crate::web::app::Route;

#[server]
async fn list_skills() -> Result<Vec<Skill>, ServerFnError> {
    let pool = crate::server_pool()?;
    let skills = sqlx::query_as::<_, Skill>("SELECT * FROM skills ORDER BY slug")
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(skills)
}

/// Sync result returned to the UI.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncResult {
    pub created_skills: u32,
    pub created_channels: u32,
}

/// Fetch all pins from xzar, parse `skill/{slug}/{channel}` pins, and upsert
/// skills + channels into the database.
#[server]
async fn sync_from_xzar() -> Result<SyncResult, ServerFnError> {
    let pool = crate::server_pool()?;
    let cfg = crate::config::config();

    let pins = crate::xzar::fetch_pins(&cfg.xzar.url, &cfg.xzar.token)
        .await
        .map_err(|e| ServerFnError::new(format!("xzar error: {e}")))?;

    let mut created_skills: u32 = 0;
    let mut created_channels: u32 = 0;

    for pin in &pins {
        if pin.abandoned || pin.roots.is_empty() {
            continue;
        }

        // Parse pins matching "skill/{slug}/{channel}"
        let parts: Vec<&str> = pin.name.splitn(3, '/').collect();
        if parts.len() != 3 || parts[0] != "skill" {
            continue;
        }
        let slug = parts[1];
        let channel = parts[2];

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
        let skill_id = sqlx::query_scalar::<_, uuid::Uuid>(
            "SELECT id FROM skills WHERE slug = $1",
        )
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

    Ok(SyncResult { created_skills, created_channels })
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
                                    "Synced: {} new skills, {} new channels",
                                    result.created_skills, result.created_channels
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
        if let Some(msg) = &*sync_msg.read() {
            p { class: "text-green-600 text-sm mb-4", "{msg}" }
        }
        if let Some(err) = &*sync_err.read() {
            p { class: "text-red-600 text-sm mb-4", "{err}" }
        }
        {match &*skills.read() {
            Some(Ok(list)) => rsx! {
                table { class: "min-w-full divide-y divide-gray-200",
                    thead { class: "bg-gray-50",
                        tr {
                            th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase", "Slug" }
                            th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase", "Name" }
                            th { class: "px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase", "Created" }
                        }
                    }
                    tbody { class: "bg-white divide-y divide-gray-200",
                        for skill in list {
                            {
                                let sid = skill.id.to_string();
                                let slug = skill.slug.clone();
                                let name = skill.name.clone();
                                let created = skill.created_at.format("%Y-%m-%d %H:%M").to_string();
                                rsx! {
                                    tr { key: "{sid}",
                                        td { class: "px-6 py-4",
                                            Link {
                                                to: Route::SkillDetail { id: sid },
                                                class: "text-blue-600 hover:underline font-mono text-sm",
                                                "{slug}"
                                            }
                                        }
                                        td { class: "px-6 py-4", "{name}" }
                                        td { class: "px-6 py-4 text-gray-500", "{created}" }
                                    }
                                }
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
