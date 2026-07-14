use dioxus::prelude::*;
use dioxus_i18n::t;

use crate::anthropic::{GenerateContext, GeneratedNameDesc, McpBundleItemContext};
use crate::api_mcp::endpoints::mcp_servers::{
    McpBundleDeleteInput, McpBundleGetInput, McpBundleItemAddInput, McpBundleItemRemoveInput,
    McpBundleItemsInput, McpBundleUpdateInput, McpServerOptionsInput, add_mcp_bundle_item,
    delete_mcp_bundle, get_mcp_bundle, list_available_mcp_servers, list_mcp_bundle_items,
    remove_mcp_bundle_item, update_mcp_bundle,
};
use crate::web::app::Route;
use crate::web::components::generate_button::GenerateButton;
use crate::web::components::hidden_badge::HiddenBadge;
use crate::web::components::topbar::use_topbar;
use crate::web::components::ui::{
    Button, ButtonKind, ButtonSize, ErrorText, HelpText, SectionHeading,
};

#[component]
pub fn McpBundleDetail(id: String) -> Element {
    use_topbar(t!("nav-mcp-bundles"), None);
    let navigator = navigator();
    let id_clone = id.clone();
    let mut bundle = use_server_future(move || {
        let id = id_clone.clone();
        async move {
            let id: uuid::Uuid = id
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            get_mcp_bundle(McpBundleGetInput { id }).await
        }
    })?;

    let id_items = id.clone();
    let mut items = use_server_future(move || {
        let id = id_items.clone();
        async move {
            let bundle_id: uuid::Uuid = id
                .parse()
                .map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
            list_mcp_bundle_items(McpBundleItemsInput { bundle_id }).await
        }
    })?;

    let mut available = use_server_future(|| async move {
        list_available_mcp_servers(McpServerOptionsInput {}).await
    })?;

    let mut editing = use_signal(|| false);
    let mut draft_name = use_signal(String::new);
    let mut draft_desc = use_signal(String::new);
    let mut draft_hide = use_signal(|| false);
    let mut selected_server = use_signal(String::new);

    match &*bundle.read() {
        Some(Ok(b)) => {
            let created = b.created_at.format("%Y-%m-%d %H:%M").to_string();
            let bid = b.id;
            let bid_del = b.id;
            let bid2 = b.id;
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
                                let id = bid;
                                let new_name = draft_name.read().clone();
                                let new_desc = draft_desc.read().clone();
                                let new_hide = *draft_hide.read();
                                spawn(async move {
                                    if !new_name.trim().is_empty() {
                                        let _ = update_mcp_bundle(McpBundleUpdateInput {
                                            id,
                                            name: new_name,
                                            description: new_desc,
                                            hide_from_public_catalog: new_hide,
                                        })
                                        .await;
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
                                {t!("mcp-bundle-detail-hide")}
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
                                    let mcp_items_ctx = match &*items.read() {
                                        Some(Ok(list)) => list.iter().map(|i| McpBundleItemContext {
                                            server_slug: i.server_slug.clone(),
                                            server_name: i.server_name.clone(),
                                        }).collect(),
                                        _ => vec![],
                                    };
                                    rsx! {
                                        GenerateButton {
                                            context: GenerateContext::McpBundle { slug: slug.clone(), items: mcp_items_ctx },
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
                        h1 { class: "h-page mb-0", "{name}" }
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
                                let id = bid_del;
                                let nav = navigator;
                                spawn(async move {
                                    if delete_mcp_bundle(McpBundleDeleteInput { id }).await.is_ok() {
                                        nav.push(Route::McpBundleList {});
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
                    SectionHeading { {t!("mcp-bundle-detail-mcp-servers")} }
                    form { class: "flex gap-2 mb-4",
                        onsubmit: move |evt: FormEvent| {
                            evt.prevent_default();
                            let bundle_id = bid2;
                            let msid = selected_server.read().clone();
                            spawn(async move {
                                let Ok(mcp_server_id) = msid.parse::<uuid::Uuid>() else {
                                    return;
                                };
                                if add_mcp_bundle_item(McpBundleItemAddInput {
                                    bundle_id,
                                    mcp_server_id,
                                })
                                .await
                                .is_ok()
                                {
                                    selected_server.set(String::new());
                                    items.restart();
                                    available.restart();
                                }
                            });
                        },
                        select { class: "input flex-1 w-auto py-1 text-sm",
                            value: "{selected_server}",
                            onchange: move |evt| selected_server.set(evt.value()),
                            option { value: "", {t!("mcp-bundle-detail-select-mcp")} }
                            {match &*available.read() {
                                Some(Ok(list)) => rsx! {
                                    for s in list {
                                        {
                                            let val = s.id.to_string();
                                            let label = format!("{} ({})", s.name, s.slug);
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
                                        let biid = item.bundle_item_id;
                                        let label = format!("{} ({})", item.server_name, item.server_slug);
                                        rsx! {
                                            li { class: "py-2 flex justify-between items-center",
                                                span { class: "text-sm font-mono", "{label}" }
                                                button { class: "link-danger text-xs",
                                                    onclick: move |_| {
                                                        let bundle_item_id = biid;
                                                        spawn(async move {
                                                            if remove_mcp_bundle_item(McpBundleItemRemoveInput { bundle_item_id }).await.is_ok() {
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
