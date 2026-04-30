use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{
    Button, ButtonSize, ButtonVariant, DataTable, ErrorText, HelpText, SectionHeading, SortState,
    SortableTh, Td, Th,
};
#[cfg(feature = "server")]
use crate::web::user::current_user;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct GroupInfo {
    id: Uuid,
    name: String,
    description: String,
    members: Vec<MemberEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct MemberEntry {
    member_id: Uuid,
    cluster_id: Uuid,
    cluster_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ClusterOption {
    id: Uuid,
    name: String,
}

#[server]
async fn get_group_detail(id: String) -> Result<GroupInfo, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let gid: Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if gid == Uuid::nil() {
        return Err(ServerFnError::new(
            "the all-clusters group is implicit and has no detail page",
        ));
    }

    #[derive(sqlx::FromRow)]
    struct GRow {
        id: Uuid,
        name: String,
        description: String,
    }

    let group =
        sqlx::query_as::<_, GRow>("SELECT id, name, description FROM rollout_groups WHERE id = $1")
            .bind(gid)
            .fetch_one(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct MRow {
        member_id: Uuid,
        cluster_id: Uuid,
        cluster_name: String,
    }

    let members = sqlx::query_as::<_, MRow>(
        "SELECT rgm.id AS member_id, rgm.cluster_id, c.name AS cluster_name \
         FROM rollout_group_members rgm \
         JOIN clusters c ON c.id = rgm.cluster_id \
         WHERE rgm.group_id = $1 ORDER BY c.name",
    )
    .bind(gid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(GroupInfo {
        id: group.id,
        name: group.name,
        description: group.description,
        members: members
            .into_iter()
            .map(|m| MemberEntry {
                member_id: m.member_id,
                cluster_id: m.cluster_id,
                cluster_name: m.cluster_name,
            })
            .collect(),
    })
}

#[server]
async fn get_available_clusters(group_id: String) -> Result<Vec<ClusterOption>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let gid: Uuid = group_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        name: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT id, name FROM clusters \
         WHERE id NOT IN (SELECT cluster_id FROM rollout_group_members WHERE group_id = $1) \
         ORDER BY name",
    )
    .bind(gid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| ClusterOption {
            id: r.id,
            name: r.name,
        })
        .collect())
}

#[server]
async fn add_member(group_id: String, cluster_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let gid: Uuid = group_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let cid: Uuid = cluster_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("INSERT INTO rollout_group_members (group_id, cluster_id) VALUES ($1, $2)")
        .bind(gid)
        .bind(cid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn add_all_clusters(group_id: String) -> Result<u64, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let gid: Uuid = group_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let result = sqlx::query(
        "INSERT INTO rollout_group_members (group_id, cluster_id) \
         SELECT $1, id FROM clusters \
         WHERE id NOT IN (SELECT cluster_id FROM rollout_group_members WHERE group_id = $1) \
         ON CONFLICT DO NOTHING",
    )
    .bind(gid)
    .execute(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(result.rows_affected())
}

#[server]
async fn remove_member(member_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let mid: Uuid = member_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("DELETE FROM rollout_group_members WHERE id = $1")
        .bind(mid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn update_description(group_id: String, description: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let gid: Uuid = group_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("UPDATE rollout_groups SET description = $2 WHERE id = $1")
        .bind(gid)
        .bind(&description)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn delete_group(id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let gid: Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    if gid.is_nil() {
        return Err(ServerFnError::new(
            "the All Clusters group cannot be deleted",
        ));
    }
    sqlx::query("DELETE FROM rollout_groups WHERE id = $1")
        .bind(gid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[component]
pub fn RolloutGroupDetail(id: String) -> Element {
    use_topbar(t!("nav-rollouts"), None);
    let id_clone = id.clone();
    let mut detail = use_server_future(move || {
        let id = id_clone.clone();
        async move { get_group_detail(id).await }
    })?;

    let id_for_clusters = id.clone();
    let mut available = use_server_future(move || {
        let id = id_for_clusters.clone();
        async move { get_available_clusters(id).await }
    })?;

    let mut selected_cluster = use_signal(|| Option::<String>::None);
    let mut editing_desc = use_signal(|| false);
    let mut draft_desc = use_signal(String::new);
    let nav = navigator();

    match &*detail.read() {
        Some(Ok(info)) => {
            let gid = info.id.to_string();

            let clusters = match &*available.read() {
                Some(Ok(list)) => list.clone(),
                _ => vec![],
            };

            rsx! {
                div { class: "flex justify-between items-center mb-4",
                    div {
                        h1 { class: "h-display", "{info.name}" }
                        if *editing_desc.read() {
                            form { class: "flex items-center gap-2 mt-1",
                                onsubmit: {
                                    let gid = gid.clone();
                                    move |evt: FormEvent| {
                                        evt.prevent_default();
                                        let gid = gid.clone();
                                        let desc = draft_desc.read().clone();
                                        async move {
                                            let _ = update_description(gid, desc).await;
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
                            let gid = gid.clone();
                            move |_| {
                                let gid = gid.clone();
                                async move {
                                    let _ = delete_group(gid).await;
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
                            let gid = gid.clone();
                            move |_| {
                                let gid = gid.clone();
                                let cid = selected_cluster.read().clone();
                                async move {
                                    if let Some(cid) = cid {
                                        let _ = add_member(gid, cid).await;
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
                                let gid = gid.clone();
                                move |_| {
                                    let gid = gid.clone();
                                    async move {
                                        let _ = add_all_clusters(gid).await;
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
            let ord = a.cluster_name.to_lowercase().cmp(&b.cluster_name.to_lowercase());
            if asc { ord } else { ord.reverse() }
        });
        items
    });

    let total = members.len();
    let filtered_count = filtered.read().len();
    let limit_val = *limit.read();
    let shown = filtered_count.min(limit_val);

    rsx! {
        DataTable {
            search, limit, total, filtered: filtered_count, shown,
            headers: rsx! {
                SortableTh { label: t!("rollout-group-col-cluster"), sort_key: "cluster".to_string(), sort }
                Th { "" }
            },
            body: rsx! {
                for m in filtered.read().iter().take(limit_val) {
                    {
                        let mid = m.member_id.to_string();
                        let cluster_name = m.cluster_name.clone();
                        rsx! {
                            tr { key: "{mid}",
                                Td { class: "text-sm", "{cluster_name}" }
                                td { class: "td text-right",
                                    button { class: "link-danger text-sm",
                                        onclick: {
                                            let mid = mid.clone();
                                            move |_| {
                                                let mid = mid.clone();
                                                let on_remove = on_remove;
                                                async move {
                                                    if remove_member(mid).await.is_ok() {
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
