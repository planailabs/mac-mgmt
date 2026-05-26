use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};

use crate::web::app::Route;
use crate::web::components::table_utils::Searchable;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{
    Button, ButtonVariant, DataTable, ErrorText, HelpText, PageHeader, SortState, SortableTh,
    TdMono, TdMuted,
};
#[cfg(feature = "server")]
use crate::web::user::{WebUserExt, current_user};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonVersionRow {
    pub version: String,
    pub created_at: DateTime<Utc>,
}

impl Searchable for DaemonVersionRow {
    fn matches_search(&self, query: &str) -> bool {
        self.version.to_lowercase().contains(query)
    }
}

#[server]
async fn list_daemon_versions() -> Result<Vec<DaemonVersionRow>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    #[derive(sqlx::FromRow)]
    struct Row {
        version: String,
        created_at: DateTime<Utc>,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT version, created_at FROM daemon_versions ORDER BY version DESC",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| DaemonVersionRow {
            version: r.version,
            created_at: r.created_at,
        })
        .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonSyncResult {
    pub created: u32,
    pub removed: u32,
}

/// Fetch xzar pins, parse `daemon/{version}/{system}` and upsert the
/// distinct versions into `daemon_versions`. Store paths are resolved
/// live from xzar on each /api/update call, so they are not stored.
#[server]
async fn sync_daemon_versions_from_xzar() -> Result<DaemonSyncResult, ServerFnError> {
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

    let mut created: u32 = 0;
    let mut valid: std::collections::HashSet<String> = std::collections::HashSet::new();

    for pin in &pins {
        if pin.abandoned || pin.roots.is_empty() {
            continue;
        }
        let parts: Vec<&str> = pin.name.splitn(3, '/').collect();
        if parts.len() != 3 || parts[0] != "daemon" {
            continue;
        }
        let version = parts[1].to_string();
        if !valid.insert(version.clone()) {
            continue;
        }

        let inserted = sqlx::query_scalar::<_, bool>(
            "INSERT INTO daemon_versions (version) VALUES ($1) \
             ON CONFLICT (version) DO NOTHING \
             RETURNING true",
        )
        .bind(&version)
        .fetch_optional(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

        if inserted.is_some() {
            created += 1;
        }
    }

    let valid_vec: Vec<String> = valid.into_iter().collect();
    let removed = sqlx::query_scalar::<_, i64>(
        "WITH deleted AS ( \
             DELETE FROM daemon_versions dv \
             WHERE NOT (dv.version = ANY($1::text[])) \
             RETURNING dv.id \
         ) SELECT count(*) FROM deleted",
    )
    .bind(&valid_vec)
    .fetch_one(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(DaemonSyncResult {
        created,
        removed: removed as u32,
    })
}

#[component]
pub fn DaemonVersionList() -> Element {
    use_topbar(t!("daemon-version-list-title"), None);
    let mut versions = use_server_future(list_daemon_versions)?;
    let mut syncing = use_signal(|| false);
    let mut sync_msg = use_signal(|| None::<String>);
    let mut sync_err = use_signal(|| None::<String>);

    rsx! {
        div { class: "flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3 mb-4",
            PageHeader { class: "mb-0", {t!("daemon-version-list-title")} }
            Button {
                variant: ButtonVariant::Primary,
                disabled: *syncing.read(),
                onclick: move |_| {
                    syncing.set(true);
                    sync_msg.set(None);
                    sync_err.set(None);
                    spawn(async move {
                        match sync_daemon_versions_from_xzar().await {
                            Ok(result) => {
                                sync_msg.set(Some(format!(
                                    "Synced: +{} new, -{} removed",
                                    result.created, result.removed,
                                )));
                                versions.restart();
                            }
                            Err(e) => {
                                sync_err.set(Some(e.to_string()));
                            }
                        }
                        syncing.set(false);
                    });
                },
                if *syncing.read() { {t!("daemon-version-list-syncing")} } else { {t!("daemon-version-list-sync")} }
            }
        }
        if let Some(msg) = &*sync_msg.read() {
            crate::web::components::ui::SuccessText { class: "mb-4", "{msg}" }
        }
        if let Some(err) = &*sync_err.read() {
            ErrorText { class: "mb-4", "{err}" }
        }
        {match &*versions.read() {
            Some(Ok(list)) => {
                if list.is_empty() {
                    rsx! { HelpText { {t!("daemon-version-list-none")} } }
                } else {
                    rsx! { VersionsTable { list: list.clone() } }
                }
            }
            Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
            None => rsx! { HelpText { {t!("loading")} } },
        }}
    }
}

#[component]
fn VersionsTable(list: Vec<DaemonVersionRow>) -> Element {
    let search = use_signal(String::new);
    let limit = use_signal(|| 20usize);
    let sort = use_signal::<SortState>(|| ("version".to_string(), false));

    let list_clone = list.clone();
    let filtered = use_memo(move || {
        let q = search.read().to_lowercase();
        let mut items: Vec<DaemonVersionRow> = if q.is_empty() {
            list_clone.clone()
        } else {
            list_clone
                .iter()
                .filter(|e| e.matches_search(&q))
                .cloned()
                .collect()
        };
        let (key, asc) = sort.read().clone();
        items.sort_by(|a, b| {
            let ord = match key.as_str() {
                "added" => a.created_at.cmp(&b.created_at),
                _ => a.version.cmp(&b.version),
            };
            if asc { ord } else { ord.reverse() }
        });
        items
    });

    let total = list.len();
    let filtered_count = filtered.read().len();
    let limit_val = *limit.read();
    let shown = filtered_count.min(limit_val);

    rsx! {
        DataTable {
            search, limit, total, filtered: filtered_count, shown,
            headers: rsx! {
                SortableTh { label: t!("version"), sort_key: "version".to_string(), sort }
                SortableTh { label: t!("daemon-version-list-col-added"), sort_key: "added".to_string(), sort }
            },
            body: rsx! {
                for v in filtered.read().iter().take(limit_val) {
                    VersionRow { key: "{v.version}", row: v.clone() }
                }
            },
        }
    }
}

#[component]
fn VersionRow(row: DaemonVersionRow) -> Element {
    let ts = row.created_at.format("%Y-%m-%d %H:%M").to_string();
    rsx! {
        tr {
            TdMono {
                Link { to: Route::DaemonVersionDetail { version: row.version.clone() }, class: "link",
                    "{row.version}"
                }
            }
            TdMuted { class: "text-sm", {ts} }
        }
    }
}
