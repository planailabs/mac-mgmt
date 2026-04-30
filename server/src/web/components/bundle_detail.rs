use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::anthropic::{BundleItemContext, GenerateContext, GeneratedNameDesc};
use crate::models::Bundle;
use crate::web::app::Route;
use crate::web::components::generate_button::GenerateButton;
use crate::web::components::hidden_badge::HiddenBadge;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{
    Button, ButtonKind, ButtonSize, ErrorText, HelpText, SectionHeading,
};
#[cfg(feature = "server")]
use crate::web::user::current_user;

/// A skill channel with its skill slug for display.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct SkillChannelDisplay {
    pub id: uuid::Uuid,
    pub skill_slug: String,
    pub channel: String,
}

/// A bundle item joined with skill info for display.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct BundleItemDisplay {
    pub bundle_item_id: uuid::Uuid,
    pub skill_slug: String,
    pub channel: String,
}

#[server]
async fn get_bundle(id: String) -> Result<Bundle, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let bundle = sqlx::query_as::<_, Bundle>("SELECT * FROM bundles WHERE id = $1")
        .bind(uuid)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(bundle)
}

#[server]
async fn delete_bundle(id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    crate::api::push::notify_federation_global();
    crate::api::push::notify_skill_bundle_global(uuid).await;
    sqlx::query("DELETE FROM bundles WHERE id = $1")
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn update_bundle(
    id: String,
    name: String,
    description: String,
    hide_from_public_catalog: bool,
) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("UPDATE bundles SET name = $1, description = $2, hide_from_public_catalog = $3 WHERE id = $4")
        .bind(&name)
        .bind(&description)
        .bind(hide_from_public_catalog)
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    crate::api::push::notify_federation_global();
    crate::api::push::notify_skill_bundle_global(uuid).await;
    Ok(())
}

#[server]
async fn list_bundle_items(bundle_id: String) -> Result<Vec<BundleItemDisplay>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = bundle_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let items = sqlx::query_as::<_, BundleItemDisplay>(
        "SELECT bi.id as bundle_item_id, s.slug as skill_slug, sc.channel \
         FROM bundle_items bi \
         JOIN skill_channels sc ON sc.id = bi.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE bi.bundle_id = $1 \
         ORDER BY s.slug, sc.channel",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(items)
}

#[server]
async fn list_available_skill_channels() -> Result<Vec<SkillChannelDisplay>, ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let channels = sqlx::query_as::<_, SkillChannelDisplay>(
        "SELECT sc.id, s.slug as skill_slug, sc.channel \
         FROM skill_channels sc \
         JOIN skills s ON s.id = sc.skill_id \
         ORDER BY s.slug, sc.channel",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(channels)
}

#[server]
async fn add_bundle_item(bundle_id: String, skill_channel_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let bid: uuid::Uuid = bundle_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let scid: uuid::Uuid = skill_channel_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("INSERT INTO bundle_items (bundle_id, skill_channel_id) VALUES ($1, $2)")
        .bind(bid)
        .bind(scid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    crate::api::push::notify_federation_global();
    crate::api::push::notify_skill_bundle_global(bid).await;
    Ok(())
}

#[server]
async fn remove_bundle_item(bundle_item_id: String) -> Result<(), ServerFnError> {
    let user = current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = bundle_item_id
        .parse()
        .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let bundle_id: Option<uuid::Uuid> =
        sqlx::query_scalar("SELECT bundle_id FROM bundle_items WHERE id = $1")
            .bind(uuid)
            .fetch_optional(&pool)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    sqlx::query("DELETE FROM bundle_items WHERE id = $1")
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    if let Some(bid) = bundle_id {
        crate::api::push::notify_federation_global();
        crate::api::push::notify_skill_bundle_global(bid).await;
    }
    Ok(())
}

#[component]
pub fn BundleDetail(id: String) -> Element {
    use_topbar(t!("nav-bundles"), None);
    let navigator = navigator();
    let id_clone = id.clone();
    let mut bundle = use_server_future(move || {
        let id = id_clone.clone();
        async move { get_bundle(id).await }
    })?;

    let id_items = id.clone();
    let mut items = use_server_future(move || {
        let id = id_items.clone();
        async move { list_bundle_items(id).await }
    })?;

    let mut available = use_server_future(list_available_skill_channels)?;

    let mut editing = use_signal(|| false);
    let mut draft_name = use_signal(String::new);
    let mut draft_desc = use_signal(String::new);
    let mut draft_hide = use_signal(|| false);
    let mut selected_sc = use_signal(String::new);

    match &*bundle.read() {
        Some(Ok(b)) => {
            let created = b.created_at.format("%Y-%m-%d %H:%M").to_string();
            let bid = b.id.to_string();
            let bid_del = bid.clone();
            let bid2 = bid.clone();
            let name = b.name.clone();
            let desc = b.description.clone();
            let slug = b.slug.clone();
            let hide_flag = b.hide_from_public_catalog;

            rsx! {
                div { class: "flex items-center gap-3 mb-1",
                    if *editing.read() {
                        form { class: "space-y-2",
                            onsubmit: move |evt: FormEvent| {
                                evt.prevent_default();
                                let id = bid.clone();
                                let new_name = draft_name.read().clone();
                                let new_desc = draft_desc.read().clone();
                                let new_hide = *draft_hide.read();
                                spawn(async move {
                                    if !new_name.trim().is_empty() {
                                        let _ = update_bundle(id, new_name, new_desc, new_hide).await;
                                        bundle.restart();
                                    }
                                    editing.set(false);
                                });
                            },
                            input {
                                class: "input text-2xl font-bold py-1",
                                r#type: "text",
                                value: "{draft_name}",
                                oninput: move |e| draft_name.set(e.value()),
                                autofocus: true,
                            }
                            textarea {
                                class: "input py-1",
                                rows: "2",
                                value: "{draft_desc}",
                                oninput: move |e| draft_desc.set(e.value()),
                            }
                            label { class: "flex items-center gap-2 text-sm text-fg-strong",
                                input {
                                    r#type: "checkbox",
                                    checked: "{draft_hide}",
                                    class: "rounded border-line",
                                    oninput: move |e| draft_hide.set(e.value() == "true"),
                                }
                                {t!("bundle-detail-hide")}
                            }
                            div { class: "flex gap-2",
                                button { class: "text-success hover:opacity-80", r#type: "submit",
                                    {t!("save")}
                                }
                                button { class: "text-fg-muted hover:text-fg-strong", r#type: "button",
                                    onclick: move |_| editing.set(false),
                                    {t!("cancel")}
                                }
                                {
                                    let bundle_items_ctx = match &*items.read() {
                                        Some(Ok(list)) => list.iter().map(|i| BundleItemContext {
                                            skill_slug: i.skill_slug.clone(),
                                            channel: i.channel.clone(),
                                        }).collect(),
                                        _ => vec![],
                                    };
                                    rsx! {
                                        GenerateButton {
                                            context: GenerateContext::Bundle { slug: slug.clone(), items: bundle_items_ctx },
                                            current_name: draft_name.read().clone(),
                                            current_desc: draft_desc.read().clone(),
                                            on_generated: move |result: GeneratedNameDesc| {
                                                draft_name.set(result.name);
                                                draft_desc.set(result.description);
                                            },
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        h2 { class: "h-page mb-0", "{name}" }
                        span { class: "text-fg-faint font-mono text-sm", "({slug})" }
                        HiddenBadge { hidden: hide_flag }
                        button { class: "text-fg-faint hover:text-fg-muted",
                            onclick: move |_| {
                                draft_name.set(name.clone());
                                draft_desc.set(desc.clone());
                                draft_hide.set(hide_flag);
                                editing.set(true);
                            },
                            {t!("edit")}
                        }
                        button { class: "link-danger",
                            onclick: move |_| {
                                let id = bid_del.clone();
                                let nav = navigator;
                                spawn(async move {
                                    if delete_bundle(id).await.is_ok() {
                                        nav.push(Route::BundleList {});
                                    }
                                });
                            },
                            {t!("delete")}
                        }
                    }
                }
                if !*editing.read() && !b.description.is_empty() {
                    p { class: "text-fg mb-2", "{b.description}" }
                }
                p { class: "help mb-6", {t!("cluster-detail-created", date: created)} }

                // Items section
                div {
                    SectionHeading { {t!("bundle-detail-skill-channels")} }
                    form { class: "flex gap-2 mb-4",
                        onsubmit: move |evt: FormEvent| {
                            evt.prevent_default();
                            let bid = bid2.clone();
                            let scid = selected_sc.read().clone();
                            spawn(async move {
                                if !scid.is_empty()
                                    && add_bundle_item(bid, scid).await.is_ok()
                                {
                                    selected_sc.set(String::new());
                                    items.restart();
                                    available.restart();
                                }
                            });
                        },
                        select { class: "input flex-1 w-auto py-1 text-sm",
                            value: "{selected_sc}",
                            onchange: move |evt| selected_sc.set(evt.value()),
                            option { value: "", {t!("bundle-detail-select-skill")} }
                            {match &*available.read() {
                                Some(Ok(list)) => rsx! {
                                    for sc in list {
                                        {
                                            let val = sc.id.to_string();
                                            let label = format!("{} / {}", sc.skill_slug, sc.channel);
                                            rsx! { option { value: "{val}", "{label}" } }
                                        }
                                    }
                                },
                                _ => rsx! {},
                            }}
                        }
                        Button { kind: ButtonKind::Submit, size: ButtonSize::Sm,
                            {t!("add")}
                        }
                    }
                    {match &*items.read() {
                        Some(Ok(list)) => rsx! {
                            ul { class: "divide-y divide-line-soft",
                                for item in list {
                                    {
                                        let biid = item.bundle_item_id.to_string();
                                        let label = format!("{} / {}", item.skill_slug, item.channel);
                                        rsx! {
                                            li { class: "py-2 flex justify-between items-center",
                                                span { class: "text-sm font-mono", "{label}" }
                                                button { class: "link-danger text-xs",
                                                    onclick: move |_| {
                                                        let biid = biid.clone();
                                                        spawn(async move {
                                                            if remove_bundle_item(biid).await.is_ok() {
                                                                items.restart();
                                                            }
                                                        });
                                                    },
                                                    {t!("remove")}
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
                        None => rsx! { HelpText { {t!("loading")} } },
                    }}
                }
            }
        }
        Some(Err(e)) => rsx! { ErrorText { {t!("error-message", message: e.to_string())} } },
        None => rsx! { HelpText { {t!("loading")} } },
    }
}
