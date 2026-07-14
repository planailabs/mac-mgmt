use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api_mcp::endpoints::daemon_versions::{
    DaemonVersionRow, DaemonVersionsListInput, DaemonVersionsSyncInput, list_daemon_versions,
    sync_daemon_versions_from_xzar,
};
use crate::web::app::Route;
use crate::web::components::table_utils::Searchable;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{
    Button, ButtonVariant, DataTable, ErrorText, HelpText, PageHeader, SortState, SortableTh,
    TdMono, TdMuted, page_window,
};

impl Searchable for DaemonVersionRow {
    fn matches_search(&self, query: &str) -> bool {
        self.version.to_lowercase().contains(query)
    }
}

#[component]
pub fn DaemonVersionList() -> Element {
    use_topbar(t!("daemon-version-list-title"), None);
    let mut versions =
        use_server_future(|| list_daemon_versions(DaemonVersionsListInput {}))?;
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
                        match sync_daemon_versions_from_xzar(DaemonVersionsSyncInput {}).await {
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
    let page = use_signal(|| 0usize);
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
    let (start, shown) = page_window(*page.read(), limit_val, filtered_count);

    rsx! {
        DataTable {
            search, limit, page, total, filtered: filtered_count, shown,
            headers: rsx! {
                SortableTh { label: t!("version"), sort_key: "version".to_string(), sort }
                SortableTh { label: t!("daemon-version-list-col-added"), sort_key: "added".to_string(), sort }
            },
            body: rsx! {
                for v in filtered.read().iter().skip(start).take(limit_val) {
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
