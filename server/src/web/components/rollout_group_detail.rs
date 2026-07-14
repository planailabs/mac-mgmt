use dioxus::prelude::*;
use dioxus_i18n::t;
use uuid::Uuid;

use crate::api_mcp::endpoints::rollouts::{
    GroupAvailableClustersInput, GroupDeleteInput, GroupGetInput, GroupSetDescriptionInput,
    MemberAddAllInput, MemberAddInput, MemberEntry, MemberRemoveInput, add_all_clusters,
    add_member, delete_group, get_available_clusters, get_group_detail, remove_member,
    update_description,
};
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{
    Button, ButtonSize, ButtonVariant, DataTable, ErrorText, HelpText, SectionHeading, SortState,
    SortableTh, Td, Th, page_window,
};

#[component]
pub fn RolloutGroupDetail(id: String) -> Element {
    use_topbar(t!("nav-rollouts"), None);
    let id_clone = id.clone();
    let mut detail = use_server_future(move || {
        let id = id_clone.clone();
        async move {
            let id: Uuid = id
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            get_group_detail(GroupGetInput { id }).await
        }
    })?;

    let id_for_clusters = id.clone();
    let mut available = use_server_future(move || {
        let id = id_for_clusters.clone();
        async move {
            let group_id: Uuid = id
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            get_available_clusters(GroupAvailableClustersInput { group_id }).await
        }
    })?;

    let mut selected_cluster = use_signal(|| Option::<String>::None);
    let mut editing_desc = use_signal(|| false);
    let mut draft_desc = use_signal(String::new);
    let nav = navigator();

    match &*detail.read() {
        Some(Ok(info)) => {
            let gid = info.id;

            let clusters = match &*available.read() {
                Some(Ok(list)) => list.clone(),
                _ => vec![],
            };

            rsx! {
                div { class: "flex justify-between items-center mb-4",
                    div {
                        h1 { class: "h-page mb-0", "{info.name}" }
                        if *editing_desc.read() {
                            form { class: "flex items-center gap-2 mt-1",
                                onsubmit: {
                                    move |evt: FormEvent| {
                                        evt.prevent_default();
                                        let desc = draft_desc.read().clone();
                                        async move {
                                            let _ = update_description(GroupSetDescriptionInput {
                                                group_id: gid,
                                                description: desc,
                                            })
                                            .await;
                                            editing_desc.set(false);
                                            detail.restart();
                                        }
                                    }
                                },
                                input { class: "input input-sm w-80",
                                    r#type: "text",
                                    value: "{draft_desc}",
                                    oninput: move |e| draft_desc.set(e.value()),
                                    autofocus: true,
                                }
                                button { class: "text-success hover:opacity-80 text-sm", r#type: "submit",
                                    {t!("save")}
                                }
                                button { class: "text-fg-muted hover:text-fg-strong text-sm", r#type: "button",
                                    onclick: move |_| editing_desc.set(false),
                                    {t!("cancel")}
                                }
                            }
                        } else {
                            {
                                let desc = info.description.clone();
                                rsx! {
                                    div { class: "flex items-center gap-2 mt-1",
                                        p { class: "help", "{info.description}" }
                                        button { class: "text-fg-faint hover:text-fg-muted text-sm",
                                            onclick: move |_| {
                                                draft_desc.set(desc.clone());
                                                editing_desc.set(true);
                                            },
                                            {t!("edit")}
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Button { variant: ButtonVariant::Danger, size: ButtonSize::Sm,
                        onclick: {
                            move |_| {
                                async move {
                                    let _ = delete_group(GroupDeleteInput { id: gid }).await;
                                    nav.push(crate::web::app::Route::RolloutGroupList {});
                                }
                            }
                        },
                        {t!("rollout-group-delete")}
                    }
                }

                SectionHeading { {t!("rollout-group-members")} }

                div { class: "flex gap-2 mb-4",
                    select { class: "input flex-1 w-auto py-1 text-sm",
                        onchange: move |e| {
                            let val = e.value();
                            if val.is_empty() {
                                selected_cluster.set(None);
                            } else {
                                selected_cluster.set(Some(val));
                            }
                        },
                        option { value: "", {t!("rollout-group-select-cluster")} }
                        for c in &clusters {
                            {
                                let cid = c.id.to_string();
                                let cname = c.name.clone();
                                rsx! { option { value: "{cid}", "{cname}" } }
                            }
                        }
                    }
                    Button { size: ButtonSize::Sm,
                        disabled: selected_cluster.read().is_none(),
                        onclick: {
                            move |_| {
                                let cid = selected_cluster.read().clone();
                                async move {
                                    if let Some(cid) = cid {
                                        let Ok(cluster_id) = cid.parse::<Uuid>() else {
                                            return;
                                        };
                                        let _ = add_member(MemberAddInput {
                                            group_id: gid,
                                            cluster_id,
                                        })
                                        .await;
                                        selected_cluster.set(None);
                                        detail.restart();
                                        available.restart();
                                    }
                                }
                            }
                        },
                        {t!("add")}
                    }
                    if !clusters.is_empty() {
                        Button { variant: ButtonVariant::Secondary, size: ButtonSize::Sm,
                            onclick: {
                                move |_| {
                                    async move {
                                        let _ = add_all_clusters(MemberAddAllInput {
                                            group_id: gid,
                                        })
                                        .await;
                                        selected_cluster.set(None);
                                        detail.restart();
                                        available.restart();
                                    }
                                }
                            },
                            {t!("rollout-group-add-all")}
                        }
                    }
                }

                if info.members.is_empty() {
                    HelpText { {t!("rollout-group-no-members")} }
                } else {
                    MembersTable { members: info.members.clone(), on_remove: move |_| { detail.restart(); available.restart(); } }
                }
            }
        }
        Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
        None => rsx! { HelpText { {t!("loading")} } },
    }
}

#[component]
fn MembersTable(members: Vec<MemberEntry>, on_remove: EventHandler<()>) -> Element {
    let search = use_signal(String::new);
    let limit = use_signal(|| 20usize);
    let page = use_signal(|| 0usize);
    let sort = use_signal::<SortState>(|| ("cluster".to_string(), true));

    let members_clone = members.clone();
    let filtered = use_memo(move || {
        let q = search.read().to_lowercase();
        let mut items: Vec<MemberEntry> = if q.is_empty() {
            members_clone.clone()
        } else {
            members_clone
                .iter()
                .filter(|m| m.cluster_name.to_lowercase().contains(&q))
                .cloned()
                .collect()
        };
        let (_key, asc) = sort.read().clone();
        items.sort_by(|a, b| {
            let ord = a
                .cluster_name
                .to_lowercase()
                .cmp(&b.cluster_name.to_lowercase());
            if asc { ord } else { ord.reverse() }
        });
        items
    });

    let total = members.len();
    let filtered_count = filtered.read().len();
    let limit_val = *limit.read();
    let (start, shown) = page_window(*page.read(), limit_val, filtered_count);

    rsx! {
        DataTable {
            search, limit, page, total, filtered: filtered_count, shown,
            headers: rsx! {
                SortableTh { label: t!("rollout-group-col-cluster"), sort_key: "cluster".to_string(), sort }
                Th { "" }
            },
            body: rsx! {
                for m in filtered.read().iter().skip(start).take(limit_val) {
                    {
                        let mid = m.member_id;
                        let cluster_name = m.cluster_name.clone();
                        rsx! {
                            tr { key: "{mid}",
                                Td { class: "text-sm", "{cluster_name}" }
                                td { class: "td text-right",
                                    button { class: "link-danger text-sm",
                                        onclick: {
                                            move |_| {
                                                let on_remove = on_remove;
                                                async move {
                                                    if remove_member(MemberRemoveInput {
                                                        member_id: mid,
                                                    })
                                                    .await
                                                    .is_ok()
                                                    {
                                                        on_remove.call(());
                                                    }
                                                }
                                            }
                                        },
                                        {t!("remove")}
                                    }
                                }
                            }
                        }
                    }
                }
            },
        }
    }
}
