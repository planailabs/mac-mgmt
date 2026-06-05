//! Native Dioxus desktop "overview" app for the sovereign-AI USB build.
//!
//! A thin control panel built on `plan-ai-design`: it polls the daemon's
//! loopback control API (`http://[::1]:<port>/status`), lists the
//! `mac-mgmt-services` supervisor's services, and offers Start / Stop / Restart
//! per service plus an "Open Memvault" link. Install/update controls are greyed
//! out when the stack is offline.
//!
//! The webview event loop owns the main thread; the caller (the daemon's
//! `usb::main`) launches this after bringing the stack up on background threads.

use dioxus::prelude::*;
use plan_ai_design::{Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, Card};
use serde::Deserialize;

/// Launch the overview desktop window. `control_port` is the loopback port of
/// the daemon's control/status API; `memvault_url` (if any) is linked from the
/// UI. Blocks running the desktop event loop until the window closes.
pub fn launch(control_port: u16, memvault_url: Option<String>) {
    // Communicate config to the component tree via env (read at render time).
    // SAFETY: set before launch, single-threaded.
    unsafe {
        std::env::set_var("MAC_MGMT_OVERVIEW_PORT", control_port.to_string());
        if let Some(url) = &memvault_url {
            std::env::set_var("MAC_MGMT_MEMVAULT_URL", url);
        }
    }
    dioxus::launch(App);
}

fn base_url() -> String {
    let port = std::env::var("MAC_MGMT_OVERVIEW_PORT").unwrap_or_else(|_| "0".into());
    format!("http://[::1]:{port}")
}

fn memvault_url() -> Option<String> {
    std::env::var("MAC_MGMT_MEMVAULT_URL").ok()
}

#[derive(Deserialize, Clone, PartialEq, Default)]
struct Status {
    #[serde(default)]
    offline: bool,
    #[serde(default)]
    services: Vec<Svc>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Deserialize, Clone, PartialEq)]
struct Svc {
    name: String,
    #[serde(default)]
    running: bool,
    #[serde(default)]
    program: Option<String>,
}

async fn fetch_status() -> Status {
    match reqwest::get(format!("{}/status", base_url())).await {
        Ok(resp) => resp.json::<Status>().await.unwrap_or_else(|e| Status {
            error: Some(format!("bad status json: {e}")),
            ..Default::default()
        }),
        Err(e) => Status {
            error: Some(format!("control API unreachable: {e}")),
            ..Default::default()
        },
    }
}

async fn post_action(action: &str, name: &str) {
    let url = format!("{}/usb/{action}", base_url());
    let _ = reqwest::Client::new()
        .post(&url)
        .json(&serde_json::json!({ "name": name }))
        .send()
        .await;
}

#[component]
fn App() -> Element {
    let mut status = use_resource(|| async move { fetch_status().await });

    let snapshot = status.read().clone();

    rsx! {
        style { {include_str!("overview.css")} }
        main { class: "overview",
            header { class: "overview-head",
                h1 { "Sovereign AI" }
                {match &snapshot {
                    Some(s) if s.offline => rsx! { Badge { variant: BadgeVariant::Warn, "offline" } },
                    Some(_) => rsx! { Badge { variant: BadgeVariant::Success, "online" } },
                    None => rsx! { Badge { "…" } },
                }}
                Button {
                    size: ButtonSize::Sm,
                    variant: ButtonVariant::Ghost,
                    onclick: move |_| status.restart(),
                    "Refresh"
                }
                if let Some(url) = memvault_url() {
                    Button {
                        size: ButtonSize::Sm,
                        variant: ButtonVariant::Secondary,
                        onclick: move |_| { let _ = open_url(&url); },
                        "Open Memvault"
                    }
                }
            }

            Card {
                {match snapshot {
                    None => rsx! { p { "Loading…" } },
                    Some(s) => {
                        if let Some(err) = s.error {
                            rsx! { p { class: "overview-error", "{err}" } }
                        } else if s.services.is_empty() {
                            rsx! { p { "No services registered." } }
                        } else {
                            let offline = s.offline;
                            rsx! {
                                table { class: "overview-table",
                                    thead { tr { th { "Service" } th { "State" } th { "Program" } th { "Actions" } } }
                                    tbody {
                                        for svc in s.services.iter().cloned() {
                                            ServiceRow { svc, offline, on_changed: move |_| status.restart() }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }}
            }
        }
    }
}

#[component]
fn ServiceRow(svc: Svc, offline: bool, on_changed: EventHandler<()>) -> Element {
    let _ = offline; // start/stop/restart are local-only and work offline.
    let name_for = |n: &str| n.to_string();
    let (n1, n2, n3) = (name_for(&svc.name), name_for(&svc.name), name_for(&svc.name));

    rsx! {
        tr {
            td { "{svc.name}" }
            td {
                if svc.running {
                    Badge { variant: BadgeVariant::Success, "running" }
                } else {
                    Badge { variant: BadgeVariant::Neutral, "stopped" }
                }
            }
            td { class: "mono", {svc.program.clone().unwrap_or_default()} }
            td { class: "overview-actions",
                Button {
                    size: ButtonSize::Xs,
                    variant: ButtonVariant::Primary,
                    disabled: svc.running,
                    onclick: move |_| {
                        let n = n1.clone();
                        let cb = on_changed;
                        spawn(async move { post_action("start", &n).await; cb.call(()); });
                    },
                    "Start"
                }
                Button {
                    size: ButtonSize::Xs,
                    variant: ButtonVariant::Secondary,
                    disabled: !svc.running,
                    onclick: move |_| {
                        let n = n2.clone();
                        let cb = on_changed;
                        spawn(async move { post_action("stop", &n).await; cb.call(()); });
                    },
                    "Stop"
                }
                Button {
                    size: ButtonSize::Xs,
                    variant: ButtonVariant::Warn,
                    onclick: move |_| {
                        let n = n3.clone();
                        let cb = on_changed;
                        spawn(async move { post_action("restart", &n).await; cb.call(()); });
                    },
                    "Restart"
                }
            }
        }
    }
}

/// Open a URL in the user's default browser.
fn open_url(url: &str) -> std::io::Result<()> {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(opener).arg(url).spawn().map(|_| ())
}
