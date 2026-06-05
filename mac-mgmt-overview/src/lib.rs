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
use mac_mgmt_config_ui::ConfigEditor;
use plan_ai_design::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, Card, LanguagePicker, ThemeToggle,
    THEME_INIT_SCRIPT,
};
use serde::Deserialize;

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Services,
    Config,
}

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

async fn fetch_json(path: &str) -> Option<serde_json::Value> {
    reqwest::get(format!("{}{path}", base_url()))
        .await
        .ok()?
        .json()
        .await
        .ok()
}

async fn put_config(json: String) -> Result<(), String> {
    let resp = reqwest::Client::new()
        .put(format!("{}/config", base_url()))
        .header("content-type", "application/json")
        .body(json)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.status().is_success() {
        Ok(())
    } else {
        let body = resp.text().await.unwrap_or_default();
        Err(format!("save failed: {body}"))
    }
}

#[component]
fn App() -> Element {
    // The shared config editor uses `t!`, so an i18n context must exist.
    // Concatenate plan-ai-design + config-ui translations.
    use dioxus_i18n::prelude::*;
    use unic_langid::langid;
    let mut i18n = use_init_i18n(|| {
        let en: &'static str = Box::leak(
            format!(
                "{}\n{}",
                plan_ai_design::i18n::EN_US,
                mac_mgmt_config_ui::EN_US
            )
            .into_boxed_str(),
        );
        let de: &'static str = Box::leak(
            format!(
                "{}\n{}",
                plan_ai_design::i18n::DE_DE,
                mac_mgmt_config_ui::DE_DE
            )
            .into_boxed_str(),
        );
        I18nConfig::new(langid!("en-US"))
            .with_locale(Locale::new_static(langid!("en-US"), en))
            .with_locale(Locale::new_static(langid!("de-DE"), de))
    });

    // Apply the saved/system theme on startup (sets `.dark` on <html>); the
    // ThemeToggle in the header then cycles system → light → dark.
    use_effect(|| {
        document::eval(THEME_INIT_SCRIPT);
    });

    // Restore the saved language (localStorage['lang']); the LanguagePicker
    // in the header changes + persists it.
    use_effect(move || {
        spawn(async move {
            if let Ok(val) =
                document::eval("try { return localStorage.getItem('lang') || ''; } catch(e) { return ''; }")
                    .await
            {
                if val.as_str() == Some("de-DE") {
                    let _ = i18n.set_language(langid!("de-DE"));
                }
            }
        });
    });

    let mut status = use_resource(|| async move { fetch_status().await });
    let mut tab = use_signal(|| Tab::Services);

    let snapshot = status.read().clone();

    rsx! {
        // Design system (Tailwind tokens + plan-ai-design + the shared config
        // editor's classes), compiled from plan-ai-design/assets/input.css.
        style { {include_str!("../assets/tailwind.css")} }
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
                    variant: if *tab.read() == Tab::Services { ButtonVariant::Primary } else { ButtonVariant::Ghost },
                    onclick: move |_| tab.set(Tab::Services),
                    "Services"
                }
                Button {
                    size: ButtonSize::Sm,
                    variant: if *tab.read() == Tab::Config { ButtonVariant::Primary } else { ButtonVariant::Ghost },
                    onclick: move |_| tab.set(Tab::Config),
                    "Config"
                }
                if *tab.read() == Tab::Services {
                    Button {
                        size: ButtonSize::Sm,
                        variant: ButtonVariant::Ghost,
                        onclick: move |_| status.restart(),
                        "Refresh"
                    }
                }
                if let Some(url) = memvault_url() {
                    Button {
                        size: ButtonSize::Sm,
                        variant: ButtonVariant::Secondary,
                        onclick: move |_| { let _ = open_url(&url); },
                        "Open Memvault"
                    }
                }
                LanguagePicker {}
                ThemeToggle {}
            }

            match *tab.read() {
                Tab::Services => rsx! {
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
                },
                Tab::Config => rsx! { ConfigView {} },
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

/// Config tab — loads the daemon config + schema and renders the shared
/// schema-driven editor (the same component the server uses). Saving PUTs the
/// edited JSON to the control API, which validates + persists config.json.
#[component]
fn ConfigView() -> Element {
    let mut config = use_resource(|| async move { fetch_json("/config").await });
    let schema = use_resource(|| async move { fetch_json("/config/schema").await });
    let mut saving = use_signal(|| false);
    let mut save_err = use_signal(|| None::<String>);
    let mut saved_note = use_signal(|| None::<String>);

    let cfg = config.read().clone().flatten();
    let sch = schema.read().clone().flatten();

    rsx! {
        Card {
            if let Some(note) = saved_note.read().clone() {
                p { class: "config-saved-note", "{note}" }
            }
            match (sch, cfg) {
                (Some(schema), Some(initial)) => rsx! {
                    ConfigEditor {
                        cluster_id: "usb".to_string(),
                        schema,
                        initial,
                        saving: saving(),
                        save_error: save_err(),
                        on_save: move |json: String| {
                            saving.set(true);
                            saved_note.set(None);
                            spawn(async move {
                                match put_config(json).await {
                                    Ok(()) => {
                                        save_err.set(None);
                                        saved_note.set(Some("Saved to config.json — restart the stack to apply.".into()));
                                        config.restart();
                                    }
                                    Err(e) => save_err.set(Some(e)),
                                }
                                saving.set(false);
                            });
                        },
                    }
                },
                _ => rsx! { p { "Loading config…" } },
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
