use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api_mcp::endpoints::rollouts::{
    RolloutDeleteInput, RolloutEntry, RolloutListHealth, RolloutListInput, delete_rollout,
    get_rollouts,
};
use crate::web::app::Route;
use crate::web::components::table_utils::Searchable;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{
    Badge, BadgeVariant, DataTable, ErrorText, HelpText, PageHeader, SortState, SortableTh, Td,
    TdMuted, Th, page_window,
};

impl Searchable for RolloutEntry {
    fn matches_search(&self, query: &str) -> bool {
        self.id.to_string().to_lowercase().contains(query)
            || self
                .name
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .contains(query)
            || self.status.to_lowercase().contains(query)
    }
}

fn status_variant(status: &str) -> (BadgeVariant, String) {
    match status {
        "rolling" => (BadgeVariant::Info, t!("rollout-status-rolling")),
        "completed" => (BadgeVariant::Success, t!("rollout-status-completed")),
        "paused" => (BadgeVariant::Warn, t!("rollout-status-paused")),
        "failed" => (BadgeVariant::Danger, t!("rollout-status-failed")),
        _ => (BadgeVariant::Neutral, t!("rollout-status-pending")),
    }
}

fn health_variant(state: &str) -> (BadgeVariant, String) {
    match state {
        "pass" => (BadgeVariant::Success, t!("rollout-health-pass")),
        "fail" => (BadgeVariant::Danger, t!("rollout-health-fail")),
        "grace" => (BadgeVariant::Warn, t!("rollout-health-grace")),
        _ => (BadgeVariant::Neutral, t!("rollout-health-no-data")),
    }
}

#[component]
pub fn RolloutList() -> Element {
    use_topbar(t!("rollout-list-title"), None);
    let rollouts =
        use_server_future(move || async move { get_rollouts(RolloutListInput {}).await })?;

    rsx! {
        // Title row stacks below sm so the two action buttons don't
        // overflow the right edge on phones; from sm up they sit on
        // the same line as the title.
        div { class: "flex flex-col sm:flex-row sm:justify-between sm:items-center gap-3 mb-4",
            PageHeader { class: "mb-0", {t!("rollout-list-title")} }
            div { class: "flex gap-2 flex-wrap",
                Link { to: Route::RolloutGroupList {}, class: "btn btn-md btn-secondary",
                    {t!("rollout-list-manage-groups")}
                }
                Link { to: Route::RolloutForm {}, class: "btn btn-md btn-primary",
                    {t!("rollout-list-new")}
                }
            }
        }
        {match &*rollouts.read() {
            Some(Ok(list)) => {
                if list.is_empty() {
                    rsx! { HelpText { {t!("rollout-list-no-rollouts")} } }
                } else {
                    rsx! { RolloutsTable { list: list.clone(), rollouts } }
                }
            }
            Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
            None => rsx! { HelpText { {t!("loading")} } },
        }}
    }
}

#[component]
fn RolloutsTable(
    list: Vec<RolloutEntry>,
    rollouts: Resource<Result<Vec<RolloutEntry>, ServerFnError>>,
) -> Element {
    let search = use_signal(String::new);
    let limit = use_signal(|| 20usize);
    let page = use_signal(|| 0usize);
    let sort = use_signal::<SortState>(|| ("created".to_string(), false));

    let list_clone = list.clone();
    let filtered = use_memo(move || {
        let q = search.read().to_lowercase();
        let mut items: Vec<RolloutEntry> = if q.is_empty() {
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
                "name" => a
                    .name
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.name.as_deref().unwrap_or("")),
                "status" => a.status.cmp(&b.status),
                "stages" => a.stage_count.cmp(&b.stage_count),
                _ => a.created_at.cmp(&b.created_at),
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
                SortableTh { label: t!("name"), sort_key: "name".to_string(), sort }
                SortableTh { label: t!("status"), sort_key: "status".to_string(), sort }
                SortableTh { label: t!("rollout-list-col-stages"), sort_key: "stages".to_string(), sort }
                Th { {t!("rollout-list-col-health")} }
                SortableTh { label: t!("created"), sort_key: "created".to_string(), sort }
                Th { "" }
            },
            body: rsx! {
                for r in filtered.read().iter().skip(start).take(limit_val) {
                    RolloutRow { key: "{r.id}", entry: r.clone(), rollouts }
                }
            },
        }
    }
}

#[component]
fn RolloutRow(
    entry: RolloutEntry,
    rollouts: Resource<Result<Vec<RolloutEntry>, ServerFnError>>,
) -> Element {
    let mut rollouts = rollouts;
    let rid = entry.id.to_string();
    let created = entry.created_at.format("%Y-%m-%d %H:%M").to_string();
    let (status_var, status_text) = status_variant(&entry.status);
    let can_delete =
        entry.status == "pending" || entry.status == "completed" || entry.status == "failed";

    rsx! {
        tr {
            Td { class: "text-sm",
                Link { to: Route::RolloutDetail { id: rid.clone() }, class: "link",
                    if let Some(ref name) = entry.name {
                        span { "{name}" }
                    } else {
                        span { class: "font-mono text-xs", "{rid}" }
                    }
                }
            }
            Td { class: "text-sm",
                Badge { variant: status_var, "{status_text}" }
            }
            Td { class: "text-sm", "{entry.stage_count}" }
            Td { class: "text-sm", HealthCell { health: entry.health.clone() } }
            TdMuted { class: "text-sm", {created} }
            td { class: "td text-right",
                if can_delete {
                    button {
                        class: "link-danger text-sm",
                        onclick: {
                            let id = entry.id;
                            move |_| {
                                async move {
                                    let _ = delete_rollout(RolloutDeleteInput { id }).await;
                                    rollouts.restart();
                                }
                            }
                        },
                        {t!("delete")}
                    }
                }
            }
        }
    }
}

#[component]
fn HealthCell(health: Option<RolloutListHealth>) -> Element {
    let Some(h) = health else {
        return rsx! { span { class: "text-fg-faint", {t!("em-dash")} } };
    };
    let (variant, label) = health_variant(&h.state);
    let title = if h.summary.is_empty() {
        t!("rollout-health-tooltip", evaluated: h.evaluated_stages, failing: h.failing_stages)
    } else {
        t!(
            "rollout-health-tooltip-summary",
            evaluated: h.evaluated_stages,
            failing: h.failing_stages,
            summary: h.summary.clone()
        )
    };
    rsx! {
        span { class: "inline-flex items-center gap-2",
            Badge { variant, title, "{label}" }
            if h.failing_stages > 0 {
                span { class: "text-xs font-mono text-danger-strong",
                    "{h.failing_stages}/{h.evaluated_stages}"
                }
            } else if h.evaluated_stages > 0 {
                span { class: "text-xs font-mono text-fg-muted",
                    "{h.evaluated_stages}/{h.evaluated_stages}"
                }
            }
        }
    }
}
