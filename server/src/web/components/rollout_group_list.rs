use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::api_mcp::endpoints::rollouts::{
    GroupCreateInput, GroupEntry, GroupListInput, create_group, get_rollout_groups,
};
use crate::web::app::Route;
use crate::web::components::table_utils::Searchable;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{
    Button, ButtonVariant, Card, DataTable, ErrorText, HelpText, PageHeader, SectionHeading,
    SortState, SortableTh, Td, TdMuted, page_window,
};

impl Searchable for GroupEntry {
    fn matches_search(&self, query: &str) -> bool {
        self.name.to_lowercase().contains(query) || self.description.to_lowercase().contains(query)
    }
}

#[component]
pub fn RolloutGroupList() -> Element {
    use_topbar(t!("nav-rollouts"), None);
    let groups =
        use_server_future(move || async move { get_rollout_groups(GroupListInput {}).await })?;

    rsx! {
        PageHeader { {t!("rollout-group-list-title")} }
        CreateGroupForm { on_created: move |_| { let mut g = groups; g.restart(); } }
        {match &*groups.read() {
            Some(Ok(list)) => rsx! { GroupTable { list: list.clone() } },
            Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
            None => rsx! { HelpText { {t!("loading")} } },
        }}
    }
}

#[component]
fn CreateGroupForm(on_created: EventHandler<()>) -> Element {
    let mut name = use_signal(String::new);
    let mut desc = use_signal(String::new);

    rsx! {
        Card { class: "mb-6 p-4",
            SectionHeading { class: "mb-2", {t!("rollout-group-list-create")} }
            div { class: "flex gap-2",
                input {
                    class: "input input-sm flex-1 w-auto",
                    placeholder: t!("rollout-group-list-name-placeholder"),
                    value: "{name}",
                    oninput: move |e| name.set(e.value()),
                }
                input {
                    class: "input input-sm flex-1 w-auto",
                    placeholder: t!("rollout-group-list-desc-placeholder"),
                    value: "{desc}",
                    oninput: move |e| desc.set(e.value()),
                }
                Button {
                    variant: ButtonVariant::Primary,
                    onclick: move |_| {
                        let n = name.read().clone();
                        let d = desc.read().clone();
                        async move {
                            if !n.trim().is_empty() {
                                let _ = create_group(GroupCreateInput {
                                    name: n,
                                    description: d,
                                })
                                .await;
                                name.set(String::new());
                                desc.set(String::new());
                                on_created.call(());
                            }
                        }
                    },
                    {t!("create")}
                }
            }
        }
    }
}

#[component]
fn GroupTable(list: Vec<GroupEntry>) -> Element {
    let search = use_signal(String::new);
    let limit = use_signal(|| 20usize);
    let page = use_signal(|| 0usize);
    let sort = use_signal::<SortState>(|| ("name".to_string(), true));

    let list_clone = list.clone();
    let filtered = use_memo(move || {
        let q = search.read().to_lowercase();
        let mut items: Vec<GroupEntry> = if q.is_empty() {
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
                "description" => a
                    .description
                    .to_lowercase()
                    .cmp(&b.description.to_lowercase()),
                "members" => a.member_count.cmp(&b.member_count),
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
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
                SortableTh { label: t!("description"), sort_key: "description".to_string(), sort }
                SortableTh { label: t!("rollout-group-list-col-members"), sort_key: "members".to_string(), sort }
            },
            body: rsx! {
                for g in filtered.read().iter().skip(start).take(limit_val) {
                    GroupRowView { key: "{g.id}", group: g.clone() }
                }
            },
        }
    }
}

#[component]
fn GroupRowView(group: GroupEntry) -> Element {
    let gid = group.id.to_string();
    rsx! {
        tr {
            Td { class: "text-sm font-medium",
                Link { to: Route::RolloutGroupDetail { id: gid }, class: "link",
                    "{group.name}"
                }
            }
            TdMuted { class: "text-sm", "{group.description}" }
            Td { class: "text-sm", "{group.member_count}" }
        }
    }
}
