use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};

use crate::web::app::Route;

#[cfg(feature = "server")]
use crate::web::user::current_user;

use super::fleet_dashboard::create_proxy_token;

/// One online node that has at least one TCP tunnel configured.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EasyAccessNode {
    instance_id: String,
    hostname: String,
    cluster_name: String,
    /// TCP tunnel names reported via heartbeat.
    tunnel_names: Vec<String>,
    relay_proxy_url: String,
    relay_proxy_hostname: String,
    /// Whether the node also exposes file tunnels.
    has_files: bool,
    /// Whether the node also exposes shell tunnels.
    has_shell: bool,
}

#[server]
async fn get_easy_access_nodes() -> Result<Vec<EasyAccessNode>, ServerFnError> {
    use chrono::Utc;

    let user = current_user().await?;
    let pool = crate::server_pool()?;

    let accessible = user
        .accessible_cluster_ids(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct Row {
        instance_id: String,
        hostname: String,
        cluster_name: String,
        tunnels: serde_json::Value,
        file_tunnels: serde_json::Value,
        shell_tunnels: serde_json::Value,
        relay_proxy_url: Option<String>,
        relay_proxy_hostname: Option<String>,
        reported_at: chrono::DateTime<Utc>,
    }

    let rows = if let Some(ids) = accessible {
        sqlx::query_as::<_, Row>(
            "SELECT dh.instance_id, dh.hostname, c.name AS cluster_name, \
             dh.tunnels, dh.file_tunnels, dh.shell_tunnels, \
             dh.relay_proxy_url, dh.relay_proxy_hostname, dh.reported_at \
             FROM daemon_heartbeats dh \
             JOIN clusters c ON c.id = dh.cluster_id \
             WHERE dh.cluster_id = ANY($1) \
             ORDER BY c.name, dh.hostname",
        )
        .bind(&ids)
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    } else {
        sqlx::query_as::<_, Row>(
            "SELECT dh.instance_id, dh.hostname, c.name AS cluster_name, \
             dh.tunnels, dh.file_tunnels, dh.shell_tunnels, \
             dh.relay_proxy_url, dh.relay_proxy_hostname, dh.reported_at \
             FROM daemon_heartbeats dh \
             JOIN clusters c ON c.id = dh.cluster_id \
             ORDER BY c.name, dh.hostname",
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    };

    let now = Utc::now();
    let mut nodes = Vec::new();
    for r in rows {
        // Only online nodes (heartbeat < 5 min ago).
        let age = now.signed_duration_since(r.reported_at);
        if age.num_seconds() >= 300 {
            continue;
        }

        // Must have a relay proxy configured.
        let (Some(relay_url), Some(relay_host)) = (r.relay_proxy_url, r.relay_proxy_hostname)
        else {
            continue;
        };

        let tunnel_names: Vec<String> = r
            .tunnels
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|t| t.get("name").and_then(|v| v.as_str()).map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let has_files = r
            .file_tunnels
            .as_array()
            .map(|a| !a.is_empty())
            .unwrap_or(false);
        let has_shell = r
            .shell_tunnels
            .as_array()
            .map(|a| !a.is_empty())
            .unwrap_or(false);

        // Only include nodes that have at least one tunnel or file/shell access.
        if tunnel_names.is_empty() && !has_files && !has_shell {
            continue;
        }

        nodes.push(EasyAccessNode {
            instance_id: r.instance_id,
            hostname: r.hostname,
            cluster_name: r.cluster_name,
            tunnel_names,
            relay_proxy_url: relay_url,
            relay_proxy_hostname: relay_host,
            has_files,
            has_shell,
        });
    }

    Ok(nodes)
}

/// Returns an SVG icon path for well-known tunnel/service names.
fn service_icon(name: &str) -> &'static str {
    match name {
        // Ollama — brain / AI
        n if n.contains("ollama") => "M9.813 15.904 9 18.75l-.813-2.846a4.5 4.5 0 0 0-3.09-3.09L2.25 12l2.846-.813a4.5 4.5 0 0 0 3.09-3.09L9 5.25l.813 2.846a4.5 4.5 0 0 0 3.09 3.09L15.75 12l-2.846.813a4.5 4.5 0 0 0-3.09 3.09ZM18.259 8.715 18 9.75l-.259-1.035a3.375 3.375 0 0 0-2.455-2.456L14.25 6l1.036-.259a3.375 3.375 0 0 0 2.455-2.456L18 2.25l.259 1.035a3.375 3.375 0 0 0 2.455 2.456L21.75 6l-1.036.259a3.375 3.375 0 0 0-2.455 2.456ZM16.894 20.567 16.5 21.75l-.394-1.183a2.25 2.25 0 0 0-1.423-1.423L13.5 18.75l1.183-.394a2.25 2.25 0 0 0 1.423-1.423l.394-1.183.394 1.183a2.25 2.25 0 0 0 1.423 1.423l1.183.394-1.183.394a2.25 2.25 0 0 0-1.423 1.423Z",
        // Open WebUI — chat bubbles
        n if n.contains("open-webui") || n.contains("openwebui") => "M20.25 8.511c.884.284 1.5 1.128 1.5 2.097v4.286c0 1.136-.847 2.1-1.98 2.193-.34.027-.68.052-1.02.072v3.091l-3-3c-1.354 0-2.694-.055-4.02-.163a2.115 2.115 0 0 1-.825-.242m9.345-8.334a2.126 2.126 0 0 0-.476-.095 48.64 48.64 0 0 0-8.048 0c-1.131.094-1.976 1.057-1.976 2.192v4.286c0 .837.46 1.58 1.155 1.951m9.345-8.334V6.637c0-1.621-1.152-3.026-2.76-3.235A48.455 48.455 0 0 0 11.25 3c-2.115 0-4.198.137-6.24.402-1.608.209-2.76 1.614-2.76 3.235v6.226c0 1.621 1.152 3.026 2.76 3.235.577.075 1.157.14 1.74.194V21l4.155-4.155",
        // Openclaw / coding agent — command line
        n if n.contains("openclaw") || n.contains("opencode") => "M6.75 7.5l3 2.25-3 2.25m4.5 0h3M3.75 4.5h16.5a1.5 1.5 0 0 1 1.5 1.5v12a1.5 1.5 0 0 1-1.5 1.5H3.75a1.5 1.5 0 0 1-1.5-1.5V6a1.5 1.5 0 0 1 1.5-1.5Z",
        // Grafana / monitoring — chart
        n if n.contains("grafana") || n.contains("monitor") => "M3 13.125C3 12.504 3.504 12 4.125 12h2.25c.621 0 1.125.504 1.125 1.125v6.75C7.5 20.496 6.996 21 6.375 21h-2.25A1.125 1.125 0 0 1 3 19.875v-6.75ZM9.75 8.625c0-.621.504-1.125 1.125-1.125h2.25c.621 0 1.125.504 1.125 1.125v11.25c0 .621-.504 1.125-1.125 1.125h-2.25a1.125 1.125 0 0 1-1.125-1.125V8.625ZM16.5 4.125c0-.621.504-1.125 1.125-1.125h2.25C20.496 3 21 3.504 21 4.125v15.75c0 .621-.504 1.125-1.125 1.125h-2.25a1.125 1.125 0 0 1-1.125-1.125V4.125Z",
        // Jupyter — beaker / science
        n if n.contains("jupyter") => "M9.75 3.104v5.714a2.25 2.25 0 0 1-.659 1.591L5 14.5M9.75 3.104c-.251.023-.501.05-.75.082m.75-.082a24.301 24.301 0 0 1 4.5 0m0 0v5.714c0 .597.237 1.17.659 1.591L19.8 15.3M14.25 3.104c.251.023.501.05.75.082M19.8 15.3l-1.57.393A9.065 9.065 0 0 1 12 15a9.065 9.065 0 0 0-6.23.693L5 14.5m14.8.8 1.402 1.402c1.232 1.232.65 3.318-1.067 3.611A48.309 48.309 0 0 1 12 21c-2.773 0-5.491-.235-8.135-.687-1.718-.293-2.3-2.379-1.067-3.61L5 14.5",
        // Files — document
        "files" => "M19.5 14.25v-2.625a3.375 3.375 0 0 0-3.375-3.375h-1.5A1.125 1.125 0 0 1 13.5 7.125v-1.5a3.375 3.375 0 0 0-3.375-3.375H8.25m2.25 0H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 0 0-9-9Z",
        // Shell — terminal
        "shell" => "M6.75 7.5l3 2.25-3 2.25m4.5 0h3M3.75 4.5h16.5a1.5 1.5 0 0 1 1.5 1.5v12a1.5 1.5 0 0 1-1.5 1.5H3.75a1.5 1.5 0 0 1-1.5-1.5V6a1.5 1.5 0 0 1 1.5-1.5Z",
        // Default — globe / network
        _ => "M12 21a9.004 9.004 0 0 0 8.716-6.747M12 21a9.004 9.004 0 0 1-8.716-6.747M12 21c2.485 0 4.5-4.03 4.5-9S14.485 3 12 3m0 18c-2.485 0-4.5-4.03-4.5-9S9.515 3 12 3m0 0a8.997 8.997 0 0 1 7.843 4.582M12 3a8.997 8.997 0 0 0-7.843 4.582m15.686 0A11.953 11.953 0 0 1 12 10.5c-2.998 0-5.74-1.1-7.843-2.918m15.686 0A8.959 8.959 0 0 1 21 12c0 .778-.099 1.533-.284 2.253m0 0A17.919 17.919 0 0 1 12 16.5c-3.162 0-6.133-.815-8.716-2.247m0 0A9.015 9.015 0 0 1 3 12c0-1.605.42-3.113 1.157-4.418",
    }
}

#[component]
pub fn EasyAccess() -> Element {
    let nodes = use_server_future(get_easy_access_nodes)?;

    match &*nodes.read() {
        Some(Ok(nodes)) => {
            if nodes.is_empty() {
                return rsx! {
                    div { class: "max-w-5xl mx-auto px-4 py-12 text-center",
                        h1 { class: "text-3xl font-bold text-gray-900 dark:text-white mb-4", {t!("easy-access-title")} }
                        p { class: "text-gray-500 dark:text-gray-400 text-lg",
                            {t!("easy-access-no-nodes")}
                        }
                    }
                };
            }

            rsx! {
                div { class: "max-w-6xl mx-auto px-4 py-8",
                    h1 { class: "text-3xl font-bold text-gray-900 dark:text-white mb-8", {t!("easy-access-title")} }

                    for node in nodes {
                        div { key: "{node.instance_id}", class: "mb-10",
                            // Node heading
                            div { class: "mb-4",
                                h2 { class: "text-2xl font-bold text-gray-900 dark:text-white",
                                    Link {
                                        to: Route::FleetDetail { instance_id: node.instance_id.clone() },
                                        class: "hover:text-blue-600 dark:hover:text-blue-400 transition-colors",
                                        "{node.hostname}"
                                    }
                                }
                                p { class: "text-sm text-gray-500 dark:text-gray-400 mt-1",
                                    "{node.cluster_name}"
                                }
                            }

                            // Service buttons grid
                            div { class: "flex flex-wrap gap-4",
                                for tname in &node.tunnel_names {
                                    {
                                        let iid = node.instance_id.chars().take(12).collect::<String>();
                                        let tn = tname.clone();
                                        let pu = node.relay_proxy_url.clone();
                                        let ph = node.relay_proxy_hostname.clone();
                                        let icon_path = service_icon(&tn);
                                        let display_name = tn.clone();
                                        rsx! {
                                            button {
                                                key: "{tn}",
                                                class: "flex flex-col items-center justify-center w-28 h-28 rounded-xl \
                                                        bg-white dark:bg-gray-800 \
                                                        border-2 border-gray-200 dark:border-gray-700 \
                                                        hover:border-blue-400 dark:hover:border-blue-500 \
                                                        hover:shadow-lg hover:scale-105 \
                                                        transition-all duration-150 cursor-pointer \
                                                        group",
                                                onclick: move |_| {
                                                    let iid = iid.clone();
                                                    let tn = tn.clone();
                                                    let pu = pu.clone();
                                                    let ph = ph.clone();
                                                    async move {
                                                        match create_proxy_token().await {
                                                            Ok(result) => {
                                                                let scheme = if pu.starts_with("https://") { "https://" } else { "http://" };
                                                                let url = format!(
                                                                    "{scheme}{iid}-{tn}.{ph}/proxy?proxy_token={}",
                                                                    result.proxy_token
                                                                );
                                                                let _ = document::eval(&format!(
                                                                    "window.open('{}', '_blank')",
                                                                    url
                                                                ));
                                                            }
                                                            Err(e) => {
                                                                tracing::error!("failed to create proxy token: {e}");
                                                            }
                                                        }
                                                    }
                                                },
                                                svg {
                                                    class: "h-10 w-10 text-gray-500 dark:text-gray-400 group-hover:text-blue-600 dark:group-hover:text-blue-400 transition-colors mb-2",
                                                    fill: "none",
                                                    stroke: "currentColor",
                                                    stroke_width: "1.5",
                                                    stroke_linecap: "round",
                                                    stroke_linejoin: "round",
                                                    view_box: "0 0 24 24",
                                                    path { d: "{icon_path}" }
                                                }
                                                span { class: "text-xs font-medium text-gray-700 dark:text-gray-300 group-hover:text-blue-600 dark:group-hover:text-blue-400 transition-colors text-center leading-tight",
                                                    "{display_name}"
                                                }
                                            }
                                        }
                                    }
                                }

                                // Files button
                                if node.has_files {
                                    {
                                        let iid = node.instance_id.clone();
                                        rsx! {
                                            Link {
                                                to: Route::FleetFiles { instance_id: iid },
                                                class: "flex flex-col items-center justify-center w-28 h-28 rounded-xl \
                                                        bg-white dark:bg-gray-800 \
                                                        border-2 border-gray-200 dark:border-gray-700 \
                                                        hover:border-blue-400 dark:hover:border-blue-500 \
                                                        hover:shadow-lg hover:scale-105 \
                                                        transition-all duration-150 cursor-pointer \
                                                        group",
                                                svg {
                                                    class: "h-10 w-10 text-gray-500 dark:text-gray-400 group-hover:text-blue-600 dark:group-hover:text-blue-400 transition-colors mb-2",
                                                    fill: "none",
                                                    stroke: "currentColor",
                                                    stroke_width: "1.5",
                                                    stroke_linecap: "round",
                                                    stroke_linejoin: "round",
                                                    view_box: "0 0 24 24",
                                                    path { d: "{service_icon(\"files\")}" }
                                                }
                                                span { class: "text-xs font-medium text-gray-700 dark:text-gray-300 group-hover:text-blue-600 dark:group-hover:text-blue-400 transition-colors text-center leading-tight",
                                                    {t!("easy-access-files")}
                                                }
                                            }
                                        }
                                    }
                                }

                                // Shell button
                                if node.has_shell {
                                    {
                                        let iid = node.instance_id.clone();
                                        rsx! {
                                            Link {
                                                to: Route::FleetShell { instance_id: iid },
                                                class: "flex flex-col items-center justify-center w-28 h-28 rounded-xl \
                                                        bg-white dark:bg-gray-800 \
                                                        border-2 border-gray-200 dark:border-gray-700 \
                                                        hover:border-blue-400 dark:hover:border-blue-500 \
                                                        hover:shadow-lg hover:scale-105 \
                                                        transition-all duration-150 cursor-pointer \
                                                        group",
                                                svg {
                                                    class: "h-10 w-10 text-gray-500 dark:text-gray-400 group-hover:text-blue-600 dark:group-hover:text-blue-400 transition-colors mb-2",
                                                    fill: "none",
                                                    stroke: "currentColor",
                                                    stroke_width: "1.5",
                                                    stroke_linecap: "round",
                                                    stroke_linejoin: "round",
                                                    view_box: "0 0 24 24",
                                                    path { d: "{service_icon(\"shell\")}" }
                                                }
                                                span { class: "text-xs font-medium text-gray-700 dark:text-gray-300 group-hover:text-blue-600 dark:group-hover:text-blue-400 transition-colors text-center leading-tight",
                                                    {t!("easy-access-shell")}
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Some(Err(e)) => rsx! {
            div { class: "max-w-5xl mx-auto px-4 py-12 text-center",
                h1 { class: "text-3xl font-bold text-gray-900 dark:text-white mb-4", {t!("easy-access-title")} }
                p { class: "text-red-600 dark:text-red-400", {t!("error-message", message: e.to_string())} }
            }
        },
        None => rsx! {
            div { class: "max-w-5xl mx-auto px-4 py-12 text-center",
                h1 { class: "text-3xl font-bold text-gray-900 dark:text-white mb-4", {t!("easy-access-title")} }
                p { class: "text-gray-500 dark:text-gray-400", {t!("loading")} }
            }
        },
    }
}
