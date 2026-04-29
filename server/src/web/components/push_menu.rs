//! Shared dropdown menu for sending SSE push events to daemons.
//! Used in fleet detail (per-instance) and cluster detail (per-cluster).

use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};

#[cfg(feature = "server")]
use crate::web::user::current_user;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushResult {
    pub ok: bool,
    pub message: String,
}

/// Send a push event to all daemons in a cluster.
#[server]
pub async fn send_push_event(
    cluster_id: String,
    event: String,
) -> Result<PushResult, ServerFnError> {
    let user = current_user().await?;
    let pool = crate::server_pool()?;
    let cid: uuid::Uuid = cluster_id
        .parse()
        .map_err(|_| ServerFnError::new("invalid cluster id"))?;
    user.require_cluster_write(&pool, cid).await?;

    let msg = match event.as_str() {
        "sync_config" => mac_mgmt_common::PushEvent::SyncConfig,
        "sync_skills" => mac_mgmt_common::PushEvent::SyncSkills,
        "sync_mcp_servers" => mac_mgmt_common::PushEvent::SyncMcpServers,
        "sync_ssh_keys" => mac_mgmt_common::PushEvent::SyncSshKeys,
        "self_update" => mac_mgmt_common::PushEvent::SelfUpdate,
        "sync_nixpkgs" => mac_mgmt_common::PushEvent::SyncNixpkgs,
        "sync_packages" => mac_mgmt_common::PushEvent::SyncPackages,
        "request_assessment" => mac_mgmt_common::PushEvent::RequestAssessment,
        other => return Ok(PushResult {
            ok: false,
            message: format!("unknown event: {other}"),
        }),
    };

    crate::api::push::notify_global(cid, msg).await;
    Ok(PushResult {
        ok: true,
        message: format!("{event} sent"),
    })
}

struct PushAction {
    key: &'static str,
    label_key: &'static str,
    desc_key: &'static str,
}

const ACTIONS: &[PushAction] = &[
    PushAction { key: "sync_config", label_key: "push-sync-config", desc_key: "push-sync-config-desc" },
    PushAction { key: "sync_skills", label_key: "push-sync-skills", desc_key: "push-sync-skills-desc" },
    PushAction { key: "sync_mcp_servers", label_key: "push-sync-mcp", desc_key: "push-sync-mcp-desc" },
    PushAction { key: "sync_ssh_keys", label_key: "push-sync-ssh", desc_key: "push-sync-ssh-desc" },
    PushAction { key: "sync_nixpkgs", label_key: "push-sync-nixpkgs", desc_key: "push-sync-nixpkgs-desc" },
    PushAction { key: "sync_packages", label_key: "push-sync-packages", desc_key: "push-sync-packages-desc" },
    PushAction { key: "self_update", label_key: "push-self-update", desc_key: "push-self-update-desc" },
    PushAction { key: "request_assessment", label_key: "push-request-assessment", desc_key: "push-request-assessment-desc" },
];

#[component]
pub fn PushMenu(cluster_id: String) -> Element {
    let mut open = use_signal(|| false);
    let mut status = use_signal(|| Option::<(bool, String)>::None);

    rsx! {
        div { class: "relative inline-block",
            button { class: "btn btn-md btn-secondary",
                onclick: move |_| { let v = *open.read(); open.set(!v); },
                {t!("push-menu-button")}
            }
            if *open.read() {
                div { class: "card shadow-lg border border-line-soft absolute right-0 mt-1 w-56 z-50",
                    for action in ACTIONS {
                        {
                            let key = action.key;
                            let label = t!(action.label_key);
                            let desc = t!(action.desc_key);
                            let cid = cluster_id.clone();
                            rsx! {
                                button { class: "w-full text-left px-3 py-2 text-sm hover:bg-surface-2",
                                    title: desc,
                                    onclick: move |_| {
                                        let cid = cid.clone();
                                        let event = key.to_string();
                                        open.set(false);
                                        async move {
                                            match send_push_event(cid, event).await {
                                                Ok(r) => status.set(Some((r.ok, r.message))),
                                                Err(e) => status.set(Some((false, e.to_string()))),
                                            }
                                        }
                                    },
                                    {label}
                                }
                            }
                        }
                    }
                }
            }
            if let Some((ok, msg)) = status.read().as_ref() {
                {
                    let cls = if *ok { "text-success" } else { "text-danger" };
                    rsx! {
                        span { class: "ml-2 text-xs {cls}", "{msg}" }
                    }
                }
            }
        }
    }
}
